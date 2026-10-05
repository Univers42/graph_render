//! The push half of an overlap correction, the test already decided.
//!
//! [`Gather`](super::gather::Gather) filters its window first and then pushes only the hits,
//! so the push must not re-run the test the filter made on the same `dx * dx + dy * dy`.
//! [`resolve`](super::resolve) keeps the test and calls [`hit`](hit), so the branched
//! reference the tests compare against cannot drift from the filtered path.
//!
//! Caveat: `hit` trusts its caller. A `dx`/`dy` whose `dx * dx + dy * dy` is NaN or not under
//! `c.d2` reaches a `sqrt` and a division the test would have refused; the NaN case is what
//! keeps the querying slot's own place out of its own window.

use super::Contact;
use crate::rng::jiggle;

/// The two jiggle passes collide uses, Barnes-Hut's own numbering.
pub(super) const PASS_X: u32 = 4;
pub(super) const PASS_Y: u32 = 5;

/// One overlap already known to be under the diameter: `(dx * push, dy * push)`, for the
/// caller to add to its own accumulator in its own order. `ids` is called only for a jiggle,
/// which most overlaps never need, so it is a closure over the grid rather than a pair read
/// up front. The arithmetic is `resolve`'s, unchanged.
pub(super) fn hit(
    c: Contact,
    ids: impl Fn() -> (u32, u32),
    (mut dx, mut dy): (f64, f64),
) -> (f64, f64) {
    let mut l = dx * dx + dy * dy;
    if dx == 0.0 {
        dx = jiggle(c.seed, c.tick, PASS_X, ids());
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(c.seed, c.tick, PASS_Y, ids());
        l += dy * dy;
    }
    let dist = f64::sqrt(l);
    let push = (c.reach - dist) / dist * 0.5;
    (dx * push, dy * push)
}
