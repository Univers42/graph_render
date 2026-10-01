//! The root: how far every node is from the nearest leaf, and which node is furthest.
//!
//! Reference `initLayout` (`circle.c:74-88`), `isLeaf` (`circle.c:55-72`),
//! `setNStepsToLeaf` (`circle.c:34-52`) and `findCenterNode` (`circle.c:96-114`).
//!
//! The reference relaxes `nStepsToLeaf` with a recursive descent from every leaf in turn;
//! that is a shortest-path-to-the-nearest-leaf computation written as a fixpoint, and a
//! multi-source breadth-first search from all the leaves at once returns the same integer
//! for every node. The descent is `depth`-recursive and would overflow the stack on a
//! long path; this form cannot, and it is the same order of work.

use super::adjacency::Neighbours;

/// A node no leaf reaches. Every node of every component is reached by one, so the
/// sentinel never survives into [`of`] — it is `u32::MAX` only so the search can start.
const UNREACHED: u32 = u32::MAX;

/// Every node's `nStepsToLeaf`: the fewest edges to a node with at most one distinct
/// neighbour.
pub(super) fn steps_to_leaf(neighbours: &Neighbours, count: u32) -> Vec<u32> {
    let mut steps = vec![UNREACHED; count as usize];
    let mut queue: Vec<u32> = Vec::new();
    for node in 0..count {
        if is_leaf(neighbours, node) {
            steps[node as usize] = 0;
            queue.push(node);
        }
    }
    let mut at = 0;
    while at < queue.len() {
        let node = queue[at];
        at += 1;
        let next = steps[node as usize] + 1;
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if next < steps[other as usize] {
                found.push(other);
            }
        });
        for other in found {
            // First discovery is the shortest, as in any FIFO breadth-first search; the
            // reference's `nsteps < SCENTER(next)` test is the same statement written as a
            // relaxation. A parallel edge names the same node twice in `found`, so this is
            // also what keeps one node from entering the queue twice.
            if steps[other as usize] != UNREACHED {
                continue;
            }
            steps[other as usize] = next;
            queue.push(other);
        }
    }
    steps
}

/// The component's root: the first node, scanning ascending, whose `nStepsToLeaf` is the
/// component's largest.
///
/// **A tie goes to the lowest dense index**, because the reference's loop is
/// `if (center == NULL || SLEAF(n) > maxNStepsToLeaf)` (`circle.c:108`) — strictly
/// greater, so the first maximum stands.
pub(super) fn of(steps: &[u32], component: &[u32]) -> u32 {
    let mut root = component[0];
    let mut best = steps[root as usize];
    for &node in &component[1..] {
        if steps[node as usize] > best {
            best = steps[node as usize];
            root = node;
        }
    }
    root
}

/// Whether `node` has at most one distinct neighbour, a self-loop not counting.
///
/// `isLeaf` (`circle.c:55-72`) skips loops and compares the neighbour against the first one
/// it saw, so a node with a doubled edge is still a leaf and an isolated node is one too
/// (`neighp` stays null and the function returns true).
fn is_leaf(neighbours: &Neighbours, node: u32) -> bool {
    let mut first: Option<u32> = None;
    let mut leaf = true;
    neighbours.for_each(node, |other| match first {
        None if other != node => first = Some(other),
        Some(seen) if seen != other => leaf = false,
        _ => {}
    });
    leaf
}
