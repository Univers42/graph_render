//! The ANALYSIS stage over the ABI (`docs/contract/wasm-abi.md` "ANALYSIS"): which
//! analysis `gm_analysis_run`'s index names, and the framed canonical-JSON face each one
//! returns.
//!
//! **Every row is graph-core's own function, called as it is.** Nothing in
//! `graph_core::analysis` is re-derived, re-weighted or re-ordered here: this module is
//! the registry (`id` + a `fn(&Topology)`), the JSON writer, and, for depth only, the one
//! place `Hierarchy::of`'s refusal becomes a caller-bug panic — `depth::bfs_depth` then
//! takes `&Hierarchy` directly, because graph-core implements `Roots` on that type. A number
//! that differs between this face and the one graph-core's own tests pin is a bug in one of
//! the two, and the tests below compare them rather than restating expectations.
//!
//! **What the JSON says, and what it does not.** Keys are in ascending order, one fixed
//! order for every analysis, so the face is canonical and byte-comparable (D7: the text
//! is the value). Every result carries `id`, `kind` (`"f64"` or `"u32"` — the element
//! type of `values`), `nodeCount` and `values` in dense-index order. Three analyses add
//! the one scalar graph-core hands back beside the column, because dropping it would
//! drop the escape hatch its own `Ponytail` marker names: `converged` for the
//! eigenvector iteration, `modularity` for Louvain, `max` for depth.
//!
//! **A path query is not here.** `graph_core::analysis::paths` needs a source node and a
//! mode, neither of which the `(handle, index)` signature can carry without inventing a
//! convention; it stays a graph-core-only capability until the ABI has a way to ask.
//!
//! Determinism: no clock, no RNG, no `HashMap` iteration, and a `Vec` in dense-index
//! order out of every function called here (D2, D3, D4, D5, D8, D10).

use graph_core::Topology;
use graph_core::analysis::depth;
use graph_core::analysis::{centrality, communities, components};
use graph_core::layout::hierarchy::Hierarchy;
use std::fmt::Write as _;

#[cfg(test)]
mod tests;

/// What one analysis computes: a per-node column, and the scalars graph-core hands back
/// beside it. The three optional fields are `None` for the analyses that have no such
/// scalar, and each is written only when it is `Some` — so a key's *presence* is itself
/// the statement of which analysis produced the object.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The analysis's capability id, restated in its own output.
    pub id: &'static str,
    /// Its per-node column, in dense-index order.
    pub values: Column,
    /// The eigenvector iteration's residual-verified convergence flag.
    pub converged: Option<bool>,
    /// Louvain's modularity of the partition it returned.
    pub modularity: Option<f64>,
    /// Depth's deepest level reached.
    pub max: Option<u32>,
}

/// One per-node analysis column. `f64` for a centrality, `u32` for a labelling — the
/// same two types the contract's own columns use, and the reason a consumer does not
/// have to know which analysis it is reading to know how to read `values`.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    /// `f64`, one per node. A centrality read as `f32` by graph-core, widened here —
    /// exact, and the wire type the analysis's own numbers are stated in.
    F64(Vec<f64>),
    /// `u32`, one per node. A label, a community id, a depth level, or the degree count.
    U32(Vec<u32>),
}

impl Column {
    /// The element type as the JSON names it — the one member of the face that says
    /// whether `values` holds scores or labels, so a consumer never has to infer it from
    /// which analysis it asked for.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::F64(_) => "f64",
            Self::U32(_) => "u32",
        }
    }
}

/// One analysis as the ABI names it.
pub struct Entry {
    /// Its capability id, e.g. `analysis.components.weak`.
    pub id: &'static str,
    /// It over `topology`, as graph-core computes it.
    pub run: fn(&Topology) -> Report,
}

/// Every analysis the ABI exposes, in registration order. Append-only: a row added here
/// is discoverable through `gm_analysis_count`/`gm_analysis_id` with no ABI change, the
/// property C1 states for layouts.
pub static ANALYSES: [Entry; 8] = [
    Entry {
        id: "analysis.components.weak",
        run: weak_components,
    },
    Entry {
        id: "analysis.components.strong",
        run: strong_components,
    },
    Entry {
        id: "analysis.communities.louvain",
        run: louvain_communities,
    },
    Entry {
        id: "analysis.centrality.degree",
        run: degree_centrality,
    },
    Entry {
        id: "analysis.centrality.closeness",
        run: closeness_centrality,
    },
    Entry {
        id: "analysis.centrality.betweenness",
        run: betweenness_centrality,
    },
    Entry {
        id: "analysis.centrality.eigenvector",
        run: eigenvector_centrality,
    },
    Entry {
        id: "analysis.depth.bfs",
        run: bfs_depth,
    },
];

/// How many analyses the ABI exposes; `gm_analysis_count`'s body.
pub fn count() -> u32 {
    u32::try_from(ANALYSES.len()).unwrap_or(0)
}

/// The id at index `i`, or `None` past the end — the same shape `gm_layout_id` and
/// `gm_post_id` answer out of range with.
pub fn id_at(i: u32) -> Option<&'static str> {
    ANALYSES.get(i as usize).map(|entry| entry.id)
}

