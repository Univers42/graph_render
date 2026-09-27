//! graph-core — the motor. Pure: no I/O, no async, no environment, no clock, no
//! wasm-bindgen. It compiles for `wasm32-unknown-unknown` and native in every phase,
//! and every transcendental goes through `libm` so both targets round alike (D1).
//!
//! Phase 0 skeleton: no algorithm yet. [`synthetic_snapshot`] exists so the hash gate
//! has a deterministic computation to hash, and so its negative control has a
//! constant inside that computation to perturb.

use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};
use graph_contract::snapshot::{CURRENT_VERSION, NonFinite, SnapshotHeader, push_f32_column};

/// Degree at which a node's weight saturates to 1.0 (`src/core/model/weights.ts:12`).
pub const REFERENCE_DEGREE: u32 = 8;

/// A small deterministic snapshot for seed `seed`: a ring of 16–63 `Point` nodes whose
/// radius is the degree weight of a pseudo-random degree, relative to `reference_degree`.
///
/// Exercises integer mixing, `libm` `log1p`/`sin`/`cos`, f64→f32 narrowing and the
/// contract's header and column writer — the operations every later stage leans on.
pub fn synthetic_snapshot(seed: u32, reference_degree: u32) -> Result<Vec<u8>, NonFinite> {
    let node_count = 16 + seed % 48;
    let mut state = mix_seed(seed);
    let weights: Vec<f32> = (0..node_count)
        .map(|_| degree_weight(next_u32(&mut state) % 12, reference_degree))
        .collect();
    let (xs, ys) = ring(&weights);
    let header = SnapshotHeader {
        version: CURRENT_VERSION,
        node_kind: NodeGeometryKind::Point,
        edge_kind: EdgeGeometryKind::Line,
        stage_count: 1,
        node_count,
        edge_count: 0,
    };
    let mut out = Vec::new();
    header.encode(&mut out);
    for column in [&xs, &ys, &weights] {
        push_f32_column(column, &mut out)?;
    }
    Ok(out)
}

/// `clamp(0.2 + 0.8·log1p(degree)/log1p(reference), 0.2, 1)`, the H3 formula.
fn degree_weight(degree: u32, reference_degree: u32) -> f32 {
    let ratio = libm::log1p(f64::from(degree)) / libm::log1p(f64::from(reference_degree));
    (0.2 + 0.8 * ratio).clamp(0.2, 1.0) as f32
}

fn ring(weights: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = weights.len() as f64;
    let angle = |i: usize| 2.0 * core::f64::consts::PI * (i as f64) / n;
    let xs = weights
        .iter()
        .enumerate()
        .map(|(i, w)| (libm::cos(angle(i)) * f64::from(*w)) as f32);
    let ys = weights
        .iter()
        .enumerate()
        .map(|(i, w)| (libm::sin(angle(i)) * f64::from(*w)) as f32);
    (xs.collect(), ys.collect())
}

/// splitmix64 finaliser: spreads a small seed over all 64 bits.
fn mix_seed(seed: u32) -> u64 {
    let mut z = u64::from(seed).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// xorshift64 step, returning the high half. Integer-only, so identical on every target.
fn next_u32(state: &mut u64) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_snapshot_is_deterministic_and_sized_by_seed() {
        let seed = 7;
        let a = synthetic_snapshot(seed, REFERENCE_DEGREE).expect("finite");
        assert_eq!(
            a,
            synthetic_snapshot(seed, REFERENCE_DEGREE).expect("finite")
        );
        let nodes = 16 + seed % 48;
        assert_eq!(a.len(), 28 + 3 * 4 * nodes as usize);
    }

    #[test]
    fn reference_degree_reaches_the_hashed_bytes() {
        let honest = synthetic_snapshot(3, REFERENCE_DEGREE).expect("finite");
        let mutated = synthetic_snapshot(3, REFERENCE_DEGREE + 1).expect("finite");
        assert_ne!(honest, mutated);
    }

    #[test]
    fn degree_weight_matches_the_h3_endpoints() {
        assert_eq!(degree_weight(0, 8), 0.2);
        assert_eq!(degree_weight(8, 8), 1.0);
        assert_eq!(degree_weight(11, 8), 1.0);
    }
}
