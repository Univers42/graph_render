//! LGL's layer-by-layer growth and per-layer annealing, split from the stage for the
//! house line cap.

use super::{Consts, EPSILON};
use crate::layout::force::SimpleGraph;
use crate::layout::force::fruchterman_reingold::sqrt;
use crate::rng::Mulberry32;
use std::collections::BTreeMap;

const NONE: u32 = u32::MAX;

/// Breadth-first order over the undirected simple graph from one root.
struct Bfs {
    order: Vec<u32>,
    /// `starts[l]..starts[l + 1]` is layer `l` of `order`.
    starts: Vec<usize>,
    parent: Vec<u32>,
    layer: Vec<u32>,
}

fn other(graph: &SimpleGraph, edge: u32, v: u32) -> u32 {
    let e = edge as usize;
    if graph.lo[e] == v {
        graph.hi[e]
    } else {
        graph.lo[e]
    }
}

fn bfs(graph: &SimpleGraph, n: usize, root: u32) -> Bfs {
    let mut out = Bfs {
        order: vec![root],
        starts: vec![0, 1],
        parent: vec![NONE; n],
        layer: vec![NONE; n],
    };
    out.layer[root as usize] = 0;
    let mut head = 0;
    while head < out.order.len() {
        let end = out.order.len();
        while head < end {
            let v = out.order[head];
            head += 1;
            for &e in graph.rows.row(v) {
                let u = other(graph, e, v);
                if out.layer[u as usize] == NONE {
                    out.layer[u as usize] = out.starts.len() as u32 - 1;
                    out.parent[u as usize] = v;
                    out.order.push(u);
                }
            }
        }
        if out.order.len() > end {
            out.starts.push(out.order.len());
        }
    }
    out
}

pub(super) fn layout(
    graph: &SimpleGraph,
    c: &Consts,
    (root, rng): (Option<u32>, &mut Mulberry32),
    pos: &mut [[f64; 2]],
) {
    let n = pos.len();
    let root = match root {
        Some(r) if (r as usize) < n => r,
        _ => ((rng.next_f64() * n as f64) as usize).min(n - 1) as u32,
    };
    for p in pos.iter_mut() {
        *p = disc_point(rng, c.radius);
    }
    pos[root as usize] = [0.0; 2];
    let tree = bfs(graph, n, root);
    let layers = tree.starts.len() - 1;
    let harmonic: f64 = (1..layers.saturating_sub(1)).map(|l| 1.0 / l as f64).sum();
    let spacing = c.radius / harmonic.max(1.0);
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for l in 1..layers {
        place_layer(&tree, (l, spacing), rng, pos);
        activate(graph, &tree, l, &mut edges);
        anneal(&tree.order[..tree.starts[l + 1]], &edges, c, pos);
    }
}

fn disc_point(rng: &mut Mulberry32, radius: f64) -> [f64; 2] {
    let r = radius * sqrt(rng.next_f64());
    let angle = 2.0 * core::f64::consts::PI * rng.next_f64();
    [r * libm::cos(angle), r * libm::sin(angle)]
}

fn unit(v: [f64; 2]) -> [f64; 2] {
    let len = sqrt(v[0] * v[0] + v[1] * v[1]);
    if len > 0.0 {
        [v[0] / len, v[1] / len]
    } else {
        [0.0; 2]
    }
}

/// Puts layer `l` around its parents: each child lands `spacing / l` from the parent's
/// anchor, which is the parent nudged away from the placed centre of mass and along the
/// parent's own branch.
fn place_layer(tree: &Bfs, (l, spacing): (usize, f64), rng: &mut Mulberry32, pos: &mut [[f64; 2]]) {
    let (lo, hi) = (tree.starts[l], tree.starts[l + 1]);
    let mut sum = [0.0; 2];
    for &v in &tree.order[..lo] {
        sum[0] += pos[v as usize][0];
        sum[1] += pos[v as usize][1];
    }
    let mut placed = lo;
    let mut next = lo;
    for &v in &tree.order[tree.starts[l - 1]..lo] {
        let from = pos[v as usize];
        let mean = [sum[0] / placed as f64, sum[1] / placed as f64];
        let branch = match tree.parent[v as usize] {
            NONE => [0.0; 2],
            p => unit([from[0] - pos[p as usize][0], from[1] - pos[p as usize][1]]),
        };
        let m = unit(mean);
        let anchor = [from[0] + m[0] + branch[0], from[1] + m[1] + branch[1]];
        while next < hi && tree.parent[tree.order[next] as usize] == v {
            let dir = direction(l, next - lo, hi - lo, rng);
            let at = [
                anchor[0] + spacing / l as f64 * dir[0],
                anchor[1] + spacing / l as f64 * dir[1],
            ];
            pos[tree.order[next] as usize] = at;
            sum = [sum[0] + at[0], sum[1] + at[1]];
            placed += 1;
            next += 1;
        }
    }
}

