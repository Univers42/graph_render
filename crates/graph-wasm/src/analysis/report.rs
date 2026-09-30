//! The report types for the ANALYSIS stage over the ABI.

use graph_core::Topology;
use std::fmt::Write as _;

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

    /// How many nodes this column holds: one per node in the analysed graph, which is
    /// what `nodeCount` on the face states.
    pub fn len(&self) -> u32 {
        u32::try_from(match self {
            Self::F64(values) => values.len(),
            Self::U32(values) => values.len(),
        })
        .unwrap_or(0)
    }

    /// Whether the analysed graph had no nodes at all — the empty column every analysis
    /// answers, and the one case where `nodeCount` is `0`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One analysis as the ABI names it.
pub struct Entry {
    /// Its capability id, e.g. `analysis.components.weak`.
    pub id: &'static str,
    /// It over `topology`, as graph-core computes it.
    pub run: fn(&Topology) -> Report,
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
