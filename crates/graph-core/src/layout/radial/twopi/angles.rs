//! The angular sweep: subtree spans, then every node's own angle, then polar to cartesian.
//!
//! Reference `setSubtreeSpans` (`circle.c:210-214`), `setChildSubtreeSpans`
//! (`circle.c:184-208`), `setPositions` (`circle.c:246-250`), `setChildPositions`
//! (`circle.c:220-244`) and `setAbsolutePos` (`circle.c:289-306`).
//!
//! The reference walks both sweeps **recursively**, depth-first. This port walks the
//! breadth-first `order` instead, which is the same numbers for a reason worth stating: a
//! child's span is its parent's span times a ratio of two leaf counts
//! (`SPAN(next) = SPAN(n) / STSIZE(n) * STSIZE(next)`), and a child's angle is its own span
//! placed inside its parent's fan (`THETA(next) = theta + SPAN(next) / 2`). Neither reads
//! anything but its parent, so parents-before-children is enough — and it cannot overflow
//! a stack on the long path a recursive descent would.

use super::Columns;
use super::adjacency::Neighbours;
use super::tree::Tree;
use super::{UNSET, radius};
use std::f64::consts::PI;

/// Writes every node of `tree`'s component at its polar position.
pub(super) fn place(neighbours: &Neighbours, tree: &Tree, root: u32, out: &mut Columns) {
    let span = spans(neighbours, tree, root);
    let theta = thetas(neighbours, tree, root, &span);
    for &node in tree.order.as_slice() {
        let hyp = radius(tree.depth[node as usize]);
        let angle = theta[node as usize];
        out.set(node, hyp * libm::cos(angle), hyp * libm::sin(angle));
    }
}

/// `SPAN`: the root's whole circle, `2*PI`, divided among siblings in proportion to the
/// leaves below each.
///
/// The reference recurses into a child the moment it has assigned the child's span
/// (`circle.c:204-207`); here the order does that job. The `span == 0.0` guard is the
/// reference's `!is_exactly_zero(SPAN(next))` (`circle.c:199`), which is how a parallel
/// edge is not assigned twice.
fn spans(neighbours: &Neighbours, tree: &Tree, root: u32) -> Vec<f64> {
    let mut span = vec![0.0; tree.slots()];
    span[root as usize] = 2.0 * PI;
    for &node in tree.order.as_slice() {
        let ratio = span[node as usize] / f64::from(tree.leaves[node as usize]);
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if tree.parent[other as usize] == node {
                found.push(other);
            }
        });
        for other in found {
            // The reference's `!is_exactly_zero(SPAN(next))` guard, at the point it makes
            // the assignment rather than beside it: a parallel edge names the same child
            // twice, and the second one must neither be assigned nor be read as a sibling.
            if span[other as usize] != 0.0 {
                continue;
            }
            span[other as usize] = ratio * f64::from(tree.leaves[other as usize]);
        }
    }
    span
}

/// `THETA`: the middle of each node's own fan, laid out left to right across its parent's.
///
/// `cursor` is the reference's local `theta`, which starts at the parent's own lower
/// boundary — `THETA(n) - SPAN(n) / 2` (`circle.c:228`) — or at zero for the root, and
/// advances by each child's whole span as it is placed.
fn thetas(neighbours: &Neighbours, tree: &Tree, root: u32, span: &[f64]) -> Vec<f64> {
    let mut theta = vec![UNSET; tree.slots()];
    theta[root as usize] = 0.0;
    for &node in tree.order.as_slice() {
        let mut cursor = lower_edge(theta[node as usize], span[node as usize], node == root);
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if tree.parent[other as usize] == node {
                found.push(other);
            }
        });
        for other in found {
            // The reference's `is_set(THETA(next))` guard, at the point of assignment: a
            // parallel edge names the same child twice, and without this the second one
            // would take a second slot in the fan and push every sibling after it round.
            if theta[other as usize] != UNSET {
                continue;
            }
            theta[other as usize] = cursor + span[other as usize] / 2.0;
            cursor += span[other as usize];
        }
    }
    theta
}

/// The fan's lower boundary: zero at the root, the node's own centre less half its span
/// everywhere else.
fn lower_edge(theta: f64, span: f64, root: bool) -> f64 {
    if root { 0.0 } else { theta - span / 2.0 }
}
