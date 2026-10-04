//! The `scale.*` rows' differential: `graph-core`'s LOD and simplify against
//! `SciGraphs/engine/scigraphs_engine/{lod.py,simplify.py}` run unchanged in the
//! `ge-python-oracle` image. Fixtures are the hand-built graphs of [`CASES`], one JSON
//! line each, read by both arms.
//!
//! The outputs are masks and index lists, so agreement is **exact equality**, not a
//! tolerance: a failing run reports one disagreement and its ceiling is zero. Every
//! float in a fixture is an exact binary fraction, and `serde_json` writes an `f64` with
//! the shortest string that round-trips to the same bits, so the reference reads back
//! the camera and the geometry we meant.
//!
//! Three reference functions are in the comparison, and each is given only what it takes:
//!
//! - `lod.apply_budget(order_key, counts, budget)` against `lod::hints(..).labelled`,
//!   with the node degree as the `order_key` and one node per block, so "how many nodes
//!   keep a label within a budget of b" is the same question on both sides;
//! - `lod.frustum_cull_spheres(centers, radii, persp_mat)` against
//!   `lod::hints(..).visible`, over the square orthographic camera [`ortho`] builds from
//!   the very `Viewport` the motor was given;
//! - `simplify.build_coarse_level(..)` against the community pass's journal: the same
//!   partition in, the same set of representative-level links out.
//!
//! **Ponytail: three reference functions, not six.** `adaptive.py` and `simplify.py`'s
//! backbone modes have no counterpart in the motor — the first cuts a hierarchy the
//! motor never builds, the second extracts a lossy backbone it deliberately does not
//! port — and `lod.py`'s two radius conventions disagree with each other. Each is a gap
//! row in `docs/measurements/fix-scale-oracle.md`, and the harness prints them beside the
//! cases it did run rather than letting a short comparison look like a full one.
//!
//! A budget case whose cut falls inside a class of equal `order_key` is one the reference
//! cannot decide: `np.argsort` is not stable, so its tie order is the sort's, while the
//! motor breaks ties by ascending dense index (D2). The harness reports those separately
//! instead of asking the reference to have agreed with an arbitrary order.

use super::Differential;
use cases::table;
use graph_core::Topology;
use graph_core::analysis::communities;
use graph_core::scale::lod::{LodParams, Viewport, hints as lod_hints};
use graph_core::scale::simplify::{Kind, Plan, Simplified, simplify};
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod cases;

/// The `Differential`'s entry point: the emit's line for case `case`, which is its index in
/// [`cases::table`]. The second argument is the emit's `--max-iter` override, which this
/// differential has no use for — no reference function here iterates.
fn line(case: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    table(case)
}

pub const SCALE: Differential = Differential {
    name: "scale",
    ceilings: &[
        ("scale.lod.apply_budget", "apply_budget", EXACT),
        (
            "scale.lod.frustum_cull_spheres",
            "frustum_cull_spheres",
            EXACT,
        ),
        (
            "scale.simplify.build_coarse_level",
            "build_coarse_level",
            EXACT,
        ),
    ],
    line,
};

/// A mask is a mask: the arms must return the same `0`/`1` column, so the worst a failing
/// run can measure is one disagreement and the ceiling above it is zero.
pub const EXACT: f64 = 0.0;

/// How many lines `emit-scale-fixtures --cases` writes by default: one per case in
/// [`line()`]. The cases are hand-built, so the count is a property of this file rather
/// than a budget a caller raises.
pub const CASES: u32 = 12;

/// How far a cull fixture's node must sit from every side's **decision** boundary, which is
/// the side grown by the radius. The reference divides by `abs(w) + 1e-9` (`lod.py:40`),
/// which moves each boundary about 4e-9 world units off its own value; a node closer than
/// [`SLACK`] could fall either way, so such a fixture is refused rather than compared.
const SLACK: f64 = 1.0e-6;

