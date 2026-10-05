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
use futures_util::stream::{self, Stream};
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
/// A struct rather than seven parameters because `Stream::new` would otherwise be over the house
/// limit of four, and a bundle is also what a test can build without a whole `App`.
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

/// One turn of the loop: the next event, or the end of the stream.
enum Turn {
    /// One event; the stream continues with `Stream`.
    Event(Event),
    /// The stream is over, after the event that says why.
    Close(Event),
    /// The stream is over with nothing to say, which is shutdown.
    End,
}

/// Everything one stream holds for its life.
pub struct Stream {
    /// The hub, for the store, the settings and the watch.
    app: Arc<App>,
    /// The workspace this stream is subscribed to.
    ws: String,
    /// Where the next read starts. Advanced by every page, never by the watch.
    cursor: Cursor,
    /// The epoch of the last read, which the notice's `id:` carries.
    epoch: u64,
    /// The workspace's position, so a wake-up knows there is something to read.
    receiver: Receiver<(u64, u64)>,
    /// The slot, freed on drop — and taken explicitly on the `busy` path, before the event is
    /// yielded. That ordering is what makes the slot free while the old connection is still open.
    subscriber: Option<Subscriber>,
    /// The caps and the pool wait.
    shape: Shape,
    /// The headers of the page being sent.
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
) -> impl Stream<Item = Result<Event, Infallible>> {
    let shape = Shape::of(app.as_ref());
    let state = Stream {
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
    stream::unfold(state, |mut state| async move {
        state
            .turn()
            .await
            .event()
            .map(|event| (Ok(event), state))
    })
}

impl Stream {
    /// One turn of the loop: send the next notice, wait, or close.
    async fn turn(&mut self) -> Turn {
        if let Some(head) = self.pending.first().cloned() {
            self.pending.remove(0);
            return Turn::Event(notice(&head, self.epoch));
        }
        if !self.caught_up {
            return self.page().await;
        }
        self.wait().await
    }

    /// Step 2: one page read, then the headers of it as notices.
    async fn page(&mut self) -> Turn {
        let request = PageReq {
            ws: self.ws.clone(),
            since: self.cursor,
            at_most: self.shape.at_most,
            max_bytes: self.shape.max_bytes,
        };
        let answer = match self.read(&request).await {
            Ok(answer) => answer,
            Err(StoreError::Gone) => return Turn::Close(close(RESYNC)),
            Err(_) => return Turn::Close(self.busy()),
        };
        self.epoch = answer.epoch;
        self.caught_up = answer.next.seq >= answer.head_seq;
        match answer.heads.first() {
            // An empty page means caught up with nothing to send: wait rather than spin. A
            // subscriber whose cursor is already at `head_seq` lands here on its first read.
            None => self.wait().await,
            Some(_) => {
                self.pending = answer.heads;
                self.next_notice()
            }
        }
    }

    /// The store read itself, with the pool wait and the test seam around it.
    async fn read(
        &self,
        request: &PageReq,
    ) -> Result<page::Page, StoreError> {
        if let Some(fault) = crate::hooks::page_fault(&self.app.hooks, self.cursor.seq) {
            return Err(fault);
        }
        let store = self.app.store().await?;
        let read = page::page(store, request);
        match tokio::time::timeout(self.shape.timeout, read).await {
            Ok(answer) => {
                if let Ok(answer) = &answer {
                    crate::hooks::count_headers(&self.app.hooks, answer.heads.len() as u64);
                }
                answer
            }
            // A read that waited past the budget is the `busy` §5.3 describes: the pool is short,
            // not the cursor wrong. The store would answer the same thing as `Busy` on its own.
            Err(_) => Err(StoreError::Busy { retry_after: 1 }),
        }
    }

    /// Step 3: caught up, so wait for the watch or for the heartbeat's tick.
    async fn wait(&mut self) -> Turn {
        let mut every = beat::ticker();
        loop {
            tokio::select! {
                changed = self.receiver.changed() => {
                    if changed.is_err() {
                        return Turn::End;
                    }
                    self.caught_up = false;
                    return self.page().await;
                }
                _ = every.tick() => return Turn::Event(heartbeat()),
            }
        }
    }

    /// The next notice of the page in hand, advancing the cursor to the header it carries.
    ///
    /// The cursor moves as the notice is *built*, so a stream dropped half way through a page
    /// resumes at the last notice the client actually received rather than at the page's end.
    fn next_notice(&mut self) -> Turn {
        let Some(head) = self.pending.first().cloned() else {
            return Turn::End;
        };
        self.pending.remove(0);
        self.cursor = Cursor {
            epoch: self.epoch,
            seq: head.seq,
        };
        Turn::Event(notice(&head, self.epoch))
    }

    /// The `busy` close, with the slot freed **before** the event is yielded.
    ///
    /// The `take` is the whole point: the `Subscriber`'s drop returns the count, and a subscriber
    /// that reconnects on the same key while this connection is still open finds the cap with room.
    fn busy(&mut self) -> Event {
        self.subscriber.take();
        close(BUSY)
    }
}

impl Turn {
    /// The event this turn produced, or `None` when the stream ends with nothing to say.
    fn event(self) -> Option<Event> {
        match self {
            Turn::Event(event) | Turn::Close(event) => Some(event),
            Turn::End => None,
        }
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
        .id(Cursor { epoch, seq: head.seq }.to_string())
        .data(notice_json(&head.as_change_head()))
}

/// One of the two early closes: a named event with **no** `id:` line and no data.
///
/// No `id:` is required, not merely nice: `Last-Event-ID` is what a reconnect resumes from, so an
/// `id:` on `busy` would tell the SDK to resume from a change it never received.
fn close(name: &str) -> Event {
    Event::default().event(name)
}