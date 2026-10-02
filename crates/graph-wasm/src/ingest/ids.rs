//! The id half of ingest validation (C12): a duplicate node or edge id, and an edge naming
//! a node id that is not in `nodes`, are refused rather than first-wins or dropped.

use super::IngestError;
use graph_core::{EdgeRecord, NodeRecord, StringArena};

/// Duplicate and dangling ids, through one `StringArena` per id space. A `BTreeSet<&str>` here
/// was 2.1 s of a 1M-node open's 15 s worker profile, nearly all of it `memcmp` down the tree.
pub(super) fn check_ids(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<(), IngestError> {
    let mut ids = StringArena::default();
    for n in nodes {
        intern_unique(&mut ids, &n.id, "node")?;
    }
    let mut edge_ids = StringArena::default();
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
