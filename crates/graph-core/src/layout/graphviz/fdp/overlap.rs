//! The overlap-removal phase `fdp_xLayout` runs once the expansion has settled
//! (`xlayout.c`).
//!
//! The reference reads its `overlap` attribute, finds `DFLT_overlap = "9:prism"`
//! (`xlayout.c:33`), and so runs `x_layout` with nine tries and then hands the graph to
//! `removeOverlapAs(g, "prism")`. **The prism packing is not ported** — see the Ponytail
//! marker on the module header, which names the failing input, the direction of the
//! failure and the escape hatch. The nine `x_layout` tries are ported, and they are what
//! carries the repulsion constants.
//!
//! The forces here are not `fdp`'s spring forces. `applyRep` is `X_ov / d^2` for a
//! *touching* pair and `X_nonov / d^2` for a clear one, and `applyAttr` is a
//! separation-squared law that acts only on pairs which are **not** touching
//! (`xlayout.c:160-186`) — an overlap is pushed apart, never pulled in. Both constants are
//! computed once per try from that try's `K`, and `x_layout` raises `K` by `K` after every
//! try (`xlayout.c:283`), so try `t` works at `K * (t + 1)`.

use super::model::Model;
use super::rng::GlibcRand;
use super::{K, OVERLAP_TRIES, REPULSION_C, SEP_INCH, X_ITERS};

/// One try's fixed quantities, so no call here takes more than the four parameters the
/// house allows. `t0` is the *initial* temperature of the phase, which `init_params` fixed
/// once at `T_T0 / 2` (`tlayout.c:132`) and which every try shares; only `k` changes.
struct Try {
    /// `xParams.K` for this try: the separation scale, raised by `K` each round.
    k: f64,
    /// `X_ov`: the repulsion applied to a pair whose boxes touch.
    touching: f64,
    /// `X_nonov`: the repulsion applied to a pair whose boxes do not.
    clear: f64,
}

impl Try {
    /// The reference's per-try constants (`xlayout.c:263-265`). `X_nonov` is the touching
    /// force spread over all the node pairs, so it falls as `1 / n^2`.
    fn new(model: &Model, round: u32) -> Self {
        let k = K * (1.0 + f64::from(round));
        let touching = REPULSION_C * k * k;
        let nodes = f64::from(model.count);
        Self {
            k,
            touching,
            clear: model.edges.len() as f64 * touching * 2.0 / (nodes * (nodes - 1.0)),
        }
    }
}

/// `x_layout` (`xlayout.c:247-293`): up to [`OVERLAP_TRIES`] rounds of "push apart what
/// touches, pull together what does not", on the phase's own cooling schedule.
///
/// Returns the number of overlaps the last try finished on, which is the reference's own
/// return value and is what says whether anything is still touching. A graph with no
/// overlap never enters the loop at all — the reference's `cntOverlaps` early return.
pub(super) fn x_layout(model: &mut Model, t0: f64, rand: &mut GlibcRand) -> u32 {
    if count_overlaps(model) == 0 {
        return 0;
    }
    let mut overlaps = 1;
    for round in 0..OVERLAP_TRIES {
        let attempt = Try::new(model, round);
        overlaps = one_try(model, &attempt, t0, rand);
        if overlaps == 0 {
            break;
        }
    }
    overlaps
}

/// One try: the tick loop, stopping on the reference's two exits — the schedule running
/// out, or the overlaps reaching zero.
fn one_try(model: &mut Model, attempt: &Try, t0: f64, rand: &mut GlibcRand) -> u32 {
    let mut overlaps = 1;
    for tick in 0..X_ITERS {
        let temp = cool(t0, tick);
        if temp <= 0.0 {
            break;
        }
        overlaps = adjust(model, attempt, temp, rand);
        if overlaps == 0 {
            break;
        }
    }
    overlaps
}

/// `cool(t) = T0 * (numIters - t) / numIters` (`xlayout.c:100`), with `numIters` the
/// `xpms->numIters` of `init_params` — which is `maxIters - pass1`, not `maxIters`.
fn cool(t0: f64, tick: u32) -> f64 {
    t0 * f64::from(X_ITERS - tick) / f64::from(X_ITERS)
}