/// One fixture graph: an indexed topology and one position per node, in index order.
/// Built by [`cases`], read by the three case writers below.
struct Fixture {
    topology: Topology,
    x: Vec<f64>,
    y: Vec<f64>,
}

/// The label budget, against `lod.apply_budget`.
///
/// The node degree is the `order_key` the reference takes as an argument and one node per
/// block, so `cumulative <= budget` answers "how many nodes keep a label" on both sides.
/// Every node is visible and the graph is small enough for the full tier, so the mask the
/// motor returns is the budget's alone.
fn budget(name: &str, f: &Fixture, budget: u32) -> Result<Value, String> {
    let params = LodParams {
        label_budget: budget,
        ..LodParams::default()
    };
    let hints = lod_hints(&f.topology, &f.x, &f.y, &params);
    let counts = vec![1.0; f.topology.node_count() as usize];
    Ok(json!({
        "case": name,
        "reference": "lod.apply_budget",
        "input": { "order_key": degrees(&f.topology), "counts": counts, "budget": budget },
        "ours": { "mask": hints.labelled },
    }))
}

/// The viewport test, against `lod.frustum_cull_spheres` over the camera [`ortho`] builds
/// from the same rectangle. Refused rather than compared when the two could disagree on
/// rounding alone: see [`decidable`] and [`clip_radii`].
fn cull(name: &str, f: &Fixture, view: Viewport) -> Result<Value, String> {
    if !decidable(view, f) {
        return Err(format!(
            "{name}: a node is within {SLACK} of a side, so the \
            reference's 1e-9 slack decides it and the case would prove nothing"
        ));
    }
    let params = LodParams {
        viewport: view,
        ..LodParams::default()
    };
    let hints = lod_hints(&f.topology, &f.x, &f.y, &params);
    let centers: Vec<[f64; 3]> = f.x.iter().zip(&f.y).map(|(&x, &y)| [x, y, 0.0]).collect();
    let nodes = f.topology.node_count() as usize;
    Ok(json!({
        "case": name,
        "reference": "lod.frustum_cull_spheres",
        "input": { "centers": centers, "radii": clip_radii(view, nodes)?, "persp_mat": ortho(view) },
        "ours": { "mask": hints.visible },
    }))
}

/// The community collapse, against `simplify.build_coarse_level`.
///
/// The two arms are handed **the same partition**: the motor's louvain membership goes
/// into the fixture as the reference's `labels`, so what is compared is the collapse, not
/// the detection (Phase 7's `analysis.communities` row is what gates louvain). Only the
/// community pass runs — folding and contraction have no reference function — and only its
/// links: `Step::edges` and the mask are the journal's, and the coarse level is not a
/// journal.
fn coarse(name: &str, f: &Fixture) -> Result<Value, String> {
    let plan = Plan {
        collapse_communities: true,
        ..Plan::nothing()
    };
    let out = simplify(&f.topology, &plan);
    let n = f.topology.node_count() as usize;
    let labels = communities::louvain(&f.topology);
    let coords: Vec<[f64; 3]> = f.x.iter().zip(&f.y).map(|(&x, &y)| [x, y, 0.0]).collect();
    let colors: Vec<[f64; 4]> = (0..n).map(|_| [0.2, 0.4, 0.6, 1.0]).collect();
    Ok(json!({
        "case": name,
        "reference": "simplify.build_coarse_level",
        "input": { "coords": coords, "edges": pairs(&f.topology), "labels": labels,
                   "colors": colors, "base_radius": 1.0 },
        "map": { "community": community_map(&labels, &out.representative) },
        "ours": { "links": community_links(&out) },
    }))
}

/// The rectangle as the reference's camera: a clip matrix whose NDC box is exactly
/// `[x0, x1] x [y0, y1]`, `y` up the screen and `z` dropped, so the cull reduces to the
/// rectangle test the motor runs. Every entry is exact for a power-of-two rectangle, so
/// nothing here is a question about rounding.
fn ortho(view: Viewport) -> Vec<Vec<f64>> {
    let (w, h) = (view.x1 - view.x0, view.y1 - view.y0);
    vec![
        vec![2.0 / w, 0.0, 0.0, -(view.x0 + view.x1) / w],
        vec![0.0, -2.0 / h, 0.0, (view.y0 + view.y1) / h],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ]
}

