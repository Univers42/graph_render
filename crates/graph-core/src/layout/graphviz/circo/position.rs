//! The placement: `circPos` (`circpos.c`), reimplemented.
//!
//! `doBlock` (`circpos.c:395-421`) lays the tree out **bottom-up**: every child block first,
//! then the block's own circle, then the children hung off it. This module keeps that order
//! with an explicit post-order walk rather than the reference's recursion.
//!
//! A block with children is a ring of child circles arranged around its own. The arithmetic is
//! the reference's, unchanged:
//!
//! - `getInfo` gives each node that has children an arc — `diameter`, the summed child
//!   diameters plus a `min_dist` gap each — and a `minRadius` at which its children fit.
//! - `setInfo` resolves two arcs against each other into one scale, so they do not overlap.
//! - `positionChildren` walks the children at that radius, one arc at a time.
//!
//! A block with **exactly one** child is *coalesced*: instead of a radius of `own + child`, it
//! takes `own + child / 2` and shifts its own centre by the difference (`circpos.c:381-384`).
//! Its origin is then no longer its centre, which is why `getRotation` has a second branch
//! for it.

use super::circle;
use super::graph::Derived;
use super::rotation::{apply_delta, get_rotation};
use super::{Layout, MIN_DIST, TAU};
use std::f64::consts::PI;

/// Lays the block tree rooted at `root` out, bottom-up.
pub(super) fn place(layout: &mut Layout, derived: &Derived, root: usize) {
    for at in post_order(layout, root) {
        layout_block(layout, derived, at);
    }
}

/// `layout_block` then `position`, in the reference's order.
fn layout_block(layout: &mut Layout, derived: &Derived, at: usize) {
    let order = layout.circle_of(derived, at);
    let children = layout.blocks[at].children.clone();
    let count = order.len();
    let radius = circle::radius_of(count);
    let rad0 = if count == 1 {
        circle::single_radius()
    } else {
        radius
    };
    {
        let block = &mut layout.blocks[at];
        block.circle = order.clone();
        block.rad0 = rad0;
        block.radius = rad0;
        block.parent_pos = -1.0;
    }
    for (k, &local) in order.iter().enumerate() {
        let node = layout.blocks[at].nodes[local as usize] as usize;
        let theta = circle::angle_of(k, count);
        layout.x[node] = radius * libm::cos(theta);
        layout.y[node] = radius * libm::sin(theta);
        layout.psi[node] = 0.0;
    }
    if children.is_empty() {
        return;
    }
    let angle = position(layout, at, &order, &children);
    if count == 1 && layout.blocks[at].parent.is_some() {
        let angle = if angle < 0.0 { angle + TAU } else { angle };
        layout.blocks[at].parent_pos = angle;
    }
}

/// `post_order` over the block tree: every child before its parent, which is the order
/// `doBlock`'s recursion visits in.
fn post_order(layout: &Layout, root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![(root, 0usize)];
    while let Some(&(at, child)) = stack.last() {
        if child < layout.blocks[at].children.len() {
            let next = layout.blocks[at].children[child];
            stack.last_mut().expect("the stack is not empty").1 += 1;
            stack.push((next, 0));
        } else {
            out.push(at);
            stack.pop();
        }
    }
    out
}

/// The per-block bookkeeping of `position` (`circpos.c:157-165`).
struct PosState {
    radius: f64,
    subtree: f64,
    node_angle: f64,
    first_angle: f64,
    last_angle: f64,
    /// `CHILD(sn)`: the node in this block its children hang off.
    neighbor: u32,
}

/// One node of the block that has children: its arc and the radius its children fit at.
#[derive(Clone)]
struct PosInfo {
    node: u32,
    theta: f64,
    min_radius: f64,
    max_radius: f64,
    diameter: f64,
    scale: f64,
    child_count: u32,
}

/// `position` (`circpos.c:307-390`): every child's placement, and the angle this block's own
/// parent should sit at.
fn position(layout: &mut Layout, at: usize, order: &[u32], children: &[usize]) -> f64 {
    let count = order.len();
    let mut state = PosState {
        radius: layout.blocks[at].radius,
        subtree: layout.blocks[at].radius,
        node_angle: TAU / count as f64,
        first_angle: -1.0,
        last_angle: -1.0,
        neighbor: layout.blocks[at].child_node,
    };
    let mut parents = Vec::new();
    let mut max_radius = 0.0;
    for (counter, &local) in order.iter().enumerate() {
        let node = layout.blocks[at].nodes[local as usize];
        if layout.is_parent(node) {
            let info = arc_for(
                layout,
                children,
                node,
                counter as f64 * state.node_angle,
                state.radius,
            );
            max_radius = info.max_radius;
            parents.push(info);
        }
    }
    scale_arcs(&mut parents);
    for info in &parents {
        place_children(layout, info, &mut state, children, count);
    }
    if children.len() == 1 {
        apply_delta(layout, at, -(max_radius + MIN_DIST / 2.0), 0.0, 0.0);
        let block = &mut layout.blocks[at];
        block.radius += MIN_DIST / 2.0 + max_radius;
        block.coalesced = true;
    } else {
        layout.blocks[at].radius = state.subtree;
    }
    (state.first_angle + state.last_angle) / 2.0 - PI
}

