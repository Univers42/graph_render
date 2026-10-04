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
//! **The count is taken over the walk, not over an assumed permutation, because the order can
//! carry a node twice.** `longest_path` reads its two halves off two leaves, so when the block's
//! thinned tree is a forest the branch node's best and runner-up leaf can be the same one and the
//! path repeats a branch; `place_residual_nodes` then adds nothing, because every node is
//! already placed. Seed 68 of the invariant sweep is such a block: 44 nodes, an order of 45.
//! [`Counter::mark`] therefore takes each node's first and second visit rather than one position
//! per node, and the second visit is what makes the count what it has always been: the walk
//! closes a node's already-closed edges a second time and counts them again, and so does this.
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
    /// Node -> the one-based position of its **first** visit in the order, 0 while the order has
    /// not carried it. This is the reference's `EDGEORDER` for every edge the node opens.
    first: Vec<u32>,
    /// Node -> the one-based position of its **second** visit, or `u32::MAX` when the order
    /// carries it once, which is the common case.
    second: Vec<u32>,
    /// Edge -> the position it opens at: the first visit to either of its endpoints.
    opens: Vec<u32>,
    /// Edge -> the position it closes at: the first visit to either endpoint after that, or
    /// `u32::MAX` when there is none, which leaves the edge open to the end as the walk does.
    closes: Vec<u32>,
    /// The Fenwick tree over positions: one counter per **open** edge, filed at the position it
    /// opened at, so `bit[p]` counts the open edges that opened at `p`.
    bit: Vec<u32>,
    /// How many edges the tree holds, which is how many are open.
    live: u32,
}

impl Counter {
    /// `count_all_crossings` (`blockpath.c:386-431`) for `order`: how many pairs of the block's
    /// edges cross as chords of this circle.
    ///
    /// **The rule the walk implements.** At a position, the walk closes every edge of that
    /// node's row that carries an `EDGEORDER` from an earlier position, and counts for each of
    /// them the still-open edges stamped *later* than it, skipping any that touch the node. Two
    /// chords cross exactly when their four endpoints interleave, so the number wanted is the
    /// interleaving count `l1 < l2 < r1 < r2`, and the only thing the walk needs the open set
    /// for is the suffix "opened after `l`". A Fenwick tree with one counter per open edge,
    /// filed at its opening position, answers that in `O(log n)`, and it is the whole data
    /// structure.
    ///
    /// **The node test, and where it goes.** An edge that touches the node at a position cannot
    /// still be open across it — it opened at that node's own first visit or earlier, and this
    /// position is a visit to one of its endpoints, so it closes no later than here. So the only
    /// edges of that row the walk can still see open are the ones it has not closed *yet* in
    /// this same row, and those are exactly what its node test throws away. Retiring every edge
    /// that closes at a position *before* asking the tree anything therefore reproduces the test
    /// rather than dropping it: what is left in the tree when a position asks are the edges
    /// spanning it, and no two of those share a node. What the test excluded is already gone.
    ///
    /// **A second visit counts again, and so does this.** A node carried twice has every one of
    /// its edges stamped already, so the walk closes and counts all of them a second time
    /// against whatever is still open — including chords that share a node with them, which the
    /// node test cannot see because the shared node is not the one being walked. Nothing here
    /// suppresses that: the row is asked about at every visit, and the tree is left holding
    /// exactly what the walk's open set would hold.
    pub(super) fn count(&mut self, block: &BlockGraph, order: &[u32]) -> u32 {
        self.mark(block, order);
        self.bit.clear();
        self.bit.resize(order.len() + 1, 0);
        self.live = 0;
        let mut crossings = 0;
        for (at, &node) in order.iter().enumerate() {
            let here = at as u32 + 1;
            self.close(block, node, here);
            crossings += self.ask(block, node, here);
            self.give(block, node, here);
        }
        crossings
    }