/// `Viewport::radius` in the units `frustum_cull_spheres` reads: it divides by `abs(w)`
/// and compares against a box that spans `2` over the rectangle, so the world radius goes
/// in scaled by `2 / width`, one column per node.
///
/// Refused for a rectangle that is not square, where one scalar cannot be both axes'
/// margin, or whose scaled radius leaves the `z` box — a radius over half the side culls
/// every node on the dropped axis alone.
fn clip_radii(view: Viewport, nodes: usize) -> Result<Vec<f64>, String> {
    let (w, h) = (view.x1 - view.x0, view.y1 - view.y0);
    if w != h {
        return Err(
            "frustum_cull_spheres reads one clip-space radius for three axes, so it \
            carries Viewport::radius only through a square rectangle"
                .into(),
        );
    }
    if view.radius * 2.0 / w > 1.0 {
        return Err(
            "a node radius over half the rectangle's side is culled on the dropped \
            z axis alone, whatever x and y say"
                .into(),
        );
    }
    Ok(vec![view.radius * 2.0 / w; nodes])
}

/// Whether every node sits further than [`SLACK`] from every **decision** boundary, which
/// is the rectangle grown by the radius — not the rectangle itself, since a node whose disc
/// only touches an edge is the one the reference's `abs(w) + 1e-9` would move.
fn decidable(view: Viewport, f: &Fixture) -> bool {
    let r = view.radius;
    f.x.iter().zip(&f.y).all(|(&x, &y)| {
        [
            x - (view.x0 - r),
            (view.x1 + r) - x,
            y - (view.y0 - r),
            (view.y1 + r) - y,
        ]
        .iter()
        .all(|gap| gap.abs() > SLACK)
    })
}

/// The undirected edge list as the reference's `edges` column takes it: every edge once,
/// source then target, in dense edge index order. A self-loop is written as it is stored.
fn pairs(t: &Topology) -> Vec<[u32; 2]> {
    let edges = t.edges();
    (0..t.edge_count() as usize)
        .map(|e| [edges.source[e], edges.target[e]])
        .collect()
}

/// Each community's id and the representative it collapsed onto, ascending by id — the
/// order `np.unique` hands the reference's super-nodes back in.
fn community_map(labels: &[u32], representative: &[u32]) -> Vec<[u32; 2]> {
    let mut map: BTreeMap<u32, u32> = BTreeMap::new();
    for (node, &community) in labels.iter().enumerate() {
        map.entry(community).or_insert(representative[node]);
    }
    map.into_iter()
        .map(|(community, representative)| [community, representative])
        .collect()
}

/// Every community step's links as one ascending, deduplicated list of pairs: the edges a
/// front draws between representatives, which is the reference's `se_src`/`se_dst` read in
/// the motor's index space.
fn community_links(out: &Simplified) -> Vec<[u32; 2]> {
    let mut links: Vec<[u32; 2]> = out
        .steps
        .iter()
        .filter(|step| step.kind == Kind::Community)
        .flat_map(|step| step.links.iter().copied())
        .map(|(a, b)| [a, b])
        .collect();
    links.sort_unstable();
    links.dedup();
    links
}

/// Each node's undirected degree, self-loop counted twice, as the float
/// `apply_budget` sorts on. This is the motor's importance key (`lod.rs`, `degree_of`)
/// written down as an argument the reference is given rather than one it derives: the
/// reference has no topology, only an `order_key`.
fn degrees(t: &Topology) -> Vec<f64> {
    (0..t.node_count() as usize)
        .map(|i| (t.out().row(i as u32).len() + t.inbound().row(i as u32).len()) as f64)
        .collect()
}

#[cfg(test)]
mod tests;
