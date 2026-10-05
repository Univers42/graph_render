//! The per-subscriber loop, as a stream of [`Event`]s: §5.3's three steps, in order, forever.
//!
//! The order is the whole design and it is not negotiable:
//!
//! 1. **subscribe to the watch first**, so a write that lands between the subscribe and the first
//!    page read is *seen by the page* rather than lost between the two;
//! 2. **page the headers after the cursor** and send each as `event: change` with
//!    `id: <epoch>.<seq>` and `data: <notice_json>`;
//! 3. **when caught up, wait on the watch** and go back to step 2.
//!
//! Because step 2 always reads from the cursor rather than from the notification, a subscriber that
//! missed a wake-up — or was never told about one — still reads every change it missed. That is why
//! the watch carries a position and not a payload, and it is the property
//! `a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate` pins.
//!
//! The two early closes are the ends the plan's Review Focus 3 is about:
//!
//! - **`busy`** — a page read that waited past `GRAPH_HUB_TIMEOUT_MS` for a pool connection. The
//!   cursor is still valid, so a `resync` would make every subscriber read a whole `/graph` from a
//!   pool that is already short. The subscriber slot is freed **before** the event is yielded, so a
//!   reconnect finds the cap with room; and the event carries **no `id:` line**, so the SDK's
//!   `Last-Event-ID` cannot skip a change it never received (Decision 12, §16(b) condition 11).
//! - **`resync`** — the cursor went invalid, which is the one case where the subscriber must read
//!   the document again.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::response::sse::Event;
use futures_util::stream::{self, Stream as StreamTrait};
use graph_contract::hub::{Cursor, notice_json};
use graph_store::StoreError;
use tokio::sync::watch::Receiver;

use crate::app::App;
use crate::events::beat;
use crate::events::page::{self, Head, PageReq};
use crate::gate::subscribers::Subscriber;

/// The event name of a change notice.
pub const CHANGE: &str = "change";

/// The event name of the mid-stream close a pool wait causes.
pub const BUSY: &str = "busy";

/// The event name of the close an invalid cursor causes.
pub const RESYNC: &str = "resync";

/// What one subscriber's stream is configured with, read once from `Settings`.
///
/// A struct rather than three extra parameters because the house limit is four per function, and a
/// bundle is also what a test builds without a whole `App`.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// How many headers one read may return, §6's `SSE_PAGE`.
    pub at_most: u64,
    /// The byte cap one read may return, §6's `CHANGES_BYTES`.
    pub max_bytes: u64,
    /// How long a page read waits for a pool connection, §6's `TIMEOUT_MS`.
    pub timeout: Duration,
}

impl Shape {
    /// The shape `app`'s own settings describe.
    pub fn of(app: &App) -> Self {
        Shape {
            at_most: app.settings.sse_page,
            max_bytes: app.settings.limits.changes_bytes,
            timeout: app.settings.timeout,
        }
    }
}

/// What woke the wait in step 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wake {
    /// The watch moved, so there is something to page.
    Changed,
    /// The heartbeat's interval elapsed.
    Ticked,
    /// The watch's sender is gone, which only happens at shutdown.
    Shut,
}

/// One turn of the loop's answer: the event to write, and whether the stream goes on after it.
///
/// A struct rather than an `Option<Event>` because the two questions are different: a shutdown ends
/// the stream with **nothing** written, while a `busy` or a `resync` ends it **after** an event that
/// says why, and that event must still reach the client.
struct Step {
    /// The event, or `None` for the end of the stream with nothing to say.
    event: Option<Event>,
    /// Whether the stream continues after this event.
    keep: bool,
}

impl Step {
    /// One more event of an ordinary stream.
    fn more(event: Event) -> Self {
        Step {
            event: Some(event),
            keep: true,
        }
    }

    /// The stream's last event, which names its own end.
    fn last(event: Event) -> Self {
        Step {
            event: Some(event),
            keep: false,
        }
    }

    /// The end of the stream with nothing written, which is only shutdown.
    fn end() -> Self {
        Step {
            event: None,
            keep: false,
        }
    }
}

