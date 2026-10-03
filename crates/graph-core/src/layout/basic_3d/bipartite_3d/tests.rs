//! `_bipartite_layout_3d`'s two rings, pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:213-242`.
//!
//! Split from [`super::tests`], which holds the assertions common to the closed forms,
//! because every claim here is about *this* graph: which two sets the colouring picks, which
//! slot in the set decides an angle, and which plane each set lands on. Nothing below can be
//! checked on a bare node count, because the node count is not this layout's input.
//!
//! **Every bit pattern in [`REFS`] was generated once, in the `ge-python-oracle` image, by
//! running the reference itself on these exact graphs** — `nx.Graph()`,
//! `add_nodes_from(range(n))`, `add_edges_from(pairs)` in the order written here, so
//! `G.nodes()` is dense index order and `G.neighbors()` is edge-first order, which is what
//! `adjacency::neighbours` reproduces. Nothing here is transcribed from the source by hand.

use super::super::bipartite_3d::{self, columns};
use super::super::tests::space;
use crate::layout::coords::probe::graph;

/// How many ULP the reference's own `np.cos`/`np.sin` may sit from `libm`'s.
///
/// **The one tolerance in this file, and not a fudge factor.** numpy's transcendental
/// kernels are its own; `libm`'s are a different correct implementation of the same
/// function, and the two disagree in the last ulp on some arguments — the `sg-common`
/// caveat. So the `f64` columns are pinned to a *count* of ulp rather than to equality,
/// while the `f32` assertion below is exact: narrowing 53 bits to 24 swallows the
/// disagreement on every node measured here. `f32` is the reachable exact target and `f64`
/// is not.
const ULP: i64 = 4;

/// One fixture: the graph, and what the reference drew for it.
struct Reference {
    /// What this fixture is here to pin.
    about: &'static str,
    /// Node count, so node ids are `n0..`.
    nodes: u32,
    /// Edges in the order the reference was given them: this fixes `G.neighbors`.
    edges: &'static [(u32, u32)],
    /// `(x, y, z)` bit patterns, in dense node order.
    coords: &'static [[u64; 3]],
}

/// The four fixtures, and the reference's answer on each.
///
/// **`CYCLE` and `TRIANGLE` share their first three coordinates** because the greedy cut
/// lands on the same split a colouring would have — which is exactly why neither of them can
/// tell the two rules apart, and why `MIXED` exists.
///
/// `#[rustfmt::skip]`, and not for tidiness: rustfmt explodes a row of three 18-digit hex
/// literals onto nine lines, and the one row per node is the only thing that lets a reader
/// check this table against the oracle's output by eye.
#[rustfmt::skip]
static REFS: [Reference; 4] = [
    Reference {
        about: "a four-cycle: bipartite, one component, colouring {0,2}/{1,3}",
        nodes: 4,
        edges: &[(0, 1), (1, 2), (2, 3), (3, 0)],
        coords: &[
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0xc004_0000_0000_0000],
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0x4004_0000_0000_0000],
            [0xc008_0000_0000_0000, 0x3cba_7939_4c9e_8a0a, 0xc004_0000_0000_0000],
            [0xc008_0000_0000_0000, 0x3cba_7939_4c9e_8a0a, 0x4004_0000_0000_0000],
        ],
    },
    Reference {
        about: "a triangle: not bipartite, so the greedy maximum cut runs",
        nodes: 3,
        edges: &[(0, 1), (1, 2), (2, 0)],
        coords: &[
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0xc004_0000_0000_0000],
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0x4004_0000_0000_0000],
            [0xc008_0000_0000_0000, 0x3cba_7939_4c9e_8a0a, 0xc004_0000_0000_0000],
        ],
    },
    Reference {
        about: "three components, the last a lone node: the skew rule runs twice",
        nodes: 5,
        edges: &[(0, 1), (2, 3)],
        coords: &[
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0xc004_0000_0000_0000],
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0x4004_0000_0000_0000],
            [0xbff7_ffff_ffff_fffd, 0x4004_c8dc_2e42_3980, 0xc004_0000_0000_0000],
            [0xc008_0000_0000_0000, 0x3cba_7939_4c9e_8a0a, 0x4004_0000_0000_0000],
            [0xbff8_0000_0000_0006, 0xc004_c8dc_2e42_397e, 0xc004_0000_0000_0000],
        ],
    },
    Reference {
        about: "an odd cycle beside an even one: the cut takes the whole graph",
        nodes: 7,
        edges: &[(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 6), (6, 3)],
        coords: &[
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0xc004_0000_0000_0000],
            [0x4008_0000_0000_0000, 0x0000_0000_0000_0000, 0x4004_0000_0000_0000],
            [0x3caa_7939_4c9e_8a0a, 0x4008_0000_0000_0000, 0xc004_0000_0000_0000],
            [0xc008_0000_0000_0000, 0x3cba_7939_4c9e_8a0a, 0xc004_0000_0000_0000],
            [0xbff7_ffff_ffff_fffd, 0x4004_c8dc_2e42_3980, 0x4004_0000_0000_0000],
            [0xbcc3_daea_f976_e788, 0xc008_0000_0000_0000, 0xc004_0000_0000_0000],
            [0xbff8_0000_0000_0006, 0xc004_c8dc_2e42_397e, 0x4004_0000_0000_0000],
        ],
    },
];

