//! The seeded start of an sfdp layout, and the glibc-compatible generator behind it.
//!
//! Graphviz seeds every run with `srand(seed)` and then draws `drand()` per coordinate
//! (`lib/sfdpgen/spring_electrical.c:282-284`). Reproducing that stream bit-for-bit needs
//! glibc's `random()` TYPE_3 additive-feedback generator, which this module implements
//! exactly: it is integer-only, so it is bit-identical everywhere (D1, D2 do not apply —
//! there is no float until the final division by `2^31`).

/// glibc's `random()` in its default TYPE_3 form: 31 words of state, seeded through the
/// Schrage trick so the initialisation stays inside 32-bit signed arithmetic, then advanced
/// by the additive feedback the TYPE_3 recurrence specifies.
///
/// The recurrence is the one glibc documents for TYPE_3:
/// `r[i] = r[i-3] + r[i-31] (mod 2^32)`, output `r[i] >> 1` scaled to 31 bits.
pub struct Glibc {
    state: [u32; Self::DEGREE],
    front: usize,
    rear: usize,
}

impl Glibc {
    /// TYPE_3's degree, the number of words in the additive-feedback state.
    const DEGREE: usize = 31;

    /// TYPE_3's separation: the two pointers into the state walk `sep` words apart.
    const SEP: usize = 3;

    /// A generator seeded exactly as glibc's `srandom` seeds `random()` for TYPE_3.
    pub fn seeded(seed: u32) -> Self {
        let mut state = [0u32; Self::DEGREE];
        state[0] = seed;
        // glibc initialises word `i` with the Schrage form of `seed * 16807`, which keeps the
        // product inside `int32` without a wider multiply, and folds a negative result back up
        // by `2^31 - 1`. Omitting that fold is off by one ulp of the *state*, and the whole
        // stream with it.
        for i in 1..Self::DEGREE {
            let previous = i64::from(state[i - 1]);
            let hi = previous / 127_773;
            let lo = previous % 127_773;
            let word = 16_807 * lo - 2_836 * hi;
            state[i] = if word < 0 { word + 2_147_483_647 } else { word } as u32;
        }
        // glibc starts the two pointers `sep` apart and discards `10 * deg` outputs; the uniform
        // advance below lands them exactly where glibc's post-warm-up table has them.
        let mut rng = Self {
            state,
            front: Self::SEP,
            rear: 0,
        };
        for _ in 0..10 * Self::DEGREE {
            rng.next();
        }
        rng
    }

    /// One 31-bit word: `state[front] += state[rear]`, then `>> 1`.
    ///
    /// Both pointers advance by one, uniformly. The published description of TYPE_3 says the
    /// recurrence is `r[i] = r[i-3] + r[i-31]`, which reads as though `rptr` skips an extra
    /// word when `fptr` wraps; implementing it that way produces a plausible stream that
    /// agrees with glibc on nothing. Measured against `initstate_r`'s own post-warm-up state
    /// table, the uniform advance is what reproduces glibc's first eight outputs exactly.
    fn next(&mut self) -> u32 {
        self.state[self.front] = self.state[self.front].wrapping_add(self.state[self.rear]);
        let value = self.state[self.front] >> 1;
        self.front = (self.front + 1) % Self::DEGREE;
        self.rear = (self.rear + 1) % Self::DEGREE;
        value
    }

    /// One draw in `[0, 1]`, Graphviz's `drand()`, which is `rand()/(double)RAND_MAX`
    /// (`lib/sparse/general.c:25-27`) — note the divisor is `RAND_MAX` = 2^31-1, **not**
    /// 2^31, and the result may therefore be exactly 1. That is the reference's arithmetic and
    /// the port keeps it: the alternative (`/ 2^31`) is a different generator.
    pub fn unit(&mut self) -> f64 {
        self.next() as f64 / (Self::RAND_MAX as f64)
    }

    /// glibc's `RAND_MAX` for `rand()`, which is `2^31 - 1`.
    const RAND_MAX: u32 = 2_147_483_647;

