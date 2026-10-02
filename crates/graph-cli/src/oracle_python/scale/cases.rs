//! The fixture graphs the `scale.*` differential compares, and the table that names them.
//!
//! These are hand-built shapes rather than a seeded sweep, because each one is chosen for
//! a question the reference can answer: a hub whose five leaves tie at degree 1, a path
//! whose middle four tie at degree 2, two triangles joined by a single edge, a self-loop
//! on a member of one of them, and two rectangles whose cull boundary the nodes either
//! straddle or clear. There is no seed to vary, so [`CASES`] counts lines rather than
//! samples and `emit-scale-fixtures --cases` writes the whole table by default.
//!
//! Ponytail: twelve cases, and none of them is a random graph. Failing input: a defect
//! that needs a shape nobody wrote down — a multigraph, a disconnected component, a
//! community of size two. Direction: the differential passes while the defect stands, and
//! the defence is graph-core's own unit tests, which do sweep those. Escape hatch: add a
//! builder here and an arm to the table in [`super::line`].

use super::{CASES, Fixture, budget, coarse, cull};
use graph_core::scale::lod::Viewport;
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord, index_model};

/// One fixture line: the reference function's own arguments under `input`, the motor's
/// answer under `ours` in the same shape, and the small map the coarse-level arm needs to
/// speak the motor's index space. `reference` names the SciGraphs function to call.
pub(super) fn table(case: u32) -> Result<serde_json::Value, String> {
    match case {
        0 => budget("lod.budget.zero", &star()?, 0),
        1 => budget("lod.budget.one", &star()?, 1),
        2 => budget("lod.budget.two", &star()?, 2),
        3 => budget("lod.budget.all", &star()?, 6),
        4 => budget("lod.budget.path", &path()?, 3),
        5 => cull("lod.cull.half", &cull_half()?, square(0.0, 8.0, 0.5)),
        6 => cull(
            "lod.cull.straddle",
            &cull_straddle()?,
            square(0.0, 8.0, 0.5),
        ),
        7 => coarse("simplify.coarse.star", &star()?),
        8 => coarse("simplify.coarse.path", &path()?),
        9 => coarse("simplify.coarse.two", &two_communities()?),
        10 => coarse("simplify.coarse.self_loop", &self_loop_on_a_leaf()?),
        11 => budget("lod.budget.tie", &tied_degrees()?, 3),
        other => Err(format!("no scale case {other}: there are {CASES}")),
    }
}

/// Degrees `[1, 2, 3, 1, 2, 3]`: two classes tied across the budget's cut, which is the one
/// case `np.argsort`'s unstable order decides and the motor's dense-index rule (D2) does
/// not. It is here so the divergence is in the artifact rather than inferred — see the
/// `ties` section of `harness/oracle-scale.py`'s result.
fn tied_degrees() -> Result<Fixture, String> {
    let edges = [(0, 2), (2, 5), (2, 1), (1, 4), (4, 5), (5, 3)];
    graph(6, &edges, &row(6))
}

/// A hub and five leaves: the degree-1 fold's own shape, and a label budget whose cut
/// falls inside the five-way tie of the leaves.
fn star() -> Result<Fixture, String> {
    let leaves = [(0, 1), (0, 2), (0, 3), (0, 4), (0, 5)];
    graph(6, &leaves, &row(6))
}

/// A path of six: the chain pass's own shape, whose middle four all tie at degree 2.
pub(super) fn path() -> Result<Fixture, String> {
    let hops = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)];
    graph(6, &hops, &row(6))
}

/// Two triangles joined by one edge at 2 and 5 — two communities under louvain, and the
/// one cross-community edge that becomes the single super-edge.
fn two_communities() -> Result<Fixture, String> {
    let pairs = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (2, 5)];
    graph(6, &pairs, &row(6))
}

/// [`two_communities`] plus a self-loop on node 1, a member of the first community: the
/// reference reads it as intra-community and never draws it, and so must the journal.
fn self_loop_on_a_leaf() -> Result<Fixture, String> {
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 0),
        (3, 4),
        (4, 5),
        (5, 3),
        (2, 5),
        (1, 1),
    ];
    graph(6, &pairs, &row(6))
}

/// Eight nodes spread along `x`, so the rectangle keeps the first four and culls the rest.
fn cull_half() -> Result<Fixture, String> {
    let x = [1.0, 3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0];
    let hops = [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7)];
    graph(8, &hops, &column(&x, 2.0))
}

/// Six nodes straddling or past the right and bottom sides: `7.75` and `8.25` overlap
/// `x1 = 8`, `8.75` does not, and the same on `y`. A rectangle test that ignores a node's
/// radius, or only widens one axis, fails here.
fn cull_straddle() -> Result<Fixture, String> {
    let x = [1.0, 7.75, 8.25, 8.75, 3.0, 5.0];
    let y = [2.0, 2.0, 2.0, 2.0, 8.25, 8.75];
    let hops = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)];
    graph(
        6,
        &hops,
        &x.iter().zip(&y).map(|(&a, &b)| (a, b)).collect::<Vec<_>>(),
    )
}

/// `n` positions along a row at `y = 1`, used by the cases whose geometry the reference
/// never reads.
fn row(n: usize) -> Vec<(f64, f64)> {
    (0..n).map(|i| (i as f64, 1.0)).collect()
}

/// One position per entry of `x`, all at `y`, for the cull cases whose spread is the point.
fn column(x: &[f64], y: f64) -> Vec<(f64, f64)> {
    x.iter().map(|&a| (a, y)).collect()
}

/// The `0, 0, 8, 8` rectangle with a radius of `0.5`: the camera every cull case shares.
/// A square, because the reference reads one clip-space radius for three axes.
pub(super) fn square(x0: f64, side: f64, radius: f64) -> Viewport {
    Viewport::from_size(x0, x0, side, side, radius)
}

/// One fixture graph: `n` nodes named `n0..`, the undirected `pairs`, and one position
/// per node in the order written. Built by [`cases`], read by the three case writers.
/// `n` nodes named `n0..`, the undirected `pairs` (a `(k, k)` pair is a self-loop), and
/// `pos` — one position per node, in index order.
fn graph(n: usize, pairs: &[(usize, usize)], pos: &[(f64, f64)]) -> Result<Fixture, String> {
    let nodes: Vec<NodeRecord> = (0..n).map(node).collect();
    let edges: Vec<EdgeRecord> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(i, *a, *b))
        .collect();
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let (x, y): (Vec<f64>, Vec<f64>) = pos.iter().map(|&(a, b)| (a, b)).unzip();
    Ok(Fixture { topology, x, y })
}

/// One fixture node. Only the id is load-bearing: the scale passes read degrees and dense
/// indices, and nothing here renders a label.
fn node(index: usize) -> NodeRecord {
    NodeRecord {
        id: format!("n{index}"),
        kind: NodeKind::Record,
        database_id: None,
        source: "scale".to_string(),
        label: format!("n{index}"),
        group: None,
        weight: 1.0,
        version: 1.0,
        has_note: false,
        icon: None,
    }
}

/// One fixture edge, undirected and unweighted, named by its index so an edge's identity is
/// a function of where it was written.
fn edge(index: usize, source: usize, target: usize) -> EdgeRecord {
    EdgeRecord {
        id: format!("e{index}"),
        source: format!("n{source}"),
        target: format!("n{target}"),
        kind: EdgeKind::Relation,
        label: String::new(),
        strength: 1.0,
        directed: false,
        record_id: None,
        child_first: false,
    }
}
