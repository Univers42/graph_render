//! The id half of ingest validation (C12): a duplicate node or edge id, and an edge naming
//! a node id that is not in `nodes`, are refused rather than first-wins or dropped.

use super::IngestError;
use graph_core::{EdgeRecord, NodeRecord, StringArena, Topology, index_model};

/// Duplicate and dangling ids, through one `StringArena` per id space. A `BTreeSet<&str>` here
/// was 2.1 s of a 1M-node open's 15 s worker profile, nearly all of it `memcmp` down the tree.
///
/// Both arenas are sized for the whole input first: the node ids are all distinct or the call
/// fails, so `nodes.len()` id slots and the summed id lengths are the counts a clean pass will
/// reach exactly — no reservation is wasted on a graph this function refuses.
pub(super) fn check_ids(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<(), IngestError> {
    let mut ids = StringArena::with_capacity(nodes.len(), sum_len(nodes.iter().map(|n| &n.id)));
    for n in nodes {
        intern_unique(&mut ids, &n.id, "node")?;
    }
    let mut edge_ids =
        StringArena::with_capacity(edges.len(), sum_len(edges.iter().map(|e| &e.id)));
    for e in edges {
        intern_unique(&mut edge_ids, &e.id, "edge")?;
        for (end, id) in [("source", &e.source), ("target", &e.target)] {
            if ids.find(id).is_none() {
                return Err(IngestError::DanglingEndpoint {
                    edge: e.id.clone(),
                    end,
                    id: id.clone(),
                });
            }
        }
    }
    if u32::try_from(nodes.len()).is_err() || u32::try_from(edges.len()).is_err() {
        return Err(IngestError::Capacity);
    }
    Ok(())
}

/// `index_model` over [`super::read_records`]' output, refusing what [`check_ids`] refuses.
/// `index_model` drops a node only for a taken id and an edge only for a taken id or a
/// missing endpoint, so a topology that kept every record had nothing `check_ids` refuses,
/// and the id pass runs only when something was dropped, to name it. The happy path probes
/// each id once instead of twice: on a 1M-node open the id pass alone was 720 ms of wasm.
pub fn index(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<Topology, IngestError> {
    let topology = index_model(nodes, edges).map_err(|_| IngestError::Capacity)?;
    let kept_all = topology.node_count() as usize == nodes.len()
        && topology.edge_count() as usize == edges.len();
    if !kept_all {
        check_ids(nodes, edges)?;
    }
    Ok(topology)
}

/// The total length of the strings an iterator yields, for an arena reservation.
fn sum_len<'a>(values: impl Iterator<Item = &'a String>) -> usize {
    values.map(|v| v.len()).sum()
}

/// Stores `id`, refusing it when `arena` already held it (`intern` grows `len` only on a new string).
fn intern_unique(arena: &mut StringArena, id: &str, what: &'static str) -> Result<(), IngestError> {
    let before = arena.len();
    arena.intern(id).map_err(|_| IngestError::Capacity)?;
    if arena.len() == before {
        return Err(IngestError::DuplicateId {
            what,
            id: id.to_owned(),
        });
    }
    Ok(())
}
