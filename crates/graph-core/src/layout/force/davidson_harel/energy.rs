//! Energy change of moving one node: the five terms of
//! `docs/layouts/layout.force.davidson_harel.md`.

use super::Weights;

/// Floor on a squared distance: the spec divides by zero for coincident nodes; we do not.
const MIN_D2: f64 = 1e-12;

pub(super) struct Field<'a> {
    pub pos: &'a [[f64; 2]],
    pub adj: &'a [Vec<u32>],
    pub edges: &'a [(u32, u32)],
    pub half_width: f64,
}

/// One candidate move's reading of the field: the geometry, plus what the node's current
/// position already contributes, as [`resting`] recorded it.
pub(super) struct Probe<'a, 'b> {
    field: Field<'a>,
    rest: &'b Rest,
}

/// The two energy terms' share of the position a node is already at.
///
/// Both terms are the cost of this layout — `O(deg(v) * m)` segment tests and `O(n)`
/// reciprocal distances per candidate, 30 candidates per node per round, 10 rounds — and
/// in both, half the work answers the same question for the position the node is already
/// at. Only `pos[v]` moves while `v` is being probed, and every edge incident to `v` is
/// skipped, so that half is the same answer for all 30 candidates and is recorded once per
/// position instead. Same values in the same order, so the energy is the same number: a
/// cache, not an approximation.
pub(super) struct Rest {
    /// One flag per non-skipped `(neighbour, edge)` pair, in the order [`crossings`] reads
    /// them, so the reduction runs in the order it did before.
    pub(super) crossings: Vec<u8>,
    /// `1 / d2(at, pos[u])` per node, `0.0` at `v` itself — the `at` half of
    /// [`node_dist`], indexed by node rather than by pair.
    pub(super) repulsion: Vec<f64>,
}

impl<'a, 'b> Probe<'a, 'b> {
    /// `field`, with what `v` contributes from its current position already recorded.
    pub(super) fn new(field: Field<'a>, rest: &'b Rest) -> Self {
        Probe { field, rest }
    }
}

/// Everything `v` contributes to the crossings and node-distance terms from `at`: one
/// crossing flag per non-skipped `(neighbour, edge)` pair, and one reciprocal distance per
/// node. The crossings run over `v`'s neighbours in adjacency order, then the edge list in
/// its own, skipping an edge whose either end is `v` or the neighbour.
pub(super) fn resting(field: &Field, v: u32, at: [f64; 2]) -> Rest {
    let pos = field.pos;
    let mut crossings = Vec::new();
    for &u in &field.adj[v as usize] {
        if u == v {
            continue;
        }
        let pu = pos[u as usize];
        for &(a, b) in field.edges {
            if a == v || b == v || a == u || b == u {
                continue;
            }
            let (pa, pb) = (pos[a as usize], pos[b as usize]);
            crossings.push(u8::from(cross(at, pu, pa, pb)));
        }
    }
    let repulsion = pos
        .iter()
        .enumerate()
        .map(|(u, &pu)| if u as u32 == v { 0.0 } else { 1.0 / d2(at, pu) })
        .collect();
    Rest {
        crossings,
        repulsion,
    }
}

fn d2(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).max(MIN_D2)
}

/// Total energy change for node `v` going from its probed position `p` to `q`.
pub(super) fn delta(probe: &Probe, w: &Weights, v: u32, moves: ([f64; 2], [f64; 2])) -> f64 {
    let field = &probe.field;
    let (p, q) = moves;
    let mut e = 0.0;
    if w.node_dist != 0.0 {
        e += w.node_dist * node_dist(probe, v, q);
    }
    if w.border != 0.0 {
        e += w.border * (border(field.half_width, q) - border(field.half_width, p));
    }
    if w.edge_lengths != 0.0 {
        e += w.edge_lengths * edge_lengths(field, v, p, q);
    }
    if w.edge_crossings != 0.0 {
        e += w.edge_crossings * crossings(probe, v, q);
    }
    if w.node_edge_dist != 0.0 {
        e += w.node_edge_dist * node_edge(field, v, p, q);
    }
    e
}

