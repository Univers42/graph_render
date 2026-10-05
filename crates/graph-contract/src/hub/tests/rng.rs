//! splitmix64: the seeded generator the property test drives.
//!
//! One function, 64-bit, no dependency. Chosen over a `u32` LCG because its output
//! passes BigCrush and its state advances with additions, so a chain of "random" choices
//! does not fall into a short cycle the way the small generators do — which matters here,
//! because the property test's claim is that 64 seeds and 40 steps each explore more than
//! one shape.
//!
//! A generator and not `rand`: the crate is dependency-free so graph-core can carry it, and
//! — more to the point — a test reproducible from its seed alone can be re-run by a reader
//! holding the failure (D2: no wall-clock, no unseeded randomness).

/// The generator's state. Seeded, never read from anywhere else.
#[derive(Debug, Clone)]
pub(super) struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator at `seed`.
    pub(super) fn seeded(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    /// The next 64 bits. The three constants are splitmix64's own multipliers; they are
    /// the algorithm, not a tuning choice.
    pub(super) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..bound`.
    ///
    /// **Caveat: modulo bias** — `bound` does not divide `2^64`, so the low values are very
    /// slightly more likely than the high ones. Every use here chooses among at most four
    /// ids or three collections, where a bias of one part in `2^61` cannot change which
    /// cases the property test explores; a use with a large bound would need rejection
    /// sampling instead, and would be a bug worth a `Caveat` of its own.
    pub(super) fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

#[cfg(test)]
mod tests {
    use super::SplitMix64;

    /// splitmix64's published first three outputs for seed 0. Pinned so a refactor of the
    /// arithmetic cannot quietly change *which* cases the property test explores: a
    /// different generator of the same shape would still pass every other test here.
    #[test]
    fn the_sequence_is_splitmix64s_own() {
        let mut rng = SplitMix64::seeded(0);
        let got: Vec<u64> = (0..3).map(|_| rng.next_u64()).collect();
        assert_eq!(
            got,
            [
                0xE220_A839_7B1D_CDAF,
                0x6E78_9E6A_A1B9_65F4,
                0x06C4_5D18_8009_454F
            ]
        );
    }

    #[test]
    fn a_seeded_generator_is_reproducible() {
        let a: Vec<u64> = (0..8)
            .scan(SplitMix64::seeded(7), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..8)
            .scan(SplitMix64::seeded(7), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn below_stays_inside_its_bound() {
        let mut rng = SplitMix64::seeded(3);
        for _ in 0..1000 {
            assert!(rng.below(7) < 7);
        }
    }
}