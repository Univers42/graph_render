//! One pass: which pairs are candidates, what each is worth, which merges are taken, and
//! the level they produce. A pass is a matching, so it is sequential by nature.

use super::level::*;
use super::{MAX_GROUP, P};

/// Floor on a pair's solo ink when `min_gain` is read as a fraction of it, the reference's
/// `np.maximum(solo, 1e-20)` (`mingle.py:218`). Ponytail: a pair under 1e-20 of ink is
/// judged as if it had that much; its gain is at most its ink, so it is never taken either
/// way. Escape hatch: none needed.
const SOLO_INK_FLOOR: f64 = 1e-20;

/// Floor on a group's total weight before its centre is divided out, the reference's `tiny`
/// (`mingle.py:135`). Ponytail: a weightless group centres on the origin, not on its points;
/// only a group of zero-weight members reaches it, and the solve then moves it. Escape hatch:
/// none needed.
const WEIGHT_FLOOR: f64 = 1e-20;

/// One scored candidate merge: the ink it saves, which pairing won, and the meeting points
/// and ink of the bundle it would become.
pub struct Score {
    /// `ink(u) + ink(v) − ink(u | v)`: the ink a merge would save. Positive is worth taking.
    pub gain: f64,
    /// Whether `v` was joined head to tail, which is the only way an antiparallel pair
    /// merges at all.
    pub flip: bool,
    /// The merged bundle's head meeting point.
    pub p: P,
    /// The merged bundle's tail meeting point.
    pub q: P,
    /// The merged bundle's own ink, measured on the fused groups.
    pub ink: f64,
    /// `ink(u) + ink(v)`: the pair's ink unmerged, which `gain` and `min_gain` are both
    /// measured against.
    pub solo: f64,
}

/// One pair's score: both pairings are tried, because pairing antiparallel edges head to
/// tail opens an X the un-flipped pairing would refuse. Strict `>` keeps the unflipped one
/// on a tie, so the choice is the same on every run.
pub fn score(a: &Bundle, b: &Bundle, eps: f64) -> Score {
    let solo = a.ink + b.ink;
    let mut best = Score {
        gain: f64::NEG_INFINITY,
        flip: false,
        p: a.p,
        q: a.q,
        ink: solo,
        solo,
    };
    for flip in [false, true] {
        let (head, tail) = if flip {
            (&b.tail, &b.head)
        } else {
            (&b.head, &b.tail)
        };
        let (p, q, ink) = solve(&join(&a.head, head), &join(&a.tail, tail), eps);
        let gain = solo - ink;
        if gain > best.gain {
            best = Score {
                gain,
                flip,
                p,
                q,
                ink,
                solo,
            };
        }
    }
    best
}

