use super::{RING_SPACING, point, run};
use crate::edgekind::child_first_from_type;
use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::layout::hierarchy::Hierarchy;
use crate::layout::hierarchy::fixture::{self, load};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;
use graph_contract::notes::NoteCode::{CycleEdgeDropped, ExtraParentDropped};

/// A hierarchy edge `source` -> `target` of wire type `wire`, as `hierarchy::tests::tree`.
fn tree(id: &str, source: &str, target: &str, wire: &str) -> EdgeRecord {
    EdgeRecord {
        kind: crate::edgekind::EdgeKind::Hierarchy,
        child_first: child_first_from_type(Some(wire)),
        label: wire.into(),
        ..edge(id, source, target)
    }
}

fn nodes(ids: &[&str]) -> Vec<NodeRecord> {
    ids.iter().map(|id| node(id, "")).collect()
}

fn build(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Topology {
    index_model(nodes, edges).expect("fits")
}

/// The point columns, or a panic: every test here is `layout.circular.radial`, `Point`.
fn points(g: &Geometry) -> (Vec<f32>, Vec<f32>) {
    let graph_contract::geometry::NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    (x.clone(), y.clone())
}

#[test]
fn a_single_node_sits_at_the_dead_centre() {
    let topology = build(&nodes(&["r"]), &[]);
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!((x, y), (vec![0.0], vec![0.0]));
}

#[test]
fn an_empty_topology_has_no_geometry_and_no_notes() {
    let geometry = run(&build(&[], &[])).expect("fits");
    assert_eq!(points(&geometry), (vec![], vec![]));
    assert_eq!(geometry.notes, []);
}

/// `r -> [a, b, c]`: root at the dead centre; three children share ring 1, ascending
/// dense index `a, b, c` getting slots `0, 1, 2` at `RING_SPACING` — slot 0 lands exactly
/// on the positive x axis (`cos 0 = 1`, `sin 0 = 0`), the one angle this hand oracle can
/// pin without leaning on `libm`'s own correctness.
#[test]
fn three_children_share_ring_one_ascending_dense_index_from_the_positive_x_axis() {
    let edges = [
        tree("r-a", "r", "a", "parent_of"),
        tree("r-b", "r", "b", "parent_of"),
        tree("r-c", "r", "c", "parent_of"),
    ];
    let topology = build(&nodes(&["r", "a", "b", "c"]), &edges);
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!((x[0], y[0]), (0.0, 0.0), "root at the centre");
    assert_eq!(
        (x[1], y[1]),
        (RING_SPACING as f32, 0.0),
        "a: slot 0, angle 0"
    );
    let (bx, by) = point(1, 1, 3);
    let (cx, cy) = point(1, 2, 3);
    assert_eq!((x[2], y[2]), (bx as f32, by as f32), "b: slot 1 of 3");
    assert_eq!((x[3], y[3]), (cx as f32, cy as f32), "c: slot 2 of 3");
    assert_ne!(
        (x[2], y[2]),
        (x[3], y[3]),
        "distinct slots, distinct positions"
    );
}

/// The ring-1 slots pinned to **independent** values, not to [`point`]'s own output.
///
/// The test above compares `run` against `point(...)`, which is self-referential: a
/// mistake *inside* `point` is invisible to it, and the module doc's `2 * PI` step and the
/// `ring * RING_SPACING` radius both live there. This test pins the same three slots to
/// the closed-form values of `cos`/`sin` at `0`, `2*pi/3` and `4*pi/3`:
///
/// | slot | angle | cos | sin |
/// |---|---|---|---|
/// | 0 | 0 | 1 | 0 |
/// | 1 | 2π/3 | −1/2 | √3/2 ≈ 0.8660254 |
/// | 2 | 4π/3 | −1/2 | −√3/2 |
///
/// A tolerance, not `to_bits()`: this layout's oracle is **hand**, not d3 (module doc),
/// and the values come from `libm`'s `cos`/`sin`, whose own accuracy is the bound. The
/// tolerance is `1e-6` — four orders of magnitude above `libm`'s documented error and far
/// below the 0.5 that separates the slots, so it pins the convention without pretending to
/// bit-exactness the module does not claim. It is still sharp enough to catch a halved
/// step: slot 1 would land on `+1/2` instead of `−1/2`.
#[test]
fn ring_one_slots_sit_at_the_closed_form_angles() {
    let edges = [
        tree("r-a", "r", "a", "parent_of"),
        tree("r-b", "r", "b", "parent_of"),
        tree("r-c", "r", "c", "parent_of"),
    ];
    let topology = build(&nodes(&["r", "a", "b", "c"]), &edges);
    let (x, y) = points(&run(&topology).expect("fits"));
    let sqrt3_over_2 = 0.866_025_4_f64;
    let want = [
        (1.0_f64, 0.0_f64),
        (-0.5, sqrt3_over_2),
        (-0.5, -sqrt3_over_2),
    ];
    for (slot, (wx, wy)) in want.iter().enumerate() {
        let i = slot + 1;
        assert!(
            (f64::from(x[i]) - wx).abs() < 1e-6 && (f64::from(y[i]) - wy).abs() < 1e-6,
            "slot {slot}: got ({}, {}), want ({wx}, {wy})",
            x[i],
            y[i]
        );
    }
}

/// D-H's repairs reach the snapshot through this layout's notes, unchanged.
#[test]
fn the_hierarchy_repairs_reach_geometry_notes_unchanged() {
    let (nodes, edges) = load("cyclic");
    let topology = build(&nodes, &edges);
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let geometry = run(&topology).expect("fits");
    assert_eq!(geometry.notes, hierarchy.notes());
    assert!(
        geometry
            .notes
            .iter()
            .any(|n| n.code == CycleEdgeDropped || n.code == ExtraParentDropped)
    );
}

/// The forest fixture has 4 roots, so its virtual root (dense index 8, never a node) sits
/// at ring 0 and is never emitted; the 4 real roots land on ring 1, radius `RING_SPACING`.
#[test]
fn the_virtual_root_never_reaches_the_output_and_real_roots_land_on_ring_one() {
    let (nodes, edges) = load("forest");
    let topology = build(&nodes, &edges);
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!((x.len(), y.len()), (topology.node_count() as usize, 8));
    for &root in hierarchy.roots() {
        assert_eq!(
            hierarchy.depth(root),
            1,
            "real root {root} is ring 1 under the virtual root"
        );
        let radius = distance(x[root as usize], y[root as usize]);
        assert!(
            (radius - RING_SPACING).abs() < 1e-6,
            "root {root}: radius {radius}"
        );
    }
}

/// Every fixture: right shape, no repeat run diverges, nothing non-finite, and every
/// node's radius matches its own ring exactly (`ring * RING_SPACING`, `f32` rounding
/// only).
#[test]
fn every_fixture_lays_out_finite_deterministic_and_ring_true() {
    for (name, _) in fixture::FIXTURES {
        let (nodes, edges) = load(name);
        check(&build(&nodes, &edges), name);
    }
}

#[test]
fn seeded_synthetic_topologies_lay_out_finite_deterministic_and_ring_true() {
    for seed in 0..48 {
        let (nodes, edges) = seeded_model(seed, 2 + seed * 7 % 300, REFERENCE_DEGREE);
        check(&build(&nodes, &edges), &format!("seed {seed}"));
    }
}

/// Every property this layout promises, for one topology: run twice (determinism), no
/// `NaN`/`±inf`, output sized to the real nodes only, and the one structural invariant a
/// circular layout owes — every node's distance from the origin is exactly its ring's
/// radius (`f32` rounding aside).
fn check(t: &Topology, label: &str) {
    let a = run(t).expect(label);
    let b = run(t).expect(label);
    assert_eq!(a, b, "{label}: two runs, byte for byte");
    let (x, y) = points(&a);
    assert_eq!(
        (x.len(), y.len()),
        (t.node_count() as usize, t.node_count() as usize)
    );
    assert!(x.iter().chain(&y).all(|v| v.is_finite()), "{label}");
    let hierarchy = Hierarchy::of(t).expect(label);
    for v in 0..t.node_count() {
        let want = f64::from(hierarchy.depth(v)) * RING_SPACING;
        let got = distance(x[v as usize], y[v as usize]);
        assert!(
            (got - want).abs() < 1e-4,
            "{label}: node {v} radius {got}, want {want}"
        );
    }
}

/// A node's distance from the origin, `f64` throughout: the test's own re-derivation of a
/// radius, independent of production code's `point`.
fn distance(x: f32, y: f32) -> f64 {
    let (x, y) = (f64::from(x), f64::from(y));
    (x * x + y * y).sqrt()
}
