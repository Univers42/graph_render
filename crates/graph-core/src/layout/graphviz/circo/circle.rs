//! The circle order: `place_residual_nodes`, `reduce_edge_crossings` and the angular
//! placement (`blockpath.c:386-628`).
//!
//! Three steps, all on the block's whole node list:
//!
//! 1. `place_residual_nodes` — anything the longest path missed is inserted next to a
//!    neighbour, preferring a spot between two neighbours already consecutive in the order.
//!    This is what makes the list cover the block exactly once.
//! 2. `reduce_edge_crossings` — up to ten passes of "move a node next to each of its
//!    neighbours and keep the move only if the crossing count drops".
//! 3. The angular placement — node `k` of `n` sits at `k * 2*PI / n` on a circle of radius
//!    `n * (min_dist + largest_node) / 2*PI`.
//!
//! **The crossing count is order-free, which is what makes this reproducible**, and it is not
//! what this file computes: [`crossings`] holds the count, now a Fenwick sweep over the
//! positions rather than the reference's quadratic walk, and this file only asks for it. The
//! reference holds the open edges in a `Dtoset` keyed on the edge *pointer* (`edgelist.c:27-37`),
//! so it walks them in address order — but it only ever asks whether an open edge's `EDGEORDER`
//! is greater than the current edge's and whether it touches the current node. Both questions
//! have the same answer whichever open edge is examined first, so the count is a function of the
//! node order alone and the sweep can take it without an order of its own.

use super::NODE_SIZE_INCH;
use super::crossings::Counter;
use super::graph::BlockGraph;

/// `CROSS_ITER` (`blockpath.c:433`): how many times the reduction is allowed to run.
const CROSS_ITER: usize = 10;

/// `largest_nodesize` (`blockpath.c:495-507`) folded over a block of default nodes:
/// `max(ND_width, ND_height)` is the same for every node, so the fold is this constant. It is
/// load-bearing: it is most of the circle's radius.
pub(super) const LARGEST_NODE: f64 = if NODE_SIZE_INCH.0 > NODE_SIZE_INCH.1 {
    NODE_SIZE_INCH.0
} else {
    NODE_SIZE_INCH.1
};

/// Every node of `block`, in the order the circle will carry them: the long path plus the
/// residual pass plus the crossing reduction.
pub(super) fn order_of(block: &BlockGraph, order: Vec<u32>) -> Vec<u32> {
    let nodes = block.nodes.len() as u32;
    let mut placed = vec![false; nodes as usize];
    for &node in &order {
        placed[node as usize] = true;
    }
    let mut order = order;
    let mut marked = vec![false; nodes as usize];
    for node in 0..nodes {
        if !placed[node as usize] {
            place_node(block, node, &mut order, &mut marked);
        }
    }
    reduce(block, order)
}

/// `place_residual_nodes` + `place_node` (`blockpath.c:510-565`): the first unplaced node in
/// block order, inserted after two consecutive marked neighbours, else after any marked
/// neighbour, else at the end.
fn place_node(block: &BlockGraph, node: u32, order: &mut Vec<u32>, marked: &mut [bool]) {
    // `agfstout` gives the heads, `agfstin` the tails: the other endpoint either way round.
    let mut neighbours: Vec<u32> = block
        .out_row(node)
        .into_iter()
        .map(|edge| block.head(edge))
        .collect();
    neighbours.extend(block.in_row(node).into_iter().map(|edge| block.tail(edge)));
    for &other in &neighbours {
        marked[other as usize] = true;
    }
    let at = pair_slot(order, &neighbours)
        .map(|one| one + 1)
        .or_else(|| first_marked(order, &neighbours).map(|one| one + 1))
        .unwrap_or(order.len());
    order.insert(at, node);
    for &other in &neighbours {
        marked[other as usize] = false;
    }
}

