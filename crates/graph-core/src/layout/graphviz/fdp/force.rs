//! The attractive force and the position update, one tick of `gAdjust` (`tlayout.c:358-381`).
//!
//! `gAdjust` is three ordered steps and this module is those three steps in that order:
//! zero the displacements, bin every node into the grid, walk the edges applying
//! `applyAttr`, then walk the grid applying the repulsion, then move. The repulsion is
//! [`Grid::repel`](super::grid::Grid::repel)'s and is called by the caller, so this module
//! owns the two halves that are pure functions of the edge list.
//!
//! `applyAttr` with `useNew` set (`tlayout.c:301-304`) is
//! `force = weight * (dist - len) / dist`, and both `weight` and `len` are the reference's
//! defaults — `ED_factor` is the `weight` attribute, absent, so 1.0; `ED_dist` is the `len`
//! attribute, absent, so `fdp_parms->K` (`fdpinit.c:72`). The motor publishes no attribute
//! channel to a layout at its default parameters, so both are constants here.

use super::K;
use super::grid::Grid;
use super::model::Model;
use super::rng::GlibcRand;

/// One tick: attract along every edge, repel through the grid, then move.
pub(super) fn tick(model: &mut Model, grid: &mut Grid, temp: f64, rand: &mut GlibcRand) {
    model.clear_displacement();
    grid.fill(model);
    attract(model, rand);
    grid.repel(model, rand);
    move_nodes(model, temp);
}

/// `applyAttr` over the whole edge list, in the reference's own two-level order.
///
/// `gAdjust` walks the nodes, and for each node walks **its** out-edges
/// (`tlayout.c:373-377`) — not the flat edge list. That is the order the additions land
/// in, so it is the order here: outer loop over nodes, inner loop over that node's
/// out-edge heads. The reference's `if (n != aghead(e))` self-loop guard is already done by
/// the deduplication in [`Model::edges`](super::model::Model), and each unordered pair is
/// applied exactly once because every edge is oriented low-to-high.
fn attract(model: &mut Model, rand: &mut GlibcRand) {
    for p in 0..model.count {
        for slot in model.out_at[p as usize]..model.out_at[p as usize + 1] {
            let q = model.out[slot as usize];
            apply_attr(p, q, model, rand);
        }
    }
}

/// `applyAttr` (`tlayout.c:286-309`): pull `q` toward `p` by `weight * (dist - len) / dist`.
fn apply_attr(p: u32, q: u32, model: &mut Model, rand: &mut GlibcRand) {
    let (p, q) = (p as usize, q as usize);
    let (mut dx, mut dy) = (model.x[q] - model.x[p], model.y[q] - model.y[p]);
    let mut dist2 = dx * dx + dy * dy;
    while dist2 == 0.0 {
        dx = rand.jitter();
        dy = rand.jitter();
        dist2 = dx * dx + dy * dy;
    }
    let dist = dist2.sqrt();
    let force = (dist - K) / dist;
    model.dx[q] -= dx * force;
    model.dy[q] -= dy * force;
    model.dx[p] += dx * force;
    model.dy[p] += dy * force;
}

/// `updatePos` (`tlayout.c:311-354`) with no ports: move by the displacement, capped at
/// the temperature.
///
/// The cap is a compare against `temp^2` and a `temp / sqrt(len2)` scale, which is the
/// reference's own way of avoiding a division by zero — there is no separate zero test and
/// none is added here, because adding one would change the answer on the tick where it
/// fires.
fn move_nodes(model: &mut Model, temp: f64) {
    let temp2 = temp * temp;
    for i in 0..model.count as usize {
        let (dx, dy) = (model.dx[i], model.dy[i]);
        let len2 = dx * dx + dy * dy;
        if len2 < temp2 {
            model.x[i] += dx;
            model.y[i] += dy;
        } else {
            let fact = temp / len2.sqrt();
            model.x[i] += dx * fact;
            model.y[i] += dy * fact;
        }
    }
}
