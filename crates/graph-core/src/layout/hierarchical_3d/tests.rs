//! Hand-pinned tests for [`super`]. Every expected coordinate below is worked by hand
//! from the reference's own closed form, not read back out of a run.
//!
//! These are the **drawing's** tests: what a plotted coordinate can falsify. The two halves
//! that are not the drawing — `_disk_positions`' ties-to-even rounding and
//! `_multi_source_levels`' re-seed — are in [`parts`], because they are arithmetic and
//! traversal, and the helpers below are shared with it so there is one spelling of "a
//! topology of ids `a`, `b`, `c`".

#[path = "tests/parts.rs"]
mod parts;

use super::{SCALE, run};
use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;

/// `x`, `y` and `z`, or a panic: every test in this module and in [`parts`] is
/// `layout.hierarchical3d`, `Point` in space, and a 2D snapshot would answer all of them
/// wrongly and quietly.
pub(super) fn space(g: &Geometry) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let graph_contract::geometry::NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    let Some(z) = g.z.as_ref() else {
        panic!("a z column: this layout is 3D");
    };
    (x.clone(), y.clone(), z.clone())
}

/// `pub(super)` because [`parts`] builds the same kinds of topology and this module is the
/// one place they are spelled.
pub(super) fn nodes(ids: &[&str]) -> Vec<NodeRecord> {
    ids.iter().map(|id| node(id, "")).collect()
}

/// `edges` as `a-b, b-c, ...` over consecutive ids of `ids`, the path/walk shape every
/// hand oracle below is built from.
fn path_edges(ids: &[&str]) -> Vec<EdgeRecord> {
    ids.windows(2)
        .enumerate()
        .map(|(i, w)| edge(&format!("e{i}"), w[0], w[1]))
        .collect()
}

