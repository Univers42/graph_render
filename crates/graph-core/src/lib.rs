//! graph-core — the motor. Pure: no I/O, no async, no environment, no clock, no
//! wasm-bindgen. It compiles for `wasm32-unknown-unknown` and native in every phase,
//! and every transcendental goes through `libm` so both targets round alike (D1).
//!
//! Phase 1: the topology layer — string arena, dense indices, SoA columns, three CSR
//! adjacencies — and the 17 pure functions of the TypeScript oracle's `core/model`,
//! byte-compared against it by `harness/oracle-diff.mjs`. No layout, no geometry.
//!
//! [`synthetic_snapshot`] and [`topology_stage`] are what the 4-way hash gate hashes;
//! both take the reference degree as a parameter so its negative control can perturb
//! one arm.

mod arena;
mod columns;
mod csr;
mod diff;
mod edgekind;
mod ids;
mod index;
mod legend;
mod neighborhood;
mod records;
mod stage;
mod synthetic;
mod weights;

pub use arena::{CapacityError, Interned, StringArena};
pub use columns::{EdgeColumns, NodeColumns, NodeKind};
pub use csr::{Csr, Incident};
pub use diff::{Patch, diff_graph, edges_equal, is_empty_patch};
pub use edgekind::{EdgeKind, edge_kind_from_type};
pub use ids::{
    RecordRef, hash_string, make_edge_id, make_note_node_id, make_record_node_id, make_tag_node_id,
    parse_node_id,
};
pub use index::{Stats, Topology, empty_model, index_model, nodes_equal};
pub use legend::{DatabaseCount, LegendCounts, TagCount, derive_legend};
pub use neighborhood::{Neighborhood, neighborhood, neighborhood_edges};
pub use records::{EdgeRecord, EdgeView, NodeRecord, NodeView};
pub use stage::{StageError, topology_stage};
pub use synthetic::{
    MAX_SYNTHETIC_NODES, build_synthetic_model, synthetic_count, synthetic_records,
};
pub use weights::{REFERENCE_DEGREE, apply_degree_weights, apply_degree_weights_against};

use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};
use graph_contract::snapshot::{
    CURRENT_VERSION, NonFinite, SnapshotHeader, StageCount, push_f32_column,
};

/// A small deterministic snapshot for seed `seed`: a ring of 16–63 `Circle` nodes
/// (columns `x`, `y`, `r`) whose radius is the degree weight of a pseudo-random degree,
/// relative to `reference_degree`, and whose distance from the centre is that radius too.
///
/// Exercises integer mixing, `libm` `log1p`/`sin`/`cos`, f64→f32 narrowing and the
/// contract's header and column writer — the operations every later stage leans on.
pub fn synthetic_snapshot(seed: u32, reference_degree: u32) -> Result<Vec<u8>, NonFinite> {
    let node_count = 16 + seed % 48;
    let mut state = mix_seed(seed);
    let weights: Vec<f32> = (0..node_count)
        .map(|_| {
            let degree = next_u32(&mut state) % 12;
            weights::degree_weight(u64::from(degree), reference_degree) as f32
        })
        .collect();
    let (xs, ys) = ring(&weights);
    let header = SnapshotHeader {
        version: CURRENT_VERSION,
        node_kind: NodeGeometryKind::Circle,
        edge_kind: EdgeGeometryKind::Line,
        stage_count: StageCount::ONE,
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
    fn mix_seed_is_splitmix64() {
        // Vigna's published first output for state 0; seed 1 from an independent
        // Python implementation of the same finaliser.
        assert_eq!(mix_seed(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(mix_seed(1), 0x910A_2DEC_8902_5CC1);
    }

    #[test]
    fn next_u32_is_xorshift64_13_7_17_high_half() {
        // Reference states from an independent Python implementation.
        let mut state = 0x0123_4567_89AB_CDEF;
        let expected = [
            (0x3F28_00D6_569E_01B4, 0x3F28_00D6),
            (0x606F_949A_3CEB_D0B7, 0x606F_949A),
            (0xC69B_BA40_DDDC_CAD6, 0xC69B_BA40),
        ];
        for (state_after, high) in expected {
            assert_eq!(next_u32(&mut state), high);
            assert_eq!(state, state_after);
        }
    }

    #[test]
    fn ring_places_node_i_at_angle_two_pi_i_over_n_scaled_by_its_weight() {
        let (xs, ys) = ring(&[0.5; 4]);
        let expected = [(0.5, 0.0), (0.0, 0.5), (-0.5, 0.0), (0.0, -0.5)];
        for (i, (x, y)) in expected.into_iter().enumerate() {
            assert!(
                (xs[i] - x).abs() < 1e-6 && (ys[i] - y).abs() < 1e-6,
                "node {i}"
            );
        }
    }
}