fn direction(layer: usize, slot: usize, size: usize, rng: &mut Mulberry32) -> [f64; 2] {
    if layer == 1 {
        let angle = 2.0 * core::f64::consts::PI * slot as f64 / size as f64;
        return [libm::cos(angle), libm::sin(angle)];
    }
    let d = [rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0];
    let u = unit(d);
    if u == [0.0; 2] { [1.0, 0.0] } else { u }
}

/// Adds every edge from layer `l` to a vertex already placed, once.
fn activate(graph: &SimpleGraph, tree: &Bfs, l: usize, edges: &mut Vec<(u32, u32)>) {
    for &v in &tree.order[tree.starts[l]..tree.starts[l + 1]] {
        for &e in graph.rows.row(v) {
            let u = other(graph, e, v);
            let lu = tree.layer[u as usize];
            if lu != NONE && (lu < l as u32 || (lu == l as u32 && u < v)) {
                edges.push((v, u));
            }
        }
    }
}

/// Cooling loop over the placed vertices `active`, ending on `maxit` or convergence.
fn anneal(active: &[u32], edges: &[(u32, u32)], c: &Consts, pos: &mut [[f64; 2]]) {
    let mut force = vec![[0.0; 2]; pos.len()];
    for it in 0..c.maxit {
        let cool = f64::from(c.maxit - it) / f64::from(c.maxit);
        let temp = c.maxdelta * libm::pow(cool, c.coolexp);
        for &v in active {
            force[v as usize] = [0.0; 2];
        }
        attract(edges, c, pos, &mut force);
        repel(active, c, pos, &mut force);
        let mut change: f64 = 0.0;
        for &v in active {
            let f = force[v as usize];
            let len = sqrt(f[0] * f[0] + f[1] * f[1]);
            let scale = if len > temp { temp / len } else { 1.0 };
            for a in 0..2 {
                let step = f[a] * scale;
                pos[v as usize][a] += step;
                change = change.max(step.abs());
            }
        }
        if change <= EPSILON {
            break;
        }
    }
}

/// Pull of magnitude `d^2 / k` along every active edge.
fn attract(edges: &[(u32, u32)], c: &Consts, pos: &[[f64; 2]], force: &mut [[f64; 2]]) {
    for &(v, u) in edges {
        let (v, u) = (v as usize, u as usize);
        let delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
        let d = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
        for a in 0..2 {
            let part = delta[a] * d / c.ideal;
            force[v][a] -= part;
            force[u][a] += part;
        }
    }
}

type Cells = BTreeMap<(i64, i64), Vec<u32>>;

fn cells(active: &[u32], size: f64, pos: &[[f64; 2]]) -> Cells {
    let mut grid = Cells::new();
    for &v in active {
        let p = pos[v as usize];
        let key = (
            libm::floor(p[0] / size) as i64,
            libm::floor(p[1] / size) as i64,
        );
        grid.entry(key).or_default().push(v);
    }
    grid
}

/// Push `k^2 (1/d - d^2 / repulserad)` for pairs closer than one cell. Each pair is seen
/// once: within a cell, or against the four cells that sort after it.
fn repel(active: &[u32], c: &Consts, pos: &[[f64; 2]], force: &mut [[f64; 2]]) {
    let grid = cells(active, c.cellsize, pos);
    for (&(cx, cy), members) in &grid {
        for (i, &v) in members.iter().enumerate() {
            for &u in &members[i + 1..] {
                push(v, u, c, pos, force);
            }
            for key in [
                (cx + 1, cy - 1),
                (cx + 1, cy),
                (cx, cy + 1),
                (cx + 1, cy + 1),
            ] {
                for &u in grid.get(&key).map_or(&[][..], |m| &m[..]) {
                    push(v, u, c, pos, force);
                }
            }
        }
    }
}

fn push(v: u32, u: u32, c: &Consts, pos: &[[f64; 2]], force: &mut [[f64; 2]]) {
    let (v, u) = (v as usize, u as usize);
    let mut delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
    let mut d = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
    if d >= c.cellsize {
        return;
    }
    if d == 0.0 {
        delta = [EPSILON, 0.0];
        d = EPSILON;
    }
    let magnitude = c.ideal * c.ideal * (1.0 / d - d * d / c.repulserad);
    for a in 0..2 {
        let part = magnitude * delta[a] / d;
        force[v][a] += part;
        force[u][a] -= part;
    }
}