    /// Each node's first and second visit in `order`, and from those every edge's opening and
    /// closing position: an edge opens at the first visit to either of its endpoints and closes
    /// at the first visit after that. An edge the order never opens — it carries neither
    /// endpoint — is in no row, so neither side of the count ever reaches it.
    fn mark(&mut self, block: &BlockGraph, order: &[u32]) {
        self.first.clear();
        self.first.resize(block.nodes.len(), 0);
        self.second.clear();
        self.second.resize(block.nodes.len(), u32::MAX);
        for (at, &node) in order.iter().enumerate() {
            let stamp = u32::try_from(at + 1).expect("a block has fewer than 2^31 nodes");
            if self.first[node as usize] == 0 {
                self.first[node as usize] = stamp;
            } else if self.second[node as usize] == u32::MAX {
                self.second[node as usize] = stamp;
            }
        }
        self.opens.clear();
        self.closes.clear();
        for &(tail, head) in block.ends() {
            let opens = self.opens_at(tail, head);
            self.opens.push(opens);
            let one = self.after(tail, opens);
            let other = self.after(head, opens);
            self.closes.push(one.min(other));
        }
    }

    /// The position an edge opens at: the first visit to either endpoint. An endpoint the order
    /// never carries has no first visit and cannot open the edge, so the other one does.
    fn opens_at(&self, tail: u32, head: u32) -> u32 {
        let (one, other) = (self.first[tail as usize], self.first[head as usize]);
        match (one == 0, other == 0) {
            (true, true) => 0,
            (true, false) => other,
            (false, true) => one,
            (false, false) => one.min(other),
        }
    }

    /// The first visit to `node` strictly after `at`, or `u32::MAX`: its first visit when that
    /// is later than `at`, else its second, else nothing.
    fn after(&self, node: u32, at: u32) -> u32 {
        let first = self.first[node as usize];
        if first > at {
            first
        } else if self.second[node as usize] > at {
            self.second[node as usize]
        } else {
            u32::MAX
        }
    }

    /// Retire every edge that closes at `here`, so what the tree still holds is the set of edges
    /// spanning this position — which, as [`Counter::count`] says, is what the walk's node test
    /// leaves behind.
    fn close(&mut self, block: &BlockGraph, node: u32, here: u32) {
        for &edge in block.row(node) {
            if self.closes[edge as usize] == here {
                self.take(self.opens[edge as usize]);
            }
        }
    }

    /// What one position contributes: the walk's closing half. Every edge of this node's row that
    /// opened at an earlier position is closed here and asks how many open edges opened after it.
    /// An edge that opened at an earlier position and has already been closed is asked again,
    /// which is a node carried twice, and the walk counts that again too.
    fn ask(&mut self, block: &BlockGraph, node: u32, here: u32) -> u32 {
        let mut crossings = 0;
        for &edge in block.row(node) {
            let opens = self.opens[edge as usize];
            if opens < here {
                crossings += self.suffix(opens);
            }
        }
        crossings
    }

    /// The walk's opening half: every edge whose opening position is this one joins the open set.
    /// It runs **after** the closing half above, exactly as the reference runs its two loops —
    /// an edge opening at this position must not be in the open set while its row-mates close.
    fn give(&mut self, block: &BlockGraph, node: u32, here: u32) {
        for &edge in block.row(node) {
            if self.opens[edge as usize] == here {
                self.enter(self.opens[edge as usize]);
            }
        }
    }

    /// How many open edges opened after position `at`: the suffix sum the tree exists for.
    fn suffix(&self, at: u32) -> u32 {
        self.live - self.upto(at)
    }

    /// Put one edge into the open set, filed at the position it opened at.
    fn enter(&mut self, at: u32) {
        self.live += 1;
        self.add(at, 1);
    }

    /// Take one edge out of the open set.
    fn take(&mut self, at: u32) {
        self.live -= 1;
        self.add(at, -1);
    }

    /// One counter at position `at`, in a tree whose `n` counters stand for the positions
    /// `0..n`. Integer arithmetic only: the counters are `u32`, the steps are shifts and an
    /// `i32` add per touched slot, and no position ever exceeds `u32::MAX` because a block has
    /// fewer than `2^31` nodes (`mark`'s `try_from` is where that is enforced, not here).
    fn add(&mut self, at: u32, delta: i32) {
        let mut slot = at as usize + 1;
        debug_assert!(slot < self.bit.len(), "a position is inside the tree");
        while slot < self.bit.len() {
            self.bit[slot] = (self.bit[slot] as i32 + delta) as u32;
            slot += slot.isolate_lowest_one();
        }
    }

    /// The counters at the positions `0..=at`, inclusive.
    fn upto(&self, at: u32) -> u32 {
        let mut slot = at as usize + 1;
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
/// closed cases in [`super::tests::crossings`].
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
