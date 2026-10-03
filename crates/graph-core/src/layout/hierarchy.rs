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
//! O(n + m) apart from two **stable comparison sorts**: [`notes`](Self::notes) is sorted
//! once (`Hierarchy::of`, at most `2m` entries) and the cycle cuts once (`break_cycles`, at
//! most one per disjoint cycle, so at most `n`), so the real bound is O(n + m log m). The
//! rest is linear: two passes over the CSR, one pointer walk touching each node once, one
//! breadth-first pass. Neither sort is a counting sort's worth of keys — note codes are a
//! small closed set but the `index` half is a dense id, so a counting pass would need a
//! second key anyway. Exact and deterministic throughout.

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
    ///
    /// # Precondition
    ///
    /// `v <= self.node_count()`, i.e. `v` is a real node or the virtual root — the same
    /// bound [`parent`](Self::parent) and [`parent_edge`](Self::parent_edge) accept as a
    /// `None`-returning `Option` without needing one. Those two return `Option` because
    /// "this node is a root" is a real, common answer; "is this row in range" is not, and
    /// every caller here reads `v` from a CSR row or an `order()` walk. Out of range
    /// panics via the CSR's own row bound rather than returning an empty slice that would
    /// read as "this node is a leaf" and silently swallow a caller's subtree.
    pub fn children(&self, v: u32) -> &[u32] {
        self.children.row(v)
    }

    /// Node `v`'s depth below [`root`](Self::root); `v` may be the virtual root `n`.
    ///
    /// **The one-deep offset is deliberate, not a shift to undo.** Depth is counted *below*
    /// `root`, so a single-rooted tree's root is at depth 0 and, under a virtual root, the
    /// real roots sit at depth 1 ([`virtual_root`](Self::virtual_root) is `Some`). A caller
    /// that wants the first real level at 0 subtracts 1 exactly when
    /// [`virtual_root`](Self::virtual_root)` is `Some`. Both of this tree's depth consumers
    /// are checked against that offset and neither disagrees with its reference:
    ///
    /// - `layout/circular.rs:72` indexes a `rings` column by it directly, so a forest leaves
    ///   ring 0 empty — the SciGraphs convention as well: conformance row 32
    ///   (`layout.circular.hierarchy`) is green and `f32`-identical on all 1020 coordinates,
    ///   forest fixtures included, so the offset is the oracle's own.
    /// - `tidy_tree`'s `normalize` uses it as `y = depth * (1 / max(depth(bottom), 1))`, a
    ///   ratio in which a common shift cancels; the `max(1)` is what keeps a one-deep
    ///   virtual-rooted forest off a division by zero.
    ///
    /// `max_depth` ([`Self::max_depth`]) inherits the offset for the same reason: it reads
    /// the last element of [`order`](Self::order), so it is a *depth*, not a level count.
    ///
    /// # Precondition
    ///
    /// `v <= self.node_count()`, the same bound and the same reasoning as
    /// [`children`](Self::children). A node reachable from [`root`](Self::root) always
    /// has its depth written by `breadth_first`, and a virtual-rooted `v` is `n`, the
    /// last row; anything else is a caller bug and panics rather than reading `0` out of
    /// an unrelated node's slot, which would place the node at ring 0 without any error.
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
