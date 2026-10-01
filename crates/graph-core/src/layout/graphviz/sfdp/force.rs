//! The sfdp force model: Barnes-Hut repulsion, edge attraction, and the step control.
//!
//! Reference: `lib/sfdpgen/spring_electrical.c` of the pinned Graphviz 16.1.0 release, read
//! as an algorithm reference and reimplemented (`docs/decisions/graphviz-oracle.md`). The
//! constants are the reference's own: `C = 0.2`, `bh = 0.6`, `tol = 0.001`, `cool = 0.90`,
//! `step = 0.1`, `maxiter = 500` (`spring_electrical.c:37-47`, `:58-60`).
//!
//! **Gather form (D10).** Every force is written as a function of node `i`'s own position and
//! its neighbours' positions, never as an in-place accumulation into a shared array. The
//! reference accumulates (`force[i] += ...`) and then moves vertex by vertex, which is
//! sequential in the vertex loop; the port instead computes each node's total force from the
//! *previous* iteration's positions and then applies it, which is the same arithmetic in an
//! order that does not depend on iteration order and is bit-identical native vs wasm32.

/// `C` (`spring_electrical.c:37`): the attraction constant in `f_a = C · d² / K · d_ij`.
pub(super) const C: f64 = 0.2;

/// Barnes-Hut opening criterion (`spring_electrical.c:43`): a cell whose width over the
/// distance to it is below `bh` is treated as one supernode.
pub(super) const BH: f64 = 0.6;

/// Convergence floor (`spring_electrical.c:47`): the loop stops once `step <= tol / K`.
pub(super) const TOL: f64 = 0.001;

/// The cooling factor (`spring_electrical.c:49`), used by both arms of `update_step`.
pub(super) const COOL: f64 = 0.90;

/// The initial step size (`spring_electrical.c:58`).
pub(super) const STEP: f64 = 0.1;

/// The iteration cap (`spring_electrical.c:57`).
pub(super) const MAX_ITER: u32 = 500;

/// The repulsive exponent `p` the reference settles on for a non-power-law graph
/// (`spring_electrical.c:279-283`): `ctrl->p` is `AUTOP`, which resolves to `-1` unless the
/// graph is power-law, in which case it is `-1.8`. The motor's fixtures are synthetic
/// pseudo-random graphs, so `-1` is the branch that runs; the power-law test is not ported
/// and is named in the module's `Ponytail` note.
pub(super) const P: f64 = -1.0;

/// `average_edge_length` (`spring_electrical.c:153-169`): the mean edge length of the current
/// positions, which becomes the ideal edge length `K` whenever the graph sets no `K`.
pub(super) fn average_edge_length(edges: &[(u32, u32)], x: &[f64], y: &[f64]) -> f64 {
    if edges.is_empty() {
        return 1.0;
    }
    let mut total = 0.0;
    for &(i, j) in edges {
        let dx = x[i as usize] - x[j as usize];
        let dy = y[i as usize] - y[j as usize];
        total += libm::sqrt(dx * dx + dy * dy);
    }
    total / edges.len() as f64
}

/// `update_step` (`spring_electrical.c:171-185`), adaptive arm.
///
/// The reference holds the step still while the force norm is within 5% of the previous one,
/// cools by `cool` when the norm did not improve, and warms by `0.99/cool` when it improved
/// clearly. Reproducing the *hold* branch matters: without it the step decays monotonically and
/// the layout stops short of its own convergence test.
pub(super) fn update_step(step: f64, norm: f64, previous: f64) -> f64 {
    if norm >= previous {
        COOL * step
    } else if norm > 0.95 * previous {
        step
    } else {
        0.99 * step / COOL
    }
}

/// The attractive force constant `CRK = C^((2-p)/3) / K` (`spring_electrical.c:291`).
pub(super) fn crk(k: f64) -> f64 {
    libm::pow(C, (2.0 - P) / 3.0) / k
}

/// The repulsive prefactor `KP = K^(1-p)` (`spring_electrical.c:290`).
pub(super) fn kp(k: f64) -> f64 {
    libm::pow(k, 1.0 - P)
}

