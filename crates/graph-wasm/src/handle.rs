//! The handle table (`docs/contract/wasm-abi.md` "Handles", C6): an opaque `u32` per
//! built graph, over its [`Topology`] and the last successful run's snapshot. Handles
//! are never reused within one instance — a monotonic counter, refusing once `u32` ids
//! are exhausted rather than wrapping onto a freed id.
//!
//! Target-independent: no wasm pointer ever appears here. Values live behind [`Box`] so
//! a later insert's `BTreeMap` rebalancing cannot move an already-handed-out column's
//! backing memory (C7: "column pointers... valid until the next motor call" — a call on
//! a *different* handle must not count as one).
//!
//! **One table, two kinds of value.** [`Table`] is generic over what it holds because the
//! force session needs exactly this and nothing else — the same monotonic never-reused id,
//! the same `Box` so an insert cannot move an already-handed-out pointer, the same
//! `0`-is-not-an-id rule — and a second table with those four properties copied into it
//! would be four properties free to drift. [`Handles`] is this module's own table over
//! [`Handle`], and [`crate::session`] holds the same table over a `ForceSession` and the
//! graph it was created over.

use crate::errors::Code;
use graph_contract::binary::Snapshot;
use graph_core::Geometry;
use graph_core::Topology;
use std::collections::BTreeMap;

/// One built graph: its topology (built by `gm_build`, appended to by each
/// `gm_graph_extend`), and the last successful [`gm_run`](crate::exports::gm_run)'s
/// geometry, cleared on a failed run and on an appended batch (C4).
#[derive(Default)]
pub struct Handle {
    /// The ingested, indexed graph. Never replaced after `gm_build`; `gm_graph_extend`
    /// appends to it in place, so every row and edge index it had keeps its meaning.
    pub topology: Topology,
    /// The last run's geometry, or `None` before the first successful run, or right
    /// after a run that failed.
    pub snapshot: Option<Snapshot>,
    /// The **layout's** geometry — the node positions and the notes, with the layout's
    /// own edges — kept beside the snapshot so a POST pass has the layout's output to
    /// read.
    ///
    /// Distinct from `snapshot` on purpose, and this is the reason a post pass cannot
    /// be driven off the snapshot: the snapshot's edge columns are whatever the last
    /// POST pass wrote, so a second pass over it would compose two passes' output rather
    /// than the layout's, and a caller running style-then-bundle and then bundle again
    /// would get a different answer from a caller running bundle once. `snapshot` is the
    /// transport face (what the columns read) and this is the input face (what a stage
    /// downstream of LAYOUT reads); both are replaced together, both cleared together.
    pub geometry: Option<Geometry>,
}

/// Live handles, keyed by the id `gm_build` returned: this module's own value.
pub type Handles = Table<Handle>;

/// Opaque ids over live values of one kind: `gm_build`'s [`Handles`], and
/// `gm_force_session_create`'s table over a `ForceSession`.
///
/// `T` is behind a [`Box`] for the reason the module doc gives, so the two tables cannot
/// differ on it: a `BTreeMap` insert rebalances, and a rebalance that moved a value whose
/// columns' address was already handed out would invalidate an address the wire promised.
#[derive(Default)]
pub struct Table<T> {
    next: u32,
    live: BTreeMap<u32, Box<T>>,
}

impl<T> Table<T> {
    /// An empty table; the first id it issues is `1` (`0` stays the failure value).
    ///
    /// `const` because a `thread_local!` holding one of these wants `const { .. }` — the
    /// tables are process-wide, and a lazily-initialised one is a thread-local initialisation
    /// that can panic.
    pub const fn new() -> Self {
        Self {
            next: 1,
            live: BTreeMap::new(),
        }
    }

    /// Inserts `value` under a fresh id, or `None` once every `u32` id has been issued.
    pub fn insert(&mut self, value: T) -> Option<u32> {
        if self.next == 0 {
            return None;
        }
        let id = self.next;
        self.next = self.next.checked_add(1).unwrap_or(0);
        self.live.insert(id, Box::new(value));
        Some(id)
    }

