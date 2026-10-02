//! The Fruchterman-Reingold iteration at `D` columns: start, repulsion, attraction, move.
//!
//! **One kernel at `D` columns, not one kernel per dimension**, exactly as
//! `layout/force/spring/forces.rs` does it and for the same reason. `D` is a const parameter and
//! every column is walked in axis order `x`, then `y`, then `z`, so `D = 2` performs exactly the
//! operations, in exactly the order, that the two-column version did: every reduction finishes a
//! squared sum of squares inside its own axis loop, so one column's total cannot depend on
//! another's. `layout.force.fruchterman_reingold_3d` is this file at `D = 3`, not a second copy.
//!
//! **Positions are `[f64; 3]` with `D` active axes, not `[f64; D]`.** The coincidence fix and
//! the noise both read a fixed three-wide vector, and the move's length test needs the norm of
//! whichever axes are live; storing three and restricting every read and every reduction to
//! `0..D` keeps `D = 2` bit-identical to the array version it replaced while letting one Hessian-
//! shaped helper serve both dimensions. Axis `D..3` is never written, never read and never
//! reduced, so a `D = 2` run cannot tell it exists.
//!
//! Written from the prose spec `docs/layouts/layout.force.fruchterman_reingold.md` and the paper
//! (Fruchterman and Reingold, Software: Practice and Experience 21(11), 1991) only, per
//! `docs/decisions/layouts-igraph.md` rule 1.

use super::FrParams;
use crate::index::Topology;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::rng::{Mulberry32, jiggle};
use crate::stage::StageError;

/// The widest position vector either dimension has; see the module doc.
pub(crate) const AXES: usize = 3;

/// One node's position, `AXES` wide, of which the first `D` are live.
pub(crate) type Axis = [f64; AXES];

/// The column names D9 reports a non-finite value under, in axis order. `D` never exceeds the
/// table, so the lookup is total.
const COLUMN_NAMES: [&str; AXES] = ["node.x", "node.y", "node.z"];

/// Amplitude of the coincident-pair fix and the per-move noise, per axis.
const NOISE: f64 = 1e-9;

/// The whole stage at `D` columns: the start, `niter` iterations of the three forces, and D9's
/// check. The geometry is the caller's, so the two dimensions differ only in what they build
/// from these columns.
pub(crate) fn solve<const D: usize>(
    topology: &Topology,
    params: &FrParams,
) -> Result<Vec<Axis>, StageError> {
    let n = topology.node_count() as usize;
    let graph = simple_graph(topology);
    let mut pos = start_positions::<D>(n, params.seed);
    let start_temp = params.start_temp.unwrap_or(sqrt(n as f64) / 10.0);
    let far = connected_far(n, &graph);
    let mut temp = start_temp;
    let mut disp = vec![[0.0; AXES]; n];
    for iter in 0..params.niter {
        disp.fill([0.0; AXES]);
        repel::<D>(&pos, far, (params.seed, iter), &mut disp);
        attract::<D>(&graph, &pos, &mut disp);
        step::<D>(&mut pos, &disp, temp, (params.seed, iter));
        temp -= start_temp / f64::from(params.niter);
    }
    if let Some(column) = first_non_finite::<D>(&pos) {
        return Err(StageError::NonFinite { column });
    }
    Ok(pos)
}

pub(crate) fn sqrt(v: f64) -> f64 {
    libm::sqrt(v)
}

/// Uniform in the box of side `sqrt(n)` centred on the origin, `D` axes wide, axis by axis per
/// node in the order the draw loop visits them (`layout_random.c:184`).
pub(crate) fn start_positions<const D: usize>(n: usize, seed: u32) -> Vec<Axis> {
    let side = sqrt(n as f64);
    let mut rng = Mulberry32::new(seed);
    (0..n)
        .map(|_| {
            let mut point = [0.0; AXES];
            for axis in 0..D {
                point[axis] = (rng.next_f64() - 0.5) * side;
            }
            point
        })
        .collect()
}

/// Weak connectivity by union-find over the simple edges (union by smaller index, so the
/// result does not depend on anything but edge order). `None` when the graph is connected, and
/// `Some(n * sqrt(n))` when it is not — the spec's `C`, the weak linear pull between every pair.
fn connected_far(n: usize, graph: &SimpleGraph) -> Option<f64> {
    if is_connected(n, graph) {
        return None;
    }
    Some(n as f64 * sqrt(n as f64))
}

