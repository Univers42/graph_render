//! Building and exchanging the two arms' positions for `graph-cli stress`.
//!
//! Split out of `../stress.rs` for the house line limit, the same way
//! `quadtree.rs`/`quadtree/tests.rs` and `barnes_hut.rs`/`barnes_hut/*.rs` already are
//! on this branch; reported as a deviation from the phase prompt's file list, which
//! named neither.
//!
//! The exchange is a pair of JSONL files rather than a pipe, and the reason is
//! reproducibility: a run that wrote its d3 positions to a file can be re-read and
//! re-correlated without re-running the simulation, so a margin can be recomputed
//! exactly from what the oracle actually produced. A pipe would make every number
//! unreproducible without the oracle beside it.

use graph_contract::geometry::NodeGeometry;
use graph_core::layout::Geometry;
use serde_json::{Value, json};
use std::path::Path;

/// One seed's case: the simple graph both arms lay out, and one arm's positions over
/// it. The same shape travels in both directions, so `edges` is carried alongside the
/// positions it belongs to and each arm is checked against the other for agreement on
/// the node count.
#[derive(Debug, Clone)]
pub struct Case {
    /// The seed of the gate model this case came from.
    pub seed: u32,
    /// Every simple edge as `[lo, hi]` with `lo < hi`.
    pub edges: Vec<[u32; 2]>,
    /// Positions in node order, as `f64`.
    pub positions: Vec<(f64, f64)>,
}

/// Our side: the gate model for seeds `0..seeds`, `layout`'s positions over it, and the
/// simple graph both arms lay out.
pub fn ours(seeds: u32, layout: super::Layout) -> Result<Vec<Case>, String> {
    use graph_core::layout::force::ForceParams;
    use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
    (0..seeds)
        .map(|seed| {
            let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
            let topology = index_model(&nodes, &edges).map_err(|e| format!("seed {seed}: {e}"))?;
            let geometry = layout(&topology, &ForceParams::default())
                .map_err(|e| format!("seed {seed}: {e}"))?;
            Ok(case(seed, &topology, &geometry))
        })
        .collect()
}

/// One case from a topology and the force layout's geometry over it.
pub fn case(seed: u32, topology: &graph_core::Topology, geometry: &Geometry) -> Case {
    Case {
        seed,
        edges: simple_edges(topology),
        positions: points(topology.node_count() as usize, geometry),
    }
}

/// Every simple edge as `[lo, hi]` with `lo < hi`, first by ascending raw edge index —
/// the same collapse and the same first-wins rule `SimpleGraph` applies inside the
/// layout (`layout/force/mod.rs`), so both arms lay out one graph. Raw source/target
/// roles are not sent: the link force's degree bias is stated in terms of lo/hi, so
/// the direction a record happened to carry means nothing here.
fn simple_edges(t: &graph_core::Topology) -> Vec<[u32; 2]> {
    let edges = t.edges();
    let mut seen = std::collections::HashSet::with_capacity(t.edge_count() as usize);
    let mut simple = Vec::with_capacity(t.edge_count() as usize);
    for e in 0..t.edge_count() as usize {
        let (a, b) = (edges.source[e], edges.target[e]);
        let pair = [a.min(b), a.max(b)];
        if a != b && seen.insert(pair) {
            simple.push(pair);
        }
    }
    simple
}

/// A force layout's point geometry as `f64`, widened from the `f32` the snapshot holds,
/// so both arms are correlated in one type.
fn points(n: usize, geometry: &Geometry) -> Vec<(f64, f64)> {
    let NodeGeometry::Point { x, y } = &geometry.nodes else {
        panic!("a force layout emitted {:?}, not Point", geometry.nodes);
    };
    assert_eq!(x.len(), n, "geometry does not cover the topology");
    x.iter()
        .zip(y)
        .map(|(&a, &b)| (f64::from(a), f64::from(b)))
        .collect()
}

/// Runs `harness/stress-d3.mjs` under Node over `cases` and reads back its positions and
/// the wall time of each case's 112 ticks.
///
/// Node is in the toolchain image; `d3-force` must be resolvable from the workspace
/// root (`node_modules`, or `NODE_PATH`; the script falls back to `createRequire`
/// because ESM resolution ignores `NODE_PATH`). Where it is not, this is a "could not
/// run" naming the module — never a silent pass, which would be the worst possible
/// outcome for a quality gate: a gate that cannot reach its baseline would otherwise
/// report a margin against nothing.
pub fn d3(scratch: &Path, cases: &[Case]) -> Result<Vec<(Case, f64)>, String> {
    let root = crate::runner::workspace_root();
    let input = scratch.join("stress-in.jsonl");
    let output = scratch.join("stress-d3.jsonl");
    std::fs::write(&input, input_text(cases)).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut node = std::process::Command::new("node");
    node.current_dir(&root)
        .arg(root.join("harness").join("stress-d3.mjs"))
        .arg(&input)
        .arg(&output);
    crate::runner::run_lines(&mut node)?;
    let text =
        std::fs::read_to_string(&output).map_err(|e| format!("{}: {e}", output.display()))?;
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() != cases.len() {
        return Err(format!(
            "the d3 arm returned {} cases, expected {}",
            lines.len(),
            cases.len()
        ));
    }
    lines
        .iter()
        .zip(cases)
        .map(|(line, case)| {
            let value: Value = serde_json::from_str(line).map_err(|e| format!("d3 case: {e}"))?;
            let positions = pairs(&value["x"], &value["y"])?;
            if positions.len() != case.positions.len() {
                return Err(format!(
                    "the d3 arm returned {} positions for seed {}, expected {}",
                    positions.len(),
                    case.seed,
                    case.positions.len()
                ));
            }
            let ms = value["ms"].as_f64().ok_or("the d3 arm reported no time")?;
            let theirs = Case {
                seed: case.seed,
                edges: case.edges.clone(),
                positions,
            };
            Ok((theirs, ms))
        })
        .collect()
}

/// The input lines the d3 arm reads. Positions are deliberately **not** sent: the d3
/// simulation seeds itself from the same golden spiral our port does, so shipping ours
/// would invite a reader to believe it started from them.
fn input_text(cases: &[Case]) -> String {
    let mut text = String::new();
    for case in cases {
        text.push_str(
            &json!({ "seed": case.seed, "n": case.positions.len(), "edges": case.edges })
                .to_string(),
        );
        text.push('\n');
    }
    text
}

/// One case's `x` and `y` columns, refusing a length mismatch or a non-finite value
/// rather than correlating a position that is not a position (D9).
fn pairs(x: &Value, y: &Value) -> Result<Vec<(f64, f64)>, String> {
    let xs = x.as_array().ok_or("x is not an array")?;
    let ys = y.as_array().ok_or("y is not an array")?;
    if xs.len() != ys.len() {
        return Err(format!("x has {} values and y has {}", xs.len(), ys.len()));
    }
    xs.iter()
        .zip(ys)
        .map(|(a, b)| {
            Ok((
                a.as_f64().ok_or("a non-finite x")?,
                b.as_f64().ok_or("a non-finite y")?,
            ))
        })
        .collect()
}
