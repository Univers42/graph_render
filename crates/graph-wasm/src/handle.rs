//! The handle table (`docs/contract/wasm-abi.md` "Handles", C6): an opaque `u32` per
//! built graph, over its [`Topology`] and the last successful run's snapshot. Handles
//! are never reused within one instance — a monotonic counter, refusing once `u32` ids
//! are exhausted rather than wrapping onto a freed id.
//!
//! Target-independent: no wasm pointer ever appears here. Values live behind [`Box`] so
//! a later insert's `BTreeMap` rebalancing cannot move an already-handed-out column's
//! backing memory (C7: "column pointers... valid until the next motor call" — a call on
//! a *different* handle must not count as one).

use graph_contract::binary::Snapshot;
use graph_core::Geometry;
use graph_core::Topology;
use std::collections::BTreeMap;

/// One built graph: its topology (fixed at `gm_build`), and the last successful
/// [`gm_run`](crate::exports::gm_run)'s geometry, cleared on a failed run (C4).
#[derive(Default)]
pub struct Handle {
    /// The ingested, indexed graph. Never replaced after `gm_build`.
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

/// Live handles, keyed by the id `gm_build` returned.
#[derive(Default)]
pub struct Handles {
    next: u32,
    live: BTreeMap<u32, Box<Handle>>,
}

impl Handles {
    /// An empty table; the first handle it issues is `1` (`0` stays the failure value).
    pub fn new() -> Self {
        Self {
            next: 1,
            live: BTreeMap::new(),
        }
    }

    /// Inserts `handle` under a fresh id, or `None` once every `u32` id has been issued.
    pub fn insert(&mut self, handle: Handle) -> Option<u32> {
        if self.next == 0 {
            return None;
        }
        let id = self.next;
        self.next = self.next.checked_add(1).unwrap_or(0);
        self.live.insert(id, Box::new(handle));
        Some(id)
    }

    /// The handle `id` names, if it is still live.
    pub fn get(&self, id: u32) -> Option<&Handle> {
        self.live.get(&id).map(Box::as_ref)
    }

    /// A mutable borrow of the handle `id` names, if it is still live.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Handle> {
        self.live.get_mut(&id).map(Box::as_mut)
    }

    /// Removes `id`, if it was live. The freed id is never handed out again.
    pub fn remove(&mut self, id: u32) -> Option<Box<Handle>> {
        self.live.remove(&id)
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
        let mut handles = Handles {
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
}