/// A four-cycle: two rings on two planes, and the coloured set on the lower one.
///
/// **The case that separates this layout from `layout.bipartite`.** Each set holds two nodes,
/// so the two-column layout would put `{0,2}` at one `x` and `{1,3}` at the other; this puts
/// them on two *planes* and around two *rings*. The assertion that matters is that `z`
/// alternates in **set order** and not by dense index: a port that coloured correctly and
/// then placed by node id would draw `0` and `2` at the same height.
#[test]
fn a_four_cycle_draws_two_rings_with_z_following_the_set_and_not_the_index() {
    let reference = &REFS[0];
    let topology = graph(reference.nodes, reference.edges);
    let (x, y, z) = columns(&topology);
    assert_within_ulp(&x, &y, &z, reference);
    assert_eq!((z[0], z[1], z[2], z[3]), (-2.5, 2.5, -2.5, 2.5));
    // The two planes are `scale` apart, and the sign is set membership.
    assert_eq!(z[0] - z[1], -5.0, "the plane separation is exactly `scale`");
    // One radius serves both planes, so every node is `scale * 0.6` from its own ring's axis.
    for (node, (&nx, &ny)) in x.iter().zip(&y).enumerate() {
        let radius = f64::sqrt(nx * nx + ny * ny);
        assert!(
            (radius - 3.0).abs() < 1e-6,
            "node {node} is {radius} from its plane's axis, not 3"
        );
    }
}

/// A triangle: `_bipartite_parts` gives up at the odd cycle and `_greedy_max_cut` runs.
///
/// **Pinned because the fallback must actually run, not because it changes the answer here.**
/// The greedy cut lands on `{0,2}`/`{1}` — the split a colouring would have chosen — so this
/// graph cannot tell the two rules apart. [`MIXED`] is the fixture that does; what this one
/// rules out is a port that returned `None` and drew nothing.
#[test]
fn a_triangle_takes_the_greedy_cut_and_still_draws_two_rings() {
    let reference = &REFS[1];
    let topology = graph(reference.nodes, reference.edges);
    let (x, y, z) = columns(&topology);
    assert_within_ulp(&x, &y, &z, reference);
    // Node 1 alone on the upper plane: `set1` has one node, so `max(1, count)` is `1` and its
    // angle is 0 — the same guard the 4-cycle exercises with `count = 2`.
    assert_eq!(y[1], 0.0, "a one-node ring sits at angle 0");
    assert_eq!(z[1], 2.5);
}

/// Three components: the component order and the per-component skew flip.
///
/// **Where a port quietly goes wrong.** `_bipartite_parts` (`hierarchical.py:149-181`)
/// accumulates `set0`/`set1` across components and reverses one only when
/// `abs(skew + len0 - len1) > abs(skew + len1 - len0)` — a strict `>`, so a tie keeps the start
/// side. Both edge components tie here and the isolated node lands on its start side too, so
/// the reference's `set0` is `{0,2,4}` (three slots) and `set1` is `{1,3}` (two). That is what
/// puts the lone node at `2/3` of a turn rather than at angle 0: node 4's coordinate is
/// `(-0.15, 2.598)`, not `(3, 0)`, and a port that ordered the sets by component instead of
/// by slot would draw it at `(3, 0)`.
#[test]
fn the_isolated_node_is_the_last_slot_of_the_first_set_and_not_a_ring_of_its_own() {
    let reference = &REFS[2];
    let topology = graph(reference.nodes, reference.edges);
    let (x, y, z) = columns(&topology);
    assert_within_ulp(&x, &y, &z, reference);
    // The lone node shares the lower plane with the first edge: it is the third slot of a
    // three-slot ring, not the only node on a ring of its own. Slot 2 of 3 is 240 degrees,
    // so its `y` is negative — the coordinate a port that gave the isolated node its own
    // ring would put at 0, and the coordinate that orders the two edge components before it.
    assert_eq!(z[4], z[0], "node 4 is on set0, with nodes 0 and 2");
    assert!(y[4] < 0.0, "node 4 is at 2/3 of a turn, at y = {}", y[4]);
    assert_eq!((x[0], y[0]), (3.0, 0.0), "slot 0 is at angle 0");
}

