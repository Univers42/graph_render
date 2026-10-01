//! Turning and moving a child circle: `getRotation` and `applyDelta` (`circpos.c:50-143`).
//!
//! A child circle arrives at its slot already placed at the right radius and angle, but it
//! still has to be **turned** so the edge from the parent leaves it sensibly, and then
//! **moved** — a rotation and a translation applied to the whole subtree below it.
//!
//! Three cases, in the reference's order:
//!
//! - A **one-node** block already knows where its parent is (`parent_pos` was fixed when it
//!   was laid out), so the turn is simply "face the parent".
//! - A **two-node** block is a line, and the reference puts that line normal to the slot
//!   angle: `theta - PI/2`.
//! - Anything else: find the block node that ends up closest to the parent — that is
//!   `CHILD(sn)`, the node that named the parent, unless some other node of the block lands
//!   nearer. If the anchor *is* the closest node the circle already faces the right way and
//!   is not turned at all; otherwise the turn puts the anchor on the ray from the parent.
//!
//! A **coalesced** block (one with a single child, whose origin is no longer its centre) takes
//! a different branch: the edge is made tangent to the anchor's own circle rather than
//! radial, which is the `asin` at `circpos.c:103`.

use super::{Layout, TAU};
use std::f64::consts::PI;

/// `getRotation(child, x, y, theta)`: the turn to apply to `at` sitting at `(x, y)`.
pub(super) fn get_rotation(layout: &Layout, at: usize, x: f64, y: f64, mut theta: f64) -> f64 {
    let block = &layout.blocks[at];
    if block.parent_pos >= 0.0 {
        theta += PI - block.parent_pos;
        if theta < 0.0 {
            theta += TAU;
        }
        return theta;
    }
    if block.circle.len() == 2 {
        return theta - PI / 2.0;
    }
    let anchor = block.child_node;
    if closest_node(layout, at, x, y, anchor) == anchor {
        return 0.0;
    }
    if block.coalesced && -(block.radius - block.rad0) < layout.x[anchor as usize] {
        tangent_turn(layout, at, x, y, theta)
    } else {
        radial_turn(layout, anchor, theta)
    }
}

/// The block's node that ends up nearest the parent — the loop at `circpos.c:79-91`. Ties keep
/// the earlier node, as the reference's strict `<` does.
fn closest_node(layout: &Layout, at: usize, x: f64, y: f64, anchor: u32) -> u32 {
    let reach = |node: u32| libm::hypot(layout.x[node as usize] + x, layout.y[node as usize] + y);
    let mut best = (anchor, reach(anchor));
    for &node in &layout.blocks[at].nodes {
        if node != anchor && reach(node) < best.1 {
            best = (node, reach(node));
        }
    }
    best.0
}

/// The uncoalesced turn (`circpos.c:104-109`): put the anchor on the ray from the parent, then
/// wind back by the anchor's own children mid-angle.
fn radial_turn(layout: &Layout, anchor: u32, mut theta: f64) -> f64 {
    let phi = libm::atan2(layout.y[anchor as usize], layout.x[anchor as usize]);
    theta += PI - phi - layout.psi[anchor as usize];
    if theta > TAU {
        theta -= TAU;
    }
    theta
}

/// The coalesced turn (`circpos.c:97-103`): make the edge from the parent tangent to the
/// anchor's own circle, on the far side of the block's true centre.
fn tangent_turn(layout: &Layout, at: usize, x: f64, y: f64, mut theta: f64) -> f64 {
    let block = &layout.blocks[at];
    let anchor = block.child_node as usize;
    let (rho, reach) = (block.rad0, block.radius - block.rad0);
    let phi = libm::atan2(layout.y[anchor], layout.x[anchor] + reach);
    let length = reach - rho / libm::cos(phi);
    theta += PI / 2.0 - phi - libm::asin(length / libm::hypot(x, y) * libm::cos(phi));
    theta
}

/// `applyDelta(block, x, y, rotate)`: turn and translate `at` and everything below it.
///
/// The reference recurses into the children (`circpos.c:141-142`); the turn and the shift are
/// the same for every node of the subtree, so this is a stack of blocks rather than a
/// recursion, and the order the blocks are visited in cannot change the result.
pub(super) fn apply_delta(layout: &mut Layout, at: usize, x: f64, y: f64, rotate: f64) {
    let (cos_r, sin_r) = (libm::cos(rotate), libm::sin(rotate));
    let mut stack = vec![at];
    while let Some(at) = stack.pop() {
        let nodes = layout.blocks[at].nodes.clone();
        for node in nodes {
            let at = node as usize;
            let (tx, ty) = (layout.x[at], layout.y[at]);
            layout.x[at] = tx * cos_r - ty * sin_r + x;
            layout.y[at] = tx * sin_r + ty * cos_r + y;
        }
        let children = layout.blocks[at].children.clone();
        stack.extend(children);
    }
}
