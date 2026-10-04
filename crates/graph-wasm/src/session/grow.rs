//! `gm_force_session_grow`'s body: a live session taken onto the rows and edges
//! `gm_graph_extend` appended to its own graph (`docs/contract/delta.md`). A child of
//! [`super`] so the session table stays private to the one module that owns it.

use super::SESSIONS;
use crate::errors::Code;
use graph_core::Topology;

/// Grows session `id` onto `topology`, the topology of graph handle `graph` as it is now, or
/// `None` when that handle is not live. Refusals, checked in this order and each leaving the
/// session unchanged: [`Code::InvalidSession`] for a session that is not live,
/// [`Code::InvalidHandle`] for a graph that is not, [`Code::SessionRefused`] for a graph other
/// than the one the session was created over, or a growth `ForceSession::grow` refuses.
///
/// The graph check is here because graph-core cannot make it: `ForceSession::grow` accepts any
/// topology with at least its rows and edges, and an unrelated graph of that size would be
/// read as an extension of this one.
pub fn grow(id: u32, graph: u32, topology: Option<&Topology>) -> Result<(), Code> {
    SESSIONS.with(|live| {
        let mut live = live.borrow_mut();
        let entry = live.get_mut(id).ok_or(Code::InvalidSession)?;
        let topology = topology.ok_or(Code::InvalidHandle)?;
        if entry.graph != graph {
            return Err(Code::SessionRefused);
        }
        entry
            .session
            .grow(topology)
            .map_err(|_| Code::SessionRefused)
    })
}
