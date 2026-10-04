//! Degree weights (`src/core/model/weights.ts:12-29`): a node's visual weight from its
//! degree on a log scale, normalised against a *fixed* reference degree so everyday
//! counts spread across the size range instead of being flattened by a few hubs.
//!
//! **D10 waiver — the degree pass is a scatter, deliberately** (`prompt.md` §6). D10 reads
//! "**Per-step kernels are gathers**: element `i` reads only start-of-step state and
//! writes only `out[i]`, summing its terms in a fixed order; no scatter into another
//! element's accumulator", and its recorded reason is that the SIMD, threaded and GPU
//! tiers (Phase 11) "either race or reorder sums" — a per-step kernel of an iterative
//! solver, of which this pass is not one. It is a single ingest walk, and it has to run
//! **before** indexing, over the raw edge records, so that an edge to a missing node, a
//! repeated id and a self-loop count exactly as the oracle counts them
//! (`synthetic.ts:103-104`). The gather D10 asks for is not reachable here: the only CSR
//! in the crate is built by `index_model`, which drops dangling endpoints and duplicate
//! ids, so a gather over it would count fewer edges and move every weight in the gate.
//! What the scatter does cost is two string hashes per edge endpoint — ponytail, not a
//! determinism defect: the result is a per-key counter, so no sum is reordered and no
//! target reads it differently.

use crate::arena::FixedState;
use crate::records::{EdgeRecord, NodeRecord};
use indexmap::IndexMap;

/// Degree at which a node's weight saturates to 1.0 (`weights.ts:12`).
pub const REFERENCE_DEGREE: u32 = 8;

/// `applyDegreeWeights`: sets every node's `weight` from its degree over `edges`.
///
/// The degree is counted over the edges exactly as given — before any indexing — so an
/// edge to a missing node still counts for its present end, a duplicate id counts twice,
/// and a self-loop counts 2. That is the oracle's order of operations
/// (`synthetic.ts:103-104`), preserved.
pub fn apply_degree_weights(nodes: &mut [NodeRecord], edges: &[EdgeRecord]) {
    apply_degree_weights_against(nodes, edges, REFERENCE_DEGREE);
}

/// [`apply_degree_weights`] against another reference degree. Exists for the hash
/// gate's negative control, which perturbs the reference in one arm only.
pub(crate) fn apply_degree_weights_against(
    nodes: &mut [NodeRecord],
    edges: &[EdgeRecord],
    reference_degree: u32,
) {
    let mut degree = IndexMap::<&str, u64, FixedState>::default();
    for edge in edges {
        *degree.entry(edge.source.as_str()).or_default() += 1;
        *degree.entry(edge.target.as_str()).or_default() += 1;
    }
    for node in nodes {
        let count = degree.get(node.id.as_str()).copied().unwrap_or(0);
        node.weight = degree_weight(count, reference_degree);
    }
}

/// `clamp(0.2 + 0.8·log1p(degree)/log1p(reference), 0.2, 1)`, with the oracle's
/// `Math.min(max, Math.max(min, v))` semantics. `libm::log1p` on every target (D1).
pub(crate) fn degree_weight(degree: u64, reference_degree: u32) -> f64 {
    let denominator = libm::log1p(f64::from(reference_degree));
    js_clamp(
        0.2 + 0.8 * (libm::log1p(degree as f64) / denominator),
        0.2,
        1.0,
    )
}

/// `Math.min(hi, Math.max(lo, v))`: NaN in, NaN out — unlike `f64::max`, which drops it.
fn js_clamp(value: f64, lo: f64, hi: f64) -> f64 {
    let js_max = |a: f64, b: f64| {
        if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.max(b)
        }
    };
    let js_min = |a: f64, b: f64| {
        if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.min(b)
        }
    };
    js_min(hi, js_max(lo, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::build::{edge, node};

    /// `applyDegreeWeights` under node:22-slim for degrees 0..=8 — the 9 reachable
    /// weights (H3) — as IEEE-754 bit patterns.
    const ORACLE_BITS: [u64; 9] = [
        0x3FC9_9999_9999_999A,
        0x3FDC_F3A9_4690_F764,
        0x3FE3_3333_3333_3332,
        0x3FE6_8D42_E02A_90FE,
        0x3FE9_26D3_4273_10B6,
        0x3FEB_46A1_7015_487E,
        0x3FED_1269_5931_D3B8,
        0x3FEE_A0B1_1D0C_A648,
        0x3FF0_0000_0000_0000,
    ];

    #[test]
    fn the_nine_reachable_weights_match_the_oracle_bit_for_bit() {
        for (degree, bits) in ORACLE_BITS.into_iter().enumerate() {
            let got = degree_weight(degree as u64, REFERENCE_DEGREE);
            assert_eq!(got.to_bits(), bits, "degree {degree}: {got}");
        }
        assert_eq!(degree_weight(9, REFERENCE_DEGREE), 1.0);
        assert_eq!(degree_weight(u64::MAX, REFERENCE_DEGREE), 1.0);
    }

    #[test]
    fn degree_counts_raw_edges_including_dangling_duplicate_and_self_loops() {
        let mut nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [
            edge("e", "a", "ghost"),
            edge("e", "a", "b"),
            edge("s", "c", "c"),
        ];
        apply_degree_weights(&mut nodes, &edges);
        let bits = nodes.map(|n| n.weight.to_bits());
        assert_eq!(bits, [ORACLE_BITS[2], ORACLE_BITS[1], ORACLE_BITS[2]]);
    }

    #[test]
    fn the_reference_degree_moves_the_saturation_point() {
        assert_eq!(degree_weight(9, 9), 1.0);
        assert!(degree_weight(8, 9) < 1.0);
        let mut nodes = [node("a", "")];
        apply_degree_weights_against(&mut nodes, &[], 9);
        assert_eq!(nodes[0].weight, 0.2);
    }

    #[test]
    fn js_clamp_propagates_nan_where_f64_max_would_drop_it() {
        assert!(js_clamp(f64::NAN, 0.2, 1.0).is_nan());
        assert!(degree_weight(0, 0).is_nan(), "0/0 in the oracle is NaN");
        assert_eq!(js_clamp(-3.0, 0.2, 1.0), 0.2);
        assert_eq!(js_clamp(3.0, 0.2, 1.0), 1.0);
        assert_eq!(js_clamp(0.5, 0.2, 1.0), 0.5);
    }

    /// F-06, evidence for the D10 waiver in the module docs: every key owns its counter,
    /// so the scatter sums nothing across elements and the weights cannot depend on the
    /// order the edges arrive in — the one thing a reordered sum would change.
    #[test]
    fn the_weights_do_not_depend_on_the_order_the_edges_arrive_in() {
        let edges = [
            edge("e1", "a", "b"),
            edge("e2", "b", "c"),
            edge("e3", "c", "a"),
            edge("e4", "a", "a"),
        ];
        let weights = |order: [usize; 4]| {
            let mut nodes = [node("a", ""), node("b", ""), node("c", "")];
            let ordered: Vec<_> = order.iter().map(|&i| edges[i].clone()).collect();
            apply_degree_weights(&mut nodes, &ordered);
            nodes.map(|n| n.weight.to_bits())
        };
        let forward = weights([0, 1, 2, 3]);
        assert_eq!(forward, weights([3, 2, 1, 0]));
        assert_eq!(forward, weights([1, 3, 0, 2]));
        // a: e1, e3, e4, e4 → 4; b: e1, e2 → 2; c: e2, e3 → 2.
        assert_eq!(forward, [ORACLE_BITS[4], ORACLE_BITS[2], ORACLE_BITS[2]]);
    }
}
