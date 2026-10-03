//! `numpy.random.SeedSequence`, the hash between an integer seed and a generator's state
//! words. Ported because `default_rng(seed)` is `PCG64(SeedSequence(seed))`
//! (`_pcg64.pyx:132`), and skipping the hash gives a generator that is *a* PCG64 and not
//! *numpy's* PCG64.
//!
//! Two passes, both integer-only (`bit_generator.pyx:341-450`):
//!
//! 1. **`mix_entropy`** fills a four-word pool — each entropy word through [`hashmix`],
//!    which advances a shared multiplier so word `i` depends on every word before it —
//!    and then a full `4 x 4` sweep of [`mix`], so a late entropy bit reaches an early
//!    pool word.
//! 2. **`generate_state`** runs a second chain over the pool, cycling it: eight `u32`
//!    words, read as four little-endian `u64`.
//!
//! **Only the one-entropy-word integer case is ported**, because that is the only one the
//! FA2 reference asks for: `forceatlas.py:122` hands `default_rng` a single `randint`
//! result.
//!
//! Ponytail (scope): a `SeedSequence` given a list, a `spawn_key`, or a pool size other
//! than four is not answered here. Failing input: any `numpy.random` use that wants
//! spawned child streams (`SeedSequence.spawn`) or a custom pool — neither of which a
//! layout start position needs. Direction: a caller outside this crate passing such a
//! seed would get the single-word stream rather than a refusal. Escape hatch:
//! [`SeedSequence::pool`] takes the seed as one `u64`, so a caller that needs a real spawn
//! key must build the pool words itself.

/// numpy's `DEFAULT_POOL_SIZE` (`bit_generator.pyx:57`): the pool is four `u32` words.
const POOL_SIZE: usize = 4;

const INIT_A: u32 = 0x43b0_d7e5;
const MULT_A: u32 = 0x931e_8875;
const INIT_B: u32 = 0x8b51_f9dd;
const MULT_B: u32 = 0x58f3_8ded;
const MIX_MULT_L: u32 = 0xca01_f9dd;
const MIX_MULT_R: u32 = 0x4973_f715;
/// `np.dtype(np.uint32).itemsize * 8 // 2` = 16 (`bit_generator.pyx:64`).
const XSHIFT: u32 = 16;

/// The running multiplier `hashmix` advances as it goes, so it is input *and* output
/// (`bit_generator.pyx:152-158`). A struct rather than a `&mut` parameter because the
/// fill and the sweep share one chain, and threading a pointer through both would be the
/// fifth parameter the house cap forbids.
struct HashConst(u32);

impl HashConst {
    /// `hashmix(value, hash_const)`: xor the running constant in, advance it by `MULT_A`,
    /// multiply, then fold the top half down.
    fn mix(&mut self, mut value: u32) -> u32 {
        value ^= self.0;
        self.0 = self.0.wrapping_mul(MULT_A);
        value = value.wrapping_mul(self.0);
        value ^ (value >> XSHIFT)
    }
}

/// `mix(x, y)`: `MIX_MULT_L * x - MIX_MULT_R * y`, folded (`bit_generator.pyx:160-163`).
fn mix(x: u32, y: u32) -> u32 {
    let result = MIX_MULT_L
        .wrapping_mul(x)
        .wrapping_sub(MIX_MULT_R.wrapping_mul(y));
    result ^ (result >> XSHIFT)
}

/// The four `u64` words `generate_state(4, np.uint64)` hands `pcg64_set_seed`.
///
/// Two 128-bit halves under the C's names, because that is how `pcg64.c:148-159` reads
/// them: `seed[0..2]` is `initstate`, `seed[2..4]` is `initseq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PcgSeedWords {
    /// `initstate.high`.
    pub(super) state_high: u64,
    /// `initstate.low`.
    pub(super) state_low: u64,
    /// `initseq.high`.
    pub(super) inc_high: u64,
    /// `initseq.low`.
    pub(super) inc_low: u64,
}

/// `SeedSequence(entropy).pool`: the four mixed words `generate_state` then draws from.
#[derive(Debug, Clone, Copy)]
pub(super) struct SeedSequence {
    pool: [u32; POOL_SIZE],
}

impl SeedSequence {
    /// `SeedSequence(seed)`: assemble the entropy words, then `mix_entropy`.
    ///
    /// `_int_to_uint32_array` (`bit_generator.pyx:67-79`) is little-endian `u32` words,
    /// lowest bits first, and a seed of `0` is the **single** word `[0]` — not an empty
    /// array, which would leave the pool all zeros instead of hashing one zero word.
    pub(super) fn pool(seed: u64) -> Self {
        let words = entropy_words(seed);
        let mut chain = HashConst(INIT_A);
        let mut pool = [0u32; POOL_SIZE];
        for (i, slot) in pool.iter_mut().enumerate() {
            *slot = chain.mix(words.get(i).copied().unwrap_or(0));
        }
        Self { pool }.sweep(&mut chain, &words)
    }

