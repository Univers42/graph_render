//! Explicit, threaded randomness for layout (`prompt.md` §6, devil C8). Three kinds:
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
//! - **The reference's own generator** — [`Mt19937`], numpy's legacy `RandomState`, for the
//!   layouts whose conformance rows compare bytes against SciGraphs. It exists beside
//!   [`Mulberry32`] rather than replacing it: only a SciGraphs arm needs these exact
//!   numbers, and a motor-side default stays on the crate's own stream.

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
    jiggle_of(fold(seed, tick, pass, lo_hi) >> 11)
}

/// [`jiggle`]'s last step, split out so it can be tested on its own: the 53-bit word
/// `w = h >> 11` becomes the nudge `(w · 2^-53 - 0.5) · 1e-6`.
///
/// **`w == 1 << 52` is mapped to `1 << 52 + 1`, the only word this function moves.** Left
/// alone it makes `u` exactly `0.5`, so the nudge is exactly `+0.0` — the input a caller's
/// zero-jiggle guard exists to catch, so returning it defeats the guard instead of feeding
/// it. It is one word in 2^53 of the draws, and it costs nothing: `fold`'s output feeds
/// nothing else, so no hash, no quadtree key and no registered layout output can move with
/// it unless a draw actually landed on that word — which `hashgate --seeds 8` is the
/// evidence against (see `docs/measurements/fix-force-jiggle.md`). Ponytail: the neighbour
/// is as arbitrary as the value it replaces — both are "not 0.5"; what it gets wrong is
/// that a pair nudged by `±5e-7` instead of `0.0` is nudged at all. Escape hatch: none
/// needed, the caller supplies `tick`/`pass` if the nudge must change.
fn jiggle_of(w: u64) -> f64 {
    let w = if w == 1u64 << 52 { (1u64 << 52) + 1 } else { w };
    let u = w as f64 * (1.0 / (1u64 << 53) as f64);
    (u - 0.5) * 1e-6
}

/// The generator the *reference* draws from, ported so a coordinate can be SciGraphs'
/// rather than merely like it.
///
/// **numpy's legacy `RandomState`, seeded the legacy way.** Two separate facts, and confusing
/// them is the easy way to get a stream that is nearly right:
///
/// - **The double.** `random_sample` is the 53-bit `genrand_res53` construction below — 27
///   bits from one tempered word and 26 from the next. The modern `Generator.random()` uses
///   that same two-word double, so the double is not what distinguishes the two APIs.
/// - **The seed.** `RandomState(seed)` runs `init_genrand(seed)` (`_mt19937.pyx`): `mt[0] =
///   seed`, then `mt[i] = 1812433253 * (mt[i-1] ^ (mt[i-1] >> 30)) + i`. The modern
///   `Generator(MT19937(seed))` does **not** — its state word 0 is `0x8000_0000`, not the
///   seed, so `Generator(MT19937(s)).random()` and `RandomState(s).rand()` disagree from the
///   first value even at the same integer `s` (measured, numpy 2.3.3, `s = 981798123`:
///   `0x1.d8b00910d3e10p-3` against `0x1.d2b611eed9d7bp-1`). Seeding is therefore part of the
///   port, and the pinned vectors below are what say which stream this is.
///
/// The state is **integer-only** and every step is xor/shift/`wrapping_*`, so the stream is
/// bit-identical on every target (D1, D2, D10). The one float is the final division in
/// [`Mt19937::next_f64`], in the order the reference writes it.
pub(crate) struct Mt19937 {
    /// The tempered-word block; `index` reads it, `twist` rewrites it in place.
    mt: [u32; Self::N],
    index: usize,
}

impl Mt19937 {
    /// The state block size, `N` in the reference's `_mt19937.pyx`.
    const N: usize = 624;
    /// The twist's recurrence offset, `M`: where the "two blocks ago" word comes from.
    const M: usize = 397;
    /// The twist's odd-word xor, `MATRIX_A`.
    const MATRIX_A: u32 = 0x9908_b0df;

    /// `init_genrand(seed)`: `mt[0] = seed`, then the reference's own recurrence — every
    /// later word depends on the one before it, so the block must be built in order and
    /// cannot be filled in parallel.
    pub(crate) fn new(seed: u32) -> Self {
        let mut mt = [0u32; Self::N];
        mt[0] = seed;
        for i in 1..Self::N {
            let prev = mt[i - 1];
            mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        Self { mt, index: Self::N }
    }

    /// The twist: one pass that folds the block's lag-`M` words back into it. Runs only
    /// when the block is spent, which is what makes the stream sequential.
    fn twist(&mut self) {
        for i in 0..Self::N {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % Self::N] & 0x7fff_ffff);
            let mut word = self.mt[(i + Self::M) % Self::N] ^ (y >> 1);
            if y & 1 == 1 {
                word ^= Self::MATRIX_A;
            }
            self.mt[i] = word;
        }
        self.index = 0;
    }

    /// `genrand_uint32`: the next tempered word, twisting first if the block is spent.
    pub(crate) fn next_u32(&mut self) -> u32 {
        if self.index >= Self::N {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `random_sample`, the `f64` in `[0, 1)`: `(a * 2^26 + b) / 2^53` with `a` the top 27
    /// bits of one word and `b` the top 26 of the next, in that order.
    ///
    /// **Two words per draw, not one.** 27 + 26 is the 53 bits a double keeps, and the two
    /// halves come from two words because that is where they are; a generator that drew one
    /// word per value would be a different stream even with the same seeding.
    pub(crate) fn next_f64(&mut self) -> f64 {
        let a = f64::from(self.next_u32() >> 5);
        let b = f64::from(self.next_u32() >> 6);
        (a * 67108864.0 + b) / 9007199254740992.0
    }
}

#[cfg(test)]
mod tests;