/// Everything one stream holds for its life.
pub struct Sub {
    /// The hub, for the store, the settings and the watch.
    app: Arc<App>,
    /// The workspace this stream is subscribed to.
    ws: String,
    /// Where the next read starts. Advanced by every notice, never by the watch.
    cursor: Cursor,
    /// The epoch of the last read, which every notice's `id:` carries.
    epoch: u64,
    /// The workspace's position, so a wake-up knows there is something to read.
    receiver: Receiver<(u64, u64)>,
    /// The slot, freed on drop — and taken explicitly on the `busy` path, before the event is
    /// yielded. That ordering is what makes the slot free while the old connection is still open.
    subscriber: Option<Subscriber>,
    /// The caps and the pool wait.
    shape: Shape,
    /// The headers of the page in hand, still to be sent.
    pending: Vec<Head>,
    /// Whether the cursor has caught up with `head_seq`, i.e. whether the stream waits.
    caught_up: bool,
}

/// The stream for one subscriber: over `app`, on `ws`, resuming after `cursor`.
pub fn subscribe(
    app: Arc<App>,
    ws: String,
    cursor: Cursor,
    epoch: u64,
    subscriber: Subscriber,
) -> impl StreamTrait<Item = Result<Event, Infallible>> {
    let shape = Shape::of(app.as_ref());
    let state = Sub {
        receiver: app.watch.subscribe(&ws),
        app,
        ws,
        cursor,
        epoch,
        subscriber: Some(subscriber),
        shape,
        pending: Vec::new(),
        caught_up: false,
    };
    // `unfold` ends when its closure answers `None`, so the "keep going" flag rides along in the
    // state and the next turn reads it: one extra turn that yields nothing, which is how the last
    // event reaches the client before the stream closes.
    let state = Done {
        inner: state,
        keep: true,
    };
    stream::unfold(state, |mut done| async move {
        if !done.keep {
            return None;
        }
        let step = done.inner.turn().await;
        done.keep = step.keep;
        let event = step.event?;
        Some((Ok(event), done))
    })
}

/// The stream's state plus whether another turn follows, which `unfold` cannot carry itself.
struct Done {
    /// The loop's own state.
    inner: Sub,
    /// Whether the stream continues after the event just yielded.
    keep: bool,
}

impl Sub {
    /// One turn of the loop: send the next notice, wait, or close.
    ///
    /// The loop rather than three mutually recursive steps, because "caught up with nothing to send"
    /// has to become "wait" without a second stack frame: `page` and `wait` call each other, and two
    /// `async fn`s that call each other need a boxed future to compile.
    async fn turn(&mut self) -> Step {
        loop {
            if !self.pending.is_empty() {
                return self.next_notice();
            }
            if self.caught_up {
                match self.wait().await {
                    Wake::Changed => self.caught_up = false,
                    Wake::Ticked => return Step::more(heartbeat()),
                    Wake::Shut => return Step::end(),
                }
                continue;
            }
            // `None` is the one case that is not an answer: the page came back empty, so there is
            // nothing to send and nothing to refuse, and the next turn waits instead of spinning.
            if let Some(step) = self.page().await {
                return step;
            }
            self.caught_up = true;
        }
    }

    /// Step 2: one page read, then the headers of it as notices.
    ///
    /// `skip-event` publishes from the watch alone: the cursor jumps to the position the watch
    /// reports and no header is read, so every seq between the old cursor and the new one is
    /// skipped. That is the claim row `negctl-skip-event` breaks — a stream driven by notifications
    /// rather than by the log has gaps — and it is why the shipped loop reads the log at all.
    async fn page(&mut self) -> Option<Step> {
        if crate::breaks::on("skip-event") {
            // The cursor jumps to the position the watch reports and no header is read, so every
            // seq between the old cursor and the new one is skipped: a stream driven by
            // notifications rather than by the log has gaps, which is the claim this break denies.
            let (epoch, seq) = *self.receiver.borrow();
            self.cursor = Cursor { epoch, seq };
            return None;
        }
        let request = PageReq {
            ws: self.ws.clone(),
            since: self.cursor,
            at_most: self.shape.at_most,
            max_bytes: self.shape.max_bytes,
        };
        let answer = match self.read(&request).await {
            Ok(answer) if self.epoch_is(answer.epoch) => answer,
            // A page answered from another epoch is a promotion, which the store reports as `Gone`
            // for a cursor of the old epoch and as a foreign `epoch` in a page that came back: both
            // are the same `resync`.
            Ok(_) | Err(StoreError::Gone) => return Some(Step::last(close(RESYNC))),
            Err(_) => return Some(Step::last(self.busy())),
        };
        self.caught_up = answer.next.seq >= answer.head_seq;
        self.pending = answer.heads;
        (!self.pending.is_empty()).then(|| self.next_notice())
    }