    /// `count` start positions, node by node: Graphviz fills `x[0..dim*n]` from `drand()` in
    /// index order (`spring_electrical.c:284`), x and y interleaved per node.
    ///
    /// **On a generator the caller keeps, not on a seed.** The reference draws the prolongation
    /// jitter from the same stream the start came from (`spring_electrical.c:1155`), so a port
    /// that reseeds per step places every node somewhere else.
    ///
    /// Gather form (D10): entry `i` depends on draws `2i` and `2i+1` and on nothing else, so
    /// the sequence is reproducible and the order is the dense node order.
    pub fn positions(&mut self, count: u32) -> Vec<[f64; 2]> {
        (0..count).map(|_| [self.unit(), self.unit()]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_draw_is_a_unit_in_the_unit_interval() {
        let mut rng = Glibc::seeded(1);
        for i in 0..10_000 {
            let u = rng.unit();
            assert!((0.0..=1.0).contains(&u), "draw {i} was {u}");
        }
    }

    /// **The generator is pinned against glibc itself**, not against a restatement of its
    /// algorithm. These are the first eight outputs of `srand(1); rand()` and of
    /// `srand(1); rand()/(double)RAND_MAX` from a C program linked against the system glibc,
    /// copied here. A recurrence that is merely *similar* to TYPE_3 passes every other test in
    /// this file and still starts every layout somewhere else.
    #[test]
    fn the_stream_is_glibc_s_rand_exactly() {
        let words = |seed| {
            let mut rng = Glibc::seeded(seed);
            (0..8).map(|_| rng.next()).collect::<Vec<_>>()
        };
        assert_eq!(
            words(1),
            vec![
                1_804_289_383,
                846_930_886,
                1_681_692_777,
                1_714_636_915,
                1_957_747_793,
                424_238_335,
                719_885_386,
                1_649_760_492
            ],
            "srand(1) rand() diverges from glibc"
        );
        assert_eq!(
            words(2),
            vec![
                1_505_335_290,
                1_738_766_719,
                190_686_788,
                260_874_575,
                747_983_061,
                906_156_498,
                1_502_820_864,
                142_559_277
            ],
            "srand(2) rand() diverges from glibc"
        );
    }

    /// `drand()` is `rand()/(double)RAND_MAX` (`lib/sparse/general.c:25-27`) — the divisor is
    /// 2^31-1, so a port that divides by 2^31 instead is off by a factor in the last bits on
    /// every coordinate of every layout. Pinned to the same C program's printed doubles.
    #[test]
    fn drand_matches_graphviz_s_divisor_exactly() {
        let draws = |seed| {
            let mut rng = Glibc::seeded(seed);
            (0..4).map(|_| rng.unit()).collect::<Vec<_>>()
        };
        for (got, want) in draws(1).iter().zip([
            0.840_187_717_154_709_5,
            0.394_382_926_819_093_04,
            0.783_099_223_758_605_9,
            0.798_440_033_476_073_3,
        ]) {
            assert!((got - want).abs() < 1e-17, "{got} vs glibc's {want}");
        }
    }

    /// A different seed must give a different first draw, or the seed is not reaching the
    /// generator — the failure this whole port is most exposed to.
    #[test]
    fn the_seed_reaches_the_stream() {
        assert_ne!(Glibc::seeded(1).unit(), Glibc::seeded(7).unit());
        assert_ne!(Glibc::seeded(7).unit(), Glibc::seeded(99).unit());
    }

    #[test]
    fn the_same_seed_replays_the_same_stream() {
        let draw = |seed| {
            let mut rng = Glibc::seeded(seed);
            (0..64).map(|_| rng.unit().to_bits()).collect::<Vec<_>>()
        };
        assert_eq!(draw(123), draw(123));
        assert_ne!(draw(123), draw(124));
    }

    /// Start positions are `2 * count` draws in index order, so entry `i` is reproducible on
    /// its own and the whole vector is gather-stable.
    #[test]
    fn positions_are_index_order_and_reproducible() {
        let draw = |seed| Glibc::seeded(seed).positions(5);
        let positions = draw(1);
        assert_eq!(positions.len(), 5);
        assert_eq!(positions, draw(1));
        assert_ne!(positions, draw(2));
        for (i, p) in positions.iter().enumerate() {
            for c in p {
                assert!((0.0..=1.0).contains(c), "node {i} at {c}");
            }
        }
    }

    /// The stream continues rather than restarting: two draws of `count` in a row are the same
    /// sequence of values as one draw of `2 * count`, which is what lets the reference's
    /// `prolongate` take its jitter from the start's generator.
    #[test]
    fn positions_continue_the_stream_they_are_given() {
        let mut split = Glibc::seeded(1);
        let first = split.positions(2);
        let second = split.positions(2);
        let whole = Glibc::seeded(1).positions(4);
        assert_eq!(first, whole[..2].to_vec());
        assert_eq!(second, whole[2..].to_vec());
    }
}
