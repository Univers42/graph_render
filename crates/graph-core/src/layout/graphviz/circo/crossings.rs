//! The crossing count of a block's circle order: a Fenwick sweep over the positions, and the
//! reference's own `O(E^2)` walk kept beside it as the oracle the sweep is tested against.
//!
//! `count_all_crossings` (`blockpath.c:386-431`) walks the order and, at every edge it closes,
//! compares that edge against every edge still open — so one count is quadratic in the edges and
//! [`crate::layout::graphviz::circo::circle`] runs one count per candidate move. The number has
//! an `O(E log E)` reading; see [`Counter::count`] for how.
//!
//! `count_all_crossings` itself is `#[cfg(test)]`: nothing in the layout calls it any more, and
//! keeping it costs nothing but the test build. It is also why [`BlockGraph`] still carries its
//! mutable `EDGEORDER` scratch.
//!
//! **The open set never shrinks, and that is the whole count.** `remove_edge`
//! (`lib/circogen/edgelist.c:64-70`) hands `dtdelete` the `Agedge_t *` the walk is holding at the
//! *closing* endpoint, and `cmpItem` keys the set on that pointer (`edgelist.c:25-34`). An
//! undirected edge is two `Agedge_t` — the out image its tail's row yields and the in image its
//! head's row yields (`lib/cgraph/edge.c:210-215`) — so the pointer the walk opens with is never
//! the pointer it closes with, the lookup misses, and the entry stays for the rest of the walk.
//! Both images share one attribute record, so `EDGEORDER` is the same number on each and the
//! edges still close on time; only the removal is lost. A node whose position closes `e` then
//! counts **every** edge opened after `e` and before this position, not only those still open.
//!
//! That is not a rounding difference: on seed 8's five-node block it is **3** where the
//! interleaving count is **0**, and it is what makes `reduce_edge_crossings` move that block's
//! circle order at all. Measured on Graphviz 16.1.0 itself — see
//! `docs/measurements/p13-gv1-circo.md` §8.
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
    /// The Fenwick tree over positions: one counter per edge the walk has opened, filed at the
    /// position it opened at, so `bit[p]` counts the edges opened at `p`. Nothing is ever taken
    /// back out — the reference's `remove_edge` never matches, as the module doc says.
    bit: Vec<u32>,
    /// How many edges the tree holds, which is how many the walk's set holds.
    live: u32,
    /// Scratch: one position per edge of the row being walked, sorted, so the node test is a
    /// pair of binary searches instead of a scan of the row per edge of the row.
    stamps: Vec<u32>,
}

impl Counter {
    /// `count_all_crossings` (`blockpath.c:386-431`) for `order`: what the reference's walk
    /// counts for this circle order.
    ///
    /// **The rule the walk implements.** At a position, the walk closes every edge of that
    /// node's row that carries an `EDGEORDER` from an earlier position, and counts for each of
    /// them the set's edges stamped *later* than it and earlier than this position, skipping any
    /// that touch the node. So the wanted number is, summed over the row's already-opened edges
    /// `e` and over the edges `ep` in `(open(e), here)`: one each, minus those sharing a node
    /// with the position's node. A Fenwick tree with one counter per opened edge, filed at its
    /// opening position, answers "how many opened in `(open(e), here)`" as `live - upto(open(e))`
    /// in `O(log n)`, and it is the whole data structure.
    ///
    /// **Nothing is retired.** The reference's `remove_edge` is a no-op for every edge of an
    /// undirected graph (module doc), so `live` only grows and `upto` is never re-based. An
    /// earlier version of this file retired each edge when it closed, which made the count the
    /// plain interleaving count and made `reduce_edge_crossings` a no-op on orders the reference
    /// moves.
    ///
    /// **A second visit counts again, and so does this.** A node carried twice has every one of
    /// its edges stamped already, so the walk closes and counts all of them a second time
    /// against the whole set — including chords that share a node with them, which the node test
    /// cannot see because the shared node is not the one being walked. Nothing here suppresses
    /// that: the row is asked about at every visit.
    pub(super) fn count(&mut self, block: &BlockGraph, order: &[u32]) -> u32 {
        self.mark(block, order);
        self.bit.clear();
        self.bit.resize(order.len() + 1, 0);
        self.live = 0;
        let mut crossings = 0;
        for (at, &node) in order.iter().enumerate() {
            let here = at as u32 + 1;
            crossings += self.ask(block, node, here);
            self.give(block, node, here);
        }
        crossings
    }

    /// Each node's first and second visit in `order`, and from those every edge's opening
    /// position: an edge opens at the first visit to either of its endpoints. An edge the order
    /// never opens — it carries neither endpoint — is in no row, so neither side of the count
    /// ever reaches it.
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
        for &(tail, head) in block.ends() {
            self.opens.push(self.opens_at(tail, head));
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

    /// What one position contributes: the walk's closing half. Every edge of this node's row that
    /// opened at an earlier position is closed here, and for each the walk counts the set's edges
    /// opened after it and before here, less those that touch `node` — its own node test, which
    /// the sweep used to get for free by retiring edges and can no longer get that way.
    ///
    /// An edge of the row opened at an earlier position and closed earlier is asked again, which
    /// is a node carried twice, and the walk counts that again too.
    fn ask(&mut self, block: &BlockGraph, node: u32, here: u32) -> u32 {
        self.stamps.clear();
        self.stamps
            .extend(block.row(node).iter().map(|&e| self.opens[e as usize]));
        self.stamps.sort_unstable();
        let opened_here = self.stamps.partition_point(|&at| at < here) as u32;
        let mut crossings = 0;
        for &edge in block.row(node) {
            let opens = self.opens[edge as usize];
            if opens < here {
                // The suffix less the row's own edges in `(opens, here)`, which share this node.
                let touching = opened_here - self.stamps.partition_point(|&at| at <= opens) as u32;
                crossings += self.suffix(opens) - touching;
            }
        }
        crossings
    }

    /// The walk's opening half: every edge whose opening position is this one joins the set.
    /// It runs **after** the closing half above, exactly as the reference runs its two loops —
    /// an edge opening at this position must not be in the set while its row-mates close.
    fn give(&mut self, block: &BlockGraph, node: u32, here: u32) {
        for &edge in block.row(node) {
            if self.opens[edge as usize] == here {
                self.enter(self.opens[edge as usize]);
            }
        }
    }

    /// How many of the set's edges opened after position `at`: the suffix sum the tree exists
    /// for.
    fn suffix(&self, at: u32) -> u32 {
        self.live - self.upto(at)
    }

    /// Put one edge into the set, filed at the position it opened at.
    fn enter(&mut self, at: u32) {
        self.live += 1;
        self.add(at, 1);
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

/// `count_all_crossings` (`blockpath.c:386-431`) as the reference wrote it: walk the order, close
/// the edges the positions leave behind, and count every crossing that closing creates.
///
/// **`open` is append-only, and that is the reference's own behaviour, not a simplification.**
/// `remove_edge` (`lib/circogen/edgelist.c:64-70`) looks the edge up in a set keyed on the
/// `Agedge_t *` the walk is holding, and an undirected edge is two of those — the out image at
/// its tail and the in image at its head (`lib/cgraph/edge.c:210-215`) — so the lookup misses
/// and the entry stays. The two images share one attribute record, so `EDGEORDER` is still the
/// same on both and the edges still close on time; only the removal is lost, and the count grows
/// by every edge opened after the closing one rather than by every edge still open. See
/// `docs/measurements/p13-gv1-circo.md` §8 for the measurement that pins it.
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
