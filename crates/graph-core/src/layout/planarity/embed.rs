//! Turns a planar orientation into the finished [`Embedding`] (`LRPlanarity`'s two
//! embedding passes): first every node's DG-out slots become its initial rotation, in
//! the order [`lr`](super::lr) already sorted them by nesting depth; then, root by root,
//! `dfs_embedding` splices every remaining slot (a node's DG-in edges) into its
//! neighbour's rotation — first for a tree edge, next to a remembered reference for a
//! back edge. Iterative, same reason as the LR test itself.
//!
//! The splice works on **slots**, not positions: a slot's identity never moves, only the
//! `cw`/`ccw` pointers around it, so "insert next to node `x` in row `v`" is always
//! `adjacency.slot(v, x)` followed by one `O(1)` link fix-up.

use super::Embedding;
use super::adjacency::Adjacency;
use super::lr::Sides;

/// Builds the finished embedding from `adjacency` and the LR test's [`Sides`].
pub(super) fn build(adjacency: &Adjacency, sides: &Sides) -> Embedding {
    let mut builder = Builder::new(adjacency, sides);
    for root in sides.roots.clone() {
        builder.embed_from(root);
    }
    builder.into_embedding()
}

/// Every table `dfs_embedding` touches, past the read-only `adjacency`/`sides` it walks:
/// the rotation under construction (`cw`/`ccw`, one entry per slot), per node its resume
/// index and its left/right reference for a later back edge, and `leftmost[v]` — the
/// slot a new tree-child insertion at `v` must land next to.
///
/// `leftmost` matters because a node's "first" neighbour keeps changing as rows are
/// built: after [`Self::new`]'s phase-1 chain, it is that row's *first* DG-out slot (a
/// `ccw`-referenced insertion always keeps the node it displaces as the reference, so
/// chaining left-to-right never moves it — `PlanarEmbedding.add_half_edge`'s
/// `move_leftmost_nbr_to_end`); every [`Self::insert_first`] then hands leftmost status
/// to the node it just inserted (it always splices next to the *current* leftmost, so
/// that never moves anything else); and a side-`-1` back edge in [`Self::insert_back_edge`]
/// takes it over only when it happens to splice next to the current leftmost too.
struct Builder<'a> {
    adjacency: &'a Adjacency,
    sides: &'a Sides,
    cw: Vec<u32>,
    ccw: Vec<u32>,
    left_ref: Vec<Option<u32>>,
    right_ref: Vec<Option<u32>>,
    leftmost: Vec<Option<u32>>,
    ind: Vec<u32>,
}

impl<'a> Builder<'a> {
    /// Phase 1: each row's DG-out slots, already sorted by (signed) nesting depth, form
    /// a complete circular list among themselves — the in-slots join in phase 2. Leftmost
    /// starts at each row's *first* DG-out slot (see the [`Builder`] doc), `None` for a
    /// row with none yet (phase 2's first insertion there starts it fresh).
    fn new(adjacency: &'a Adjacency, sides: &'a Sides) -> Self {
        let total = adjacency.total_slots() as usize;
        let n = adjacency.node_count() as usize;
        let mut builder = Self {
            adjacency,
            sides,
            cw: vec![0; total],
            ccw: vec![0; total],
            left_ref: vec![None; n],
            right_ref: vec![None; n],
            leftmost: sides
                .ordered
                .iter()
                .map(|row| row.first().copied())
                .collect(),
            ind: vec![0; n],
        };
        for row in &sides.ordered {
            builder.link_cycle(row);
        }
        builder
    }

    fn link_cycle(&mut self, row: &[u32]) {
        let k = row.len();
        for (i, &slot) in row.iter().enumerate() {
            self.cw[slot as usize] = row[(i + 1) % k];
            self.ccw[slot as usize] = row[(i + k - 1) % k];
        }
    }

    fn splice_after(&mut self, anchor: u32, new: u32) {
        let next = self.cw[anchor as usize];
        self.cw[anchor as usize] = new;
        self.ccw[new as usize] = anchor;
        self.cw[new as usize] = next;
        self.ccw[next as usize] = new;
    }

