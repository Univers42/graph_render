//! The 15-second comment heartbeat, and the epoch re-read that turns it into a `resync`.
//!
//! A keep-alive exists so a proxy does not decide an idle connection is dead. §5.3's version also
//! carries a fact: it re-reads the workspace's epoch, and an epoch that moved means the workspace
//! was promoted from a restore, every cursor this stream holds is void, and the only correct answer
//! is `event: resync` and a close.
//!
//! Caveat: the interval is a floor and not a schedule. A subscriber that is being written to
//! continuously pushes its own events ahead of the tick (`MissedTickBehavior::Skip` drops the tick
//! rather than queueing it), so a busy stream may never show a heartbeat at all — which is correct,
//! because it is not idle. The SDK's own read timeout is what actually bounds a dead peer; this
//! only keeps the wire warm.

use std::time::Duration;

use tokio::time::{Instant, MissedTickBehavior, interval_at};

/// §5.3's heartbeat interval, in seconds.
///
/// Caveat: 15 s is the spec's number and not `GRAPH_HUB_*`, so it is a constant rather than a
/// setting: a deployment that lengthens it lengthens every subscriber's silence at once, and §6
/// bounds the caps rather than the liveness interval.
pub const EVERY: Duration = Duration::from_secs(15);

/// The heartbeat's ticker, `MissedTickBehavior::Skip`.
///
/// The first tick is `EVERY` away rather than immediate: a tick at once would put a comment in
/// front of the first change on every stream, which is noise a client cannot tell from a change.
pub fn ticker() -> tokio::time::Interval {
    let mut every = interval_at(Instant::now() + EVERY, EVERY);
    every.set_missed_tick_behavior(MissedTickBehavior::Skip);
    every
}