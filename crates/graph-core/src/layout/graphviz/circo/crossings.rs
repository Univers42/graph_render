//! The crossing count of a block's circle order: a Fenwick sweep over the positions, and the
//! reference's own `O(E^2)` walk kept beside it as the oracle the sweep is tested against.
//!
//! `count_all_crossings` (`blockpath.c:386-431`) walks the order and, at every edge it closes,
//! compares that edge against every edge still open — so one count is quadratic in the edges and
//! [`crate::layout::graphviz::circo::circle`] runs one count per candidate move. The number it
//! produces is the ordinary chord-crossing number of the drawing, which has an `O(E log E)`
//! reading; see [`Counter::count`] for how.
//!
//! `count_all_crossings` itself is `#[cfg(test)]`: nothing in the layout calls it any more, and
//! keeping it costs nothing but the test build.

use super::graph::BlockGraph;

/// One crossing count, with the scratch it keeps, so `pass` can count a candidate move without
/// allocating.
///
/// Everything here is a `Vec` indexed by a dense position or a dense edge id, so no map, no
/// pointer and no address takes part in the count (`prompt.md` §6 D2, D10).
#[derive(Default)]
pub(super) struct Counter {
    /// Node -> the position it holds in the order, `-1` for a node the order does not carry.
    at: Vec<i32>,
    /// Edge -> the position it opens at.
    left: Vec<u32>,
    /// Position -> the first edge closing there, `-1` when none closes there.
    close_head: Vec<i32>,
    /// Position -> the first edge opening there, `-1` when none opens there.
    open_head: Vec<i32>,
    /// Edge -> the next edge closing at the same position, `-1` at the end of that list.
    close_next: Vec<i32>,
    /// Edge -> the next edge opening at the same position, `-1` at the end of that list.
    open_next: Vec<i32>,
    /// The Fenwick tree: one counter per open edge's opening position.
    bit: Vec<u32>,
    /// How many edges the tree holds, which is how many are open.
    live: u32,
}

impl Counter {
    /// `count_all_crossings` (`blockpath.c:386-431`) for `order`: how many pairs of the block's
    /// edges cross as chords of this circle.
    ///
    /// Two chords cross exactly when their four endpoints *interleave* around the circle, so
    /// with `l`/`r` an edge's first and second position in the order a pair crosses iff
    /// `l1 < l2 < r1 < r2`. Sweeping the positions `0..n` with the Fenwick tree — one counter
    /// per **open** edge, filed at the position it opened at — turns that into a suffix sum:
    /// when an edge closes at `r`, every edge still open that opened strictly after `l`
    /// crosses it. Each crossing is counted once, at the close of the edge that closes first,
    /// and two edges sharing a node share a position, so they are never both open across the
    /// gap between them and are never counted — the same exclusion the reference's
    /// `head != node && tail != node` guard makes.
    ///
    /// One thing is done in two steps where the reference does it in one: every edge closing at
    /// a position leaves the tree *before* any of them is asked, or two edges sharing that node
    /// would count each other. The numbers are identical, which is what the 2 000 random blocks
    /// in [`super::tests::crossings`] measure.
    ///
    /// `order` carries each of `block`'s nodes exactly once — `order_of` has placed them all
    /// before any count runs — which is what lets a position stand in for a node here, and the
    /// `debug_assert!` says so rather than trusting it. An edge with an endpoint the order does
    /// not carry is on no circle and is left out of the sweep, as the walk leaves it alone too.
    pub(super) fn count(&mut self, block: &BlockGraph, order: &[u32]) -> u32 {
        debug_assert_eq!(order.len(), block.nodes.len(), "the order carries every node");
        self.index(block, order);
        self.bit.clear();
        self.bit.resize(order.len() + 1, 0);
        self.live = 0;
        let mut crossings = 0;
        for at in 0..order.len() as u32 {
            self.drop_closed(at);
            crossings += self.crossed_at(at);
            self.take_opened(at);
        }
        crossings
    }

    /// Every edge's opening position, and the two per-position edge lists the sweep walks.
    fn index(&mut self, block: &BlockGraph, order: &[u32]) {
        self.at.clear();
        self.at.resize(block.nodes.len(), -1);
        for (at, &node) in order.iter().enumerate() {
            self.at[node as usize] = i32::try_from(at).expect("a block has fewer than 2^31 nodes");
        }
        let edges = block.ends().len();
        self.left.clear();
        self.left.resize(edges, 0);
        self.close_next.clear();
        self.close_next.resize(edges, -1);
        self.open_next.clear();
        self.open_next.resize(edges, -1);
        self.close_head.clear();
        self.close_head.resize(order.len(), -1);
        self.open_head.clear();
        self.open_head.resize(order.len(), -1);
        for (id, &(tail, head)) in block.ends().iter().enumerate() {
            let ends = [self.at[tail as usize], self.at[head as usize]];
            if ends[0] < 0 || ends[1] < 0 {
                continue;
            }
            let (opens, closes) = (ends[0].min(ends[1]) as u32, ends[0].max(ends[1]) as u32);
            self.left[id] = opens;
            self.close_next[id] = self.close_head[closes as usize];
            self.close_head[closes as usize] = id as i32;
            self.open_next[id] = self.open_head[opens as usize];
            self.open_head[opens as usize] = id as i32;
        }
    }

    /// Every edge closing at `at` leaves the tree, so none of them can see another.
    fn drop_closed(&mut self, at: u32) {
        let mut edge = self.close_head[at as usize];
        while edge >= 0 {
            self.add(self.left[edge as usize], -1);
            edge = self.close_next[edge as usize];
        }
    }

    /// How many of the edges closing at `at` cross an edge open at this point: for each, the
    /// open edges whose opening position is strictly after its own.
    fn crossed_at(&self, at: u32) -> u32 {
        let mut crossings = 0;
        let mut edge = self.close_head[at as usize];
        while edge >= 0 {
            crossings += self.live - self.upto(self.left[edge as usize]);
            edge = self.close_next[edge as usize];
        }
        crossings
    }

    /// Every edge opening at `at` enters the tree.
    fn take_opened(&mut self, at: u32) {
        let mut edge = self.open_head[at as usize];
        while edge >= 0 {
            self.add(self.left[edge as usize], 1);
            edge = self.open_next[edge as usize];
        }
    }

    /// One counter at position `at`, and the running number of open edges.
    fn add(&mut self, at: u32, delta: i32) {
        self.live = if delta > 0 {
            self.live + 1
        } else {
            self.live - 1
        };
        let mut slot = at as usize + 1;
        while slot < self.bit.len() {
            self.bit[slot] = (self.bit[slot] as i32 + delta) as u32;
            slot += slot & slot.wrapping_neg() as usize;
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
/// This is the oracle [`Counter::count`] is measured against, and it is also why
/// [`BlockGraph`] still carries its mutable `EDGEORDER` scratch.
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