    /// The value `id` names, if it is still live.
    pub fn get(&self, id: u32) -> Option<&T> {
        self.live.get(&id).map(Box::as_ref)
    }

    /// A mutable borrow of the value `id` names, if it is still live.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        self.live.get_mut(&id).map(Box::as_mut)
    }

    /// Removes `id`, if it was live. The freed id is never handed out again.
    pub fn remove(&mut self, id: u32) -> Option<Box<T>> {
        self.live.remove(&id)
    }
}

impl Handles {
    /// The last run's snapshot for `id`: `InvalidHandle` if `id` is not live (never issued,
    /// or released), `NoGeometryYet` if no run has succeeded on it since the last failure.
    pub fn snapshot(&self, id: u32) -> Result<&Snapshot, Code> {
        let entry = self.get(id).ok_or(Code::InvalidHandle)?;
        entry.snapshot.as_ref().ok_or(Code::NoGeometryYet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::index_model;

    fn handle() -> Handle {
        Handle {
            topology: index_model(&[], &[]).expect("empty fits"),
            snapshot: None,
            geometry: None,
        }
    }

    #[test]
    fn ids_are_issued_from_1_and_never_reused() {
        let mut handles = Handles::new();
        let a = handles.insert(handle()).expect("first id");
        let b = handles.insert(handle()).expect("second id");
        assert_eq!((a, b), (1, 2));
        assert_eq!(
            handles.get(a).expect("live").topology.node_count(),
            0,
            "the empty topology"
        );
        handles.remove(a);
        let c = handles.insert(handle()).expect("third id");
        assert_eq!(c, 3, "the id gm_release freed is never reissued");
        assert!(handles.get(a).is_none(), "the old handle stays refused");
        assert!(handles.get(c).is_some());
    }

    #[test]
    fn exhaustion_refuses_rather_than_wraps() {
        let mut handles = Table::<Handle> {
            next: u32::MAX,
            live: BTreeMap::new(),
        };
        let last = handles.insert(handle()).expect("the final id");
        assert_eq!(last, u32::MAX);
        assert_eq!(
            handles.insert(handle()),
            None,
            "no id is left; must not wrap to a freed or live one"
        );
    }

    #[test]
    fn get_mut_reaches_the_same_handle_get_does() {
        let mut handles = Handles::new();
        let id = handles.insert(handle()).expect("id");
        handles.get_mut(id).expect("live").snapshot = None;
        assert!(handles.get(id).is_some());
        assert!(handles.get(id + 1).is_none(), "an unissued id is not live");
    }

    /// A handle holds both faces of a run, and both are `None` before any run: a POST
    /// pass reads `geometry`, the column views read `snapshot`, and a handle that had one
    /// without the other would let a post pass draw a layout's output the transport
    /// cannot show, or serve columns for geometry no stage produced.
    #[test]
    fn a_fresh_handle_has_neither_face_of_a_run() {
        let handle = handle();
        assert!(handle.snapshot.is_none());
        assert!(handle.geometry.is_none());
    }

    /// The read path every column and face export takes. A released handle is refused by
    /// name, and so is a live one whose last run failed: neither is served the snapshot it
    /// used to have, so a column read after `gm_release` or a failed `gm_run` is a refusal.
    #[test]
    fn a_released_or_unrun_handle_is_refused_by_name_never_served_stale() {
        let mut handles = Handles::new();
        let mut ran = handle();
        let geometry = (graph_core::registry::LAYOUTS[0].run)(&ran.topology).expect("grid");
        let snapshot = graph_core::layout::snapshot(&ran.topology, geometry).expect("fits");
        ran.snapshot = Some(snapshot);
        let id = handles.insert(ran).expect("id");
        assert!(handles.snapshot(id).is_ok());
        handles.get_mut(id).expect("live").snapshot = None;
        assert_eq!(handles.snapshot(id).err(), Some(Code::NoGeometryYet));
        handles.remove(id);
        assert_eq!(handles.snapshot(id).err(), Some(Code::InvalidHandle));
    }
}
