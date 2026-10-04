//! The crossing count of a block's circle order: a Fenwick sweep over the positions, and the
//! reference's own `O(E^2)` walk kept beside it as the oracle the sweep is tested against.
//!
//! `count_all_crossings` (`blockpath.c:386-431`) walks the order and, at every edge it closes,
//! compares that edge against every edge still open — so one count is quadratic in the edges and
//! [`crate::layout::graphviz::circo::circle`] runs one count per candidate move. The number is
//! the ordinary chord-crossing number of the drawing, which has an `O(E log E)` reading; see
//! [`Counter::count`] for how.
//!
//! `count_all_crossings` itself is `#[cfg(test)]`: nothing in the layout calls it any more, and
//! keeping it costs nothing but the test build. It is also why [`BlockGraph`] still carries its
//! mutable `EDGEORDER` scratch.
//!
//! **The order can carry a node twice, and the count is taken over the walk, not over an
//! assumed permutation.** `longest_path`'s two climbs both start at a leaf of a node, so when the
//! block's thinned tree is a forest the branch node's best and runner-up leaf can be the same
//! one and the path repeats a branch; `place_residual_nodes` then adds nothing, because every
//! node is already placed. Seed 68 of the invariant sweep is such a block: 44 nodes, an order of
//! 45. So [`Counter::count`] walks the order the way the reference walks it, and derives each
//! edge's opening and closing position from the **first** and **second** visit of its
//! endpoints, rather than from a permutation it cannot assume.
//!
//! Determinism: integer arithmetic, dense indices, no map and no pointer takes part in a count
//! (`prompt.md` §6 D2, D10).

use super::graph::BlockGraph;

/// One crossing count, with the scratch it keeps, so `pass` can count a candidate move without
/// allocating.
///
/// Every array is a `Vec` indexed by a dense position, a dense node index or a dense edge id.
#[derive(Default)]
pub(super) struct Counter {
    /// Node -> the one-based position of its **first** visit in the order, 0 while the order
    /// has not carried it. This is the reference's `EDGEORDER` for every edge the node opens.
    opened_at: Vec<u32>,
    /// Node -> the one-based position of its **second** visit, or `u32::MAX` when it is
    /// visited once, which is the common case.
    closed_at: Vec<u32>,
    /// Edge -> the position it opens at: the earlier of its two endpoints' first visits.
    opens: Vec<u32>,
    /// Edge -> the position it closes at: the first visit to either endpoint after that, so the
    /// later of the earlier endpoint's second visit and the other endpoint's first visit.
    closes: Vec<u32>,
    /// The Fenwick tree over positions, one counter per **open** edge filed at the position it
    /// opened at: `bit[p]` counts the open edges that opened at `p`.
    bit: Vec<u32>,
    /// How many edges the tree holds, which is how many are open.
    live: u32,
}

impl Counter {
    /// `count_all_crossings` (`blockpath.c:386-431`) for `order`: how many pairs of the block's
    /// edges cross as chords of this circle.
    ///
    /// **The rule the walk implements, and what it becomes here.** At a position, the reference
    /// closes every edge of that node's row that was stamped at an earlier position, and counts
    /// for each of them the open edges stamped *later* than it, skipping any that touch the
    /// node. Two chords cross exactly when their four endpoints interleave, so that is the
    /// interleaving count `l1 < l2 < r1 < r2` — and the one thing the walk needs the open set
    /// for is the suffix "opened after `l`". A Fenwick tree over opening positions answers
    /// that in `O(log n)`, and one counter per open edge filed at its opening position is the
    /// whole data structure.
    ///
    /// **The node test cancels, and that is what makes the tree enough.** An edge incident to
    /// the node at a position cannot still be open there: it opened at that node's own first
    /// visit, or earlier, and this position is a visit to one of its endpoints, so it closed
    /// no later than now. The only edges of that row the walk can still see in its open set
    /// are the ones it has not closed *yet* in this very row — and those are precisely the ones
    /// its node test throws away. What is added and what is subtracted are the same suffix, so
    /// the crossings at a position are the plain sum over the row of "open edges that opened
    /// after this one", with no test at all.
    pub(super) fn count(&mut self, block: &BlockGraph, order: &[u32]) -> u32 {
        self.mark(block, order);
        self.bit.clear();
        self.bit.resize(order.len() + 1, 0);
        self.live = 0;
        let mut crossings = 0;
        for (at, &node) in order.iter().enumerate() {
            self.close(block, node, at as u32 + 1);
            crossings += self.open_now(block, node, at as u32 + 1);
        }
        crossings
    }

