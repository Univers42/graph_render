//! The 3D many-body force over the octree — the octant's counterpart of
//! `barnes_hut/charge.rs`, in the octree's own module so the tree and the walk that reads
//! it are edited and tested together.
//!
//! It is a second implementation, not a shared one, for the reason `octree.rs`'s header
//! gives: sharing the walk would have meant sharing the tree's cell bounds, and the 2D
//! walk's bytes are pinned by the 65 frozen session digests.
//!
//! What is *not* re-invented: the theta test is the quadtree's (`open >= l`, approximation
//! in the `else`), the aggregate runs in reverse preorder, a query walks cells front to
//! back and jumps past an approximated subtree, and the coincident chain's exact terms are
//! accumulated in chain order — all of it, in the same order, so a reader who knows the 2D
//! walk can check the 3D one line by line. The three differences are stated where they
//! happen: `PASS_Z` is a third jiggle lane, `Gap` carries `dz`, and the mass-weighted
//! centre divides a third accumulator.

pub(super) mod threshold;

use super::preorder::Cell;
use super::{Octree, Points3};
use crate::layout::force::LiveParams;
use crate::rng::jiggle;

/// The jiggle lanes. They are `PASS_X`/`PASS_Y` of the 2D walk **shifted up by two** so a
/// 3D key can never collide with a 2D one for the same `(seed, tick, i, cell)` — the lanes
/// are an input to the hash, not a label.
const PASS_X: u32 = 2;
const PASS_Y: u32 = 3;
const PASS_Z: u32 = 6;

/// One tree cell as the walk reads it.
///
/// The mass is `count`, a point count, exactly as in 2D: whole counts below 2^53 sum
/// exactly, so `f64::from(count)` is the same number. `open` is `w² / θ²`, the cell's
/// opening threshold over the cube's edge `w`, computed once per tick rather than once per
/// visit; a query at squared distance `l` opens the cell when `open >= l`.
#[derive(Debug, Clone, Copy, Default)]
pub(in crate::layout::force) struct Body {
    pub(in crate::layout::force::octree) comx: f64,
    pub(in crate::layout::force::octree) comy: f64,
    pub(in crate::layout::force::octree) comz: f64,
    pub(in crate::layout::force::octree) open: f64,
    pub(in crate::layout::force::octree) count: u32,
    pub(in crate::layout::force::octree) skip: u32,
    pub(in crate::layout::force::octree) start: u32,
}

impl Body {
    /// The body one cell gets, from the centre its own subtree resolved to, its opening
    /// threshold and the cell's own extent.
    pub(super) fn of(com: (f64, f64, f64), open: f64, cell: &Cell) -> Self {
        Body {
            comx: com.0,
            comy: com.1,
            comz: com.2,
            open,
            count: cell.end - cell.start,
            skip: cell.skip,
            start: cell.start,
        }
    }
}

/// The numbers a query reads that do not depend on the query: the two distance cutoffs, the
/// charge scalar, the tick's heat, and the jiggle's seed and tick.
#[derive(Debug, Clone, Copy)]
pub(in crate::layout::force) struct Terms {
    pub(in crate::layout::force::octree) dmin2: f64,
    pub(in crate::layout::force::octree) dmax2: f64,
    pub(in crate::layout::force::octree) charge: f64,
    pub(in crate::layout::force::octree) alpha: f64,
    pub(in crate::layout::force::octree) seed: u32,
    pub(in crate::layout::force::octree) tick: u32,
}

impl Terms {
    /// The terms one tick runs with. `(alpha, seed, tick)` is bundled so the call stays at
    /// two parameters (`prompt.md` §0's cap).
    pub(in crate::layout::force) fn of(params: &LiveParams, state: (f64, u32, u32)) -> Self {
        Terms {
            dmin2: params.distance_min * params.distance_min,
            dmax2: params.distance_max * params.distance_max,
            charge: params.charge,
            alpha: state.0,
            seed: state.1,
            tick: state.2,
        }
    }
}

/// Bottom-up mass and centre of mass per cell, in reverse preorder so every child is final
/// before its parent reads it. The order is the 2D walk's, and it is the whole reason the
/// aggregate is not parallelisable here.
pub(in crate::layout::force) fn aggregate(
    tree: &Octree,
    pts: Points3<'_>,
    theta: f64,
    bodies: &mut Vec<Body>,
) {
    use threshold::{centre, opening_threshold};
    let theta2 = theta * theta;
    let (cells, order) = (tree.cells(), tree.order());
    bodies.clear();
    bodies.resize(cells.len(), Body::default());
    for k in (0..cells.len()).rev() {
        let cell = cells[k];
        let com = if cell.skip == k as u32 + 1 {
            let head = order[cell.start as usize] as usize;
            (pts.xs[head], pts.ys[head], pts.zs[head])
        } else {
            centre(bodies, k as u32 + 1, cell.skip)
        };
        let w = cell.bounds.x1 - cell.bounds.x0;
        bodies[k] = Body::of(com, opening_threshold(w, theta, theta2), &cell);
    }
}

/// Everything a query reads but does not own, gathered once per tick rather than once per
/// node. Four fields so [`Walk::new`] stays inside the parameter cap.
pub(in crate::layout::force) struct Walk<'a> {
    bodies: &'a [Body],
    tree: &'a Octree,
    pts: Points3<'a>,
    terms: Terms,
}

impl<'a> Walk<'a> {
    pub(in crate::layout::force) fn new(
        bodies: &'a [Body],
        tree: &'a Octree,
        pts: Points3<'a>,
        terms: Terms,
    ) -> Self {
        Walk {
            bodies,
            tree,
            pts,
            terms,
        }
    }

