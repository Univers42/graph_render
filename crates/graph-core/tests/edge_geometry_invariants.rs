//! An external, public-API-only check that every POST style's `Polyline`/`Curve` CSR is
//! well formed **at every boundary**, not on a sweep of well-behaved graphs. Every test
//! lives inside `mod edge_geometry_invariants`, so
//! `cargo test -p graph-core edge_geometry_invariants` selects them by the
//! (module-qualified) test name, whatever this file itself is called.
//!
//! The contract (`graph_contract::geometry::Paths::check`) says a row is CSR-shaped
//! (`offsets[e]..offsets[e + 1]`), starts at 0, never decreases, and is backed by
//! exactly `2 * offsets[m]` finite coordinates. A generator can satisfy all of that and
//! still be wrong: an `offsets` array that forgets the `m + 1` tail is fine at every
//! interior edge and wrong at the last one, and a self-loop written as a zero-length row
//! is a perfectly well-formed row. So each boundary is checked by name, and states what
//! its own row has to contain:
//!
//! - **zero edges** — `offsets == [0]` and `pts` empty. A stage that forgets to push the
//!   final offset is the classic failure and only this boundary sees it.
//! - **a single edge** — `offsets == [0, that row's length]`.
//! - **a self-loop** — a real loop, not a zero-length row: at least three interior
//!   points, none of them the node centre (the documented shape centres the loop half a
//!   radius above the node, so a vertex can only reach the node at a zero radius), and a
//!   bounding box with a non-zero width *and* height.
//! - **a parallel pair** — two edges between the same pair, whose rows must *differ*.
//!   The whole point of the fan is that they do not overlay, and two identical rows are
//!   well formed and useless.
//!
//! Two more cases are here because they are where the arithmetic divides: two nodes at
//! one position (the perpendicular has no direction there, so the fan offset is the
//! zero vector), and the shipped `fixtures/post/parallel-edges.json` at every style.

mod edge_geometry_invariants {
    use graph_contract::canonical_json::{Value, parse};
    use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
    use graph_core::post::styles::{STYLES, Style, StyleParams, style_edges};
    use graph_core::{
        EdgeRecord, NodeKind, NodeRecord, Topology, child_first_from_type, edge_kind_from_type,
        index_model,
    };

    /// `fixtures/post/parallel-edges.json`, read as records. The crate is pure — no I/O
    /// — so the text is compiled in, exactly as `layout/hierarchy/fixture.rs` does it.
    const PARALLEL_EDGES: &str = include_str!("../../../fixtures/post/parallel-edges.json");

    mod csr;
    mod fan;
    mod ledger;

    /// The two chord endpoints, in the order the edges are listed.
    type Endpoints = (&'static str, &'static str);

    /// One boundary: the graph, and the node centres the style reads out of it.
    struct Case {
        /// What this boundary is, for the assertion messages.
        name: &'static str,
        /// Each node's centre, in the order the records list them.
        centres: Vec<(f32, f32)>,
        /// The graph.
        topology: Topology,
    }

    impl Case {
        /// A case over `centres`, whose `i`th node is `node_id(i)`, with `edges` between
        /// them. Every edge is kept: a dropped edge would make the boundary it is meant
        /// to test a different boundary.
        fn new(name: &'static str, centres: &[(f32, f32)], edges: &[Endpoints]) -> Self {
            let nodes: Vec<NodeRecord> = centres
                .iter()
                .enumerate()
                .map(|(i, _)| node(node_id(i)))
                .collect();
            let records: Vec<EdgeRecord> = edges.iter().map(|&(s, t)| record(s, t)).collect();
            let topology = index_model(&nodes, &records).expect("a case always indexes");
            assert_eq!(topology.node_count() as usize, centres.len(), "{name}");
            assert_eq!(
                topology.edge_count() as usize,
                edges.len(),
                "{name}: an edge was dropped"
            );
            Self {
                name,
                centres: centres.to_vec(),
                topology,
            }
        }

        /// The shipped fixture, loaded and indexed on the same shape. Its centres are
        /// spread along `x` at `4.0` apart, so no two coincide and no chord is vertical.
        fn fixture() -> Self {
            let root = parse(PARALLEL_EDGES).expect("the fixture parses");
            let nodes: Vec<NodeRecord> = array(&root, "nodes").iter().map(record_node).collect();
            let edges: Vec<EdgeRecord> = array(&root, "edges").iter().map(record_edge).collect();
            let centres: Vec<(f32, f32)> =
                (0..nodes.len()).map(|i| (4.0 * i as f32, 0.0)).collect();
            let topology = index_model(&nodes, &edges).expect("the fixture indexes");
            assert_eq!(topology.node_count() as usize, nodes.len());
            assert_eq!(
                topology.edge_count() as usize,
                edges.len(),
                "an edge was dropped"
            );
            Self {
                name: "parallel-edges.json",
                centres,
                topology,
            }
        }

        /// The node geometry a style reads centres out of. `Circle` and `Box` carry the
        /// same `x`/`y` as `Point`, which is what makes a style layout-agnostic, so the
        /// invariant is checked on all three node kinds.
        fn geometry(&self, kind: u8) -> NodeGeometry {
            let x: Vec<f32> = self.centres.iter().map(|c| c.0).collect();
            let y: Vec<f32> = self.centres.iter().map(|c| c.1).collect();
            let n = x.len();
            match kind {
                0 => NodeGeometry::Point { x, y },
                1 => NodeGeometry::Circle {
                    x,
                    y,
                    r: vec![1.0; n],
                },
                _ => NodeGeometry::Box {
                    x,
                    y,
                    w: vec![2.0; n],
                    h: vec![3.0; n],
                },
            }
        }