/// The pairs one pass may score: each bundle's `k` nearest in edge proximity space,
/// deduplicated, ascending `(u, v)`. Ink is symmetric, so `u` holding `v` and `v` holding
/// `u` is one pair scored twice, and that order is the matching's last tie-break key.
pub fn candidates(level: &[Bundle], k: usize) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for u in 0..level.len() as u32 {
        for v in nearest(u, level, k) {
            let (u, v) = (u as usize, v as usize);
            pairs.push((u.min(v) as u32, u.max(v) as u32));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// The merges one pass accepts: best gain first, each bundle at most once, and neither side
/// wider than [`MAX_GROUP`]. Descending gain, ties by the candidate's position — a total
/// order, so two runs of one input accept the same merges. A matching rather than a chain,
/// since once two accepted merges share a bundle the second's score was measured against
/// something already gone.
pub fn matching(
    level: &[Bundle],
    cand: &[(u32, u32)],
    scores: &[Score],
    min_gain: f64,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..cand.len())
        .filter(|&i| worth(&scores[i], min_gain))
        .collect();
    order.sort_by(|a, b| scores[*b].gain.total_cmp(&scores[*a].gain).then(a.cmp(b)));
    let mut used = vec![false; level.len()];
    let mut out = Vec::new();
    for i in order {
        let (u, v) = (cand[i].0 as usize, cand[i].1 as usize);
        if used[u] || used[v] || level[u].head.pts.len() + level[v].head.pts.len() > MAX_GROUP {
            continue;
        }
        used[u] = true;
        used[v] = true;
        out.push(i);
    }
    out.sort_unstable();
    out
}

/// Whether a scored candidate is in the running at all: a positive gain that is
/// also at least `min_gain` of the pair's unbundled ink, so `min_gain` holds at any size
/// rather than only on a small drawing.
fn worth(score: &Score, min_gain: f64) -> bool {
    score.gain > 0.0 && score.gain / score.solo.max(SOLO_INK_FLOOR) > min_gain
}

/// Applies a pass's matching: the next level, where each old bundle's id went, and whether
/// its head and tail swapped sides. A swapped bundle's members reverse orientation with it,
/// so each member's chain stays on its own source side.
pub fn fused(
    level: &[Bundle],
    cand: &[(u32, u32)],
    picks: &[usize],
    scores: &[Score],
) -> (Vec<Bundle>, Vec<u32>, Vec<bool>) {
    let mut dead = vec![false; level.len()];
    let mut slot: Vec<Option<usize>> = vec![None; level.len()];
    let mut rev = vec![false; level.len()];
    for &i in picks {
        let (u, v) = (cand[i].0 as usize, cand[i].1 as usize);
        dead[v] = true;
        slot[u] = Some(i);
        rev[v] = scores[i].flip;
    }
    let mut out = Vec::with_capacity(level.len());
    let mut remap = vec![0u32; level.len()];
    for (i, b) in level.iter().enumerate() {
        if dead[i] {
            continue;
        }
        remap[i] = out.len() as u32;
        out.push(match slot[i] {
            Some(k) => fuse(
                &level[cand[k].0 as usize],
                &level[cand[k].1 as usize],
                &scores[k],
            ),
            None => b.clone(),
        });
    }
    for &i in picks {
        remap[cand[i].1 as usize] = remap[cand[i].0 as usize];
    }
    (out, remap, rev)
}

/// The bundle two edges make: `b`'s sides join `a`'s, flipped when the score says so, and
/// the ink is the score's — measured on the merged groups, not carried over, so every later
/// gain is compared against what is actually drawn.
fn fuse(a: &Bundle, b: &Bundle, s: &Score) -> Bundle {
    let (head, tail) = if s.flip {
        (&b.tail, &b.head)
    } else {
        (&b.head, &b.tail)
    };
    Bundle {
        head: join(&a.head, head),
        tail: join(&a.tail, tail),
        p: s.p,
        q: s.q,
        ink: s.ink,
    }
}

/// `a`'s points then `b`'s, in that order, weights along.
pub fn join(a: &Group, b: &Group) -> Group {
    Group {
        pts: a.pts.iter().chain(&b.pts).copied().collect(),
        w: a.w.iter().chain(&b.w).copied().collect(),
    }
}

/// The meeting points of a merged bundle: a weighted Jacobi fixed point on each of the two,
/// from the weighted centroids. Each step reads start-of-iteration state and writes both at
/// once, so the step count and the order are the only things that decide the result (D3,
/// D10) — there is no in-place update whose order would depend on the schedule.
fn solve(head: &Group, tail: &Group, eps: f64) -> (P, P, f64) {
    let (mut p, mut q) = (centroid(head), centroid(tail));
    for _ in 0..super::SOLVE_ITERS {
        let (np, nq) = (step(head, p, q, eps), step(tail, q, p, eps));
        p = np;
        q = nq;
    }
    (p, q, group_ink(head, p) + dist(p, q) + group_ink(tail, q))
}

/// One Jacobi step: a group's weighted pull on its meeting point, plus the other meeting
/// point's own share of the trunk, over the sum of both. A zero weight is a padding entry and
/// contributes nothing, which is what lets the groups stay padded rather than ragged.
fn step(g: &Group, at: P, other: P, eps: f64) -> P {
    let share = 1.0 / dist(at, other).max(eps);
    let (mut total, mut acc) = (0.0, [0.0; 2]);
    for (pt, w) in g.pts.iter().zip(&g.w) {
        if *w <= 0.0 {
            continue;
        }
        let pull = w / dist(*pt, at).max(eps);
        total += pull;
        acc[0] += pull * pt[0];
        acc[1] += pull * pt[1];
    }
    [
        (acc[0] + other[0] * share) / (total + share),
        (acc[1] + other[1] * share) / (total + share),
    ]
}

/// A group's weighted centre: the solve's starting point. A group of no weight has no
/// centre, and the floor holds the result at the origin rather than dividing by zero.
fn centroid(g: &Group) -> P {
    let (mut total, mut acc) = (0.0, [0.0; 2]);
    for (pt, w) in g.pts.iter().zip(&g.w) {
        total += *w;
        acc[0] += *w * pt[0];
        acc[1] += *w * pt[1];
    }
    [
        acc[0] / total.max(WEIGHT_FLOOR),
        acc[1] / total.max(WEIGHT_FLOOR),
    ]
}

/// The `k` nearest bundles to bundle `u` by proximity distance, ties by the lower index —
/// the order a stable sort of the index-ordered row gives. A full scan of the level, never a
/// partial sort: `k` is small and a fixed prefix of a total order is what makes the
/// candidate set the same on every run.
fn nearest(u: u32, level: &[Bundle], k: usize) -> Vec<u32> {
    let here = oriented(&level[u as usize]);
    let mut row: Vec<(f64, u32)> = level
        .iter()
        .enumerate()
        .filter(|&(v, _)| v as u32 != u)
        .map(|(v, other)| (prox(here, oriented(other)), v as u32))
        .collect();
    row.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    row.truncate(k.min(row.len()));
    row.into_iter().map(|(_, v)| v).collect()
}