    /// The store read itself, with the pool wait and the test seam around it.
    ///
    /// A hub with no database answers `Busy`, not `Gone`: there is nothing to resync from, and the
    /// SDK's jittered reconnect is the right answer to a hub that is not up yet.
    async fn read(&self, request: &PageReq) -> Result<page::Page, StoreError> {
        if let Some(fault) = crate::hooks::page_fault(&self.app.hooks, self.cursor.seq) {
            return Err(fault);
        }
        let store = match self.app.store().await {
            Ok(store) => store,
            Err(_) => return Err(StoreError::Busy { retry_after: 1 }),
        };
        let read = page::page(store, request);
        match tokio::time::timeout(self.shape.timeout, read).await {
            Ok(answer) => {
                if let Ok(answer) = &answer {
                    crate::hooks::count_headers(&self.app.hooks, answer.heads.len() as u64);
                }
                answer
            }
            // A read that waited past the budget is the `busy` §5.3 describes: the pool is short, not
            // the cursor wrong. The store answers the same thing as `Busy` on its own.
            Err(_) => Err(StoreError::Busy { retry_after: 1 }),
        }
    }

    /// Step 3: caught up, so wait for the watch or for the heartbeat's tick.
    ///
    /// The epoch is **not** re-read here. §5.3's heartbeat re-reads it, and axum's `KeepAlive`
    /// writes a comment on its own interval, so the two would be two intervals racing; the epoch
    /// check is the stream's business and lives in [`Sub::epoch_is`], which the next page read calls
    /// before it trusts the position the watch reported.
    async fn wait(&mut self) -> Wake {
        let mut every = beat::ticker();
        loop {
            tokio::select! {
                changed = self.receiver.changed() => {
                    return match changed {
                        Ok(()) => Wake::Changed,
                        // The sender is gone, which only happens at shutdown.
                        Err(_) => Wake::Shut,
                    };
                }
                _ = every.tick() => return Wake::Ticked,
            }
        }
    }

    /// Is the epoch this read answered with still the one the stream's cursor belongs to?
    ///
    /// A workspace promoted from a restore gets a new epoch, and every cursor a subscriber holds is
    /// void from that moment: the seqs it names mean something else. The store's page read answers
    /// with the epoch it read, so this costs nothing — the check is a comparison of a number the read
    /// already returned, and it is what turns a promotion into a `resync` instead of a stream that
    /// silently resumes in the middle of a different log.
    ///
    /// Caveat: a stream with nothing to read is not told. The check rides on the page read, so an
    /// idle subscriber of a promoted workspace stays open until its client times out or a change
    /// arrives; a per-heartbeat re-read would close that gap at one statement per subscriber per
    /// 15 s, and §5.3 puts the check on the read instead.
    fn epoch_is(&self, epoch: u64) -> bool {
        epoch == self.epoch
    }

    /// The next notice of the page in hand, advancing the cursor to the header it carries.
    ///
    /// The cursor moves as the notice is built, so a stream dropped half way through a page resumes
    /// at the last notice the client actually received and not at the page's end.
    fn next_notice(&mut self) -> Step {
        let Some(head) = self.pending.first().cloned() else {
            return Step::end();
        };
        self.pending.remove(0);
        self.cursor = Cursor {
            epoch: self.epoch,
            seq: head.seq,
        };
        Step::more(notice(&head, self.epoch))
    }

    /// The `busy` close, with the slot freed **before** the event is yielded.
    ///
    /// The `take` is the whole point: the `Subscriber`'s drop returns the count, so a subscriber that
    /// reconnects on the same key while this connection is still open finds the cap with room.
    fn busy(&mut self) -> Event {
        self.subscriber.take();
        close(BUSY)
    }
}

/// The header heartbeat, which never carries data: a client that reads it learns nothing about the
/// workspace.
fn heartbeat() -> Event {
    Event::default().comment("keep-alive")
}

/// One change notice: `event: change`, `id: <epoch>.<seq>`, `data: <notice_json>`.
fn notice(head: &Head, epoch: u64) -> Event {
    Event::default()
        .event(CHANGE)
        .id(Cursor {
            epoch,
            seq: head.seq,
        }
        .to_string())
        .data(notice_json(&head.as_change_head()))
}

/// One of the two early closes: a named event with **no** `id:` line and no data.
///
/// No `id:` is required, not merely nice: `Last-Event-ID` is what a reconnect resumes from, so an
/// `id:` on `busy` would tell the SDK to resume from a change it never received.
fn close(name: &str) -> Event {
    Event::default().event(name)
}
