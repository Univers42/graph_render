//! The packing and grid oracles, each held to a snapshot built for it: an exact planar
//! packing (so the tangency branch really runs — the gate's large models never reach it,
//! every model past about two dozen nodes being non-planar and carrying note 3), a
//! fallback packing carrying that note (so its early return is exercised from both
//! sides), and hand-built circles whose numbers this file chose, for the states a real
//! packing can never be: exactly tangent, a thousandth off either way, radius zero.

mod circular;
mod packing;

use super::*;
use graph_contract::binary::StringTable;
use graph_contract::geometry::NodeGeometryKind;
use graph_contract::notes::{Note, Notes, SNAPSHOT_WIDE};
use graph_contract::snapshot::{Dim, label_for};
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord, gate_node_count, registry, run_with};

/// `n` nodes named `n0..` and one `relation` edge per `(u32, u32)` pair: the terse record
/// shape `graph-core`'s own circle-packing tests build, restated here because that
/// module's builders live inside its test cfg.
fn records(n: u32, edges: &[(u32, u32)]) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let nodes = (0..n)
        .map(|i| NodeRecord {
            id: format!("n{i}"),
            kind: NodeKind::Record,
            database_id: None,
            source: "hand".into(),
            label: format!("L{i}"),
            group: None,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon: None,
        })
        .collect();
    let edges = edges
        .iter()
        .enumerate()
        .map(|(i, &(u, v))| EdgeRecord {
            id: format!("e{i}"),
            source: format!("n{u}"),
            target: format!("n{v}"),
            kind: EdgeKind::Relation,
            child_first: false,
            label: String::new(),
            strength: 0.5,
            directed: false,
            record_id: None,
        })
        .collect();
    (nodes, edges)
}

/// A hub joined to a 6-node rim: planar, and maximal planar once triangulated, so the
/// exact path certifies it — the shape `graph-core`'s own `a_wheel_packs_exactly` pins.
const WHEEL_RIM: u32 = 6;

/// A wheel packed through the registered layout: the snapshot the tangency branch of
/// [`packing`] is really written for.
fn exact_packing() -> Snapshot {
    let mut edges: Vec<(u32, u32)> = (1..=WHEEL_RIM).map(|i| (0, i)).collect();
    edges.extend((1..=WHEEL_RIM).map(|i| (i, if i == WHEEL_RIM { 1 } else { i + 1 })));
    let (nodes, edges) = records(WHEEL_RIM + 1, &edges);
    let layout = registry::find("layout.packing.circle").expect("registered");
    run_with(&nodes, &edges, layout.id, layout.run)
        .expect("runs")
        .snapshot
}

/// The gate's own model at 30 nodes, non-planar at `REFERENCE_DEGREE` density: its
/// packing carries note 3 and promises no tangency. (Below about two dozen nodes the
/// gate's own models are planar and reach the exact path instead — see
/// [`the_gate_models_of_the_small_sweep_seeds_reach_the_exact_path`].)
fn fallback_packing() -> Snapshot {
    let (nodes, edges) = seeded_model(4, 30, REFERENCE_DEGREE);
    let layout = registry::find("layout.packing.circle").expect("registered");
    run_with(&nodes, &edges, layout.id, layout.run)
        .expect("runs")
        .snapshot
}

/// The hand-built circle snapshot: `centres` as `(x, y)`, `radii`, and `edges` as dense
/// `(source, target)` pairs. The smallest legal input, so every branch the oracle can
/// take is reachable here without a layout run.
fn circles(centres: &[(f32, f32)], radii: &[f32], edges: &[(u32, u32)]) -> Snapshot {
    let table = |column: &'static str, items: Vec<String>| {
        StringTable::from_strs(column, items.iter().map(String::as_str)).expect("fits")
    };
    let parts = SnapshotParts {
        version: label_for(Dim::D2),
        node_ids: table(
            "node.id",
            (0..radii.len()).map(|i| format!("n{i}")).collect(),
        ),
        edge_ids: table(
            "edge.id",
            (0..edges.len()).map(|i| format!("e{i}")).collect(),
        ),
        source: edges.iter().map(|e| e.0).collect(),
        target: edges.iter().map(|e| e.1).collect(),
        nodes: NodeGeometry::Circle {
            x: centres.iter().map(|c| c.0).collect(),
            y: centres.iter().map(|c| c.1).collect(),
            r: radii.to_vec(),
        },
        z: None,
        edges: EdgeGeometry::Line,
        notes: Notes::default(),
    };
    Snapshot::new(parts).expect("valid")
}