/// `getInfo` (`circpos.c:178-199`): the arc `node`'s children need, and the radius they fit at.
fn arc_for(layout: &Layout, children: &[usize], node: u32, theta: f64, radius: f64) -> PosInfo {
    let (mut max_radius, mut diameter, mut child_count) = (0.0f64, 0.0f64, 0u32);
    for &child in children {
        if layout.blocks[child].hangs_at == node {
            child_count += 1;
            let child_radius = layout.blocks[child].radius;
            max_radius = max_radius.max(child_radius);
            diameter += 2.0 * child_radius + MIN_DIST;
        }
    }
    PosInfo {
        node,
        theta,
        min_radius: radius + MIN_DIST + max_radius,
        max_radius,
        diameter,
        scale: 0.0,
        child_count,
    }
}

/// The scale the arcs agree on (`circpos.c:343-367`). One arc takes 1; two reconcile against
/// each other over the shorter way round; three or more each reconcile against their clockwise
/// neighbour, the last one against the first across the full turn.
fn scale_arcs(parents: &mut [PosInfo]) {
    let count = parents.len();
    match count {
        0 => {}
        1 => parents[0].scale = 1.0,
        2 => {
            let gap = parents[1].theta - parents[0].theta;
            let delta = if gap > PI { TAU - gap } else { gap };
            let (mut left, mut right) = (parents[0].clone(), parents[1].clone());
            reconcile(&mut left, &mut right, delta);
            parents[0].scale = left.scale;
            parents[1].scale = right.scale;
        }
        _ => {
            for at in 0..count {
                let next = if at + 1 == count { 0 } else { at + 1 };
                let turn = if at + 1 == count { TAU } else { 0.0 };
                let delta = parents[next].theta - parents[at].theta + turn;
                let (mut left, mut right) = (parents[at].clone(), parents[next].clone());
                reconcile(&mut left, &mut right, delta);
                parents[at].scale = parents[at].scale.max(left.scale);
                parents[next].scale = parents[next].scale.max(right.scale);
            }
        }
    }
}

/// `setInfo(p0, p1, delta)`: the scale two arcs agree on.
fn reconcile(p0: &mut PosInfo, p1: &mut PosInfo, delta: f64) {
    let t = (p0.diameter * p1.min_radius + p1.diameter * p0.min_radius)
        / (2.0 * delta * p0.min_radius * p1.min_radius);
    let t = t.max(1.0);
    p0.scale = p0.scale.max(t);
    p1.scale = p1.scale.max(t);
}

/// `positionChildren` (`circpos.c:214-297`): the children of one node, at the arc's radius and
/// one after another along it.
fn place_children(
    layout: &mut Layout,
    info: &PosInfo,
    state: &mut PosState,
    children: &[usize],
    count: usize,
) {
    let mut child_radius = info.scale * info.min_radius;
    let mut gap = MIN_DIST;
    let mut child_angle;
    if count == 1 {
        child_angle = 0.0;
        child_radius = child_radius.max(info.diameter / TAU);
        let slack = TAU * child_radius - info.diameter;
        if slack > 0.0 {
            gap += slack / f64::from(info.child_count);
        }
    } else {
        child_angle = info.theta - info.diameter / (2.0 * child_radius);
    }
    let subtree = state.subtree.max(child_radius + info.max_radius);
    let mindist_angle = gap / child_radius;
    let middle = info.child_count.div_ceil(2) as usize;
    let (mut seen, mut mid_angle) = (0usize, 0.0f64);
    for &child in children {
        if layout.blocks[child].hangs_at != info.node || layout.blocks[child].circle.is_empty() {
            continue;
        }
        let incident = layout.blocks[child].radius / child_radius;
        child_angle = if count == 1 {
            solo_angle(child_angle, incident, info.child_count, state)
        } else if info.child_count == 1 {
            info.theta
        } else {
            child_angle + incident + mindist_angle / 2.0
        };
        let (dx, dy) = (
            child_radius * libm::cos(child_angle),
            child_radius * libm::sin(child_angle),
        );
        let rotate = get_rotation(layout, child, dx, dy, child_angle);
        apply_delta(layout, child, dx, dy, rotate);
        child_angle += incident
            + if count == 1 {
                mindist_angle
            } else {
                mindist_angle / 2.0
            };
        seen += 1;
        if seen == middle {
            mid_angle = child_angle;
        }
    }
    if count > 1 && info.node == state.neighbor {
        layout.psi[info.node as usize] = mid_angle;
    }
    state.subtree = subtree;
}

/// The one-node-block branch of `positionChildren`'s sweep: the first child sits at the slot
/// angle and records the extremes, the rest step on — except a pair, which is placed opposite.
fn solo_angle(angle: f64, incident: f64, child_count: u32, state: &mut PosState) -> f64 {
    let angle = if angle == 0.0 {
        angle
    } else if child_count == 2 {
        PI
    } else {
        angle + incident
    };
    if state.first_angle < 0.0 {
        state.first_angle = angle;
    }
    state.last_angle = angle;
    angle
}
