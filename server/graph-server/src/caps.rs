//! The per-id work caps (Verdict condition 2): after ingest, a graph with more nodes or edges
//! than the cap of its layout, or of its POST pass, is a 413 `IngestTooLarge` before anything
//! runs. One row per registered id, `id<TAB>cap_n<TAB>cap_m`; `0 0` means no size fits.
//!
//! Caveat: the table is a placeholder, not a measurement. svc-caps measures the real caps into
//! `docs/measurements/service-caps.tsv` (same format); until it lands, `caps.placeholder.tsv`
//! holds layouts at `min(scale_ceiling, 1000)` nodes and 4 edges per node, and POST passes at
//! 1000 nodes and `min(scale_ceiling, 4000)` edges. It refuses graphs a slot could run, and it
//! can still admit a graph whose run outlasts `GRAPH_TIMEOUT_MS` on a slow host. Switch the
//! `include_str!` below when the measured file lands.

use crate::breaks;
use crate::error::ApiError;
use std::collections::BTreeMap;

/// The committed table.
const TABLE: &str = include_str!("../caps.placeholder.tsv");
/// The table's first line.
const HEADER: &str = "id\tcap_n\tcap_m";

/// One id's cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cap {
    /// The most nodes admitted.
    pub nodes: u64,
    /// The most edges admitted.
    pub edges: u64,
}

/// Every id's cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caps(BTreeMap<String, Cap>);

impl Caps {
    /// The committed table, parsed.
    pub fn committed() -> Result<Self, String> {
        Self::parse(TABLE)
    }

    /// Parses a table. A malformed row names its line number.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Err("caps table: the first line is not `id\\tcap_n\\tcap_m`".to_owned());
        }
        let mut rows = BTreeMap::new();
        for (index, line) in lines.enumerate() {
            let refuse = |reason| format!("caps table line {}: {reason}", index + 2);
            let fields: Vec<&str> = line.split('\t').collect();
            let [id, nodes, edges] = fields.as_slice() else {
                return Err(refuse("expected three tab-separated fields"));
            };
            let number = |text: &str| text.parse::<u64>().map_err(|_| refuse("not a count"));
            let cap = Cap {
                nodes: number(nodes)?,
                edges: number(edges)?,
            };
            if rows.insert((*id).to_owned(), cap).is_some() {
                return Err(refuse("a second row for one id"));
            }
        }
        Ok(Self(rows))
    }

    /// The cap of `id`, if the table has a row for it.
    pub fn get(&self, id: &str) -> Option<Cap> {
        self.0.get(id).copied()
    }

    /// Every id in the table, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Refuses a graph of `nodes` and `edges` past the cap of `layout` or of `post`. An id with
    /// no row is refused too: a missing row never means "uncapped".
    pub fn admit(
        &self,
        layout: &str,
        post: Option<&str>,
        size: (u32, u32),
    ) -> Result<(), ApiError> {
        if breaks::on("lift-caps") {
            return Ok(());
        }
        let (nodes, edges) = (u64::from(size.0), u64::from(size.1));
        for id in std::iter::once(layout).chain(post) {
            let cap = self.get(id).unwrap_or(Cap { nodes: 0, edges: 0 });
            if nodes > cap.nodes || edges > cap.edges {
                return Err(ApiError::too_large(format!(
                    "{id} is capped at {} nodes and {} edges here; the graph has {nodes} and {edges}",
                    cap.nodes, cap.edges
                )));
            }
        }
        Ok(())
    }
}