fn is_connected(n: usize, graph: &SimpleGraph) -> bool {
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    let mut components = n;
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (ra, rb) = (root(&mut parent, a as usize), root(&mut parent, b as usize));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
            components -= 1;
        }
    }
    components <= 1
}

/// Pair repulsion `delta / r^2`, with the linear pull `-delta * r / far` on disconnected
/// graphs. A coincident pair is separated by a hash nudge, never divided by zero (D9).
///
/// **One reduction order for both dimensions (D3):** `v` ascending, then `u > v` ascending, and
/// inside a pair every axis in turn, adding to `v` and subtracting from `u` before the next axis.
fn repel<const D: usize>(pos: &[Axis], far: Option<f64>, key: (u32, u32), disp: &mut [Axis]) {
    for v in 0..pos.len() {
        for u in v + 1..pos.len() {
            let mut delta = [0.0; AXES];
            for axis in 0..D {
                delta[axis] = pos[v][axis] - pos[u][axis];
            }
            let mut r2 = squared::<D>(&delta);
            if r2 == 0.0 {
                delta = coincident_nudge(key, (v as u32, u as u32));
                r2 = squared::<D>(&delta);
            }
            let scale = match far {
                None => 1.0 / r2,
                Some(c) => (c - r2 * sqrt(r2)) / (r2 * c),
            };
            for axis in 0..D {
                disp[v][axis] += delta[axis] * scale;
                disp[u][axis] -= delta[axis] * scale;
            }
        }
    }
}

/// The coincident-pair fix: a hash nudge per axis, and a non-zero fallback so a hash that lands
/// on zero twice still separates the pair.
fn coincident_nudge(key: (u32, u32), pair: (u32, u32)) -> Axis {
    let amp = NOISE * 2e6;
    let mut d = [0.0; AXES];
    for axis in 0..AXES {
        d[axis] = jiggle(key.0, key.1, axis as u32 + 2, pair) * amp;
    }
    if d == [0.0; AXES] {
        [NOISE, 0.0, 0.0]
    } else {
        d
    }
}

/// Edge pull of magnitude `r^2` (unit weight): `delta * |delta|`.
fn attract<const D: usize>(graph: &SimpleGraph, pos: &[Axis], disp: &mut [Axis]) {
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (v, u) = (a as usize, b as usize);
        let mut delta = [0.0; AXES];
        for axis in 0..D {
            delta[axis] = pos[v][axis] - pos[u][axis];
        }
        let len = sqrt(squared::<D>(&delta));
        for axis in 0..D {
            disp[v][axis] -= delta[axis] * len;
            disp[u][axis] += delta[axis] * len;
        }
    }
}

/// Moves every vertex by its displacement capped at `temp`, after a tiny hash noise so a
/// perfectly balanced vertex still leaves a saddle.
fn step<const D: usize>(pos: &mut [Axis], disp: &[Axis], temp: f64, key: (u32, u32)) {
    for (v, d) in disp.iter().enumerate() {
        let vv = (v as u32, v as u32);
        let mut move_by = [0.0; AXES];
        for axis in 0..D {
            move_by[axis] = d[axis] + jiggle(key.0, key.1, axis as u32, vv) * NOISE * 2e6;
        }
        let len = sqrt(squared::<D>(&move_by));
        if len > temp {
            for axis in 0..D {
                move_by[axis] = move_by[axis] / len * temp;
            }
        }
        if len > 0.0 {
            for axis in 0..D {
                pos[v][axis] += move_by[axis];
            }
        }
    }
}

/// `|delta|^2` over the `D` live axes, the sum finished inside the axis loop so `D = 2` is
/// `dx * dx + dy * dy` — `0.0 + dx * dx` is `dx * dx`, and no product of two `f64`s is a negative
/// zero, so the leading zero cannot move a bit.
fn squared<const D: usize>(delta: &Axis) -> f64 {
    let mut sum = 0.0;
    for axis in 0..D {
        sum += delta[axis] * delta[axis];
    }
    sum
}

/// D9: the first live column holding a value `f64` cannot pin, named as the snapshot names its
/// columns. The axes are walked in order, so the answer does not depend on where the bad value
/// sits among several.
fn first_non_finite<const D: usize>(pos: &[Axis]) -> Option<&'static str> {
    debug_assert!(D <= COLUMN_NAMES.len(), "no name for column {D}");
    for axis in 0..D {
        if pos.iter().any(|p| !p[axis].is_finite()) {
            return Some(COLUMN_NAMES[axis]);
        }
    }
    None
}
