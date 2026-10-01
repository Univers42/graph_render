//! The two random sequences `fdp` draws on, in the exact form the reference draws them.
//!
//! This is the one place where a native port of `fdp` has to agree with the C library
//! rather than with itself, so both sequences are reproduced rather than approximated:
//!
//! 1. **`srand48`/`drand48`** — the 48-bit linear congruential generator
//!    `X <- (0x5DEECE66D * X + 0xB) mod 2^48`, seeded by `X0 = (seed << 16) | 0x330E`,
//!    returning `X / 2^48` per draw. `initPositions` (`tlayout.c:487,554-561`) seeds it
//!    with `T_seed` and then draws exactly two values per node, x before y, in node order.
//!    That draw order **is** the layout, so there is no freedom here: this is a named
//!    sequence, not a call into a shared generator.
//! 2. **`rand()`** — the reference's tie-break jitter (`tlayout.c:194-198`, `tlayout.c:
//!    295-299`, `xlayout.c:127-131`), drawn only when two positions coincide bit for bit.
//!    `fdp` never calls `srand`, so the stream is glibc's `TYPE_3` additive feedback from
//!    the default seed 1, and [`GlibcRand`] reproduces that generator exactly.
//!
//! The house's own [`Mulberry32`](crate::rng::Mulberry32) is deliberately **not** used
//! here. D5 forbids a global generator and this has none; but a differential against
//! Graphviz measures the gap between two *specific* streams, so substituting a different
//! one would not measure this port, it would measure a different layout.

/// `2^48`: the modulus of the 48-bit generator and the scale of its output.
const TWO_POW_48: f64 = 281_474_976_710_656.0;

/// The 48-bit multiplier and increment of the POSIX `drand48` family.
const MULTIPLIER: u64 = 0x5DEE_CE66D;
const INCREMENT: u64 = 0xB;

/// The 16 low bits `srand48` forces into the state so the first draws are well spread.
const SEED_TAG: u64 = 0x330E;

/// The POSIX `drand48` sequence, seeded exactly as `srand48` seeds it.
pub(super) struct Rand48 {
    state: u64,
}

impl Rand48 {
    /// `srand48(seed)`: the reference's only seeding call, at `tlayout.c:487`.
    pub(super) fn new(seed: u64) -> Self {
        Self {
            state: ((seed & 0xFFFF_FFFF) << 16) | SEED_TAG,
        }
    }

    /// `drand48()`, in `[0, 1)`. The multiply and add run in `u64` and are masked to 48
    /// bits, which is the generator's own arithmetic: no `f64` rounding can enter, and so
    /// the stream is bit-identical to the reference's on every platform.
    pub(super) fn next_f64(&mut self) -> f64 {
        self.state = MULTIPLIER.wrapping_mul(self.state).wrapping_add(INCREMENT) & 0xFFFF_FFFF_FFFF;
        self.state as f64 / TWO_POW_48
    }
}

/// glibc's `TYPE_3` additive-feedback generator: `r[i] = r[i-3] + r[i-31] (mod 2^32)`.
const DEGREE: usize = 31;
const SEPARATION: usize = 3;

/// The modulus of the Lehmer generator glibc seeds `TYPE_3` with.
const LEHMER_MODULUS: u32 = 2_147_483_647;
const LEHMER_MULTIPLIER: u32 = 16_807;
const LEHMER_QUOTIENT: u32 = 127_773;
const LEHMER_REMAINDER_CORRECTION: u32 = 2_836;

/// glibc's `rand()` as `fdp`'s three tie-break loops see it: never seeded, so always the
/// `srandom(1)` stream. The 31-word state is the whole generator, so this is a copy of
/// the C library's own recurrence rather than a table of the first few thousand values.
pub(super) struct GlibcRand {
    state: [u32; DEGREE],
    /// How many outputs have been handed out, so the two pre-recurrence seeds are not
    /// re-emitted.
    drawn: u32,
}

impl GlibcRand {
    /// A fresh `rand()`. The reference never calls `srand`, so the default seed is the
    /// only seed there is.
    pub(super) fn new() -> Self {
        Self {
            state: seed_state(1),
            drawn: 0,
        }
    }

    /// `5 - rand() % 10`, one draw, in `-4..=5`. That is the whole of what the three
    /// tie-break loops use the sequence for.
    pub(super) fn jitter(&mut self) -> f64 {
        f64::from(5 - (self.next() % 10))
    }

    /// The next value of the stream: the seeded words verbatim until the recurrence has
    /// both of its taps available, then their sum.
    fn next(&mut self) -> u32 {
        let n = self.drawn as usize;
        self.drawn = self.drawn.wrapping_add(1);
        if n < DEGREE {
            return self.state[n];
        }
        self.state[n % DEGREE].wrapping_add(self.state[(n + DEGREE - SEPARATION) % DEGREE])
    }
}

/// The 31-word `TYPE_3` seed table for `srandom(seed)`: the Lehmer generator of
/// `srandom(1)`, stepped 31 times by the Schrage form glibc uses so the intermediate
/// product cannot overflow.
fn seed_state(seed: u32) -> [u32; DEGREE] {
    let mut state = [0_u32; DEGREE];
    state[0] = seed;
    for i in 1..DEGREE {
        let previous = state[i - 1];
        let quotient = previous / LEHMER_QUOTIENT;
        let remainder = previous % LEHMER_QUOTIENT;
        // Signed, because Schrage's form really does go negative (`remainder == 0` with a
        // large `quotient`), and the correction is a wrap of the signed value into
        // `[0, 2^31)`. Unsigned arithmetic here would give a different table.
        let word = i64::from(LEHMER_MULTIPLIER) * i64::from(remainder)
            - i64::from(LEHMER_REMAINDER_CORRECTION) * i64::from(quotient);
        state[i] = if word < 0 {
            (word + i64::from(LEHMER_MODULUS)) as u32
        } else {
            word as u32
        };
    }
    state
}
