//! The seeded start, `random_positions` (`simulation.py:1088-1094`).

use crate::rng::Pcg64;

/// `((default_rng(seed).random((n, 3)) - 0.5) * scale).astype(DTYPE)`, row-major.
///
/// **The `f64` subtraction and scaling happen before the narrowing.** `Generator.random`
/// fills C-order, so the draw order is `pos[0][0], pos[0][1], pos[0][2], pos[1][0], ...` and
/// a row-major flat loop consumes the stream in exactly that order. Measured first row at
/// the layout seed, `n = 77`: `[1.4719001, 0.8315206, 0.57074285]` as `f32`, which is what
/// `forcesim/tests.rs` pins.
///
/// `dimensions < 3` would zero the z column afterwards (`simulation.py:1092-1093`); this
/// layout is 3-D at every `n`, so there is no branch to port.
pub(super) fn start_positions(n: usize, seed: u64, scale: f64) -> Vec<f32> {
    let mut rng = Pcg64::new(seed);
    (0..3 * n)
        .map(|_| ((rng.next_f64() - 0.5) * scale) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `random_positions(77, scale=5.0, seed=1767573729, dimensions=3)[0]`, as `f32` bits.
    /// Measured in `ge-python-oracle`, pasted in `docs/measurements/sg-fa2-seed.md`.
    const ROW0: [u32; 3] = [0x3fbc_6739, 0x3f54_de89, 0x3f12_1c34];

    #[test]
    fn the_first_row_is_the_measured_random_positions_row() {
        let pos = start_positions(77, 1_767_573_729, 5.0);
        let bits: Vec<u32> = pos[..3].iter().map(|&v| v.to_bits()).collect();
        assert_eq!(bits, ROW0.to_vec());
    }

    /// `n` rows of three draws each: the stream is consumed in C order, so a transposed
    /// fill would give the right multiset of numbers in the wrong places and the first row
    /// would already differ.
    #[test]
    fn the_stream_is_consumed_row_major() {
        let pos = start_positions(3, 1_767_573_729, 5.0);
        assert_eq!(pos.len(), 9);
        let mut flat = Pcg64::new(1_767_573_729);
        let want: Vec<f32> = (0..9)
            .map(|_| ((flat.next_f64() - 0.5) * 5.0) as f32)
            .collect();
        assert_eq!(pos, want);
    }

    #[test]
    fn a_neighbouring_seed_moves_every_coordinate() {
        assert_ne!(
            start_positions(4, 1_767_573_728, 5.0),
            start_positions(4, 1_767_573_729, 5.0)
        );
    }
}