/// The index of a node with two neighbours already consecutive in `order`, wrapping round.
fn pair_slot(order: &[u32], marked: &[u32]) -> Option<usize> {
    if marked.len() < 2 || order.is_empty() {
        return None;
    }
    let is = |node: u32| marked.contains(&node);
    (0..order.len()).find(|&one| is(order[one]) && is(order[(one + 1) % order.len()]))
}

/// The index of the first node of `order` that is a neighbour.
fn first_marked(order: &[u32], marked: &[u32]) -> Option<usize> {
    order.iter().position(|node| marked.contains(node))
}

/// `reduce_edge_crossings` (`blockpath.c:477-492`). One counter and one scratch order are
/// built here and threaded through every pass, so the whole reduction allocates O(1) times.
fn reduce(block: &BlockGraph, order: Vec<u32>) -> Vec<u32> {
    let mut counter = Counter::default();
    let mut crossings = counter.count(block, &order);
    let mut order = order;
    if crossings == 0 {
        return order;
    }
    for _ in 0..CROSS_ITER {
        let before = crossings;
        order = pass(block, order, &mut crossings, &mut counter);
        if before == crossings || crossings == 0 {
            return order;
        }
    }
    order
}

/// `reduce` (`blockpath.c:439-475`): every node, then every one of its edges, then the two
/// slots beside that neighbour — keep the move only while the crossing count drops.
///
/// **The same moves are kept as before**, which is the point: the candidate is still the whole
/// order with one node moved, and it is still kept only on a strict drop. What changed is the
/// bookkeeping — the row is borrowed instead of copied, the candidate is written into one
/// scratch `Vec` and swapped in rather than cloned per try, and the count is the sweep's
/// `O(E log E)` instead of the reference's quadratic walk. A kept move therefore costs a `swap`
/// rather than a clone, and a rejected one costs a copy into the scratch.
fn pass(
    block: &BlockGraph,
    mut order: Vec<u32>,
    crossings: &mut u32,
    counter: &mut Counter,
) -> Vec<u32> {
    let nodes = block.nodes.len() as u32;
    let mut scratch = Vec::new();
    for current in 0..nodes {
        for &edge in block.row(current) {
            let neighbour = block.other(edge, current);
            for slot in 0..2 {
                scratch.clear();
                scratch.extend_from_slice(&order);
                insert(&mut scratch, current, neighbour, slot);
                let found = counter.count(block, &scratch);
                if found < *crossings {
                    *crossings = found;
                    std::mem::swap(&mut order, &mut scratch);
                    if *crossings == 0 {
                        return order;
                    }
                }
            }
        }
    }
    order
}

/// `insertNodelist` (`nodelist.c:47-64`): lift `node` out and put it back either just before
/// or just after `neighbour`.
fn insert(order: &mut Vec<u32>, node: u32, neighbour: u32, slot: usize) {
    let at = order
        .iter()
        .position(|&n| n == node)
        .expect("the node is in the order");
    order.remove(at);
    if let Some(at) = order.iter().position(|&n| n == neighbour) {
        order.insert(at + slot, node);
    }
}

/// `realignNodelist` (`nodelist.c:38-45`): rotate the order left so the node at `at` comes
/// first. Only the first `PARENT_F` node is realigned, and only once (`blockpath.c:596-603`).
pub(super) fn realign(order: &mut Vec<u32>, at: usize) {
    for _ in 0..at {
        let head = order.remove(0);
        order.push(head);
    }
}

/// `layout_block`'s radius (`blockpath.c:588-594`), in inches: the circumference the block's
/// nodes need, over the full turn.
pub(super) fn radius_of(count: usize) -> f64 {
    if count == 1 {
        0.0
    } else {
        count as f64 * (super::MIN_DIST + LARGEST_NODE) / super::TAU
    }
}

/// Node `k`'s angle, `k * 2*PI / count`.
pub(super) fn angle_of(k: usize, count: usize) -> f64 {
    k as f64 * (super::TAU / count as f64)
}

/// A block's own radius when it is a single node (`blockpath.c:618-621`): half the node.
pub(super) fn single_radius() -> f64 {
    LARGEST_NODE / 2.0
}
