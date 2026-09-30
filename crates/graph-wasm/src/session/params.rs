//! The force session's parameter wire format: [`LiveParams`]'s thirteen fields as `f64`
//! little-endian, in declaration order, [`LEN`] bytes.
//!
//! **Thirteen `f64`s, read bytewise.** Two decisions here, both forced by the existing ABI
//! and neither of them free:
//!
//! - **A buffer, not thirteen arguments.** The house's four-parameter cap, and the shape
//!   `gm_run` already takes for its (empty) parameters. A layout id stays an index
//!   (`gm_run`'s `layout_id`); a parameter *set* is data, and data crosses the ABI as bytes.
//! - **Read as bytes, not as `f64`s.** `gm_alloc`'s buffers are aligned to `alloc::ALIGN`,
//!
//!   which is 4 — the wire's own word alignment, chosen so a caller can write `u32`/`f32`
//!
//!   directly. An `f64` load from a 4-aligned address is a trap on wasm32, so a 104-byte
//!
//!   parameter buffer handed out by `gm_alloc` is **not** safe to read as thirteen `&f64`s.
//!
//!   `as_chunks::<8>` + [`f64::from_le_bytes`] has no alignment requirement and states the
//!
//!   byte order the wire's other faces already use (`h1-byte-order.md`).
//!
//! `LiveParams::validate` is the only authority on which values are legal; this module
//! refuses a *length* and nothing else, so a refusal about a value keeps naming the field
//! and the range the way graph-core words it.

use graph_core::layout::force::LiveParams;

/// The parameter buffer's byte length: one little-endian `f64` per field.
pub const LEN: usize = 13 * size_of::<f64>();

/// `params` as the wire's bytes, in declaration order.
pub fn encode(params: &LiveParams) -> Vec<u8> {
    let mut out = Vec::with_capacity(LEN);
    for field in ordered(params) {
        out.extend_from_slice(&field.to_le_bytes());
    }
    out
}

/// The [`LiveParams`] a wire buffer of exactly [`LEN`] bytes carries, or `None` for any
/// other length — including `0`.
///
/// **`0` is not this function's answer to "the defaults".** The caller decides that, because
/// an absent parameter buffer and a buffer of the wrong length are different mistakes and
/// the second one must not be silently read as the first (see [`Code::SessionParamsInvalid`]`
/// in `crate::errors`).
pub fn decode(bytes: &[u8]) -> Option<LiveParams> {
    if bytes.len() != LEN {
        return None;
    }
    let mut fields = [0.0; 13];
    let (chunks, _) = bytes.as_chunks::<8>();
    for (slot, chunk) in fields.iter_mut().zip(chunks) {
        *slot = f64::from_le_bytes(*chunk);
    }
    Some(LiveParams {
        charge: fields[0],
        theta: fields[1],
        distance_min: fields[2],
        distance_max: fields[3],
        link_distance: fields[4],
        link_strength_scale: fields[5],
        collide_radius: fields[6],
        center_strength: fields[7],
        gravity: fields[8],
        velocity_decay: fields[9],
        alpha_decay: fields[10],
        alpha_min: fields[11],
        initial_alpha: fields[12],
    })
}

/// Every field, in declaration order — the one place the wire's field order is written, and
/// the only place `encode` and `decode` could otherwise disagree about it.
fn ordered(params: &LiveParams) -> [f64; 13] {
    [
        params.charge,
        params.theta,
        params.distance_min,
        params.distance_max,
        params.link_distance,
        params.link_strength_scale,
        params.collide_radius,
        params.center_strength,
        params.gravity,
        params.velocity_decay,
        params.alpha_decay,
        params.alpha_min,
        params.initial_alpha,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field crosses the wire, including the ones no hash gate perturbs: a field the
    /// round trip forgot would be a parameter a host could set that never reached the
    /// simulation, and the picture would look plausible.
    #[test]
    fn every_field_round_trips_bit_for_bit() {
        let params = LiveParams {
            charge: -1.5,
            theta: 0.3,
            distance_min: 0.0,
            distance_max: 1_000_000.0,
            link_distance: 1.0,
            link_strength_scale: 0.0,
            collide_radius: 400.0,
            center_strength: 0.25,
            gravity: 1.0,
            velocity_decay: 0.99,
            alpha_decay: 0.5,
            alpha_min: 0.125,
            initial_alpha: 1.0,
        };
        let bytes = encode(&params);
        assert_eq!(bytes.len(), LEN, "LEN is one f64 per field");
        assert_eq!(decode(&bytes), Some(params));
    }

    /// The defaults themselves are a value, not an absence: a buffer carrying them and no
    /// buffer at all must reach the same session, and this is the statement that the
    /// `LiveParams::default()` the ABI starts from is the one graph-core defines.
    #[test]
    fn the_defaults_cross_the_wire_unchanged() {
        let bytes = encode(&LiveParams::default());
        assert_eq!(decode(&bytes), Some(LiveParams::default()));
        assert!(
            bytes.windows(8).any(|w| w != 0.0f64.to_le_bytes()),
            "not all zeros"
        );
    }

    /// A length that is not [`LEN`] is refused, and `0` is not read as "the defaults" here:
    /// the export decides that, so a short buffer cannot quietly become a default session.
    #[test]
    fn a_length_that_is_not_the_buffers_own_is_refused() {
        let bytes = encode(&LiveParams::default());
        let mut long = bytes.clone();
        long.extend_from_slice(&[0; 8]);
        for wrong in [&bytes[..0], &bytes[..8], &bytes[..LEN - 1], long.as_slice()] {
            assert_eq!(decode(wrong), None, "{} bytes must not decode", wrong.len());
        }
    }

    /// Little-endian, read bytewise: a buffer whose bytes are in the other order is not this
    /// one's buffer, and the decode must refuse to be byte-order agnostic about it.
    #[test]
    fn the_bytes_are_little_endian_regardless_of_alignment() {
        let bytes = encode(&LiveParams {
            theta: 1.0,
            initial_alpha: 0.25,
            ..LiveParams::default()
        });
        // theta is the second field, so its `1.0` (0x3ff0000000000000) starts at byte 8:
        // little-endian is 00 00 00 00 00 00 f0 3f, big-endian 3f f0 00 00 00 00 00 00.
        assert_eq!(&bytes[8..16], &[0, 0, 0, 0, 0, 0, 0xf0, 0x3f]);
        let mut reversed = bytes;
        reversed.reverse();
        assert_ne!(
            decode(&reversed).map(|params| params.theta),
            Some(1.0),
            "the other byte order must not decode to the same value"
        );
    }
}
