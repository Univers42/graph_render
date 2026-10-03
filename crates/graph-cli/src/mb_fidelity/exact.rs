//! The exact all-pairs many-body sum, and the per-node error of a solver against it.
//!
//! This is the *reference*, and it lives in graph-cli rather than in graph-core on purpose:
//! graph-core is the thing under measurement, and a reference it also owns is a reference
//! that cannot contradict it. The law is d3's, line for line, from the pinned
//! `d3-force-3.0.0/src/manyBody.js` — the `apply` function's per-pair tail:
//!
//! ```text
//! if (l >= distanceMax2) return;                      // :77, the cutoff
//! if (x === 0) x = jiggle(random), l += x * x;        // :82, coincidence
//! if (y === 0) y = jiggle(random), l += y * y;        // :83
//! if (l < distanceMin2) l = sqrt(distanceMin2 * l);   // :84
//! node.vx += x * strength * alpha / l;                // :88, alpha = 1 here
//! ```
//!
//! **The one liberty is the coincidence jiggle, and it is taken deliberately.** d3 jitters
//! the zeroed axis with a value from the simulation's own seeded generator; Barnes-Hut
//! replaces that with a counter hash (`charge.rs`'s `settle`, devil C8). Reproducing either
//! here would measure the two *jiggles*, not the two *solvers*, so a pair at exactly equal
//! coordinates is skipped and counted instead. The golden-angle seed never produces one and
//! a settled layout essentially never does, so the count is the honest statement of how much
//! of the sum this reference actually skipped — and a run with a non-zero count says so in
//! its output rather than quietly reporting a number that is missing terms.
//!
//! The sum is the one d3 would do with no tree: every ordered pair `(i, j)`, `j != i`, in
//! dense index order, folded into `i`'s accumulator. Barnes-Hut at `θ = 0` visits every cell
//! and therefore every point, but visits them in the quadtree's preorder, so the two agree
//! to rounding and not bit for bit — which is why the self-check is a tolerance
//! ([`SELF_CHECK_TOLERANCE`]) and not an equality.

/// How close Barnes-Hut at `θ = 0` must come to this sum for the reference to be trusted.
///
/// 1e-9 relative RMS. Caveat: it is a threshold on a heuristic comparison, not a derived
/// constant — it is three orders of magnitude above the `~1e-15` two different summation
/// orders of the same `f64` terms can differ by at accumulation depths this size, and four
/// orders below the error a real defect in either side produces. Failing input: a set whose
/// nodes pile up so tightly that catastrophic cancellation in `sqrt(Σ|F*|²)` dominates —
/// direction then is a *larger* reported relative error, never a smaller one. Escape hatch:
/// lower `n` and re-run; if the self-check holds there and fails at size, the reference is
/// fine and the summation is the thing to look at.
pub const SELF_CHECK_TOLERANCE: f64 = 1e-9;

/// The parameters the exact sum reads, so a caller cannot pass a `distance_min` the
/// solvers did not run under.
#[derive(Debug, Clone, Copy)]
pub struct Law {
    /// `distanceMin`, squared, as `manyBody.js:13` holds it.
    pub dmin2: f64,
    /// `distanceMax`, squared.
    pub dmax2: f64,
    /// `chargeStrength`, the constant every node carries (`params.rs`).
    pub charge: f64,
}

/// What one exact sum produced: the force per node in row order, and how many pairs it
/// skipped as coincident.
///
/// The count is reported, never folded into the force: a skipped pair is a term this sum
/// does not have, and a caller that does not read the count would read a number that is
/// quietly short of the sum it is compared against.
#[derive(Debug, Clone)]
pub struct Exact {
    /// `(x, y)` per node, in row order.
    pub force: Vec<(f64, f64)>,
    /// Ordered pairs skipped because one axis of the offset was exactly zero.
    pub coincident: u64,
}

/// The exact many-body force at `(xs, ys)`: every ordered pair, d3's per-pair rule, at
/// `alpha` 1.
///
/// `O(n²)` and the point of it: this is the definition the solvers are measured against,
/// so it is written as the definition reads rather than as something faster.
pub fn sum(xs: &[f64], ys: &[f64], law: Law) -> Exact {
    let mut out = Exact {
        force: vec![(0.0, 0.0); xs.len()],
        coincident: 0,
    };
    for i in 0..xs.len() {
        let (xi, yi) = (xs[i], ys[i]);
        let (mut ax, mut ay) = (0.0, 0.0);
        for j in 0..xs.len() {
            if i == j {
                continue;
            }
            let (dx, dy) = (xs[j] - xi, ys[j] - yi);
            let mut l = dx * dx + dy * dy;
            if l >= law.dmax2 {
                continue;
            }
            if dx == 0.0 || dy == 0.0 {
                out.coincident += 1;
                continue;
            }
            if l < law.dmin2 {
                l = f64::sqrt(law.dmin2 * l);
            }
            let f = law.charge / l;
            ax += dx * f;
            ay += dy * f;
        }
        out.force[i] = (ax, ay);
    }
    out
}

/// How far a solver's force is from `exact`, in the three numbers a row prints.
///
/// Relative, throughout: an absolute tolerance would grade a solver on how far the layout
/// happens to have spread, which is not a property of the solver.
#[derive(Debug, Clone, Copy)]
pub struct Error {
    /// `sqrt(Σ|F − F*|²) / sqrt(Σ|F*|²)` over both components of every node.
    pub rms: f64,
    /// The per-node relative error's median.
    pub p50: f64,
    /// Its 99th percentile.
    pub p99: f64,
    /// How many nodes were scored: those with `|F*| > 0`.
    pub scored: u32,
}

/// Compares `got` against `exact`. `exact`'s `coincident` count is *not* consulted: a
/// skipped pair is missing from both sides the same way, since the solver saw the same
/// positions, and the caller's row reports the count beside the error.
pub fn error(got: &[(f64, f64)], exact: &[(f64, f64)]) -> Error {
    let mut num = 0.0;
    let mut den = 0.0;
    let mut per_node: Vec<f64> = Vec::with_capacity(got.len());
    for (g, e) in got.iter().zip(exact) {
        let (ex, ey) = (g.0 - e.0, g.1 - e.1);
        num += ex * ex + ey * ey;
        den += e.0 * e.0 + e.1 * e.1;
        let mag = f64::sqrt(e.0 * e.0 + e.1 * e.1);
        if mag > 0.0 {
            per_node.push(f64::sqrt(ex * ex + ey * ey) / mag);
        }
    }
    Error {
        rms: f64::sqrt(num) / f64::sqrt(den),
        p50: quantile(&mut per_node, 0.50),
        p99: quantile(&mut per_node, 0.99),
        scored: per_node.len() as u32,
    }
}

/// The `q` quantile of `values`, by the nearest-rank method on the sorted values.
///
/// Nearest-rank and not an interpolation: this number grades a solver, and a quantile that
/// depends on two neighbours' average is a number whose last digits move when a node is
/// added. Nearest-rank reads one value, so it is the value at some node.
fn quantile(values: &mut [f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let rank = (q * values.len() as f64).ceil().max(1.0) as usize;
    values[rank.min(values.len()) - 1]
}