        /// Row `e`'s interior points, as `(x, y)` pairs.
        fn row(&self, paths: &Paths, e: u32) -> Vec<(f32, f32)> {
            let e = e as usize;
            let (from, to) = (paths.offsets[e] as usize, paths.offsets[e + 1] as usize);
            (from..to)
                .map(|p| (paths.pts[2 * p], paths.pts[2 * p + 1]))
                .collect()
        }
    }

    /// The four named boundaries plus the two the arithmetic divides on, in a fixed
    /// order so a failure names its case.
    fn cases() -> Vec<Case> {
        vec![
            Case::new("zero edges", &[(0.0, 0.0), (4.0, 0.0)], &[]),
            Case::new("a single edge", &[(0.0, 0.0), (4.0, 0.0)], &[("a", "b")]),
            Case::new("a self-loop", &[(0.0, 0.0)], &[("a", "a")]),
            Case::new(
                "a parallel pair",
                &[(0.0, 0.0), (4.0, 0.0)],
                &[("a", "b"), ("b", "a")],
            ),
            Case::new(
                "two nodes at one position",
                &[(0.0, 0.0), (0.0, 0.0)],
                &[("a", "b"), ("b", "a")],
            ),
            Case::fixture(),
        ]
    }

    /// Node `i`'s stable id. The dense index is the order the records are listed in, so
    /// this is what makes "the lower dense index of a pair" a testable thing.
    fn node_id(i: usize) -> String {
        ["a", "b", "c", "d", "e", "f"][i].to_owned()
    }

    fn node(id: String) -> NodeRecord {
        NodeRecord {
            kind: NodeKind::Record,
            database_id: None,
            source: "fixture".into(),
            label: id.clone(),
            group: None,
            weight: 1.0,
            version: 0.0,
            has_note: false,
            icon: None,
            id,
        }
    }

    /// An edge between two node ids, classified the way the ingest classifies it.
    fn record(source: &str, target: &str) -> EdgeRecord {
        wire_edge(&format!("{source}->{target}"), source, target, "relates_to")
    }

    fn record_node(value: &Value) -> NodeRecord {
        node(text(value, "id"))
    }

    fn record_edge(value: &Value) -> EdgeRecord {
        wire_edge(
            &text(value, "id"),
            &text(value, "source"),
            &text(value, "target"),
            &text(value, "type"),
        )
    }

    fn wire_edge(id: &str, source: &str, target: &str, wire_type: &str) -> EdgeRecord {
        EdgeRecord {
            id: id.to_owned(),
            source: source.to_owned(),
            target: target.to_owned(),
            kind: edge_kind_from_type(Some(wire_type)),
            child_first: child_first_from_type(Some(wire_type)),
            label: wire_type.to_owned(),
            strength: 1.0,
            directed: true,
            record_id: None,
        }
    }

    fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
        let Value::Object(members) = value else {
            panic!("not an object: {value:?}");
        };
        members
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no {key}"))
    }

    fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
        match member(value, key) {
            Value::Array(items) => items,
            other => panic!("{key}: {other:?}"),
        }
    }

    fn text(value: &Value, key: &str) -> String {
        match member(value, key) {
            Value::String(text) => text.clone(),
            other => panic!("{key}: {other:?}"),
        }
    }

    /// The `Paths` a style emitted, or `None` for the styles that emit `Line` and so
    /// carry no columns at all.
    fn paths(edges: &EdgeGeometry) -> Option<&Paths> {
        match edges {
            EdgeGeometry::Line => None,
            EdgeGeometry::Polyline(paths) | EdgeGeometry::Curve { paths, .. } => Some(paths),
        }
    }

    /// `style`'s output over `case`, at the pinned default parameters.
    fn styled(case: &Case, style: Style, kind: u8) -> EdgeGeometry {
        let params = StyleParams::for_style(style);
        style_edges(&case.topology, &case.geometry(kind), &params)
            .unwrap_or_else(|err| panic!("{}: {style:?}: {err}", case.name))
    }

    /// The parallel-pair boundary, shared by the two tests that need it.
    fn parallel_pair() -> Case {
        Case::new(
            "a parallel pair",
            &[(0.0, 0.0), (4.0, 0.0)],
            &[("a", "b"), ("b", "a")],
        )
    }

    /// The `Paths` `style` stored over `case`. Panics when it stored none, so a test
    /// can never pass by inspecting nothing.
    fn paths_of(case: &Case, style: Style) -> Paths {
        let edges = styled(case, style, 0);
        paths(&edges)
            .unwrap_or_else(|| panic!("{}: {style:?} stored no rows", case.name))
            .clone()
    }

    /// Every edge's row of interior points, in edge order.
    ///
    /// `Straight` stores nothing — a straight edge's endpoints are its nodes, so
    /// `EdgeGeometry::Line` carries no columns at all — and the tests that call this
    /// skip that one style explicitly. Every *other* style must produce rows, and a
    /// generator that emitted `Line` (or an empty CSR) instead would make these
    /// boundaries pass by having nothing to check, which is why this panics rather than
    /// returning empty.
    fn rows_of(case: &Case, style: Style) -> Vec<Vec<(f32, f32)>> {
        let edges = styled(case, style, 0);
        let rows = paths(&edges).unwrap_or_else(|| {
            panic!(
                "{}: {style:?} stored no rows: Line carries no columns",
                case.name
            )
        });
        (0..case.topology.edge_count())
            .map(|e| case.row(rows, e))
            .collect()
    }
}