/// `pub(super)` for [`parts`], which indexes the same records.
pub(super) fn build(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Topology {
    index_model(nodes, edges).expect("fits")
}

/// A graph on ids `0..count` whose single hub `0` is joined to every other node.
fn star(count: usize) -> Topology {
    let ids: Vec<String> = (0..count).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let edges: Vec<EdgeRecord> = ids
        .iter()
        .skip(1)
        .enumerate()
        .map(|(i, leaf)| edge(&format!("e{i}"), &ids[0], leaf))
        .collect();
    build(&nodes(&refs), &edges)
}

/// A node's distance from its level's axis, in `f64`: the test's own re-derivation of a
/// ring radius, independent of the layout's `disk`.
fn radius(x: f32, y: f32) -> f64 {
    let (x, y) = (f64::from(x), f64::from(y));
    f64::sqrt(x * x + y * y)
}

#[test]
fn an_empty_topology_has_three_empty_columns_and_is_still_three_dimensional() {
    let geometry = run(&build(&[], &[])).expect("fits");
    assert_eq!(space(&geometry), (Vec::new(), Vec::new(), Vec::new()));
    assert_eq!(geometry.notes, []);
    assert_eq!(
        geometry.dim(),
        graph_contract::snapshot::Dim::D3,
        "the reference returns np.zeros((0, 3)), so an empty input is 3D too"
    );
}

/// `count == 1` is `_disk_positions`' first line (`hierarchical.py:95-96`): the centre,
/// whatever radius was computed for it. One node is also its own component's root, so
/// `max(1, max_level)` is 1 and `z = (0/1) * 10 - 5`.
#[test]
fn a_single_node_sits_at_the_centre_of_the_negative_end() {
    let (x, y, z) = space(&run(&build(&nodes(&["r"]), &[])).expect("fits"));
    assert_eq!((x, y, z), (vec![0.0], vec![0.0], vec![-5.0]));
}

/// A 5-node path `a-b-c-d-e` is the hand case with **three** levels.
///
/// `_component_roots` roots it at the midpoint of its diameter (`hierarchical.py:31-52`):
/// the first sweep from `a` ends at `e`, the second from `e` ends at `a`, and the parent
/// chain `a -> b -> c -> d -> e` has its middle at index `5 // 2 = 2`, which is `c`. So
/// `c` is level 0, `{b, d}` level 1 in BFS discovery order, `{a, e}` level 2 — and with
/// `max_level = 2`, `z` is `-5`, `0`, `+5`.
///
/// Each level holds at most 2 nodes against `widest = 2`, so `radius = 2.5 * sqrt(1) =
/// 2.5` for the two-node levels. `disk(2, 2.5)`: `sqrt(2/pi) = 0.798` rounds to 1 ring,
/// `take = [2]` already sums to `count`, and the one ring sits at `2.5 * 0.5 / 1 = 1.25`
/// with `angle = arange(2) * (2pi/2)`, i.e. `0` and `pi`. So each two-node level is the
/// pair `(+1.25, 0)` and `(-1.25, 0)`, and the lone level-0 node is at the origin.
#[test]
fn a_five_node_path_fills_three_levels_at_minus_five_zero_and_plus_five() {
    let ids = ["a", "b", "c", "d", "e"];
    let topology = build(&nodes(&ids), &path_edges(&ids));
    let (x, y, z) = space(&run(&topology).expect("fits"));
    assert_eq!(
        z,
        vec![5.0_f32, 0.0, -5.0, 0.0, 5.0],
        "levels 2, 1, 0, 1, 2"
    );
    assert_eq!(
        (x[0], x[2], x[3], x[4]),
        (1.25_f32, 0.0, -1.25, -1.25),
        "the (+1.25, 0) / (-1.25, 0) pairs, `x` exact: cos 0 = 1 and cos pi = -1"
    );
    assert!(
        (y[0] as f64).abs() < 1e-6 && (y[3] as f64).abs() < 1e-6,
        "y 0 and -1.25"
    );
    assert_eq!((x[1], y[1]), (1.25_f32, 0.0), "b, first slot of level 1");
    assert_eq!((x[2], y[2]), (0.0_f32, 0.0), "c, the count-1 disk case");
    assert_eq!(
        SCALE, 5.0,
        "the dispatcher's own scale, and the z step is 2 * scale"
    );
}

/// A **3**-node path is two levels, not three, and this is the reference's answer, not a
/// port slip: `_component_roots` roots `a-b-c` at its own centre `b` (`path[3 // 2] = 1`),
/// so `b` is level 0 and `{a, c}` level 1. A three-level drawing would need the end to be
/// the root, which is the *directed* branch (`hierarchical.py:127-128`) that this port
/// does not take (module doc).
#[test]
fn a_three_node_path_is_two_levels_because_its_own_centre_is_the_root() {
    let ids = ["a", "b", "c"];
    let topology = build(&nodes(&ids), &path_edges(&ids));
    let (x, y, z) = space(&run(&topology).expect("fits"));
    assert_eq!(
        z,
        vec![5.0_f32, -5.0, 5.0],
        "b at level 0, a and c at level 1"
    );
    assert_eq!((x[0], y[0]), (1.25_f32, 0.0), "a, slot 0 of level 1");
    assert!(
        (radius(x[2], y[2]) - 1.25).abs() < 1e-6,
        "c, slot 1 of level 1"
    );
    assert_eq!(
        (x[1], y[1]),
        (0.0_f32, 0.0),
        "the root, the count-1 disk case"
    );
}

/// A 10-leaf star: two levels, every leaf on `z = +5`, and the disk split **`[2, 8]`**.
///
/// `sqrt(10/pi) = 1.784` rounds to 2 rings, `share = [0.5, 1.5]` sums to 2, and
/// `take = np.round([0.25, 0.75] * 10) = np.round([2.5, 7.5])`. Both are exact halves, and
/// numpy rounds **ties to even**: `2.5 -> 2` and `7.5 -> 8`. `f64::round` would give
/// `[3, 8]`, summing to 11, and the reference's own `while take.sum() > count` loop would
/// then move one unit to the argmax and land on `[3, 7]` — a different drawing. The
/// rings are `2.5 * 0.5 / 2 = 0.625` and `2.5 * 1.5 / 2 = 1.875`, so the split is visible
/// in the radii: two leaves in, eight out.
#[test]
fn a_ten_leaf_star_puts_every_leaf_on_one_plane_two_in_and_eight_out() {
    let (x, y, z) = space(&run(&star(11)).expect("fits"));
    assert_eq!(z[0], -5.0_f32, "the hub is level 0");
    assert!(
        z[1..].iter().all(|&v| v == 5.0),
        "every leaf on the same plane: {z:?}"
    );
    let radii: Vec<f64> = (1..11).map(|i| radius(x[i], y[i])).collect();
    assert_eq!(
        radii.iter().filter(|&&r| (r - 0.625).abs() < 1e-6).count(),
        2,
        "inner ring, take[0] = 2"
    );
    assert_eq!(
        radii.iter().filter(|&&r| (r - 1.875).abs() < 1e-6).count(),
        8,
        "outer ring, take[1] = 8"
    );
    assert_eq!((x[1], y[1]), (0.625_f32, 0.0), "leaf 1, ring 0 slot 0");
    for i in 1..11 {
        for j in i + 1..11 {
            assert_ne!(
                (x[i], y[i]),
                (x[j], y[j]),
                "leaves {i} and {j} collide at one point"
            );
        }
    }
}

#[test]
fn seeded_synthetic_topologies_lay_out_finite_deterministic_and_inside_the_scale() {
    for seed in 0..32 {
        let (nodes, edges) = seeded_model(seed, 2 + seed * 7 % 300, REFERENCE_DEGREE);
        let topology = build(&nodes, &edges);
        let a = run(&topology).expect("fits");
        let b = run(&topology).expect("fits");
        assert_eq!(a, b, "seed {seed}: two runs, byte for byte");
        let (x, y, z) = space(&a);
        let n = topology.node_count() as usize;
        assert_eq!((x.len(), y.len(), z.len()), (n, n, n), "seed {seed}");
        assert!(
            x.iter().chain(&y).chain(&z).all(|v| v.is_finite()),
            "seed {seed}: no NaN or infinity"
        );
        assert!(
            z.iter().all(|&v| v >= -(SCALE as f32) && v <= SCALE as f32),
            "seed {seed}: every level inside [-scale, scale], got {z:?}"
        );
        assert_eq!(a.edges, graph_contract::geometry::EdgeGeometry::Line);
    }
}
