//! `GD_rank`'s `rank_t`: one row per rank, holding that rank's nodes left to right.
//!
//! The reference keeps this array on the graph and three passes fill it: `mincross` numbers
//! every node's slot inside it ([`Node::order`](super::super::fast::Node::order)), and
//! `set_ycoords`, `make_LR_constraints` and `set_xcoords` all walk it row by row. This port's
//! mincross pass keeps its rows as pass state that stops existing when the pass returns, so
//! the position pass rebuilds the array from the two durable fields it left behind — every
//! node's rank and every node's position within it.
//!
//! Rebuilding rather than carrying is not a copy of the array: the row *order* is the answer
//! `mincross` computed, and it is recovered by sorting each rank's nodes on their own slot
//! number, which is a total order inside a rank because two nodes on one rank cannot share a
//! slot. Nothing else about the array survives a pass, so nothing else is reconstructed.
//!
//! One row per slot between rank 0 and the highest rank in use, including the empty ones: the
//! rank pass may leave a rank with nothing on it, and the reference's array has a row for it
//! whatever its count.
//!
//! Determinism: nodes are collected in dense-index order and each row is sorted on a key that
//! is unique within it, so the array is a function of the ranks and orders alone
//! (`prompt.md` §6 D1-D10).

use super::super::fast::{Fast, Kind};
use super::super::mincross::ranks::{max_rank, row_of};

/// Every rank's row, indexed by rank. `rows[r]` is rank `r`'s nodes from left to right, and
/// is empty for a rank nothing landed on.
pub struct Rows {
    /// The rows, one per rank from 0 to the highest rank in use.
    ranks: Vec<Vec<u32>>,
}

impl Rows {
    /// Every node grouped by its rank and ordered by its slot within that rank.
    pub fn of(g: &Fast) -> Self {
        let mut ranks: Vec<Vec<u32>> = vec![Vec::new(); max_rank(g) + 1];
        for node in 0..g.nodes.len() as u32 {
            ranks[row_of(g, node)].push(node);
        }
        for row in &mut ranks {
            row.sort_by_key(|&node| g.nodes[node as usize].order);
        }
        Self { ranks }
    }

    /// The rows, left to right.
    pub fn iter(&self) -> impl Iterator<Item = &[u32]> {
        self.ranks.iter().map(Vec::as_slice)
    }

    /// How many rows there are: the highest rank in use, plus one.
    pub fn len(&self) -> usize {
        self.ranks.len()
    }

    /// The number of the lowest rank that has a node on it, or `None` for a graph with no
    /// nodes.
    ///
    /// The rank pass normalises the lowest rank to 0, so this is normally 0; it is found
    /// rather than assumed so an unranked graph answers honestly instead of returning a row
    /// that was never filled.
    pub fn lowest_rank(&self) -> Option<usize> {
        self.ranks.iter().position(|row| !row.is_empty())
    }

    /// The number of the highest rank that has a node on it, or `None` for a graph with no
    /// nodes. This is the **bottom** row of the drawing: the rank pass stacks rank 0 at the
    /// top.
    pub fn highest_rank(&self) -> Option<usize> {
        self.ranks.iter().rposition(|row| !row.is_empty())
    }

    /// The row of rank `r`, or an empty slice for a rank past the end.
    pub fn row(&self, r: usize) -> &[u32] {
        self.ranks.get(r).map_or(&[], Vec::as_slice)
    }

    /// The first node on `row` that is a real node rather than a chain dummy, or `None` when
    /// the row is all dummies.
    ///
    /// The drawing's left edge is measured from a real node's box and a dummy has no box, so
    /// this is the node the bounding box starts from.
    pub fn first_real(row: &[u32], g: &Fast) -> Option<u32> {
        row.iter().copied().find(|&node| g.nodes[node as usize].kind == Kind::Normal)
    }

    /// The last node on `row` that is a real node rather than a chain dummy, or `None` when
    /// the row is all dummies. The mirror of [`Rows::first_real`].
    pub fn last_real(row: &[u32], g: &Fast) -> Option<u32> {
        row.iter().rev().copied().find(|&node| g.nodes[node as usize].kind == Kind::Normal)
    }
}