/// The edge attraction on node `i` from one edge `(i, j)`, gathered into `out`.
///
/// `CRK · (x_i - x_j) · ‖x_i - x_j‖` (`spring_electrical.c:320-328`), *subtracted*, and with
/// the reference's `ja[j] == i` self-edge skip applied by the caller's edge list — the
/// reference skips self loops, and a self loop would contribute exactly zero anyway.
pub(super) fn attract(out: &mut [f64; 2], i: u32, j: u32, x: &[f64], y: &[f64], crk: f64) {
    let (a, b) = (i as usize, j as usize);
    let dx = x[a] - x[b];
    let dy = y[a] - y[b];
    let dist2 = dx * dx + dy * dy;
    // A coincident pair has `dist == 0`; the reference's expression is then 0 * 0 = 0, so
    // taking the root first and multiplying would give `NaN` only if the distance were
    // divided by. Skipping the degenerate case states the same answer explicitly, which is
    // also what keeps a stacked drawing from producing NaN for every node at once.
    if dist2 <= 0.0 {
        return;
    }
    let dist = libm::sqrt(dist2);
    out[0] -= crk * dx * dist;
    out[1] -= crk * dy * dist;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `K` is the mean edge length, and a graph with no edges leaves it at the reference's
    /// 1.0 rather than dividing by zero.
    #[test]
    fn average_edge_length_is_the_mean_and_survives_an_empty_edge_set() {
        let x = [0.0, 3.0];
        let y = [0.0, 4.0];
        assert!((average_edge_length(&[(0, 1)], &x, &y) - 5.0).abs() < 1e-12);
        assert_eq!(average_edge_length(&[], &x, &y), 1.0);
    }

    /// Two edges of different length: the mean is over *edges*, not over nodes, and not the
    /// bounding-box diagonal. A port that averaged per node would get a different `K` and so a
    /// different attraction everywhere.
    #[test]
    fn average_edge_length_averages_over_edges() {
        let x = [0.0, 1.0, 10.0];
        let y = [0.0, 0.0, 0.0];
        assert!((average_edge_length(&[(0, 1), (1, 2)], &x, &y) - 5.0).abs() < 1e-12);
    }

    /// The three arms of the step control, which is the difference between converging and
    /// merely shrinking: cool when the norm got worse, hold when it is within 5%, warm when it
    /// improved clearly.
    #[test]
    fn update_step_cools_holds_and_warms() {
        assert!(
            (update_step(1.0, 2.0, 1.0) - COOL).abs() < 1e-12,
            "worse cools"
        );
        assert!(
            (update_step(1.0, 0.98, 1.0) - 1.0).abs() < 1e-12,
            "within 5% holds"
        );
        assert!(update_step(1.0, 0.5, 1.0) > 1.0, "clearly better warms");
    }

    /// Attraction points from `j` back toward `i` and is antisymmetric under swapping the
    /// ends, which is what makes the model an edge spring rather than a pull to the origin.
    #[test]
    fn attraction_is_antisymmetric_and_scales_with_distance() {
        let x = [0.0, 2.0];
        let y = [0.0, 0.0];
        let mut a = [0.0, 0.0];
        let mut b = [0.0, 0.0];
        attract(&mut a, 0, 1, &x, &y, 1.0);
        attract(&mut b, 1, 0, &x, &y, 1.0);
        assert!(
            (a[0] + b[0]).abs() < 1e-12 && (a[1] + b[1]).abs() < 1e-12,
            "{a:?} {b:?}"
        );
        assert!(a[0] > 0.0, "node 0 is pulled toward node 1, got {a:?}");
    }

    /// A coincident pair must not produce `NaN`: `dist` is 0, and the reference's expression
    /// is 0 · 0, not 0/0.
    #[test]
    fn coincident_nodes_attract_to_zero_rather_than_nan() {
        let x = [1.0, 1.0];
        let y = [1.0, 1.0];
        // A zeroed accumulator, as the caller has after the repulsion: a coincident pair adds
        // nothing to it and must not poison it.
        let mut out = [0.0, 0.0];
        attract(&mut out, 0, 1, &x, &y, 1.0);
        assert_eq!(
            out,
            [0.0, 0.0],
            "a coincident pair contributed force: {out:?}"
        );
    }

    #[test]
    fn the_constants_are_the_reference_s_own() {
        assert_eq!((C, BH, TOL, COOL, STEP), (0.2, 0.6, 0.001, 0.90, 0.1));
        assert_eq!(MAX_ITER, 500);
        assert_eq!(P, -1.0);
    }
}