    fn splice_before(&mut self, anchor: u32, new: u32) {
        let prev = self.ccw[anchor as usize];
        self.ccw[anchor as usize] = new;
        self.cw[new as usize] = anchor;
        self.ccw[new as usize] = prev;
        self.cw[prev as usize] = new;
    }

    /// `dfs_embedding(v)`, iterative.
    fn embed_from(&mut self, start: u32) {
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            let row = self.sides.ordered[v as usize].clone();
            let mut i = self.ind[v as usize] as usize;
            while i < row.len() {
                let ei = row[i];
                i += 1;
                self.ind[v as usize] = i as u32;
                let w = self.adjacency.slot_neighbour(ei);
                if self.sides.parent_edge[w as usize] == Some(ei) {
                    self.insert_first(w, v);
                    self.left_ref[v as usize] = Some(w);
                    self.right_ref[v as usize] = Some(w);
                    stack.push(v);
                    stack.push(w);
                    break;
                }
                self.insert_back_edge(ei, w, v);
            }
        }
    }

    /// Inserts `v` as `w`'s leftmost neighbour (`add_half_edge_first`): immediately
    /// before `w`'s current leftmost slot, or alone if `w` has none yet — and always
    /// takes over as the new leftmost itself (see the [`Builder`] doc).
    fn insert_first(&mut self, w: u32, v: u32) {
        let new_slot = self.adjacency.slot(w, v);
        match self.leftmost[w as usize] {
            None => {
                self.cw[new_slot as usize] = new_slot;
                self.ccw[new_slot as usize] = new_slot;
            }
            Some(anchor) => self.splice_before(anchor, new_slot),
        }
        self.leftmost[w as usize] = Some(new_slot);
    }

    /// Inserts back edge `(v, w)`, `v` a descendant of ancestor `w`, next to `w`'s
    /// remembered reference on the side `sides.side[ei]` picked. A right-side insertion
    /// (`ccw=`) always keeps `w`'s current leftmost as is; a left-side one (`cw=`) hands
    /// leftmost over only when it was splicing right next to it (see the [`Builder`]
    /// doc).
    fn insert_back_edge(&mut self, ei: u32, w: u32, v: u32) {
        let new_slot = self.adjacency.slot(w, v);
        if self.sides.side[ei as usize] == 1 {
            let anchor = self.right_ref[w as usize]
                .expect("an ancestor with a return edge has a right reference");
            self.splice_after(self.adjacency.slot(w, anchor), new_slot);
        } else {
            let anchor = self.left_ref[w as usize]
                .expect("an ancestor with a return edge has a left reference");
            let anchor_slot = self.adjacency.slot(w, anchor);
            let takes_leftmost = self.leftmost[w as usize] == Some(anchor_slot);
            self.splice_before(anchor_slot, new_slot);
            if takes_leftmost {
                self.leftmost[w as usize] = Some(new_slot);
            }
            self.left_ref[w as usize] = Some(v);
        }
    }

    /// Reads every row's finished clockwise cycle out into CSR form, starting each row
    /// from its lowest-numbered slot — an arbitrary but deterministic choice.
    fn into_embedding(self) -> Embedding {
        let n = self.adjacency.node_count();
        let mut offsets = vec![0u32; n as usize + 1];
        let mut neighbours = Vec::with_capacity(self.adjacency.total_slots() as usize);
        for v in 0..n {
            offsets[v as usize] = neighbours.len() as u32;
            let degree = self.adjacency.degree(v);
            if degree > 0 {
                let mut slot = self.adjacency.start(v);
                for _ in 0..degree {
                    neighbours.push(self.adjacency.slot_neighbour(slot));
                    slot = self.cw[slot as usize];
                }
            }
        }
        offsets[n as usize] = neighbours.len() as u32;
        Embedding::new(n, offsets, neighbours)
    }
}
