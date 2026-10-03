//! `Topology::extend`: one strict batch appended in O(batch), so a graph grown batch by
//! batch is the graph [`index_model`](super::index_model) builds from every record at once
//! (`docs/contract/delta.md`).
//!
//! The whole batch is validated before the first write. Interning writes to the arena and
//! claiming writes to the id tables, so a refusal found halfway would leave a half state.

use super::Topology;
use crate::arena::{CapacityError, FixedState};
use crate::csr::AppendCsr;
use crate::edgekind::EdgeKind;
use crate::records::{EdgeRecord, NodeRecord};
use core::fmt;
use indexmap::IndexSet;

/// Why [`Topology::extend`] refused a batch. The topology is unchanged on every variant.
///
/// `delta.md`'s fifth refusal, a batch longer than `MAX_INGEST_BYTES`, has no variant: it
/// is a limit on the wire buffer, and the motor only ever sees decoded records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendError {
    /// Batch node `index`'s id is already a node of the graph or of an earlier batch row.
    NodeId {
        /// The node's position in the batch.
        index: u32,
    },
    /// Batch edge `index`'s id is already an edge of the graph or of an earlier batch row.
    EdgeId {
        /// The edge's position in the batch.
        index: u32,
    },
    /// Batch edge `index` has an endpoint that names no node of the graph or the batch.
    Endpoint {
        /// The edge's position in the batch.
        index: u32,
    },
    /// The batch could overflow `what`, counting every string in it as new.
    Capacity {
        /// The arena, the node index or the adjacency.
        what: &'static str,
    },
}

impl fmt::Display for ExtendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeId { index } => write!(f, "batch node {index}: id already taken"),
            Self::EdgeId { index } => write!(f, "batch edge {index}: id already taken"),
            Self::Endpoint { index } => write!(f, "batch edge {index}: endpoint names no node"),
            Self::Capacity { what } => write!(f, "batch could overflow the {what}"),
        }
    }
}

impl From<CapacityError> for ExtendError {
    fn from(err: CapacityError) -> Self {
        Self::Capacity { what: err.what }
    }
}

/// The most edges the three append CSRs take with no append able to fail: each holds at
/// most one value per edge, so it is [`AppendCsr::SAFE_LIVE`].
///
/// **Caveat:** conservative by 4x. A topology that `index_model` built with more than
/// about 1.07e9 edges refuses every batch carrying an edge; `MAX_INGEST_BYTES` keeps a
/// graph that large off the wire, so only a direct caller of the motor can meet it.
const ADJACENCY_LIMIT: u64 = AppendCsr::SAFE_LIVE;

/// What a topology holds, or what a batch would add, in the units its limits count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Load {
    pub(super) strings: u64,
    pub(super) bytes: u64,
    pub(super) nodes: u64,
    pub(super) edges: u64,
}

impl Load {
    /// What `t` holds now.
    fn of(t: &Topology) -> Self {
        Self {
            strings: t.strings.len() as u64,
            bytes: t.strings.byte_len() as u64,
            nodes: u64::from(t.node_count()),
            edges: u64::from(t.edge_count()),
        }
    }

    /// What the batch would add if every string in it were new.
    ///
    /// Caveat: a batch that repeats strings the arena already holds is counted in full, so
    /// near the `u32` arena limit it is refused although interning would fit it. The escape
    /// hatch is a smaller batch.
    fn of_batch(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Self {
        let mut load = Self {
            nodes: nodes.len() as u64,
            edges: edges.len() as u64,
            ..Self::default()
        };
        for n in nodes {
            let optional = [&n.database_id, &n.group, &n.icon].map(Option::as_ref);
            for text in [Some(&n.id), Some(&n.source), Some(&n.label)]
                .into_iter()
                .chain(optional)
            {
                load.count(text);
            }
        }
        for e in edges {
            for text in [Some(&e.id), Some(&e.label), e.record_id.as_ref()] {
                load.count(text);
            }
        }
        load
    }

    fn count(&mut self, text: Option<&String>) {
        if let Some(text) = text {
            self.strings += 1;
            self.bytes += text.len() as u64;
        }
    }

    /// `Ok` when `self` plus `batch` fits every limit the append path checks.
    pub(super) fn check(self, batch: Self) -> Result<(), ExtendError> {
        let limit = u64::from(u32::MAX);
        let refuse = |what| Err(ExtendError::Capacity { what });
        if self.strings + batch.strings > limit || self.bytes + batch.bytes > limit {
            return refuse("string arena");
        }
        if self.nodes + batch.nodes > limit {
            return refuse("node index");
        }
        if batch.edges > 0 && self.edges + batch.edges > ADJACENCY_LIMIT {
            return refuse("adjacency");
        }
        Ok(())
    }
}

impl Topology {
    /// Appends one strict batch, nodes first, then edges in order: what `index_model`
    /// would build from every record so far. Validates everything, then mutates; on `Err`
    /// `self` is unchanged. An empty batch is `Ok` and changes nothing.
    pub fn extend(
        &mut self,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), ExtendError> {
        Load::of(self).check(Load::of_batch(nodes, edges))?;
        let batch = self.check_nodes(nodes)?;
        self.check_edges(edges, &batch)?;
        self.append_nodes(nodes)?;
        self.append_edges(edges)
    }

