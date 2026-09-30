//! A test-only SHA-256, because the closed dependency allow-list (`prompt.md` §3.1:
//! `libm`, `indexmap`, `petgraph`) has no hash crate and the session's goldens have to
//! be **byte-exact** — D7 names an explicit algorithm and `DefaultHasher` is banned.
//!
//! The goldens here are digests of a few kilobytes of position bytes each, and 65 of
//! them do not fit in a source file as literals. A digest is only as good as its
//! algorithm, so [`sha256_matches_the_published_vectors`] pins this one against the
//! published SHA-256 test vectors: if the compression loop were wrong, the goldens it
//! produces would be wrong *and* the test would be blind, which is the failure mode a
//! hand-rolled hash exists to prevent.
//!
//! Test-only by construction: this module is under `#[cfg(test)]` and is never part of
//! the library surface.

/// The 64 round constants, FIPS 180-4 §4.2.2.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The initial hash value, FIPS 180-4 §5.3.3.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The digest of `message` as 64 lowercase hex characters.
pub fn sha256_hex(message: &[u8]) -> String {
    let mut state = H0;
    let (blocks, rest) = message.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }
    pad_and_compress(&mut state, rest, message.len());
    state.iter().map(|word| format!("{word:08x}")).collect()
}

/// The padding block(s): `0x80`, zeros, then the length in **bits** as a big-endian
/// `u64`. Split out so [`sha256_hex`] stays under the 40-line cap.
fn pad_and_compress(state: &mut [u32; 8], rest: &[u8], len: usize) {
    let mut block = [0u8; 64];
    block[..rest.len()].copy_from_slice(rest);
    block[rest.len()] = 0x80;
    if rest.len() + 9 > 64 {
        compress(state, &block);
        block = [0u8; 64];
    }
    let bits = (len as u64).wrapping_mul(8);
    block[56..].copy_from_slice(&bits.to_be_bytes());
    compress(state, &block);
}

/// One 64-byte block: the message schedule, then the 64 rounds.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, word) in w.iter_mut().enumerate().take(16) {
        *word = u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().expect("four bytes"));
    }
    for i in 16..64 {
        w[i] = w[i - 16]
            .wrapping_add(sigma0(w[i - 15]))
            .wrapping_add(w[i - 7])
            .wrapping_add(sigma1(w[i - 2]));
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let t1 = h
            .wrapping_add(big1(e))
            .wrapping_add((e & f) ^ (!e & g))
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let t2 = big0(a).wrapping_add((a & b) ^ (a & c) ^ (b & c));
        (h, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
    }
    for (word, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(add);
    }
}

fn big0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

fn big1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

fn sigma0(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

fn sigma1(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    /// FIPS 180-4 / the widely published SHA-256 test vectors, including the two
    /// padding boundary cases: 55 bytes fills the first block exactly (`+0x80` at 55,
    /// length at 56..64), 56 spills into a second. A padding bug that only shows up at
    /// one of those lengths is the bug this pins.
    #[test]
    fn sha256_matches_the_published_vectors() {
        for (message, want) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
            (
                &"a".repeat(55),
                "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
            ),
            (
                &"a".repeat(56),
                "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
            ),
            (
                &"a".repeat(64),
                "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
            ),
        ] {
            assert_eq!(sha256_hex(message.as_bytes()), want, "{message:?}");
        }
    }
}