/// `adjust` (`xlayout.c:191-234`): accumulate, then move only if something was touching.
///
/// The reference's order is two loops per node — every repulsion against the nodes after
/// it, then every attraction along its own out-edges — and not one interleaved pass over
/// the pairs. The order is the order the additions land in, so it is kept.
fn adjust(model: &mut Model, attempt: &Try, temp: f64, rand: &mut GlibcRand) -> u32 {
    model.clear_displacement();
    let mut overlaps = 0;
    for p in 0..model.count {
        for q in (p + 1)..model.count {
            overlaps += u32::from(repel(p, q, attempt, model, rand));
        }
        for slot in model.out_at[p as usize]..model.out_at[p as usize + 1] {
            separate(p, model.out[slot as usize], attempt, model);
        }
    }
    if overlaps == 0 {
        return 0;
    }
    limit(model, temp);
    overlaps
}

/// `cntOverlaps` (`xlayout.c:110-119`) over the upper triangle only, which is the
/// reference's `agnxtnode` inner walk.
fn count_overlaps(model: &Model) -> u32 {
    let mut count = 0;
    for p in 0..model.count {
        for q in (p + 1)..model.count {
            count += u32::from(touching(p, q, model));
        }
    }
    count
}

/// `overlap` (`xlayout.c:103-107`): the two boxes, each grown by the separation margin,
/// intersect on both axes. Every node has the same box at the default margin, so the sum
/// of the half-extents is a constant.
fn touching(p: u32, q: u32, model: &Model) -> bool {
    let (p, q) = (p as usize, q as usize);
    let width = super::NODE_W + 2.0 * SEP_INCH;
    let height = super::NODE_H + 2.0 * SEP_INCH;
    (model.x[q] - model.x[p]).abs() <= width && (model.y[q] - model.y[p]).abs() <= height
}

/// `applyRep` (`xlayout.c:154-158`): `X_ov / d^2` when the pair touches, `X_nonov / d^2`
/// when it does not. Returns whether it touched, which is the reference's overlap counter.
fn repel(p: u32, q: u32, attempt: &Try, model: &mut Model, rand: &mut GlibcRand) -> bool {
    let (p, q) = (p as usize, q as usize);
    let (mut dx, mut dy) = (model.x[q] - model.x[p], model.y[q] - model.y[p]);
    let mut dist = super::distance(dx, dy);
    // The reference's `while (!(dist > 0))`, spelled out: true for a zero distance *and* for
    // a NaN, which a plain `<= 0.0` would not be, so the NaN case is named.
    while dist <= 0.0 || dist.is_nan() {
        dx = rand.jitter();
        dy = rand.jitter();
        dist = super::distance(dx, dy);
    }
    let overlaps = touching(p as u32, q as u32, model);
    let force = if overlaps {
        attempt.touching
    } else {
        attempt.clear
    } / (dist * dist);
    model.dx[q] += dx * force;
    model.dy[q] += dy * force;
    model.dx[p] -= dx * force;
    model.dy[p] -= dy * force;
    overlaps
}

/// `applyAttr` (`xlayout.c:160-186`): a separation-squared pull, and nothing at all for a
/// pair that is still touching.
fn separate(p: u32, q: u32, attempt: &Try, model: &mut Model) {
    if touching(p, q, model) {
        return;
    }
    let (p, q) = (p as usize, q as usize);
    let (dx, dy) = (model.x[q] - model.x[p], model.y[q] - model.y[p]);
    let dist = super::distance(dx, dy);
    let inner = super::radius();
    let force = (dist - inner) * (dist - inner) / ((attempt.k + inner) * dist);
    model.dx[q] -= dx * force;
    model.dy[q] -= dy * force;
    model.dx[p] += dx * force;
    model.dy[p] += dy * force;
}

/// The temperature cap of `adjust` (`xlayout.c:216-232`), with the `P_PIN` skip dropped:
/// the motor's [`Topology`](crate::index::Topology) has no pinned node.
fn limit(model: &mut Model, temp: f64) {
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
