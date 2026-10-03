//! The level: what a bundle is, where its meeting points go, and what it costs. Geometry,
//! with no notion of rounds or passes.

use super::P;
use crate::stage::StageError;

/// One side of a bundle: the points that meet at a single `p` (or `q`), and their weights. A
/// weight is a position's pull on the meeting point, never a term in the ink.
#[derive(Clone)]
pub struct Group {
    /// The points meeting there, in the order they joined.
    pub pts: Vec<P>,
    /// Each point's weight, the same length as `pts`.
    pub w: Vec<f64>,
}

/// A bundle at one level: two point groups meeting at `p` and `q`, and the ink drawn from the
/// first of its head's points to the last of its tail's through those two.
#[derive(Clone)]
pub struct Bundle {
    /// The points meeting at `p`.
    pub head: Group,
    /// The points meeting at `q`.
    pub tail: Group,
    /// The head's meeting point.
    pub p: P,
    /// The tail's meeting point.
    pub q: P,
    /// The ink this bundle draws, measured on the groups as they now stand.
    pub ink: f64,
}

/// Round 0: every edge is its own bundle, meeting at its own endpoints, and a singleton's
/// ink is `|a − b|` — all trunk.
pub fn seed(ends: &[(P, P)]) -> Vec<Bundle> {
    ends.iter()
        .map(|&(a, b)| Bundle {
            head: Group {
                pts: vec![a],
                w: vec![1.0],
            },
            tail: Group {
                pts: vec![b],
                w: vec![1.0],
            },
            p: a,
            q: b,
            ink: dist(a, b),
        })
        .collect()
}

/// The next round's edge: one trunk per bundle, weighted by its load, so a bundle's point
/// count cannot grow with the graph and a heavy bundle holds its place.
pub fn trunk(b: &Bundle) -> Bundle {
    let weight = load(&b.head);
    Bundle {
        head: Group {
            pts: vec![b.p],
            w: vec![weight],
        },
        tail: Group {
            pts: vec![b.q],
            w: vec![weight],
        },
        p: b.p,
        q: b.q,
        ink: dist(b.p, b.q),
    }
}

/// Each edge's two endpoints, in edge order, every index checked.
pub fn endpoints(pos: &[P], src: &[u32], dst: &[u32]) -> Result<Vec<(P, P)>, StageError> {
    let at = |i: u32| -> Result<P, StageError> {
        pos.get(i as usize).copied().ok_or(StageError::Param {
            name: "src/dst",
            rule: "a dense node index below the position count",
        })
    };
    src.iter()
        .zip(dst)
        .map(|(&a, &b)| Ok((at(a)?, at(b)?)))
        .collect()
}

/// The paths the input drew before anything was bundled: straight, endpoints only.
pub fn straight(ends: &[(P, P)]) -> Vec<Vec<P>> {
    ends.iter().map(|&(a, b)| vec![a, b]).collect()
}

/// The drawing's diagonal: the scale `eps` is a fraction of. Zero for one position, and
/// `eps`'s own floor then holds the solve.
pub fn diagonal(pos: &[P]) -> f64 {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in pos {
        for c in 0..2 {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    dist(hi, lo)
}

/// The unweighted drawn length of a group meeting at `at`: what the bundle costs, regardless
/// of what its members weigh. The weights are deliberately absent — the trunk is one drawn
/// line whatever it carries.
pub fn group_ink(g: &Group, at: P) -> f64 {
    let mut sum = 0.0;
    for (pt, w) in g.pts.iter().zip(&g.w) {
        if *w > 0.0 {
            sum += dist(*pt, at);
        }
    }
    sum
}

/// A group's total weight: its load, which is what the next round's trunk weighs.
pub fn load(g: &Group) -> f64 {
    let mut sum = 0.0;
    for w in &g.w {
        if *w > 0.0 {
            sum += *w;
        }
    }
    sum
}

/// A bundle's two meeting points in a lexicographic total order — the reference's
/// `_proximity_points`, with the smaller pair first. Without it, two bundles stored either
/// way round sit at opposite corners of proximity space and are each other's *farthest*
/// neighbour instead of their nearest.
pub fn oriented(b: &Bundle) -> (P, P) {
    let lead = if b.p[0] != b.q[0] {
        b.p[0] - b.q[0]
    } else {
        b.p[1] - b.q[1]
    };
    if lead > 0.0 { (b.q, b.p) } else { (b.p, b.q) }
}

/// Squared proximity distance between two oriented bundles: the sum over the four
/// coordinates. Squared, which orders as distance does and costs no square root a pair.
pub fn prox(a: (P, P), b: (P, P)) -> f64 {
    let mut sum = 0.0;
    for c in 0..2 {
        let (x, y) = (a.0[c] - b.0[c], a.1[c] - b.1[c]);
        sum += x * x + y * y;
    }
    sum
}

/// Euclidean distance. IEEE-754's `sqrt`, never `powf` or `mul_add` (D1, D2).
pub fn dist(a: P, b: P) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    f64::sqrt(dx * dx + dy * dy)
}