    /// Each node's first and second visit, and from those each edge's opening and closing
    /// position: the earlier endpoint's first visit opens the edge, and the first visit to
    /// either endpoint after that closes it. An edge the order never opens — the order carries
    /// neither endpoint — opens and closes nowhere, so it enters no tree and is counted by
    /// neither side.
    fn mark(&mut self, block: &BlockGraph, order: &[u32]) {
        self.opened_at.clear();
        self.opened_at.resize(block.nodes.len(), 0);
        self.closed_at.clear();
        self.closed_at.resize(block.nodes.len(), u32::MAX);
        for (at, &node) in order.iter().enumerate() {
            let stamp = u32::try_from(at + 1).expect("a block has fewer than 2^31 nodes");
            if self.opened_at[node as usize] == 0 {
                self.opened_at[node as usize] = stamp;
            } else if self.closed_at[node as usize] == u32::MAX {
                self.closed_at[node as usize] = stamp;
            }
        }
        self.opens.clear();
        self.closes.clear();
        for &(tail, head) in block.ends() {
            let (open, close) = self.span(tail, head);
            self.opens.push(open);
            self.closes.push(close);
        }
    }

    /// One edge's opening and closing position from its endpoints' first and second visits. The
    /// endpoint visited first opens it, and the next visit to *either* endpoint closes it.
    fn span(&self, tail: u32, head: u32) -> (u32, u32) {
        let (first, later) = (self.opened_at[tail as usize], self.opened_at[head as usize]);
        let (twice, after) = if first < later {
            (self.closed_at[tail as usize], later)
        } else {
            (self.closed_at[head as usize], first)
        };
        (first.min(later), twice.min(after))
    }

    /// Take out of the tree every edge that closes at `here`, so what is left is exactly the
    /// edges open across this position.
    fn close(&mut self, block: &BlockGraph, node: u32, here: u32) {
        for &edge in block.row(node) {
            if self.closes[edge as usize] == here {
                self.release(self.opens[edge as usize]);
            }
        }
    }

    /// The crossings one position contributes, and the edges that open at it. An edge of this
    /// row that opened at an earlier position closes here (this is its first close, or a repeat
    /// of one the walk has already counted — it counts it again, and so does this).
    fn open_now(&mut self, block: &BlockGraph, node: u32, here: u32) -> u32 {
        let mut crossings = 0;
        for &edge in block.row(node) {
            let opens = self.opens[edge as usize];
            if opens < here {
                crossings += self.after(opens);
            } else if opens == here {
                self.live = self.live + 1;
                self.bit_add(opens, 1);
            }
        }
        crossings
    }

    /// How many open edges opened after position `at`: the suffix sum the tree exists for.
    fn after(&self, at: u32) -> u32 {
        self.live - self.upto(at)
    }

    /// Take one edge out of the open set.
    fn release(&mut self, at: u32) {
        self.live = self.live - 1;
        self.bit_add(at, -1);
    }

    /// One counter at position `at`, in a tree over `1..=self.bit.len() - 1`.
    fn bit_add(&mut self, at: u32, delta: i32) {
        let mut slot = at as usize;
        while slot < self.bit.len() {
            self.bit[slot] = (self.bit[slot] as i32 + delta) as u32;
            slot += slot & slot.wrapping_neg() as usize;
        }
    }

    /// The counters at the positions `1..=at`, inclusive.
    fn upto(&self, at: u32) -> u32 {
        let mut slot = at as usize;
        let mut sum = 0;
        while slot > 0 {
            sum += self.bit[slot];
            slot &= slot - 1;
        }
        sum
    }
}

/// `count_all_crossings` (`blockpath.c:386-431`) exactly as the reference wrote it: walk the
/// order, close the edges it leaves behind, and count every crossing that closing creates.
///
/// This is the oracle [`Counter::count`] is measured against, on 2 000 random blocks and on the
/// five closed cases in [`super::tests::crossings`].
#[cfg(test)]
pub(super) fn count_all_crossings(block: &mut BlockGraph, order: &[u32]) -> u32 {
    block.clear_orders();
    let mut open: Vec<u32> = Vec::new();
    let mut crossings = 0;
    for (at, &node) in order.iter().enumerate() {
        let row = block.row(node).to_vec();
        for &edge in &row {
            if block.order(edge) > 0 {
                let mine = block.order(edge);
                for &other in &open {
                    if block.order(other) > mine
                        && block.head(other) != node
                        && block.tail(other) != node
                    {
                        crossings += 1;
                    }
                }
                open.retain(|&kept| kept != edge);
            }
        }
        let stamp = i32::try_from(at + 1).expect("a block has fewer than 2^31 nodes");
        for &edge in &row {
            if block.order(edge) == 0 {
                block.set_order(edge, stamp);
                open.push(edge);
            }
        }
    }
    crossings
}