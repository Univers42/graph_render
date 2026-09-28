//! Collins–Stephenson's radius solver: the angle at one packed corner, each vertex's
//! target angle sum (`_packing_aims`, `circle_packing.py:107-132`), and the Jacobi sweep
//! that finds radii whose corners close on those aims (`_solve_packing_radii`,
//! `:134-177`). Exact port, `f64` throughout; the only approximation is the fixed-point
//! iteration itself, which is SciGraphs' own algorithm, not a shortcut this port takes.
//!
//! **The solver and the placement walk use two different corner angles.** The reference
//! has one `_packing_angle` helper (`:93-105`) that *guards* `denom < 1e-12` and returns
//! `pi/3`, and it uses that helper only for placement (`:196`). The per-sweep angles at
//! `:160-161` are written inline and *clamp* instead — `np.maximum(2 * a * b, 1e-12)`
//! with no `pi/3` branch. So the solver's angle is [`solver_angle`] and placement keeps
//! [`packing_angle`]; conflating them puts a wrong angle in every corner of every flower
//! whose radii are small enough to underflow the denominator.

use core::f64::consts::PI;

/// Below this angle-sum error a sweep has converged (`circle_packing.py:134`, its own
/// `tolerance=1e-9` default — every caller in SciGraphs uses the default).
pub(super) const TOLERANCE: f64 = 1e-9;

/// One radius-solving run: the radii themselves and the worst free-vertex angle-sum
/// error the last sweep left behind.
pub(super) struct Solved {
    pub(super) radii: Vec<f64>,
    pub(super) max_error: f64,
}

/// The angle at the shared corner of three mutually tangent circles of radii `r_i`,
/// `r_j`, `r_k` — `_packing_angle`, `circle_packing.py:93-105`, which the reference uses
/// for **placement only** (`:196`). Degenerates to `PI / 3` when the three centres would
/// coincide (denominator underflow), exactly as the reference does. The solver needs
/// [`solver_angle`] instead.
pub(super) fn packing_angle(r_i: f64, r_j: f64, r_k: f64) -> f64 {
    let (a, b, c) = (r_i + r_j, r_i + r_k, r_j + r_k);
    let denom = 2.0 * a * b;
    if denom < 1e-12 {
        return PI / 3.0;
    }
    let cos_val = ((a * a + b * b - c * c) / denom).clamp(-1.0, 1.0);
    libm::acos(cos_val)
}

/// The same corner as [`packing_angle`], but in the form the **radius solver** uses
/// (`circle_packing.py:160-161`): `denom = np.maximum(2 * a * b, 1e-12)` *clamps* the
/// denominator and takes the `arccos` of whatever survives, where `_packing_angle`
/// *guards* it and returns `PI / 3` instead. The two differ by up to 2.1 radians once
/// `2ab` underflows, and only the placement walk may use the guard — see
/// `tests/solver/angle.rs::a_collapsed_sweep_sums_the_clamped_angles_and_not_sixty_degrees`,
/// which pins the kernel against the reference's own formula.
pub(super) fn solver_angle(r_i: f64, r_j: f64, r_k: f64) -> f64 {
    let (a, b, c) = (r_i + r_j, r_i + r_k, r_j + r_k);
    let denom = (2.0 * a * b).max(1e-12);
    libm::acos(((a * a + b * b - c * c) / denom).clamp(-1.0, 1.0))
}

/// Every vertex's target angle sum (`_packing_aims`, `circle_packing.py:107-132`): `2*PI`
/// inside, and on the boundary a share of `(len(boundary) - 2) * PI` in proportion to
/// each boundary vertex's triangle count, capped at `PI` and found by 60 rounds of
/// bisection — discrete Gauss–Bonnet, so the two halves add up exactly.
pub(super) fn packing_aims(at: &[Vec<(u32, u32)>], boundary: &[u32]) -> Vec<f64> {
    let mut aims = vec![2.0 * PI; at.len()];
    let counts: Vec<f64> = boundary
        .iter()
        .map(|&v| at[v as usize].len() as f64)
        .collect();
    let total = (boundary.len() as f64 - 2.0) * PI;
    let sum: f64 = counts.iter().sum();
    if boundary.len() < 3 || total <= 0.0 || sum == 0.0 {
        return aims;
    }
    let share = bisect_share(&counts, total);
    for (&v, &k) in boundary.iter().zip(&counts) {
        aims[v as usize] = (share * k).min(PI);
    }
    aims
}

