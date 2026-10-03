//! `_gravity` (`simulation.py:726-733`), the weak-gravity branch.
//!
//! ```text
//! center = pos.mean(axis=0)                                   f32, sequential per column
//! delta  = pos - center                                       f32
//! dist   = sqrt(einsum("ij,ij->i", delta, delta))              f32
//! scale  = -(gravity * mass / maximum(dist, f32(1e-9)))        f64  <- gravity is np.float64
//! return (scale[:, None] * delta).astype(DTYPE)                f64, narrowed to f32
//! ```
//!
//! **`f64` throughout, because `self.gravity` is `np.float64`.** It is
//! `_raw_gravity * _gravity_norm`, and `_gravity_norm` comes from `self.k`, which comes
//! from `np.cbrt` — so the scalar is not weak and promotes the `f32` arrays under NEP 50.
//! Only the `.astype(DTYPE)` at the end narrows it.
//!
//! `strong_gravity` (`simulation.py:729-730`) is the other branch and is not ported; see
//! the module doc's `Ponytail (params)`.

use super::reduce::{column_means_f32, dot3_f32};

/// `np.maximum(dist, DTYPE(1e-9))`: the floor under the `1/d`, so a node sitting exactly on
/// the centre does not produce an infinity.
const MIN_DIST: f32 = 1e-9;

/// The gravity field, added into `force` by the caller.
pub(super) fn accumulate(force: &mut [f32], pos: &[f32], mass: &[f32], gravity: f64) {
    let center = column_means_f32(pos, mass.len());
    let mut delta = [0.0f32; 3];
    for i in 0..mass.len() {
        for axis in 0..3 {
            delta[axis] = pos[3 * i + axis] - center[axis];
        }
        let dist = dot3_f32(&delta).sqrt();
        let scale = -(gravity * f64::from(mass[i]) / f64::from(dist.max(MIN_DIST)));
        for axis in 0..3 {
            force[3 * i + axis] += (scale * f64::from(delta[axis])) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_pulls_every_node_towards_the_centre_of_mass() {
        // Two nodes symmetric about x = 0: both must be pulled inwards, and neither the y
        // nor the z component may move at all.
        let pos = [-2.0f32, 0.0, 0.0, 2.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let mut force = [0.0f32; 6];
        accumulate(&mut force, &pos, &mass, 0.028_008_094_259_695_266);
        assert!(force[0] > 0.0 && force[3] < 0.0, "inwards, got {force:?}");
        assert_eq!(force[1], 0.0);
        assert_eq!(force[4], 0.0);
        // Symmetric: equal and opposite.
        assert_eq!(force[0], -force[3]);
    }

    /// A node exactly on the centre has `dist = 0`, and `1/0` is floored rather than
    /// refused: the reference's `np.maximum(dist, DTYPE(1e-9))` makes the force zero there,
    /// because `delta` is zero too.
    #[test]
    fn a_node_on_the_centre_gets_no_force_and_no_infinity() {
        let pos = [-1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let mut force = [0.0f32; 6];
        accumulate(&mut force, &pos, &mass, 0.5);
        assert!(force.iter().all(|v| v.is_finite()), "{force:?}");
    }

    /// Gravity scales with mass: `gravity * mass / d`, so a node twice as heavy is pulled
    /// twice as hard from the same distance.
    #[test]
    fn the_pull_scales_with_the_nodes_mass() {
        let pos = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let light = [1.0f32, 1.0];
        let heavy = [1.0f32, 3.0];
        let mut a = [0.0f32; 6];
        let mut b = [0.0f32; 6];
        accumulate(&mut a, &pos, &light, 0.5);
        accumulate(&mut b, &pos, &heavy, 0.5);
        assert_eq!(b[3], 3.0 * a[3]);
    }
}
