//! The Fruchterman-Reingold iteration at a const number of dimensions, from the prose spec
//! `docs/layouts/layout.force.fruchterman_reingold.md` alone (`docs/decisions/layouts-igraph.md`
//! rule 1: spec and papers, never igraph's sources).
//!
//! **One kernel at `D` columns, not one kernel per dimension.** `D` is a const parameter and
//! every axis loop walks `x`, then `y`, then `z`, so `D = 2` performs exactly the operations,
//! in exactly the order, the two-column version performed: the start draws one number per axis
//! per vertex in that order, the pair loop runs `v` ascending then `u > v` ascending, and the
//! noise passes are the axis index. `layout.force.fruchterman_reingold` is this file at
//! `D = 2`; `layout.force.fruchterman_reingold_3d` is the same file at `D = 3`, which is the
//! dimension SciGraphs actually calls (`igraph_layouts.py:74`).
//!
//! The spec's grid variant (2D, `n > 1000`) is not implemented at either dimension: igraph has
//! no grid in 3D (`fruchterman_reingold.md` "Grid variant"), and past 1000 nodes this is the
//! same `O(n^2)` loop as below.
//!
//! **Not gather form (D10), and why that is stated rather than fixed.** The reference sums the
//! pair term into both endpoints of an unordered pair, so a gather kernel would have to recompute
//! each pair twice — the same additions in the same order, so the result is bit-identical and
//! the cost is 2x, but the accumulation *order within a vertex* would then be over `u` ascending
//! in one gather and `v` descending in the other. The scatter below is the only form that keeps
//! igraph's accumulation order, so this kernel is scatter and D10 is relaxed for it in writing.
//! The step is still per-vertex independent and the snapshot is bit-identical native vs wasm32
//! (hash-gate), which is the part of D10 that is observable.
//!
//! **Rounding notes that are load-bearing.** Two spellings of the same amplitude are kept apart
//! on purpose: the coincident-pair nudge multiplies by a precomputed `NOISE * 2e6`, while the
//! per-move noise writes `NOISE * 2e6` after the hash. `(h * NOISE) * 2e6` and `h * (NOISE * 2e6)`
//! are different `f64` values, and `layout.force.fruchterman_reingold` is pinned byte for byte,
//! so the two are not merged.

use super::fruchterman_reingold::FrParams;
use super::SimpleGraph;
use crate::rng::{Mulberry32, jiggle};

/// Amplitude of the coincident-pair fix and the per-move noise, per axis.
pub(super) const NOISE: f64 = 1e-9;

/// `params.niter` iterations of repulsion, attraction and one capped move per vertex, from
/// the box start. `n` is the node count, already read from the topology by the caller.
pub(super) fn layout<const D: usize>(
    graph: &SimpleGraph,
    n: usize,
    params: &FrParams,
) -> Vec<[f64; D]> {
    let mut pos = start(n, params.seed);
    let start_temp = params.start_temp.unwrap_or(libm::sqrt(n as f64) / 10.0);
    let far = reach(n, graph);
    let mut temp = start_temp;
    let mut disp = vec![[0.0; D]; n];
    for iter in 0..params.niter {
        disp.fill([0.0; D]);
        repel(&pos, far, (params.seed, iter), &mut disp);
        attract(graph, &pos, &mut disp);
        step(&mut pos, &disp, temp, (params.seed, iter));
        temp -= start_temp / f64::from(params.niter);
    }
    pos
}

/// Uniform in the box of side `sqrt(n)` centred on the origin, drawn one axis at a time in
/// axis order so the stream is interleaved per vertex and not per column (D5: a seeded
/// generator, never a global one).
pub(super) fn start<const D: usize>(n: usize, seed: u32) -> Vec<[f64; D]> {
    let side = libm::sqrt(n as f64);
    let mut rng = Mulberry32::new(seed);
    let mut pos = vec![[0.0; D]; n];
    for p in pos.iter_mut() {
        for slot in p.iter_mut() {
            *slot = (rng.next_f64() - 0.5) * side;
        }
    }
    pos
}

