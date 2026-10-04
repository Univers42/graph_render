//! The 3D collide force: the 2D pass's Jacobi gather over the **octree** rather than the
//! quadtree, over three axes.
//!
//! It is the 2D pass line for line — this tick's projected positions `p + v` are the tree's
//! keys, an internal cell wholly outside the reach is skipped whole, a leaf contributes each
//! of its chained points, both directions of an overlap derive their delta from one shared
//! subtraction negated, and the split is the symmetric 50/50 one the equal radii give.
//!
//! **Why the octree rather than a dense pair scan.** A dense scan would be `O(n²)` per tick
//! against the 2D pass's `O(n log n)`, and this arm runs the same 112 ticks per level over
//! up to 12 levels. The tree is already built for many-body, so a second instance over the
//! projected columns costs one more build and nothing else.

use super::sim3d::Sim3;
use crate::layout::force::octree::Bounds3;
use crate::rng::jiggle;

const PASS_X: u32 = 12;
const PASS_Y: u32 = 13;
const PASS_Z: u32 = 14;

/// The whole pass: project, build, then one gather per node and one addition onto its own
/// velocity. The gather's outputs are in the collide tree's point order, exactly as the 2D
/// pass's are, and each node receives exactly one addition, so the order moves no byte.
pub(in crate::layout::force) fn apply(sim: &mut Sim3) {
    prepare(sim);
    let d2 = reach_squared(sim);
    let order: Vec<u32> = sim.collide_tree.order().to_vec();
    for &i in &order {
        let d = node_delta(sim, i, d2);
        let i = i as usize;
        sim.vx[i] += d.0;
        sim.vy[i] += d.1;
        sim.vz[i] += d.2;
    }
}

/// The single-threaded prologue: this tick's projected positions, and the octree over them.
fn prepare(sim: &mut Sim3) {
    for i in 0..sim.x.len() {
        sim.px[i] = sim.x[i] + sim.vx[i];
        sim.py[i] = sim.y[i] + sim.vy[i];
        sim.pz[i] = sim.z[i] + sim.vz[i];
    }
    sim.collide_tree.build(sim.projected());
}

/// `(2 * collideRadius)²`: the squared diameter two nodes must be closer than to overlap.
fn reach_squared(sim: &Sim3) -> f64 {
    let diameter = 2.0 * sim.params.collide_radius;
    diameter * diameter
}

/// Node `i`'s own delta: a stackless preorder walk of the collide octree, jumping past every
/// internal cell wholly outside the reach and resolving every leaf it lands on.
pub(in crate::layout::force) fn node_delta(sim: &Sim3, i: u32, d2: f64) -> (f64, f64, f64) {
    let q = Query {
        i,
        p: (sim.px[i as usize], sim.py[i as usize], sim.pz[i as usize]),
        d2,
        reach: f64::sqrt(d2),
        seed: sim.seed,
        tick: sim.tick_no,
        pts: sim.projected(),
    };
    let (cells, order) = (sim.collide_tree.cells(), sim.collide_tree.order());
    let mut out = (0.0, 0.0, 0.0);
    let mut k = 0;
    while let Some(cell) = cells.get(k as usize) {
        k = if cell.skip == k + 1 {
            for &p in &order[cell.start as usize..cell.end as usize] {
                if p != q.i {
                    resolve(&q, p, &mut out);
                }
            }
            k + 1
        } else if outside(cell.bounds, &q) {
            cell.skip
        } else {
            k + 1
        };
    }
    out
}

/// Everything node `q.i`'s query reads but does not own.
pub(in crate::layout::force) struct Query<'a> {
    i: u32,
    p: (f64, f64, f64),
    d2: f64,
    reach: f64,
    seed: u32,
    tick: u32,
    pts: crate::layout::force::octree::Points3<'a>,
}

/// An internal cell lies wholly outside the reach of `q`'s position, on any of the three
/// axes. The three tests are the 2D pair plus the z one, and they are `||`ed rather than
/// chained so a cell that is outside in z alone is skipped.
fn outside(b: Bounds3, q: &Query<'_>) -> bool {
    let r = q.reach;
    b.x0 > q.p.0 + r
        || b.x1 < q.p.0 - r
        || b.y0 > q.p.1 + r
        || b.y1 < q.p.1 - r
        || b.z0 > q.p.2 + r
        || b.z1 < q.p.2 - r
}

/// `q.i`'s own half of the overlap correction against neighbour `p` (zero if they do not
/// overlap), over three axes.
fn resolve(q: &Query<'_>, p: u32, out: &mut (f64, f64, f64)) {
    let (px, py, pz) = q.pts.at(p);
    let (mut dx, mut dy, mut dz) = (q.p.0 - px, q.p.1 - py, q.p.2 - pz);
    let mut l = dx * dx + dy * dy + dz * dz;
    if l >= q.d2 {
        return;
    }
    let key = (q.i, p);
    if dx == 0.0 {
        dx = jiggle(q.seed, q.tick, PASS_X, key);
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(q.seed, q.tick, PASS_Y, key);
        l += dy * dy;
    }
    if dz == 0.0 {
        dz = jiggle(q.seed, q.tick, PASS_Z, key);
        l += dz * dz;
    }
    let dist = f64::sqrt(l);
    let push = (q.reach - dist) / dist * 0.5;
    out.0 += dx * push;
    out.1 += dy * push;
    out.2 += dz * push;
}