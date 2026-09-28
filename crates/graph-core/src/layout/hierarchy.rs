//! The shared hierarchy substrate (user decision D-H, `docs/decisions/hierarchy-repair.md`):
//! the one tree every hierarchy layout lays out, repaired from the topology's hierarchy
//! CSR, with every repair recorded as a snapshot note (`graph_contract::notes`).
//!
//! - **Parent candidates** are the hierarchy CSR's edges, read through
//!   [`Topology::parent`] and [`Topology::child`], so a `child_of` edge is already
//!   flipped (D-Q1). A self-loop is never a parent and never a note.
//! - **One parent each**: a node keeps the parent of its hierarchy edge with the
//!   **lowest edge index** — topology edge order, not the order the CSR rows are walked
//!   in. Every other parent edge, a repeated parallel one included, is dropped as note
//!   code 2, `hierarchy.extra_parent_dropped`.
//! - **Cycles**: with one parent each, a cycle is a parent-pointer cycle. Each is broken
//!   at its lowest dense index, which becomes a root; its parent edge is dropped as note
//!   code 1, `hierarchy.cycle_edge_dropped`. Cycles are disjoint and are handled in
//!   ascending order of that node.
//! - **Roots**: every node with no kept parent — an isolated node too — ascending.
//!   Children: ascending dense index, the order `d3.stratify` gives.
//! - **The root**: a single root is the tree's root. Two or more hang, ascending, off one
//!   hidden **virtual root** at dense index `n` — no id, no geometry, never emitted —
//!   and every layout lays out that virtual-rooted tree. No node, no root.
//! - **Depth**: breadth first from the root, so under a virtual root the real roots sit
//!   at depth 1.
//!
//! O(n + m): two passes over the CSR, one pointer walk touching each node once, one
//! counting sort, one breadth-first pass. Exact and deterministic throughout.

use crate::arena::CapacityError;
use crate::csr::Csr;
use crate::index::Topology;
use graph_contract::notes::{Note, NoteCode};

/// The repaired hierarchy of one topology. Build it with [`Hierarchy::of`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hierarchy {
    parent_edge: Vec<Option<u32>>,
    parent: Vec<Option<u32>>,
    roots: Vec<u32>,
    root: Option<u32>,
    children: Csr,
    depth: Vec<u32>,
    order: Vec<u32>,
    notes: Vec<Note>,
}

impl Hierarchy {
    /// Repairs `topology`'s hierarchy. Refused only when `n + 1` rows (the virtual root's
    /// included) do not fit the `u32` index space.
    pub fn of(topology: &Topology) -> Result<Self, CapacityError> {
        let n = topology.node_count();
        let rows = n
            .checked_add(1)
            .ok_or(CapacityError { what: "hierarchy" })?;
        let mut notes = Vec::new();
        let mut parent_edge = keep_lowest_parent_edges(topology, &mut notes);
        break_cycles(topology, &mut parent_edge, &mut notes);
        notes.sort();
        let parent: Vec<_> = parent_edge
            .iter()
            .map(|e| e.map(|e| topology.parent(e)))
            .collect();
        let roots: Vec<u32> = (0..n).filter(|&v| parent[v as usize].is_none()).collect();
        let root = if roots.len() >= 2 {
            Some(n)
        } else {
            roots.first().copied()
        };
        let children = children(&parent, &roots, root.filter(|&r| r == n), rows)?;
        let (order, depth) = breadth_first(&children, root, rows);
        Ok(Self {
            parent_edge,
            parent,
            roots,
            root,
            children,
            depth,
            order,
            notes,
        })
    }

    /// Real nodes, `n`.
    pub fn node_count(&self) -> u32 {
        self.parent.len() as u32
    }

    /// Every node with no kept parent, ascending.
    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// The root of the tree the layouts lay out: the single real root, or the virtual
    /// root `n` when there are two or more; `None` when there is no node.
    pub fn root(&self) -> Option<u32> {
        self.root
    }

    /// The virtual root, `n`, when there are two or more real roots.
    pub fn virtual_root(&self) -> Option<u32> {
        (self.roots.len() >= 2).then_some(self.node_count())
    }

    /// Node `v`'s kept parent; `None` for a root (and for the virtual root).
    pub fn parent(&self, v: u32) -> Option<u32> {
        self.parent.get(v as usize).copied().flatten()
    }

    /// The hierarchy edge that makes `v` a child: the kept one, by position.
    pub fn parent_edge(&self, v: u32) -> Option<u32> {
        self.parent_edge.get(v as usize).copied().flatten()
    }

    /// Node `v`'s children, ascending; `v` may be the virtual root `n`.
    pub fn children(&self, v: u32) -> &[u32] {
        self.children.row(v)
    }