/// `C = n * sqrt(n)` on a disconnected graph and nothing on a connected one, where the
/// constant is unused. Weak connectivity by union-find over the simple edges, union towards
/// the smaller index, so the answer depends on nothing but edge order.
pub(super) fn reach(n: usize, graph: &SimpleGraph) -> Option<f64> {
    if is_connected(n, graph) {
        return None;
    }
    Some(n as f64 * libm::sqrt(n as f64))
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
/// The `far.is_some()` scale is where the spec's decision about igraph's mistyped 3D
/// disconnected axis lives: each component of `delta` goes to the accumulator of its own axis
/// (`layout.force.fruchterman_reingold.md`, "Resolved for this tree"). Restoring igraph's
/// literal behaviour means folding the `z` term into the `y` accumulator here.
fn repel<const D: usize>(
    pos: &[[f64; D]],
    far: Option<f64>,
    key: (u32, u32),
    disp: &mut [[f64; D]],
) {
    for v in 0..pos.len() {
        for u in v + 1..pos.len() {
            let mut delta = separation(&pos[v], &pos[u]);
            let mut r2 = squared(&delta);
            if r2 == 0.0 {
                delta = coincident_nudge(key, (v as u32, u as u32));
                r2 = squared(&delta);
            }
            let scale = match far {
                None => 1.0 / r2,
                Some(c) => (c - r2 * libm::sqrt(r2)) / (r2 * c),
            };
            for (axis, d) in delta.iter().enumerate() {
                disp[v][axis] += d * scale;
                disp[u][axis] -= d * scale;
            }
        }
    }
}

/// `pos[v] - pos[u]`, axis by axis in axis order.
fn separation<const D: usize>(v: &[f64; D], u: &[f64; D]) -> [f64; D] {
    let mut delta = [0.0; D];
    for (axis, slot) in delta.iter_mut().enumerate() {
        *slot = v[axis] - u[axis];
    }
    delta
}

/// The coincident-pair fix of the spec's "Where randomness enters": a deterministic direction
/// keyed on the pair, never the generator, so two threads see the same separation (C8).
///
/// Ponytail: exactly coincident vertices are the only failing input; the direction is a hash,
/// not a physically meaningful one. Escape hatch: `key` carries the tick, so a pair that is
/// still coincident on a later iteration separates differently.
fn coincident_nudge<const D: usize>(key: (u32, u32), pair: (u32, u32)) -> [f64; D] {
    let amp = NOISE * 2e6;
    let mut d = [0.0; D];
    for (axis, slot) in d.iter_mut().enumerate() {
        *slot = jiggle(key.0, key.1, axis as u32 + 2, pair) * amp;
    }
    if d.iter().all(|v| *v == 0.0) {
        d[0] = NOISE;
    }
    d
}

/// Edge pull of magnitude `r^2` (unit weight): `delta * |delta|`, once per edge.
fn attract<const D: usize>(graph: &SimpleGraph, pos: &[[f64; D]], disp: &mut [[f64; D]]) {
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (v, u) = (a as usize, b as usize);
        let delta = separation(&pos[v], &pos[u]);
        let len = libm::sqrt(squared(&delta));
        for (axis, d) in delta.iter().enumerate() {
            disp[v][axis] -= d * len;
            disp[u][axis] += d * len;
        }
    }
}

/// Moves every vertex by its displacement capped at `temp`, after a tiny hash noise so a
/// perfectly balanced vertex still leaves a saddle. The length test is on the *perturbed*
/// vector, as the spec states, so a balanced vertex is moved by the noise alone.
fn step<const D: usize>(
    pos: &mut [[f64; D]],
    disp: &[[f64; D]],
    temp: f64,
    key: (u32, u32),
) {
    for (v, raw) in disp.iter().enumerate() {
        let pair = (v as u32, v as u32);
        let mut d = *raw;
        for (axis, slot) in d.iter_mut().enumerate() {
            *slot += jiggle(key.0, key.1, axis as u32, pair) * NOISE * 2e6;
        }
        let len = libm::sqrt(squared(&d));
        if len > temp {
            for slot in d.iter_mut() {
                *slot = *slot / len * temp;
            }
        }
        if len > 0.0 {
            for (axis, slot) in d.iter().enumerate() {
                pos[v][axis] += *slot;
            }
        }
    }
}

/// The squared length over the `D` columns, finished inside the axis loop in axis order, so
/// the rounding is axis-order only. For `D = 2` the leading `0.0 + dx * dx` is `dx * dx`:
/// no product of two `f64`s is a negative zero, so the leading zero cannot move a bit.
fn squared<const D: usize>(delta: &[f64; D]) -> f64 {
    let mut sum = 0.0;
    for &v in delta {
        sum += v * v;
    }
    sum
}