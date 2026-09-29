//! **M1c — a command script replays to identical bytes.** A session is driven through every
//! verb it has, in an order where each one is read by the next, and the result is compared
//! twice and against a digest.
//!
//! Comparing two runs of a pure function is the determinism the brief asks for; the digest
//! is the part that catches a *semantic* change. Without it, a mutant that swapped the
//! gravity sign, moved `reheat` to the wrong end, or read `alpha` after the forces instead
//! of before would still replay identically to itself, twice.

use super::digest::sha256_hex;
use super::support;
use crate::layout::force::session::ForceSession;

#[test]
fn the_command_script_replays_to_identical_bytes() {
    let first = support::scripted(7);
    let second = support::scripted(7);
    assert_eq!(support::bits(&first), support::bits(&second));
    assert_eq!(
        sha256_hex(&bits_of_f64(&first)),
        SCRIPT_DIGEST,
        "the script's own bytes, so a change in what any verb *does* fails here"
    );
}

/// `x` then `y`, `f64` little-endian — the same convention as the `f32` goldens, one
/// precision up, because the script's positions are read before any `f32` cast.
fn bits_of_f64(session: &ForceSession) -> Vec<u8> {
    let mut bytes = Vec::new();
    for v in session.xs().iter().chain(session.ys()) {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// The script's own bytes, captured from the implemented session on seed 7 — the case the
/// five determinism tests cannot supply from outside, because the script is this crate's
/// own command list and there is no second implementation to compare it against. It is a
/// *replay* golden, not an oracle golden: it says "this command list produces these bytes,
/// and any change to what a verb does is a change to this constant", and the reviewer's
/// way to check it is the diff.
const SCRIPT_DIGEST: &str = "7aac52efbbb2ed8ef0948d66994c22944db26c73af2bbf4f2e097002d5effb52";