    /// Node `v`'s depth below [`root`](Self::root); `v` may be the virtual root `n`.
    pub fn depth(&self, v: u32) -> u32 {
        self.depth[v as usize]
    }

    /// The deepest depth; 0 when there is no node.
    pub fn max_depth(&self) -> u32 {
        self.order.last().map_or(0, |&v| self.depth(v))
    }

    /// Every node of the laid-out tree, breadth first from [`root`](Self::root), children
    /// ascending: the virtual root first when there is one.
    pub fn order(&self) -> &[u32] {
        &self.order
    }

    /// What the repair dropped, ascending by `(code, index)`.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }
}

/// Every hierarchy edge from the CSR that is not a self-loop.
fn candidates(t: &Topology) -> impl Iterator<Item = u32> + '_ {
    (0..t.node_count())
        .flat_map(|p| t.hierarchy().row(p).iter().copied())
        .filter(|&e| t.parent(e) != t.child(e))
}

/// Each node's lowest-indexed parent edge; every other one noted as code 2.
fn keep_lowest_parent_edges(t: &Topology, notes: &mut Vec<Note>) -> Vec<Option<u32>> {
    let mut kept: Vec<Option<u32>> = vec![None; t.node_count() as usize];
    for e in candidates(t) {
        let slot = &mut kept[t.child(e) as usize];
        *slot = Some(slot.map_or(e, |k| k.min(e)));
    }
    let extra = candidates(t).filter(|&e| kept[t.child(e) as usize] != Some(e));
    notes.extend(extra.map(|index| Note {
        code: NoteCode::ExtraParentDropped,
        index,
    }));
    kept
}

/// Breaks every parent-pointer cycle at its lowest node, noting the edge as code 1.
fn break_cycles(t: &Topology, parent_edge: &mut [Option<u32>], notes: &mut Vec<Note>) {
    let parent = |edges: &[Option<u32>], v: u32| edges[v as usize].map(|e| t.parent(e));
    let mut walked = vec![0u32; parent_edge.len()];
    let mut cuts = Vec::new();
    for start in 0..t.node_count() {
        let found = walk(start, &mut walked, |v| parent(parent_edge, v));
        if let Some(on) = found {
            cuts.push(lowest_on_cycle(on, |v| parent(parent_edge, v)));
        }
    }
    cuts.sort();
    for cut in cuts {
        if let Some(index) = parent_edge[cut as usize].take() {
            let code = NoteCode::CycleEdgeDropped;
            notes.push(Note { code, index });
        }
    }
}

/// Follows parents from `start` through nodes no walk has seen, stamping them; a node on
/// a cycle when the walk meets its own stamp, `None` when it reaches a root or an
/// earlier walk.
fn walk(start: u32, walked: &mut [u32], parent: impl Fn(u32) -> Option<u32>) -> Option<u32> {
    let stamp = start + 1;
    let mut v = start;
    while walked[v as usize] == 0 {
        walked[v as usize] = stamp;
        v = parent(v)?;
    }
    (walked[v as usize] == stamp).then_some(v)
}

/// The lowest node on the cycle through `on`.
fn lowest_on_cycle(on: u32, parent: impl Fn(u32) -> Option<u32>) -> u32 {
    let (mut v, mut low) = (on, on);
    while let Some(p) = parent(v).filter(|&p| p != on) {
        low = low.min(p);
        v = p;
    }
    low
}

/// The children CSR: `rows` rows, a kept parent's children then, under a virtual root,
/// the real roots — both in ascending dense index, the order the pairs arrive in.
fn children(
    parent: &[Option<u32>],
    roots: &[u32],
    virtual_root: Option<u32>,
    rows: u32,
) -> Result<Csr, CapacityError> {
    let kept = (0..).zip(parent).filter_map(|(v, p)| p.map(|p| (p, v)));
    let hung = virtual_root
        .into_iter()
        .flat_map(|r| roots.iter().map(move |&c| (r, c)));
    Csr::from_pairs(rows, kept.chain(hung))
}

/// Breadth-first order from `root` and every node's depth below it.
fn breadth_first(children: &Csr, root: Option<u32>, rows: u32) -> (Vec<u32>, Vec<u32>) {
    let mut depth = vec![0u32; rows as usize];
    let mut order = Vec::with_capacity(rows as usize);
    order.extend(root);
    let mut at = 0;
    while let Some(&v) = order.get(at) {
        for &c in children.row(v) {
            depth[c as usize] = depth[v as usize] + 1;
            order.push(c);
        }
        at += 1;
    }
    (order, depth)
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
