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
use crate::layout::force::octree::charge::{self as octree_charge, Terms, Walk};
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
    pub(in crate::layout::force) bodies: Vec<octree_charge::Body>,
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
        (x, y, z): (Vec<f64>, Vec<f64>, Vec<f64>),
    ) -> Self {
        let n = x.len();
        let (link_distance, link_strength, link_bias) = super::link::geometry(&graph, &params);
        Sim3 {
            alpha: params.initial_alpha,
            alpha_target: 0.0,
            graph,
            params,
            seed,
            tick_no: 0,
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            vz: vec![0.0; n],
            px: vec![0.0; n],
            py: vec![0.0; n],
            pz: vec![0.0; n],
            charge_tree: Octree::default(),
            collide_tree: Octree::default(),
            bodies: Vec::new(),
            x,
            y,
            z,
            link_distance,
            link_strength,
            link_bias,
            link_forces: Vec::new(),
        }
    }

    /// How many node columns a row may name.
    pub(in crate::layout::force) fn rows(&self) -> u32 {
        self.x.len() as u32
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
        self.alpha += (self.alpha_target - self.alpha) * self.params.alpha_decay;
        super::link3d::apply(self);
        self.charge();
        self.center();
        super::collide3d::apply(self);
        self.integrate();
        self.tick_no += 1;
    }

    /// The many-body pass over the **octree**: build, aggregate bottom-up in reverse
    /// preorder, then one gather per node in the tree's own point order, each node
    /// receiving exactly one addition onto its own velocity.
    ///
    /// Destructured so the tree and body borrows are disjoint from the three velocity
    /// columns the gathers write.
    fn charge(&mut self) {
        let Sim3 {
            x,
            y,
            z,
            vx,
            vy,
            vz,
            params,
            alpha,
            seed,
            tick_no,
            charge_tree,
            bodies,
            ..
        } = self;
        let pts = Points3 { xs: x, ys: y, zs: z };
        charge_tree.build(pts);
        octree_charge::aggregate(charge_tree, pts, params.theta, bodies);
        let terms = Terms::of(params, (*alpha, *seed, *tick_no));
        let walk = Walk::new(bodies, charge_tree, pts, terms);
        for &i in charge_tree.order() {
            let d = walk.node(i);
            let i = i as usize;
            vx[i] += d.0;
            vy[i] += d.1;
            vz[i] += d.2;
        }
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