/// The per-triangle-count multiplier whose capped sum matches `total`, by 60 rounds of
/// bisection over `[0, PI]` (`circle_packing.py:121-128`: 60 is the reference's own
/// fixed round count, not a tolerance this port chose).
fn bisect_share(counts: &[f64], total: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, PI);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let sum: f64 = counts.iter().map(|&k| (mid * k).min(PI)).sum();
        if sum < total {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Collins–Stephenson's Jacobi sweep (`_solve_packing_radii`, `circle_packing.py:134-177`):
/// each sweep replaces every vertex's flower with a uniform-neighbour flower of the same
/// angle sum, then rescales so it closes on the vertex's aim. `free` lists which vertices
/// move; the rest hold radius `1`. Vectorised in SciGraphs (Jacobi, not Gauss-Seidel, so
/// one sweep is one pass from a fixed snapshot) — this port keeps that property with
/// plain loops instead of `numpy`.
pub(super) fn solve_packing_radii(
    at: &[Vec<(u32, u32)>],
    aims: &[f64],
    free: &[u32],
    max_sweeps: u32,
) -> Solved {
    let n = at.len();
    let corners = corners_of(at);
    let mut free_mask = vec![false; n];
    for &f in free {
        free_mask[f as usize] = true;
    }
    let mut radii = vec![1.0; n];
    if corners.is_empty() || !free_mask.iter().any(|&f| f) {
        return Solved {
            radii,
            max_error: 0.0,
        };
    }
    let coeffs = Coeffs::build(&corners, aims, &free_mask, n);
    let mut max_error = 0.0;
    for _ in 0..max_sweeps.max(1) {
        let angle_sum = angle_sums(at, &radii);
        max_error = worst_free_error(&angle_sum, aims, &coeffs.free);
        if max_error < TOLERANCE {
            break;
        }
        sweep(&mut radii, &angle_sum, &coeffs);
    }
    Solved { radii, max_error }
}

fn corners_of(at: &[Vec<(u32, u32)>]) -> Vec<(u32, u32, u32)> {
    (0..)
        .zip(at)
        .flat_map(|(i, pairs)| pairs.iter().map(move |&(v, w)| (i, v, w)))
        .collect()
}

/// Precomputed per-vertex constants a sweep does not change: each vertex's corner count
/// (floored at 1), its `delta` target, and whether it moves.
struct Coeffs {
    counts: Vec<f64>,
    delta: Vec<f64>,
    free: Vec<bool>,
}

impl Coeffs {
    fn build(corners: &[(u32, u32, u32)], aims: &[f64], free_mask: &[bool], n: usize) -> Self {
        let mut counts: Vec<f64> = vec![0.0; n];
        for &(c, _, _) in corners {
            counts[c as usize] += 1.0;
        }
        for c in &mut counts {
            *c = c.max(1.0);
        }
        let delta = aims
            .iter()
            .zip(&counts)
            .map(|(&aim, &k)| libm::sin((aim / (2.0 * k)).min(0.5 * PI)))
            .collect();
        Self {
            counts,
            delta,
            free: free_mask.to_vec(),
        }
    }
}

/// Every vertex's corner angles summed into `out[i]` — the gather D10 requires: element
/// `i` reads only start-of-step `radii` and writes only its own slot, folding its own
/// corners in the **corner array's** order, which is exactly the order `np.bincount`
/// accumulates them in (`circle_packing.py:143`, `:162` — the corners are flattened
/// vertex-major, so a vertex's run of the array is its own flower, in flower order).
///
/// This replaces a scatter that walked the flat corner array writing into each centre's
/// accumulator; it is bit-identical to it, proved over seeded flowers by
/// `tests/solver/gather.rs::the_angle_sum_gather_is_bit_identical_to_the_scatter_over_seeded_flowers`.
/// The angle is [`solver_angle`], not [`packing_angle`] — the solver clamps its
/// denominator where placement guards it.
fn angle_sums(at: &[Vec<(u32, u32)>], radii: &[f64]) -> Vec<f64> {
    (0..at.len())
        .map(|i| {
            let r_i = radii[i];
            let mut sum = 0.0;
            for &(l, r) in &at[i] {
                sum += solver_angle(r_i, radii[l as usize], radii[r as usize]);
            }
            sum
        })
        .collect()
}

fn worst_free_error(angle_sum: &[f64], aims: &[f64], free: &[bool]) -> f64 {
    (0..angle_sum.len())
        .filter(|&i| free[i])
        .map(|i| (angle_sum[i] - aims[i]).abs())
        .fold(0.0, f64::max)
}

/// One Jacobi sweep: every free vertex's radius replaced by its uniform-neighbour
/// closure, then the whole array renormalised by its mean (`circle_packing.py:168-175`).
fn sweep(radii: &mut [f64], angle_sum: &[f64], c: &Coeffs) {
    for i in 0..radii.len() {
        if !c.free[i] {
            continue;
        }
        let beta = libm::sin(angle_sum[i] / (2.0 * c.counts[i])).min(1.0 - 1e-12);
        let uniform_neighbour = beta * radii[i] / (1.0 - beta);
        radii[i] = (uniform_neighbour * (1.0 - c.delta[i]) / c.delta[i]).max(1e-12);
    }
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    if mean > 0.0 {
        for r in radii.iter_mut() {
            *r /= mean;
        }
    }
}

#[cfg(test)]
mod tests;