    /// Node `i`'s own many-body delta, visited in the tree's preorder.
    ///
    /// The 2D walk's own loop: a cell the opening angle resolves as one blob adds its delta
    /// and jumps past its subtree; a cell too close descends, or, at a leaf, adds its
    /// chain's exact terms. The test is written `open >= l` with the approximation in the
    /// `else`, so a `NaN` distance approximates as it always did.
    pub(in crate::layout::force) fn node(&self, i: u32) -> (f64, f64, f64) {
        let q = Query {
            i,
            p: self.pts.at(i),
        };
        let mut out = (0.0, 0.0, 0.0);
        let mut k = 0;
        while let Some(body) = self.bodies.get(k as usize) {
            let (dx, dy, dz) = (body.comx - q.p.0, body.comy - q.p.1, body.comz - q.p.2);
            let gap = Gap {
                dx,
                dy,
                dz,
                l: dx * dx + dy * dy + dz * dz,
            };
            let (delta, next) = if body.open >= gap.l {
                if body.skip != k + 1 {
                    k += 1;
                    continue;
                }
                (direct(self, &q, body, gap), k + 1)
            } else {
                (approx(self, &q, k, gap), body.skip)
            };
            out.0 += delta.0;
            out.1 += delta.1;
            out.2 += delta.2;
            k = next;
        }
        out
    }
}

pub(super) struct Query {
    pub(in crate::layout::force::octree) i: u32,
    pub(in crate::layout::force::octree) p: (f64, f64, f64),
}

/// The offset from the query to a cell's centre and its squared length.
#[derive(Clone, Copy, Debug)]
pub(in crate::layout::force::octree) struct Gap {
    pub(in crate::layout::force::octree) dx: f64,
    pub(in crate::layout::force::octree) dy: f64,
    pub(in crate::layout::force::octree) dz: f64,
    pub(in crate::layout::force::octree) l: f64,
}

/// Cell `k` resolved as one blob of its whole mass; zero past `distanceMax`. The jiggle is
/// keyed on the cell's arena node id, the key the 2D walk has always used.
pub(super) fn approx(walk: &Walk<'_>, q: &Query, k: u32, gap: Gap) -> (f64, f64, f64) {
    let t = walk.terms;
    let Some(Gap { dx, dy, dz, l }) = settle(&t, q, walk.tree.node_id(k), gap) else {
        return (0.0, 0.0, 0.0);
    };
    let m = f64::from(walk.bodies[k as usize].count);
    let f = m * t.charge * t.alpha / l;
    (dx * f, dy * f, dz * f)
}

/// A leaf too close (or too coarse) to approximate: every distinct chained point
/// contributes its own charge directly, unless `distanceMax` has already been reached.
pub(super) fn direct(walk: &Walk<'_>, q: &Query, body: &Body, gap: Gap) -> (f64, f64, f64) {
    let chain = &walk.tree.order()[body.start as usize..][..body.count as usize];
    let t = walk.terms;
    let Some(Gap { dx, dy, dz, l }) = settle(&t, q, chain[0], gap) else {
        return (0.0, 0.0, 0.0);
    };
    let mut dv = (0.0, 0.0, 0.0);
    for &p in chain {
        if p == q.i {
            continue;
        }
        let f = t.charge * t.alpha / l;
        dv.0 += dx * f;
        dv.1 += dy * f;
        dv.2 += dz * f;
    }
    dv
}

/// The tail both ends of the 2D walk's `apply` share, with the third axis: `None` past
/// `distanceMax`, else the gap with every exactly-zero axis jiggled (keyed on `(q.i, key)`)
/// and `l` clamped up to `distanceMin`.
pub(in crate::layout::force::octree) fn settle(
    terms: &Terms,
    q: &Query,
    key: u32,
    gap: Gap,
) -> Option<Gap> {
    let (dx, dy, dz, l0) = (gap.dx, gap.dy, gap.dz, gap.l);
    if l0 >= terms.dmax2 {
        return None;
    }
    let (dx, dy, dz, l) = jiggled(terms, q, key, (dx, dy, dz, l0));
    let mut l = l;
    if l < terms.dmin2 {
        l = f64::sqrt(terms.dmin2 * l);
    }
    if l == 0.0 {
        // Ponytail: `jiggle` maps exactly one 53-bit word to `0.0`, so all three axes of a
        // fully coincident pair can come back zero and the division goes infinite, then
        // `NaN`. It costs one word in 2^53 of the keys, and it gets the whole pair wrong
        // rather than one. Escape hatch: give the two points distinct coordinates.
        l = terms.dmin2.max(f64::MIN_POSITIVE);
    }
    Some(Gap { dx, dy, dz, l })
}

/// Every exactly-zero axis replaced by its jiggle and `l` re-summed, in axis order `x`, `y`,
/// `z`.
///
/// The three lanes are the 2D walk's `PASS_X`/`PASS_Y` **shifted up by two**, and `PASS_Z` is
/// six, so a 3D key can never collide with a 2D one for the same `(seed, tick, i, cell)` —
/// the lane is an input to the hash, not a label.
fn jiggled(terms: &Terms, q: &Query, key: u32, gap: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let (mut dx, mut dy, mut dz, mut l) = gap;
    let pair = (q.i, key);
    if dx == 0.0 {
        dx = jiggle(terms.seed, terms.tick, PASS_X, pair);
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(terms.seed, terms.tick, PASS_Y, pair);
        l += dy * dy;
    }
    if dz == 0.0 {
        dz = jiggle(terms.seed, terms.tick, PASS_Z, pair);
        l += dz * dz;
    }
    (dx, dy, dz, l)
}