/// An odd cycle beside an even one: the cut takes **the whole graph**.
///
/// **The fixture that separates the greedy cut from a two-colouring.** Both components would
/// two-colour, but `_bipartite_parts` gives up on the whole graph the moment it meets the
/// triangle (`:174-175`), so `_greedy_max_cut` runs over all seven nodes, the 4-cycle
/// included, and the 4-cycle is never coloured. A port that coloured component by component
/// and fell back only for the offending one would put node 3 on the other plane from node 0;
/// the reference puts them together, and that is asserted directly.
#[test]
fn one_odd_cycle_sends_the_bipartite_component_through_the_cut_with_it() {
    let reference = &REFS[3];
    let topology = graph(reference.nodes, reference.edges);
    let (x, y, z) = columns(&topology);
    assert_within_ulp(&x, &y, &z, reference);
    assert_eq!(
        (z[0], z[3]),
        (-2.5, -2.5),
        "0 and 3 are on the same side of the cut"
    );
    assert_eq!((z[4], z[6]), (2.5, 2.5), "4 and 6 are on the other");
    // Four slots against three: the cut's `set0` is `{0, 2, 3, 5}`, so node 5 is at `3/4` of a
    // turn and its `y` is negative — the coordinate a two-colouring cannot produce.
    assert!(y[5] < 0.0, "node 5 is past the half turn, at y = {}", y[5]);
}

/// The `f32` snapshot is the exact target, and this is the assertion that is exact.
///
/// `numpy`'s last-ulp disagreement with `libm` is 29 bits below the narrowing, so on every
/// node measured here the two agree bit for bit after narrowing. Run over **all four**
/// fixtures, because the claim is "narrowing hides the ulp" and it is only worth anything on
/// the nodes where the ulp is largest: `3*sin(pi) = 3.67e-16`, whose `f64` neighbour is a
/// different `f32`.
#[test]
fn every_reference_coordinate_survives_the_f64_to_f32_narrowing_exactly() {
    for reference in &REFS {
        let topology = graph(reference.nodes, reference.edges);
        let geometry = bipartite_3d::run(&topology).expect("never refuses");
        let (x, y, z) = space(&geometry);
        assert_eq!(x.len(), reference.nodes as usize);
        for (node, want) in reference.coords.iter().enumerate() {
            for (got, axis) in [(x[node], "x"), (y[node], "y"), (z[node], "z")] {
                let narrowed = f64::from_bits(
                    want[match axis {
                        "x" => 0,
                        "y" => 1,
                        _ => 2,
                    }],
                ) as f32;
                assert_eq!(
                    got, narrowed,
                    "{axis} of node {node} on {}",
                    reference.about
                );
            }
        }
    }
}

/// Every `f64` column within [`ULP`] of its reference bit pattern.
fn assert_within_ulp(x: &[f64], y: &[f64], z: &[f64], reference: &Reference) {
    assert_eq!(x.len(), reference.coords.len(), "{}", reference.about);
    for (node, want) in reference.coords.iter().enumerate() {
        for (got, bits) in [(x[node], want[0]), (y[node], want[1]), (z[node], want[2])] {
            let want = f64::from_bits(bits);
            let ulps = ulps_between(got, want);
            assert!(
                ulps <= ULP,
                "node {node} on {}: {got:e} is {ulps} ulp from the reference {want:e}",
                reference.about
            );
        }
    }
}

/// `|got - want|` in units in the last place, by the monotone bit ordering.
///
/// **An integer distance, not `(|a-b| / ulp(a))`, because that divides by a zero at a
/// power-of-two boundary** and `3.0` is one. The monotone map is total and has no special
/// case.
fn ulps_between(got: f64, want: f64) -> i64 {
    (ordered(got) - ordered(want)).abs()
}

/// A `f64`'s position in the total order of the finite values, as an `i64`.
fn ordered(value: f64) -> i64 {
    let bits = value.to_bits() as i64;
    if bits < 0 {
        i64::MIN.wrapping_sub(bits)
    } else {
        bits
    }
}