/// The analysis at index `i` run over `topology`, or `None` past the end. The one body
/// `gm_analysis_run` delegates to, so both of its refusals are pinned natively.
pub fn run(i: u32, topology: &Topology) -> Option<Report> {
    ANALYSES.get(i as usize).map(|entry| (entry.run)(topology))
}

/// The JSON face of the analysis at index `i` over `topology`: canonical, ascending-key
/// order, one line, no trailing newline. `None` past the end of the registry.
pub fn to_json(i: u32, topology: &Topology) -> Option<String> {
    let report = run(i, topology)?;
    Some(report.to_json())
}

impl Report {
    /// The canonical JSON text of this report. Keys ascending, so two runs of one
    /// analysis are byte-identical and a diff between two of them names the analysis.
    pub fn to_json(&self) -> String {
        let mut out = String::from("{");
        if let Some(converged) = self.converged {
            let _ = write!(out, "\"converged\":{converged},");
        }
        let _ = write!(out, "\"id\":\"{}\",", self.id);
        let _ = write!(out, "\"kind\":\"{}\",", self.values.kind());
        if let Some(max) = self.max {
            let _ = write!(out, "\"max\":{max},");
        }
        if let Some(modularity) = self.modularity {
            let _ = write!(out, "\"modularity\":{modularity},");
        }
        let _ = write!(out, "\"nodeCount\":{}", self.values.len());
        let _ = write!(out, ",\"values\":");
        list(&mut out, &self.values);
        out.push('}');
        out
    }
}

impl Column {
    /// How many nodes this column holds: one per node in the analysed graph, which is
    /// what `nodeCount` on the face states.
    pub fn len(&self) -> u32 {
        u32::try_from(match self {
            Self::F64(values) => values.len(),
            Self::U32(values) => values.len(),
        })
        .unwrap_or(0)
    }
}

fn list(out: &mut String, column: &Column) {
    out.push('[');
    match column {
        Column::F64(values) => {
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{value}");
            }
        }
        Column::U32(values) => {
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{value}");
            }
        }
    }
    out.push(']');
}

fn weak_components(topology: &Topology) -> Report {
    labelled("analysis.components.weak", components::weak(topology))
}

fn strong_components(topology: &Topology) -> Report {
    labelled("analysis.components.strong", components::strong(topology))
}

fn louvain_communities(topology: &Topology) -> Report {
    let labels = communities::louvain(topology);
    let modularity = communities::modularity(topology, &labels);
    Report {
        id: "analysis.communities.louvain",
        values: Column::U32(labels),
        converged: None,
        modularity: Some(modularity),
        max: None,
    }
}

fn labelled(id: &'static str, labels: Vec<u32>) -> Report {
    Report {
        id,
        values: Column::U32(labels),
        converged: None,
        modularity: None,
        max: None,
    }
}

fn degree_centrality(topology: &Topology) -> Report {
    let values = centrality::degree(topology)
        .iter()
        .map(|&d| f64::from(d))
        .collect();
    plain("analysis.centrality.degree", values)
}

fn closeness_centrality(topology: &Topology) -> Report {
    plain(
        "analysis.centrality.closeness",
        widened(centrality::closeness(topology)),
    )
}

fn betweenness_centrality(topology: &Topology) -> Report {
    plain(
        "analysis.centrality.betweenness",
        widened(centrality::betweenness(topology)),
    )
}

fn eigenvector_centrality(topology: &Topology) -> Report {
    let (values, converged) = centrality::eigenvector(topology);
    Report {
        id: "analysis.centrality.eigenvector",
        values: Column::F64(widened(values)),
        converged: Some(converged),
        modularity: None,
        max: None,
    }
}

/// `f32` as graph-core states a centrality, widened to the `f64` the wire carries. Exact:
/// every `f32` is a `f64`, so this loses nothing and changes no comparison.
fn widened(values: Vec<f32>) -> Vec<f64> {
    values.into_iter().map(f64::from).collect()
}

fn plain(id: &'static str, values: Vec<f64>) -> Report {
    Report {
        id,
        values: Column::F64(values),
        converged: None,
        modularity: None,
        max: None,
    }
}

/// BFS depth over the repaired hierarchy — graph-core's own `Hierarchy`, which
/// graph-core's `analysis::depth` reads directly, so this face and graph-core's own
/// depth column are one convention and not two (see [`depth::Roots`]).
fn bfs_depth(topology: &Topology) -> Report {
    let depth = forest_depth(topology);
    Report {
        id: "analysis.depth.bfs",
        values: Column::U32(depth.levels().to_vec()),
        converged: None,
        modularity: None,
        max: Some(depth.max()),
    }
}

/// The depth of every node of `topology` under the hierarchy convention, including the
/// virtual root's two-plus-roots case, which graph-core's own `Hierarchy` already
/// encodes — so this reads one convention, not two.
///
/// `Hierarchy::of`'s only refusal is `n + 1` not fitting `u32`, i.e. a graph with
/// `u32::MAX` nodes, which `index_model` cannot have produced (its own capacity check
/// runs first). So this is a caller-bug panic, the same discipline `Csr::from_pairs`
/// takes for an out-of-range row, and not a path a real handle reaches.
fn forest_depth(topology: &Topology) -> depth::Depth {
    let hierarchy = Hierarchy::of(topology).expect("n + 1 fits u32 for any indexed topology");
    depth::bfs_depth(&hierarchy)
}
