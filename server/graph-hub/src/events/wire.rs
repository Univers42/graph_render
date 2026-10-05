//! The vocabulary of one turn of the loop: what woke it, what it produced, and the three events the
//! stream can write.
//!
//! It is here rather than in [`super::stream`] so that file reads as a loop — `page`, then `wait`,
//! then `subscribe` — and the words the loop is written in sit beside it. Every event on the wire is
//! built here, so §5.3's text has exactly one producer: a notice is graph-contract's `notice_json`
//! and nothing else, a heartbeat is a comment, and both early closes are a name with no `id:`.

use axum::response::sse::Event;
use graph_contract::hub::Cursor;

use crate::events::page::Head;
use crate::events::stream::{BUSY, CHANGE, RESYNC};

/// What woke the wait in step 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
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
pub struct Step {
    /// The event, or `None` for the end of the stream with nothing to say.
    pub event: Option<Event>,
    /// Whether the stream continues after this event.
    pub keep: bool,
}

impl Step {
    /// One more event of an ordinary stream.
    pub fn more(event: Event) -> Self {
        Step {
            event: Some(event),
            keep: true,
        }
    }

    /// The stream's last event, which names its own end.
    pub fn last(event: Event) -> Self {
        Step {
            event: Some(event),
            keep: false,
        }
    }

    /// The end of the stream with nothing written, which is only shutdown.
    pub fn end() -> Self {
        Step {
            event: None,
            keep: false,
        }
    }
}

/// The heartbeat: a comment, and never a `data:` line.
///
/// A client that parses `data:` lines must find none from a keep-alive, or every idle period would
/// look like an empty change. `MISSING` is what axum writes for a comment with no text; the hub's
/// own text is added by the route's `KeepAlive`, so this is the *stream's* idle event and the two
/// never overlap.
pub fn heartbeat() -> Event {
    Event::default().comment("keep-alive")
}

/// One change notice: `event: change`, `id: <epoch>.<seq>`, `data: <notice_json>`.
///
/// The `id:` is the **cursor**, not the bare seq: `Last-Event-ID` is what a reconnect resumes from,
/// and a bare seq could not be told from another epoch's.
pub fn notice(head: &Head, epoch: u64) -> Event {
    Event::default()
        .event(CHANGE)
        .id(Cursor {
            epoch,
            seq: head.seq,
        }
        .to_string())
        .data(graph_contract::hub::notice_json(&head.as_change_head()))
}

/// One of the two early closes: a named event with **no** `id:` line and no data.
///
/// No `id:` is required, not merely nice: an `id:` on `busy` would tell the SDK to resume from a
/// change it never received, which is exactly the gap Decision 12 exists to avoid.
pub fn close(name: &str) -> Event {
    Event::default().event(name)
}

/// The two early closes' names, so a caller never spells them out.
pub const CLOSES: [&str; 2] = [BUSY, RESYNC];