/// Two circles `dist` apart on the x axis, of radius `r` and `r + bias`. With `dist` and
/// the radii sum chosen here, the tangency error is exactly this file's to pick.
fn pair(dist: f32, r: f32, bias: f32) -> Snapshot {
    circles(&[(0.0, 0.0), (dist, 0.0)], &[r, r + bias], &[(0, 1)])
}

/// `snapshot` with node `i` moved by `(dx, dy)`, rebuilt through the constructor, so the
/// perturbation stays a legal snapshot and only the geometry differs.
fn moved(snapshot: &Snapshot, i: usize, (dx, dy): (f32, f32)) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    match &mut parts.nodes {
        NodeGeometry::Point { x, y } | NodeGeometry::Circle { x, y, .. } => {
            x[i] += dx;
            y[i] += dy;
        }
        other => panic!("{other:?}"),
    }
    Snapshot::new(parts).expect("valid")
}

/// `snapshot` with `notes` in place of its own, rebuilt through the constructor: the
/// only way a note can legitimately come or go.
fn renoted(snapshot: &Snapshot, notes: &[Note]) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    parts.notes = Notes::of(notes);
    Snapshot::new(parts).expect("valid")
}

/// The one note a packing may carry: `packing.approximate`, snapshot-wide.
fn approximate() -> [Note; 1] {
    [Note {
        code: NoteCode::PackingApproximate,
        index: SNAPSHOT_WIDE,
    }]
}

/// The largest `|distance - (r_u + r_v)|` over the graph's own edges, in `f32`, the way
/// the oracle measures it.
fn worst_error(exact: &Snapshot) -> f32 {
    let p = exact.parts();
    let NodeGeometry::Circle { x, y, r } = &p.nodes else {
        panic!("circles");
    };
    p.source
        .iter()
        .zip(&p.target)
        .map(|(&u, &v)| (u as usize, v as usize))
        .map(|(u, v)| {
            let (dx, dy) = (x[u] - x[v], y[u] - y[v]);
            (dx * dx + dy * dy).sqrt() - (r[u] + r[v])
        })
        .fold(0.0_f32, |worst, e| worst.max(e.abs()))
}

/// A snapshot of another kind is refused before any arithmetic: the exact message, for
/// each oracle and for a kind the exercise draws (`Box` is as foreign to the packing
/// oracle as `Point` is).
#[test]
fn a_snapshot_of_another_kind_is_refused_before_any_arithmetic() {
    let circles = exact_packing();
    assert_eq!(
        super::circular(0, 0, &circles),
        Err("not Point nodes with Line edges".into())
    );
    let grid = super::super::pipeline(1, 9, "grid").expect("runs").snapshot;
    assert_eq!(
        super::packing(&grid),
        Err("not Circle nodes with Line edges".into())
    );
    let boxed = super::super::exercise::snapshot(2).expect("valid");
    assert_eq!(boxed.header().node_kind, NodeGeometryKind::Box);
    assert_eq!(
        super::packing(&boxed),
        Err("not Circle nodes with Line edges".into())
    );
}

/// Which of the gate's own models reach the exact path, measured rather than assumed:
/// the unit sweep's seeds (2 to 14 nodes) are planar and carry no note 3, so the tangency
/// branch really does run there — the wheel above is not its only way in — while a model
/// of 30 nodes is not planar and never reaches it.
#[test]
fn the_gate_models_of_the_small_sweep_seeds_reach_the_exact_path() {
    for seed in 0..12u32 {
        let nodes = gate_node_count(seed);
        let exact = super::super::pipeline(seed, nodes, "packing.circle")
            .expect("runs")
            .snapshot;
        let code = NoteCode::PackingApproximate as u32;
        assert!(
            !exact.parts().notes.code.contains(&code),
            "seed {seed} at {nodes} nodes: a small gate model is planar and exact"
        );
        assert_eq!(
            super::packing(&exact),
            Ok(()),
            "seed {seed}: branch ran and passed"
        );
    }
    assert_eq!(
        fallback_packing().parts().notes.code,
        vec![NoteCode::PackingApproximate as u32]
    );
}