    /// The `4 x 4` sweep of `mix_entropy`, then the tail that mixes entropy beyond the
    /// pool size (`bit_generator.pyx:362-374`).
    ///
    /// **Sequential in both indices, and the order is load-bearing.** `i_src` is the outer
    /// loop, so entry `i` has already been rewritten by every earlier pass when the sweep
    /// reads it; running the two the other way round gives a different pool and therefore
    /// a different stream from the first draw on.
    fn sweep(mut self, chain: &mut HashConst, words: &[u32]) -> Self {
        for i_src in 0..POOL_SIZE {
            for i_dst in 0..POOL_SIZE {
                if i_src != i_dst {
                    self.pool[i_dst] = mix(self.pool[i_dst], chain.mix(self.pool[i_src]));
                }
            }
        }
        for &extra in words.iter().skip(POOL_SIZE) {
            for slot in self.pool.iter_mut() {
                *slot = mix(*slot, chain.mix(extra));
            }
        }
        self
    }

    /// `generate_state(4, np.uint64)` (`bit_generator.pyx:403-450`): eight `u32` words
    /// through a second chain over the cycled pool, read as four little-endian `u64`.
    pub(super) fn generate_state(self) -> PcgSeedWords {
        let mut chain = INIT_B;
        let mut words = [0u32; POOL_SIZE * 2];
        for (i, slot) in words.iter_mut().enumerate() {
            let mut value = self.pool[i % POOL_SIZE] ^ chain;
            chain = chain.wrapping_mul(MULT_B);
            value = value.wrapping_mul(chain);
            *slot = value ^ (value >> XSHIFT);
        }
        let pair = |lo: usize| (words[lo] as u64) | ((words[lo + 1] as u64) << 32);
        PcgSeedWords {
            state_high: pair(0),
            state_low: pair(2),
            inc_high: pair(4),
            inc_low: pair(6),
        }
    }
}

/// `_int_to_uint32_array(seed)`: little-endian `u32` words, lowest bits first.
fn entropy_words(seed: u64) -> Vec<u32> {
    let mut out = Vec::new();
    let mut rest = seed;
    if rest == 0 {
        return vec![0];
    }
    while rest > 0 {
        out.push(rest as u32);
        rest >>= 32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SeedSequence(1767575729).pool`, the four words `mix_entropy` leaves. Not
    /// observable from Python; it is what the first hashed pass must produce for the
    /// second to land on [`LAYOUT_SEED_WORDS`].
    const LAYOUT_SEED_POOL: [u32; POOL_SIZE] =
        [0xfd3b_e523, 0x8e66_9e33, 0xc482_920b, 0x6cf1_2044];

    /// `SeedSequence(1767575729).generate_state(4, np.uint64)`, the four words
    /// `pcg64_set_seed` receives. These two constants together are the whole hash chain:
    /// the first is `mix_entropy`, the second is `generate_state`.
    const LAYOUT_SEED_WORDS: [u64; 4] = [
        0x569b_4f09_bff9_8037,
        0x8430_4ed1_ec3c_9312,
        0x5756_2eb4_6460_edde,
        0xce36_6cf7_bcb3_c4ed,
    ];

    #[test]
    fn seedsequence_pool_is_the_mixed_entropy_pool() {
        assert_eq!(SeedSequence::pool(1_767_573_729).pool, LAYOUT_SEED_POOL);
    }

    #[test]
    fn seedsequence_generate_state_is_the_four_u64_words_pcg64_is_seeded_with() {
        let words = SeedSequence::pool(1_767_573_729).generate_state();
        let got = [words.state_high, words.state_low, words.inc_high, words.inc_low];
        assert_eq!(got, LAYOUT_SEED_WORDS);
    }

    /// The negative control for the hash chain itself. Without it, a `SeedSequence` that
    /// passed the seed straight through would also satisfy the two tests above on any
    /// stream it happened to be checked against — the pool and the words are what say the
    /// seed was *hashed*.
    #[test]
    fn seedsequence_neighbouring_seeds_move_pool_and_words() {
        let other = SeedSequence::pool(1_767_573_728);
        assert_ne!(other.pool, LAYOUT_SEED_POOL);
        let words = other.generate_state();
        let got = [words.state_high, words.state_low, words.inc_high, words.inc_low];
        assert_ne!(got, LAYOUT_SEED_WORDS);
    }

    /// A pool is four `u32` words wide whatever the seed: a `u64` seed above `2^32` is
    /// two entropy words and the tail loop, not a five-word pool.
    #[test]
    fn seedsequence_a_wide_seed_uses_the_tail_not_a_bigger_pool() {
        let wide = 0x0000_0001_0000_0001u64;
        assert_eq!(entropy_words(wide).len(), 2);
        assert_eq!(SeedSequence::pool(wide).pool.len(), POOL_SIZE);
    }

    /// `_int_to_uint32_array(0)` is `[0]`, not `[]`: an empty entropy array would leave
    /// the pool as `hashmix(0)` four times instead of hashing one zero word through a
    /// chain that starts at `INIT_A`.
    #[test]
    fn seedsequence_entropy_words_keep_a_zero_seed_as_one_word() {
        assert_eq!(entropy_words(0), vec![0]);
        assert_eq!(entropy_words(1), vec![1]);
        assert_eq!(entropy_words(0x1_0000_0001), vec![1, 1]);
    }
}
