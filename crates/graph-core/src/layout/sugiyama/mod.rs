//! Sugiyama-style layered DAG drawing (Sugiyama, Tagawa & Toda, "Methods for Visual
//! Understanding of Hierarchical System Structures", 1981). Ported from
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:1-693`; see
//! `docs/decisions/sugiyama-heuristics.md` for the full citation list and every
//! deviation.
//!
//! Pipeline: [`acyclic`] breaks cycles, [`layering`] assigns layers and dummy chains,
//! [`ordering`] reduces crossings, [`coords`] assigns X, [`routing`] builds the geometry.

mod acyclic;
mod coords;
mod layering;
mod ordering;
mod routing;

use super::Geometry;
use crate::index::Topology;
use crate::stage::Stage;
use crate::stage::StageError;
use acyclic::{Acyclic, Arcs};
use coords::Coords;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use layering::{DUMMY_BUDGET, Layering, assign_layers};
use ordering::Ordering;
use routing::{LAYER_SPACING, Routing, edge_paths, node_positions};

/// Cycle breaking through crossing reduction, the three stages [`run`] and
/// [`crossings_for`] share.
fn layered(topology: &Topology) -> (Acyclic, Layering, Ordering) {
    let acyclic = Acyclic::of(topology);
    let arcs = Arcs::new(topology, &acyclic);
    let layer = assign_layers(&arcs);
    let layering = Layering::build(&arcs, &layer, DUMMY_BUDGET);
    let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
    let ordering = Ordering::build(&layering, num_layers);
    (acyclic, layering, ordering)
}

/// The layered-DAG stage.
#[derive(Debug, Clone, Copy)]
pub struct Sugiyama;

/// The layered-DAG stage's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SugiyamaParams {
    /// Y distance between adjacent layers. Finite and above 0.
    pub layer_spacing: f32,
}

impl Default for SugiyamaParams {
    fn default() -> Self {
        Self {
            layer_spacing: LAYER_SPACING,
        }
    }
}

impl Stage for Sugiyama {
    type Params = SugiyamaParams;
    const ID: &'static str = "layout.dag.sugiyama";

    fn run(topology: &Topology, params: &SugiyamaParams) -> Result<Geometry, StageError> {
        run(topology, params.layer_spacing)
    }
}

/// Runs the whole pipeline: cycle breaking, layering, crossing reduction, X assignment,
/// then the geometry itself. Fails only on a `layer_spacing` that is not finite and above
/// 0: every `Topology` this crate can build, including the empty one, has a layered drawing.
pub fn run(topology: &Topology, layer_spacing: f32) -> Result<Geometry, StageError> {
    if !(layer_spacing.is_finite() && layer_spacing > 0.0) {
        return Err(StageError::Param {
            name: "layer_spacing",
            rule: "finite and above 0",
        });
    }
    let (acyclic, layering, ordering) = layered(topology);
    let coords = Coords::build(&ordering, &layering, topology.node_count());
    let routing = Routing {
        layering: &layering,
        coords: &coords,
        acyclic: &acyclic,
        spacing: layer_spacing,
    };
    let (x, y) = node_positions(&routing, topology.node_count());
    let paths = edge_paths(&routing);
    let mut notes = acyclic.notes;
    notes.extend(layering.notes);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Polyline(paths),
        notes,
    ))
}

/// The crossing count [`run`] would draw `topology` with: the oracle differential's
/// measurement hook (`docs/measurements/phase05-crossings.md`). Test-only: `run` itself
/// never needs it, since the geometry it returns carries no crossing count of its own.
#[cfg(test)]
pub(crate) fn crossings_for(topology: &Topology) -> u64 {
    layered(topology).2.crossings
}

/// `dot`'s weighted median (Gansner, Koutsofios, North & Vo 1993): the middle of
/// `values`, `-1.0` for none, the two middle values averaged for an even count except
/// that the pair is weighted by how far each sits from its own end when both gaps are
/// nonzero (`hierarchical.py:444-458`). Shared by [`ordering`] (over integer layer
/// positions) and [`coords`] (over the real-valued X being assigned).
pub(super) fn weighted_median<T>(mut values: Vec<T>) -> f64
where
    T: Copy + Into<f64> + PartialOrd,
{
    if values.is_empty() {
        return -1.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("no NaN reaches a median"));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        return values[middle].into();
    }
    let (first, last) = (values[0].into(), values[values.len() - 1].into());
    let (below, above) = (values[middle - 1].into(), values[middle].into());
    let (left, right) = (below - first, last - above);
    if left + right == 0.0 {
        // Also the len==2 case: both gaps are 0 against the same two values.
        return (below + above) / 2.0;
    }
    (below * right + above * left) / (left + right)
}

#[cfg(test)]
mod tests;

/// Dumps our own crossing counts on the 6 fixtures plus a synthetic sweep, for the oracle
/// differential in `docs/measurements/phase05-crossings.md`. Not a correctness check: run
/// alone, `--ignored`, and read by `harness/oracle-layouts.mjs --dag`.
#[cfg(test)]
mod measurement {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};
    use crate::synthetic::Mulberry32;
    use graph_contract::canonical_json::{Value, parse};
    use std::fmt::Write as _;

    const FIXTURES: [(&str, &str); 6] = [
        (
            "chain",
            include_str!("../../../../../fixtures/dag/chain.json"),
        ),
        (
            "diamond",
            include_str!("../../../../../fixtures/dag/diamond.json"),
        ),
        (
            "cyclic",
            include_str!("../../../../../fixtures/dag/cyclic.json"),
        ),
        (
            "multi-span",
            include_str!("../../../../../fixtures/dag/multi-span.json"),
        ),
        (
            "wide-layer",
            include_str!("../../../../../fixtures/dag/wide-layer.json"),
        ),
        (
            "disconnected",
            include_str!("../../../../../fixtures/dag/disconnected.json"),
        ),
    ];

    fn member<'a>(v: &'a Value, k: &str) -> &'a Value {
        let Value::Object(m) = v else {
            panic!("not an object")
        };
        &m.iter().find(|(key, _)| key == k).expect("member").1
    }
    fn text(v: &Value, k: &str) -> String {
        match member(v, k) {
            Value::String(s) => s.clone(),
            other => panic!("{k}: {other:?}"),
        }
    }
    fn array<'a>(v: &'a Value, k: &str) -> &'a [Value] {
        match member(v, k) {
            Value::Array(a) => a,
            other => panic!("{k}: {other:?}"),
        }
    }

    type Graph = (Vec<String>, Vec<(String, String, String)>);

    /// One fixture's `(node ids, (edge id, source, target))`.
    fn load(fixture: &str) -> Graph {
        let root = parse(fixture).expect("valid fixture json");
        let nodes = array(&root, "nodes")
            .iter()
            .map(|n| text(n, "id"))
            .collect();
        let edges = array(&root, "edges")
            .iter()
            .map(|e| (text(e, "id"), text(e, "source"), text(e, "target")))
            .collect();
        (nodes, edges)
    }

    /// A random DAG over `6..26` nodes, edges only `i < j` so it is acyclic and
    /// parallel-edge-free by construction, at density 0.2, from `seed`.
    fn synthetic_dag(seed: u32) -> Graph {
        let mut rnd = Mulberry32::new(seed);
        let n = 6 + rnd.pick(20) as u32;
        let nodes: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        let mut edges = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                if rnd.next_f64() < 0.2 {
                    edges.push((format!("{i}-{j}"), i.to_string(), j.to_string()));
                }
            }
        }
        (nodes, edges)
    }

    fn topology_of(nodes: &[String], edges: &[(String, String, String)]) -> Topology {
        let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
        let e: Vec<_> = edges.iter().map(|(id, s, t)| edge(id, s, t)).collect();
        index_model(&n, &e).expect("fixture and synthetic graphs always fit")
    }

    /// Appends `{"name","nodes","edges":[[s,t],...],"our_crossings"}` for one graph.
    fn dump_one(
        out: &mut String,
        name: &str,
        nodes: &[String],
        edges: &[(String, String, String)],
    ) {
        let crossings = crossings_for(&topology_of(nodes, edges));
        write!(out, r#"{{"name":{name:?},"nodes":["#).expect("string write");
        for (i, id) in nodes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(out, "{id:?}").expect("string write");
        }
        out.push_str(r#"],"edges":["#);
        for (i, (_, s, t)) in edges.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(out, "[{s:?},{t:?}]").expect("string write");
        }
        write!(out, r#"],"our_crossings":{crossings}}}"#).expect("string write");
    }

    #[test]
    #[ignore = "writes target/dag-crossings.json for the Node oracle differential"]
    fn dump_crossing_measurements() {
        let mut out = String::from("[");
        for (i, (name, fixture)) in FIXTURES.into_iter().enumerate() {
            let (nodes, edges) = load(fixture);
            if i > 0 {
                out.push(',');
            }
            dump_one(&mut out, name, &nodes, &edges);
        }
        for seed in 0..230u32 {
            let (nodes, edges) = synthetic_dag(seed);
            if edges.is_empty() {
                continue;
            }
            out.push(',');
            dump_one(&mut out, &format!("synthetic-{seed}"), &nodes, &edges);
        }
        out.push(']');
        let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&target).expect("mkdir target");
        std::fs::write(target.join("dag-crossings.json"), out).expect("write dump");
    }
}