/// Reciprocal-distance repulsion gained at `q`, less what [`Probe::rest`] recorded for the
/// position being left. The reduction runs over the nodes in index order, as before.
fn node_dist(probe: &Probe, v: u32, q: [f64; 2]) -> f64 {
    let pos = probe.field.pos;
    let mut sum = 0.0;
    for (u, &pu) in pos.iter().enumerate() {
        if u as u32 != v {
            sum += 1.0 / d2(q, pu) - probe.rest.repulsion[u];
        }
    }
    sum
}

fn border(half: f64, at: [f64; 2]) -> f64 {
    let sides = [half - at[0], at[0] + half, half - at[1], at[1] + half];
    sides
        .iter()
        .map(|&d| {
            let d = if d < 0.0 { 2.0 } else { d };
            1.0 / (d * d).max(MIN_D2)
        })
        .sum()
}

fn edge_lengths(field: &Field, v: u32, p: [f64; 2], q: [f64; 2]) -> f64 {
    let mut sum = 0.0;
    for &u in &field.adj[v as usize] {
        if u != v {
            let pu = field.pos[u as usize];
            sum += d2(q, pu) - d2(p, pu);
        }
    }
    sum
}

/// Crossings gained by moving `v` to `q`, less the ones [`Probe`] recorded for the position
/// it is leaving. The reductions run over the same pairs in the same order as before, the
/// second reading the probe's flags rather than answering each intersection again.
fn crossings(probe: &Probe, v: u32, q: [f64; 2]) -> f64 {
    let field = &probe.field;
    let pos = field.pos;
    let mut count = 0.0;
    let mut old = probe.rest.crossings.iter();
    for &u in &field.adj[v as usize] {
        if u == v {
            continue;
        }
        let pu = pos[u as usize];
        for &(a, b) in field.edges {
            if a == v || b == v || a == u || b == u {
                continue;
            }
            let (pa, pb) = (pos[a as usize], pos[b as usize]);
            count += f64::from(u8::from(cross(q, pu, pa, pb)));
            count -= f64::from(*old.next().expect("one flag per pair"));
        }
    }
    count
}

/// Parametric segment intersection; parallel segments never cross.
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let denom = r[0] * s[1] - r[1] * s[0];
    if denom == 0.0 {
        return false;
    }
    let ca = [c[0] - a[0], c[1] - a[1]];
    let t = (ca[0] * s[1] - ca[1] * s[0]) / denom;
    let u = (ca[0] * r[1] - ca[1] * r[0]) / denom;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

fn point_segment_d2(x: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let s = [b[0] - a[0], b[1] - a[1]];
    let len2 = s[0] * s[0] + s[1] * s[1];
    if len2 == 0.0 {
        return d2(x, a);
    }
    let t = (((x[0] - a[0]) * s[0] + (x[1] - a[1]) * s[1]) / len2).clamp(0.0, 1.0);
    d2(x, [a[0] + t * s[0], a[1] + t * s[1]])
}

fn node_edge(field: &Field, v: u32, p: [f64; 2], q: [f64; 2]) -> f64 {
    let mut sum = 0.0;
    for &(a, b) in field.edges {
        if a != v && b != v {
            let (pa, pb) = (field.pos[a as usize], field.pos[b as usize]);
            sum += 1.0 / point_segment_d2(q, pa, pb) - 1.0 / point_segment_d2(p, pa, pb);
        }
    }
    for &u in &field.adj[v as usize] {
        let pu = field.pos[u as usize];
        for (w, &pw) in field.pos.iter().enumerate() {
            if w as u32 != v && w as u32 != u {
                sum += 1.0 / point_segment_d2(pw, q, pu) - 1.0 / point_segment_d2(pw, p, pu);
            }
        }
    }
    sum
}