    /// The batch's node ids, refused on the first one the graph or the batch already has.
    fn check_nodes<'a>(
        &self,
        nodes: &'a [NodeRecord],
    ) -> Result<IndexSet<&'a str, FixedState>, ExtendError> {
        let mut batch = IndexSet::with_capacity_and_hasher(nodes.len(), FixedState::default());
        for (index, node) in (0..).zip(nodes) {
            if self.node_index(&node.id).is_some() || !batch.insert(node.id.as_str()) {
                return Err(ExtendError::NodeId { index });
            }
        }
        Ok(batch)
    }

    /// Refuses the first edge whose id is taken or whose endpoint names no node.
    fn check_edges(
        &self,
        edges: &[EdgeRecord],
        batch: &IndexSet<&str, FixedState>,
    ) -> Result<(), ExtendError> {
        let mut ids = IndexSet::with_capacity_and_hasher(edges.len(), FixedState::default());
        let known = |id: &str| self.node_index(id).is_some() || batch.contains(id);
        for (index, edge) in (0..).zip(edges) {
            if self.edge_index(&edge.id).is_some() || !ids.insert(edge.id.as_str()) {
                return Err(ExtendError::EdgeId { index });
            }
            if !known(&edge.source) || !known(&edge.target) {
                return Err(ExtendError::Endpoint { index });
            }
        }
        Ok(())
    }

    /// Admits each node through `index_model`'s own path, gives it an empty row in each
    /// CSR and a zero degree, then groups it. The checks above leave nothing to refuse.
    fn append_nodes(&mut self, nodes: &[NodeRecord]) -> Result<(), ExtendError> {
        for node in nodes {
            let kept = self.admit_node(&node.view())?;
            debug_assert!(kept, "validated: a batch node id is new");
            for csr in [&mut self.out, &mut self.inbound, &mut self.hierarchy] {
                csr.push_row()?;
            }
            self.nodes.degree.push(0);
            self.group_node(self.node_count() - 1);
        }
        Ok(())
    }

    /// Admits each edge through `index_model`'s own path and files it in the CSRs.
    fn append_edges(&mut self, edges: &[EdgeRecord]) -> Result<(), ExtendError> {
        for (index, edge) in (0..).zip(edges) {
            let at = self.endpoints(&edge.source, &edge.target);
            let at = at.ok_or(ExtendError::Endpoint { index })?;
            let e = self.edge_count();
            let kept = self.admit_edge(at, &edge.view().fields())?;
            debug_assert!(kept, "validated: a batch edge id is new");
            self.file_edge(e)?;
        }
        Ok(())
    }

    /// Edge `e` in the out, in and hierarchy rows `build_adjacency` would give it, after
    /// the values already there, and one more degree on each endpoint (two on a loop).
    fn file_edge(&mut self, e: u32) -> Result<(), CapacityError> {
        let at = e as usize;
        let (source, target) = (self.edges.source[at], self.edges.target[at]);
        self.out.append(source, e)?;
        self.inbound.append(target, e)?;
        if self.edges.kind[at] == EdgeKind::Hierarchy {
            self.hierarchy.append(self.parent(e), e)?;
        }
        self.nodes.degree[source as usize] += 1;
        self.nodes.degree[target as usize] += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
