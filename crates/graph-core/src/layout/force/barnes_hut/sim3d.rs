//! The 3D simulation state the `layout.force.yifan_hu.3d` arm runs: dense three-axis
//! positions and velocities, the two Barnes-Hut **octrees** (one over current positions for
//! many-body, one over projected `p + v` positions for collide, matching `sim.rs`'s own
//! two quadtrees), and the frozen per-edge link geometry.
//!
//! **A second tick, not a parameterised one.** `sim.rs`'s tick is pinned by the 65 frozen
//! force-session digests and by every 2D row of the hash gate, and its arithmetic is two
//! columns wide in a dozen places; threading a dimension through it would move all of them.
//! So this is a sibling tick beside it, over [`octree`](crate::layout::force::octree)
//! instead of the quadtree, and the two share no line — which is the same bargain
//! `octree.rs` strikes with the quadtree and the reason it is written out.
//!
//! What is *not* re-invented: the tick order (link, many-body, center, collide, integrate),
//! the `alpha` decay in d3's own form, the Jacobi link gather with the degree-bias split,
//! the collide pass's `x + vx` projection keys, and the `velocityDecay` tail. A reader who
//! knows `sim.rs` can check this one line by line; the third axis is the only difference.
//!
//! Two departures, both stated where they happen:
//!
//! - **no pins and no gravity.** The 2D tick carries `node.fx`/`node.fy` and a gravity
//!   force because a *live session* needs them. This tick backs a frozen multilevel solve
//!   only, so both are absent rather than present-and-zero: applying a force at zero is
//!   explicitly not the same bytes as not applying it (`sim.rs`'s gravity comment).
//! - **no threaded tier.** Every pass here runs on one thread in one fixed order, so the
//!   arm has no worker-count dimension to hash-equal across. That is the same position
//!   `layout.force.fruchterman_reingold.3d` and the other dense 3D arms are in.

use crate::index::Topology;
use crate::layout::force::octree::charge::Body;
use crate::layout::force::octree::{Octree, Points3};
use crate::layout::force::{LiveParams, SimpleGraph};

/// The 3D simulation state: dense columns, two octrees, and the frozen link geometry.
#[derive(Debug, Clone)]
pub(in crate::layout::force) struct Sim3 {
    pub(in crate::layout::force) graph: SimpleGraph,
    pub(in crate::layout::force) params: LiveParams,
    pub(in crate::layout::force) seed: u32,
    pub(in crate::layout::force) tick_no: u32,
    pub(in crate::layout::force) alpha: f64,
    /// d3's `alphaTarget`, zero here as it is for the frozen layout.
    pub(in crate::layout::force) alpha_target: f64,
    pub(in crate::layout::force) x: Vec<f64>,
    pub(in crate::layout::force) y: Vec<f64>,
    pub(in crate::layout::force) z: Vec<f64>,
    pub(in crate::layout::force) vx: Vec<f64>,
    pub(in crate::layout::force) vy: Vec<f64>,
    pub(in crate::layout::force) vz: Vec<f64>,
    /// This tick's projected positions, the octree's collide keys.
    pub(in crate::layout::force) px: Vec<f64>,
    pub(in crate::layout::force) py: Vec<f64>,
    pub(in crate::layout::force) pz: Vec<f64>,
    pub(in crate::layout::force) charge_tree: Octree,
    pub(in crate::layout::force) collide_tree: Octree,
    pub(in crate::layout::force) bodies: Vec<Body>,
    pub(in crate::layout::force) link_distance: Vec<f64>,
    pub(in crate::layout::force) link_strength: Vec<f64>,
    pub(in crate::layout::force) link_bias: Vec<f64>,
    /// Each simple edge's force this tick, the link pass's scratch.
    pub(in crate::layout::force) link_forces: Vec<(f64, f64, f64)>,
}

impl Sim3 {
    /// A simulation over `topology` from the 3D golden sphere.
    pub(in crate::layout::force) fn new(
        topology: &Topology,
        params: LiveParams,
        seed: u32,
    ) -> Self {
        let graph = crate::layout::force::simple_graph(topology);
        let (x, y, z) = super::seed::golden_sphere(topology.node_count());
        Self::from_parts(graph, params, seed, (x, y, z))
    }

    /// A simulation over `graph` starting from the given positions (one per node).
    pub(in crate::layout::force) fn from_parts(
        graph: SimpleGraph,
        params: LiveParams,
        seed: u32,
        start: (Vec<f64>, Vec<f64>, Vec<f64>),
    ) -> Self {
        unimplemented!("RED: Sim3::from_parts")
    }

    /// How many node columns a row may name.
    pub(in crate::layout::force) fn rows(&self) -> u32 {
        self.x.len() as u32
    }

    /// The three position columns as the octree takes them.
    pub(in crate::layout::force) fn points(&self) -> Points3<'_> {
        Points3 {
            xs: &self.x,
            ys: &self.y,
            zs: &self.z,
        }
    }

    /// The three projected columns as the octree takes them.
    pub(in crate::layout::force) fn projected(&self) -> Points3<'_> {
        Points3 {
            xs: &self.px,
            ys: &self.py,
            zs: &self.pz,
        }
    }

    /// One tick: `alpha` decays first, then link, many-body, center and collide run in the
    /// engine's registration order, then the velocities integrate into position.
    pub(in crate::layout::force) fn tick(&mut self) {
        unimplemented!("RED: Sim3::tick")
    }

    /// `center.js` over three axes: shifts every position by its own mean, toward the
    /// origin — position, not velocity, and with no `alpha` scaling.
    pub(in crate::layout::force) fn center(&mut self) {
        let Some((dx, dy, dz)) = self.center_shift() else {
            return;
        };
        for i in 0..self.x.len() {
            self.x[i] -= dx;
            self.y[i] -= dy;
            self.z[i] -= dz;
        }
    }

    /// What [`center`](Self::center) subtracts: each axis' own mean position, scaled by the
    /// centering strength, folded in node order. `None` for no nodes.
    pub(in crate::layout::force) fn center_shift(&self) -> Option<(f64, f64, f64)> {
        let n = self.x.len();
        if n == 0 {
            return None;
        }
        let mut sum = (0.0, 0.0, 0.0);
        for i in 0..n {
            sum.0 += self.x[i];
            sum.1 += self.y[i];
            sum.2 += self.z[i];
        }
        let s = self.params.center_strength;
        Some((
            (sum.0 / n as f64) * s,
            (sum.1 / n as f64) * s,
            (sum.2 / n as f64) * s,
        ))
    }

    /// `simulation.js`'s tick tail over three axes: a free axis integrates
    /// `node.p += node.v *= velocityDecay`. There are no pins here (see the module header).
    pub(in crate::layout::force) fn integrate(&mut self) {
        let decay = self.params.velocity_decay;
        for i in 0..self.x.len() {
            self.vx[i] *= decay;
            self.x[i] += self.vx[i];
            self.vy[i] *= decay;
            self.y[i] += self.vy[i];
            self.vz[i] *= decay;
            self.z[i] += self.vz[i];
        }
    }
}