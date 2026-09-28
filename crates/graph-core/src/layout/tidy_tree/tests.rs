use super::run;
use crate::edgekind::child_first_from_type;
use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::layout::hierarchy::Hierarchy;
use crate::layout::hierarchy::fixture::{self, load};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::notes::NoteCode::{CycleEdgeDropped, ExtraParentDropped};

/// A hierarchy edge `source` → `target` of wire type `wire`, as `hierarchy::tests::tree`.
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

/// The point columns, or a panic: every test here is `layout.tree.tidy`, `Point` nodes.
fn points(g: &Geometry) -> (Vec<f32>, Vec<f32>) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    (x.clone(), y.clone())
}

fn paths(g: &Geometry) -> &Paths {
    let EdgeGeometry::Polyline(paths) = &g.edges else {
        panic!("polyline edges");
    };
    paths
}

#[test]
fn a_single_node_sits_at_the_centre_of_the_unit_square() {
    let topology = build(&nodes(&["r"]), &[]);
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!((x, y), (vec![0.5], vec![0.0]));
}

/// A root with two leaves: the textbook Reingold–Tilford result, `size([1, 1])`.
#[test]
fn a_root_with_two_leaves_splits_a_quarter_either_side() {
    let edges = [
        tree("r-a", "r", "a", "parent_of"),
        tree("r-b", "r", "b", "parent_of"),
    ];
    let topology = build(&nodes(&["r", "a", "b"]), &edges);
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!(x, vec![0.5, 0.25, 0.75]);
    assert_eq!(y, vec![0.0, 1.0, 1.0]);
}

/// `r -> [a, b]`, `b -> [c, d]`: asymmetric enough that `apportion`'s contour threading
/// engages (`a`'s subtree is shallower than `b`'s). Traced by hand from `tree.js`, not
/// from this module's code: `secondWalk` leaves raw `x` at `r=0, a=-0.5, b=0.5, c=0,
/// d=1` (depths `0,1,1,2,2`); the extremes are `left=a, right=d, bottom=c`, `tx = 1.5`,
/// `kx = 1/3.5`, `ky = 1/2`.
#[test]
fn an_asymmetric_tree_engages_the_contour_threading() {
    let edges = [
        tree("r-a", "r", "a", "parent_of"),
        tree("r-b", "r", "b", "parent_of"),
        tree("b-c", "b", "c", "parent_of"),
        tree("b-d", "b", "d", "parent_of"),
    ];
    let topology = build(&nodes(&["r", "a", "b", "c", "d"]), &edges);
    let (x, y) = points(&run(&topology).expect("fits"));
    let (tx, kx, ky) = (1.5_f64, 1.0_f64 / 3.5, 1.0_f64 / 2.0);
    let want_x = |raw: f64| ((raw + tx) * kx) as f32;
    let want_y = |depth: u32| (f64::from(depth) * ky) as f32;
    assert_eq!(x, [0.0, -0.5, 0.5, 0.0, 1.0].map(want_x), "r, a, b, c, d");
    assert_eq!(y, [0, 1, 1, 2, 2].map(want_y), "r, a, b, c, d");
}

/// `r -> a -> b`, a `relation` edge folded in between: the first edge and the last edge
/// are the kept tree edges (the elbow, two interior points each) and the middle one is
/// the zero-interior-point case — the three shapes a Polyline CSR row can take.
#[test]
fn polyline_offsets_are_right_at_both_csr_boundaries_and_at_a_zero_length_row() {
    let edges = [
        tree("e0", "r", "a", "parent_of"),
        edge("e1", "a", "b"),
        tree("e2", "a", "b", "parent_of"),
    ];
    let topology = build(&nodes(&["r", "a", "b"]), &edges);
    let geometry = run(&topology).expect("fits");
    let (x, y) = points(&geometry);
    let paths = paths(&geometry);
    assert_eq!(paths.offsets, vec![0, 2, 2, 4], "e0 kept, e1 not, e2 kept");
    assert_eq!(paths.pts.len(), 8);
    let ym = |p: usize, c: usize| (f64::from(y[p]) + f64::from(y[c])) as f32 / 2.0;
    let want = [
        x[0],
        ym(0, 1),
        x[1],
        ym(0, 1),
        x[1],
        ym(1, 2),
        x[2],
        ym(1, 2),
    ];
    assert_eq!(paths.pts, want, "the elbow at e0 then at e2, nothing at e1");
}

#[test]
fn an_empty_topology_has_no_geometry_and_no_notes() {
    let geometry = run(&build(&[], &[])).expect("fits");
    assert_eq!(points(&geometry), (vec![], vec![]));
    assert_eq!(paths(&geometry), &Paths::default());
    assert_eq!(geometry.notes, []);
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

/// The virtual root is a row of the hierarchy's own bookkeeping, never a node: the
/// forest fixture has 4 roots (a virtual root at dense index 8), and the layout emits
/// exactly the 8 real nodes.
#[test]
fn the_virtual_root_never_reaches_the_output() {
    let (nodes, edges) = load("forest");
    let topology = build(&nodes, &edges);
    let (x, y) = points(&run(&topology).expect("fits"));
    assert_eq!((x.len(), y.len()), (topology.node_count() as usize, 8));
}

/// Every fixture: right shapes, no repeat run diverges, nothing non-finite, siblings
/// never cross (Reingold–Tilford's whole point) and same-depth nodes share `y`.
#[test]
fn every_fixture_lays_out_finite_deterministic_and_sibling_ordered() {
    for (name, _) in fixture::FIXTURES {
        let (nodes, edges) = load(name);
        let topology = build(&nodes, &edges);
        check(&topology, name);
    }
}

#[test]
fn seeded_synthetic_topologies_lay_out_finite_deterministic_and_sibling_ordered() {
    for seed in 0..48 {
        let (nodes, edges) = seeded_model(seed, 2 + seed * 7 % 300, crate::REFERENCE_DEGREE);
        let topology = build(&nodes, &edges);
        check(&topology, &format!("seed {seed}"));
    }
}

/// Every property this layout promises, for one topology: run twice (determinism), no
/// `NaN`/`±inf`, output sized to the real nodes only, and both structural invariants a
/// tidy tree owes — siblings ordered by `x` and same-depth nodes sharing `y`.
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
    assert!(paths(&a).pts.iter().all(|v| v.is_finite()), "{label}");
    let hierarchy = Hierarchy::of(t).expect(label);
    for p in 0..=t.node_count() {
        let children = hierarchy.children(p);
        for w in children.windows(2) {
            assert!(
                x[w[0] as usize] <= x[w[1] as usize],
                "{label}: siblings of {p}"
            );
        }
    }
    for v in 0..t.node_count() {
        for u in (v + 1)..t.node_count() {
            if hierarchy.depth(v) == hierarchy.depth(u) {
                assert_eq!(y[v as usize], y[u as usize], "{label}: {v} and {u}");
            }
        }
    }
}
