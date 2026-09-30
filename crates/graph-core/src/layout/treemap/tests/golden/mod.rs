//! Golden values pinned against **d3-hierarchy@3.1.2** (the version vendored in
//! `node_modules/d3-hierarchy`, identical to
//! `/goinfre/dlesieur/refs/npm/d3-hierarchy-3.1.2/`).
//!
//! Every constant here was produced by *running* that oracle on the same tree, with the
//! call sequence `treemap.rs`'s module doc states:
//!
//! ```js
//! d3.hierarchy(data, n => n.children)
//!   .sum(n => Number.isFinite(n.weight) && n.weight > 0 ? n.weight : 1e-6)
//!   .sort((a, b) => b.value - a.value);
//! d3.treemap().tile(d3.treemapSquarify).size([1, 1])(root);
//! ```
//!
//! They are compared with `f64::to_bits()`, not a tolerance: the port claims to be exact
//! in `f64` (the `f32` tolerance lives in [`super::invariants`]), so an ulp of drift is a
//! bug, not noise. This golden sits **on top of** the behavioural tests — the containment
//! sweep in [`super::invariants`] and the structural tests in the parent module — so it
//! cannot pass on its own.

use super::*;

mod fallback;
use fallback::*;

/// Every box of [`ROUND_TRIP_TREE`], keyed by dense index, as raw `f64` bit patterns, as
/// printed by d3-hierarchy@3.1.2. Hoisted out of the test so the test itself stays under
/// the house line limit.
const WANT: [(u32, &str, [u64; 4]); 6] = [
    // R: the unit square.
    (
        0,
        "R",
        [
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3ff0_0000_0000_0000,
        ],
    ),
    // S: y1 one ulp past 1.0 — the round trip, observable.
    (
        1,
        "S",
        [
            0x0000_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x3feb_fffe_64f5_64af,
            0x3ff0_0000_0000_0001,
        ],
    ),
    // P: spans S's column, down to D's row.
    (
        2,
        "P",
        [
            0x3feb_fffe_64f5_64af,
            0x0000_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3edd_5c23_e133_0691,
        ],
    ),
    // C (dense 3): a zero-width hairline at P's right edge.
    (
        3,
        "C",
        [
            0x3ff0_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3edd_5c23_e133_0691,
        ],
    ),
    // D (dense 4): the full-width row, its y1 one ulp below P's.
    (
        4,
        "D",
        [
            0x3feb_fffe_64f5_64af,
            0x0000_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3edd_5c23_e133_0690,
        ],
    ),
    // E: the same hairline as C.
    (
        5,
        "E",
        [
            0x3ff0_0000_0000_0000,
            0x0000_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x3edd_5c23_e133_0691,
        ],
    ),
];

/// `R` (weight 1/7) parents `S` (1) and `P`; `P`'s own weight is `1e-300`, far below the
/// ulp of its largest child `D` (`1e-6`), and `P` is **not** the root, so `P`'s box has a
/// non-zero origin.
///
/// The load-bearing value is `S.y1`: d3 gives it `1.0000000000000002`
/// (`0x3ff0000000000001`), one ulp **past** the unit square. `squarifyRatio` grows the
/// row by folding each candidate into the running total and, on rejecting the next
/// child, undoes that with `sumValue -= nodeValue` — a round trip through `f64` that does
/// not return to where it started (`(1 + 1e-6) - 1e-6 == 0.9999999999999999`, not `1`).
/// The row's value drives `slice`'s `k = (y1 - y0) / parent.value` and every edge
/// downstream, so an `extend_row` that discards the rejected candidate instead of
/// subtracting it back misses this by an ulp across the whole subtree.
///
/// The boxes are keyed by **dense index** (the order the nodes are admitted in). d3 walks
/// in pre-order, which after the descending-value sort visits `D` before `C`; only the
/// print order differs.
#[test]
fn the_extend_row_value_round_trip_matches_d3_bit_for_bit() {
    let topology = round_trip_tree(1e-300);
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let boxes = compute(&topology, &hierarchy);
    let want = WANT;
    for (v, id, edges) in want {
        let got = boxes.rect(v);
        let got_bits = [got.x0, got.y0, got.x1, got.y1].map(f64::to_bits);
        assert_eq!(
            got_bits, edges,
            "node {v} ({id}) differs from d3-hierarchy 3.1.2"
        );
    }
}

/// The negative control for the golden above: the same tree with `P`'s own weight raised
/// to `1e-6`, which lifts it clear of the cancellation. If the control produced the same
/// bits, the golden would be pinning the shape of the tree rather than the round trip,
/// and would still pass against a broken `extend_row`.
#[test]
fn the_round_trip_control_input_does_not_reach_the_same_bits() {
    let bits = |p_weight: f64| {
        let topology = round_trip_tree(p_weight);
        let b = compute(&topology, &Hierarchy::of(&topology).expect("fits")).rect(1);
        [b.x0, b.y0, b.x1, b.y1].map(f64::to_bits)
    };
    assert_ne!(
        bits(1e-300),
        bits(1e-6),
        "the control must not reproduce the round-tripped bits, or the golden is vacuous"
    );
}

#[test]
fn the_zero_remaining_row_edge_is_the_far_side_not_origin_plus_extent() {
    for (s_weight, want) in FALLBACK_EDGE {
        let topology = fan_tree(s_weight, 1e-300);
        let boxes = compute(&topology, &Hierarchy::of(&topology).expect("fits"));
        for (v, edges) in [4_u32, 3, 5].into_iter().zip(want) {
            let r = boxes.rect(v);
            assert_eq!(
                [r.x0, r.y0, r.x1, r.y1].map(f64::to_bits),
                edges,
                "S weight {s_weight}, node {v}"
            );
        }
    }
}

/// `R` (1/7) with children `S` (1) and `P`; `P` with children `C` (1e-300), `D` (1e-6)
/// and `E` (1e-300). `P`'s own weight is the parameter. Dense index: R 0, S 1, P 2,
/// C 3, D 4, E 5.
fn round_trip_tree(p_weight: f64) -> Topology {
    fan_tree(1.0, p_weight)
}

/// [`round_trip_tree`] with `S`'s weight as a parameter, so `P`'s column starts at
/// `x0 != 0`.
fn fan_tree(s_weight: f64, p_weight: f64) -> Topology {
    let nodes = vec![
        weighted("R", 1.0 / 7.0),
        weighted("S", s_weight),
        weighted("P", p_weight),
        weighted("C", 1e-300),
        weighted("D", 1e-6),
        weighted("E", 1e-300),
    ];
    let edges = vec![
        tree("R-S", "R", "S", "parent_of"),
        tree("R-P", "R", "P", "parent_of"),
        tree("P-C", "P", "C", "parent_of"),
        tree("P-D", "P", "D", "parent_of"),
        tree("P-E", "P", "E", "parent_of"),
    ];
    index_model(&nodes, &edges).expect("fits")
}
