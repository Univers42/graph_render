//! Explicit, threaded randomness for layout (`prompt.md` §6, devil C8). Two kinds, and
//! only two:
//!
//! - **Sequential** — initial state *outside* the per-tick kernels (e.g. a layout's start
//!   positions). [`Mulberry32`] already exists in `synthetic.rs`; this module re-exports
//!   it rather than adding a second sequential generator, per C8's instruction.
//! - **Counter-based** — randomness *inside* a kernel (Barnes-Hut/link/collide jiggle for
//!   exactly-coincident points). d3 draws jiggle from one shared LCG consumed in quadtree
//!   visit order (`d3-force/src/jiggle.js`, `lcg.js`): a sequential generator threaded
//!   through a gather would make the result depend on visit order, and later on thread
//!   order (Phase 11) — exactly what D10 forbids. [`jiggle`] is a pure function of
//!   `(seed, tick, pass, i, j)` instead: order-independent and thread-safe for free.

pub(crate) use crate::synthetic::Mulberry32;

/// MurmurHash3's `fmix64` finalizer (Austin Appleby, public domain): full avalanche of a
/// 64-bit word using only xor/shift/`wrapping_mul` — never a transcendental, so it is
/// exact and bit-identical on every target (D1, D2 do not even apply: there is no float
/// here until the final division).
const fn fmix64(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    k ^= k >> 33;
    k
}

/// Folds `(seed, tick, pass, lo, hi)` into one 64-bit word, three avalanche rounds deep so
/// a one-bit change in any input differs in about half the output bits. `(lo, hi)` travel
/// as one pair, not two more parameters (house cap: `refactor-rust.md`'s max 4).
const fn fold(seed: u32, tick: u32, pass: u32, pair: (u32, u32)) -> u64 {
    let (lo, hi) = pair;
    let a = fmix64((seed as u64) ^ ((tick as u64) << 32));
    let b = fmix64(a ^ ((pass as u64) << 32) ^ lo as u64);
    fmix64(b ^ ((hi as u64) << 32) ^ a)
}

/// A stateless, counter-based nudge for two exactly-coincident points, the same shape as
/// d3's `jiggle(random) = (random() - 0.5) * 1e-6` (`jiggle.js:1-3`) but keyed on
/// `(seed, tick, pass, min(i, j), max(i, j))` instead of shared sequential state: node `i`
/// and node `j` computing the *same* pair see the same nudge no matter which one runs
/// first, or on which thread (C8) — the property a sequential LCG cannot offer a gather.
///
/// `pass` distinguishes kernels sharing one tick (link vs many-body vs collide) so they do
/// not accidentally draw the same stream.
///
/// Ponytail: exactly-coincident points are the only failing input this exists for; the
/// nudge is a deterministic direction, not a physically meaningful one — two nodes at the
/// same point separate the same way every run, never a different way. Escape hatch: the
/// caller supplies `tick`/`pass` so a still-coincident pair keeps nudging on later ticks
/// instead of sticking. `i, j` travel as one pair, not two more parameters (house cap:
/// `refactor-rust.md`'s max 4) — order does not matter, [`fold`] sorts them.
pub(crate) fn jiggle(seed: u32, tick: u32, pass: u32, ij: (u32, u32)) -> f64 {
    let (i, j) = ij;
    let lo_hi = if i <= j { (i, j) } else { (j, i) };
    let h = fold(seed, tick, pass, lo_hi);
    let u = (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
    (u - 0.5) * 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jiggle_is_order_independent_in_i_and_j() {
        for (i, j) in [(0u32, 1u32), (5, 2), (100, 99), (0, 0)] {
            assert_eq!(jiggle(7, 3, 0, (i, j)), jiggle(7, 3, 0, (j, i)));
        }
    }

    #[test]
    fn jiggle_is_a_pure_function_of_its_five_inputs() {
        let a = jiggle(1, 2, 3, (4, 5));
        let b = jiggle(1, 2, 3, (4, 5));
        assert_eq!(a.to_bits(), b.to_bits());
        assert_ne!(a.to_bits(), jiggle(1, 2, 3, (4, 6)).to_bits(), "j moved");
        assert_ne!(a.to_bits(), jiggle(1, 2, 4, (4, 5)).to_bits(), "pass moved");
        assert_ne!(a.to_bits(), jiggle(1, 3, 3, (4, 5)).to_bits(), "tick moved");
        assert_ne!(a.to_bits(), jiggle(2, 2, 3, (4, 5)).to_bits(), "seed moved");
    }

    #[test]
    fn jiggle_stays_in_d3s_magnitude_band_and_is_finite() {
        for n in 0..2_000u32 {
            let v = jiggle(0x9E37_79B9, n / 37, n % 5, (n, n.wrapping_mul(31)));
            assert!(v.is_finite(), "{n}: {v}");
            assert!((-5e-7..5e-7).contains(&v), "{n}: {v} out of band");
        }
    }

    #[test]
    fn run_twice_is_bit_identical_determinism() {
        let run = || {
            (0..500)
                .map(|n| jiggle(9, n, 1, (n, n + 1)).to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
