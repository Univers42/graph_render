//! `_integrate` (`simulation.py:735-765`) plus the recentring `step` does right after it
//! (`simulation.py:1081-1082`).
//!
//! ```text
//! swing_v    = force - prev_force                                   f32
//! swing      = sqrt(einsum("ij,ij->i", swing_v, swing_v))           f32
//! trac_v     = 0.5 * (force + prev_force)                           f32  (Python float, weak)
//! traction   = sqrt(einsum("ij,ij->i", trac_v, trac_v))             f32
//! total_swing    = float(np.dot(mass, swing))                       f32 reduction, then f64
//! total_traction = float(np.dot(mass, traction))                     f32 reduction, then f64
//! target = jitter_tolerance * total_traction / total_swing           f64
//! speed  = clip(target, speed * 0.5, speed * 1.5); clip(speed, 1e-4, 10.0)
//! factor = speed / (1.0 + speed * np.sqrt(swing))                   f32  <- speed is *weak*
//! disp   = force * factor[:, None]                                   f32
//! norm   = sqrt(einsum("ij,ij->i", disp, disp))                     f32
//! disp[norm > k] *= k / norm[norm > k]                     f64 product, narrowed to f32
//! pos += disp                                                        f32
//! ```
//!
//! ## The weak scalar that decides this method's dtype
//!
//! `self.speed` is a Python `float` — `float(np.clip(...))` at `simulation.py:749-751` —
//! and under NEP 50 a Python float is **weak**: it does not promote an `f32` array the way
//! an `np.float64` scalar does. So `factor` above is `f32`, and so are `disp` and `norm`,
//! and `self.pos += disp` is an `f32` addition with no narrowing at all. `self.k` is the
//! opposite case: it comes from `np.cbrt`, so it is `np.float64`, and the one place it
//! enters (`cap / norm[over]`) is `f64`. Reading `speed` as `f64` — which is what `k`,
//! `self.repulsion` and `self.gravity` are — puts every move one `f32` ULP away from the
//! reference's.
//!
//! ## The one reduction this port cannot reproduce
//!
//! `np.dot(self.mass, swing)` is a BLAS **`sdot`** on `f32` operands — `f32` in, `f32` out,
//! `float()` only re-wraps it. It is *not* numpy's pairwise sum: measured in
//! `ge-python-oracle` over `n` in {4, 7, 8, 15, 16, 77, 128, 129, 200, 1000} at two seeds, it
//! matches the pairwise `f32` order at some sizes and not at others, matches
//! `f64`-then-narrowed at yet others, and matches none of them at every size.
//!
//! This port uses [`pairwise_f32`] — the nearest fixed order, and the one `mass.mean()` and
//! `coeff.sum(axis=1)` need as well, so a single implementation covers all three. D2 wants a
//! fixed-order reduction and this is the fixed order; matching OpenBLAS's kernel is not
//! available without an FMA.
//!
//! **Measured cost:** with the two BLAS products in `pair_force.rs` also on fixed orders,
//! the finished layouts differ from the reference arm's own `ref/FORCEATLAS2.f64` by a
//! median mean absolute `1.0e-6` across the 24 fixtures, worst single coordinate `6.2e-4`,
//! on coordinates of magnitude about `2.5`. That is the row's residual cause and the reason
//! it is not bitwise. `docs/measurements/sg-fa2-forcesim.md` has the probe and the numbers.

use super::reduce::{column_means_f32, dot3_f32, pairwise_f32};

/// `np.clip(self.speed, 1e-4, 10.0)` (`simulation.py:751`).
const SPEED_MIN: f64 = 1e-4;
const SPEED_MAX: f64 = 10.0;

pub(super) struct Integrator {
    /// `self._prev_force`: last iteration's field, zeroed at construction.
    prev: Vec<f32>,
    /// `self.speed`, carried across iterations exactly as the reference does — it is state,
    /// not a per-step constant, and resetting it would change every step after the first.
    speed: f64,
    /// `self.k`, the move cap: every move is clipped to it (`simulation.py:757-761`).
    k: f64,
    jitter_tolerance: f64,
}

impl Integrator {
    pub(super) fn new(n: usize, k: f64, jitter_tolerance: f64) -> Self {
        Self {
            prev: vec![0.0f32; 3 * n],
            speed: 1.0,
            k,
            jitter_tolerance,
        }
    }

    /// `_integrate(force)` followed by `step`'s recentring.
    ///
    /// `center_was` is the mean taken **before** the force field was computed
    /// (`simulation.py:1075`), which is why it is a parameter and not recomputed here.
    pub(super) fn apply(
        &mut self,
        pos: &mut [f32],
        force: &[f32],
        mass: &[f32],
        center_was: [f32; 3],
    ) {
        let swing = self.swing(force, mass);
        self.speed = self.next_speed(&swing, force, mass);
        self.move_nodes(pos, force, &swing);
        self.prev.copy_from_slice(force);
        if std::env::var("GM_FA2_TRACE").is_ok() {

        }
        recentre(pos, center_was);
    }

    /// `swing[i] = |force[i] - prev[i]|`: the `f32` three-term dot, then `sqrt`.
    fn swing(&self, force: &[f32], mass: &[f32]) -> Vec<f32> {
        (0..mass.len())
            .map(|i| {
                let mut v = [0.0f32; 3];
                for axis in 0..3 {
                    v[axis] = force[3 * i + axis] - self.prev[3 * i + axis];
                }
                dot3_f32(&v).sqrt()
            })
            .collect()
    }

    /// `clip(target, speed * 0.5, speed * 1.5)` then `clip(speed, 1e-4, 10.0)`.
    ///
    /// The 50% cap is the reference's own (`simulation.py:748`): one noisy step must not be
    /// able to oscillate, so `speed` rises by at most half and falls by at most half. The
    /// `total_swing > 0` guard matters too — a graph whose forces cancel exactly skips the
    /// update and leaves `speed` where it was, rather than dividing by zero.
    fn next_speed(&mut self, swing: &[f32], force: &[f32], mass: &[f32]) -> f64 {
        let traction: Vec<f32> = (0..mass.len())
            .map(|i| {
                let mut v = [0.0f32; 3];
                for axis in 0..3 {
                    v[axis] = 0.5f32 * (force[3 * i + axis] + self.prev[3 * i + axis]);
                }
                dot3_f32(&v).sqrt()
            })
            .collect();
        let w = weighted(mass, swing);
        let total_swing = pairwise_f32(&w);
        let t = weighted(mass, &traction);
        let total_traction = pairwise_f32(&t);
        if std::env::var("GM_FA2_TRACE").is_ok() {
            eprintln!("ts 0x{:08X} tt 0x{:08X} wbits {:?}", total_swing.to_bits(),
                total_traction.to_bits(),
                w.iter().map(|v| format!("0x{:08X}", v.to_bits())).collect::<Vec<_>>());
        }
        if total_swing > 0.0 {
            let target = self.jitter_tolerance * f64::from(total_traction)
                / f64::from(total_swing);
            self.speed = target.clamp(self.speed * 0.5, self.speed * 1.5);
        }
        self.speed.clamp(SPEED_MIN, SPEED_MAX)
    }

    /// `disp = force * factor[:, None]`, each move clipped to `k`, then `pos += disp`.
    ///
    /// **`factor`, `disp` and `norm` are `f32`, not `f64`** — and that is the whole reason
    /// this method exists in this shape. `self.speed` is a Python `float`
    /// (`float(np.clip(...))`, `simulation.py:749-751`), and under NEP 50 a Python float is
    /// **weak**: it does not promote an `f32` array. So `speed * np.sqrt(swing)` stays
    /// `f32`, `1.0 +` stays `f32`, `speed /` stays `f32`, and `force * factor[:, None]` is
    /// `f32 * f32`. Reading `self.speed` as an `f64` here — which is what `self.k` and
    /// `self.repulsion` are — is a difference of a relative `6e-8` per move, which is one
    /// `f32` ULP, which is a different layout.
    ///
    /// The cap is the one place `f64` returns: `cap` is `self.k`, an `np.float64`, so
    /// `cap / norm[over]` is `f64` and `disp[over] *= ...` narrows each product back.
    fn move_nodes(&self, pos: &mut [f32], force: &[f32], swing: &[f32]) {
        let speed = self.speed as f32;
        let mut disp = [0.0f32; 3];
        for i in 0..swing.len() {
            // `factor = speed / (1.0 + speed * np.sqrt(swing))`. `np.sqrt(swing)` is a
            // *second* square root of an already-normalised length; that is the reference's
            // own expression (`simulation.py:753`) and `swing` is not the same number.
            let factor = speed / (1.0f32 + speed * swing[i].sqrt());
            for axis in 0..3 {
                disp[axis] = force[3 * i + axis] * factor;
            }
            // `norm` is the `f32` three-term dot of the displacement, and the cap is
            // applied to the *displacement* before it is added — never to the position.
            let norm = dot3_f32(&disp).sqrt();
            if f64::from(norm) > self.k {
                let shrink = self.k / f64::from(norm);
                for value in disp.iter_mut() {
                    *value = (f64::from(*value) * shrink) as f32;
                }
            }
            for axis in 0..3 {
                pos[3 * i + axis] += disp[axis];
            }
        }
    }
}

/// `mass[i] * values[i]` in `f32` — the operands of the two `np.dot` calls.
fn weighted(mass: &[f32], values: &[f32]) -> Vec<f32> {
    mass.iter()
        .zip(values)
        .map(|(&m, &v)| m * v)
        .collect()
}

/// `self.pos -= (self.pos.mean(axis=0) - center_was).astype(DTYPE)`
/// (`simulation.py:1081`).
///
/// **The recentring is part of the step, not a tidy-up.** The reference re-centres after
/// every iteration, so the mean the next step starts from is the mean this one began at.
/// The subtraction is `f32` throughout: both means are `f32`, and the `.astype(DTYPE)` of an
/// `f32` difference is a no-op.
fn recentre(pos: &mut [f32], center_was: [f32; 3]) {
    let center_now = column_means_f32(pos, pos.len() / 3);
    for chunk in pos.chunks_exact_mut(3) {
        for (axis, value) in chunk.iter_mut().enumerate() {
            *value -= center_now[axis] - center_was[axis];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integrator(k: f64) -> Integrator {
        Integrator::new(2, k, 1.0)
    }

    #[test]
    fn a_move_longer_than_k_is_clipped_to_exactly_k() {
        let mut pos = [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let force = [1.0e6f32, 0.0, 0.0, -1.0e6, 0.0, 0.0];
        let mut sim = integrator(0.5);
        // speed is 1.0 and the previous force is zero, so swing = |force| and the factor is
        // 1 / (1 + 1) = 0.5; the displacement is far over the cap either way.
        sim.apply(&mut pos, &force, &mass, [0.0; 3]);
        let moved = f64::from(pos[0]) - 0.0;
        assert!(moved > 0.0, "node 0 must move outward");
        assert!(
            (moved - 0.5).abs() < 1e-6,
            "moved {moved}, expected the cap 0.5"
        );
    }

    /// The cap is a projection on the *displacement*: a node already inside the cap is not
    /// moved by it. Clipping the position instead would move every node to the sphere.
    #[test]
    fn a_move_shorter_than_k_is_left_alone() {
        let mut pos = [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let force = [1.0e-6f32, 0.0, 0.0, -1.0e-6, 0.0, 0.0];
        let mut sim = integrator(1.0);
        sim.apply(&mut pos, &force, &mass, [0.0; 3]);
        assert!(f64::from(pos[0]).abs() < 1e-9, "moved {}", pos[0]);
    }

    #[test]
    fn the_step_recentres_so_the_mean_does_not_drift() {
        let mut pos = [1.0f32, 2.0, 3.0, 4.0, 6.0, 9.0];
        let before = column_means_f32(&pos, 2);
        let mass = [1.0f32, 1.0];
        let force = [0.0f32; 6];
        let mut sim = integrator(1.0);
        sim.apply(&mut pos, &force, &mass, before);
        let after = column_means_f32(&pos, 2);
        assert_eq!(after, before);
    }

    /// A force field that is exactly zero everywhere has `total_swing == 0`, and the
    /// reference skips the speed update rather than dividing by it.
    #[test]
    fn a_zero_field_leaves_the_speed_where_it_was() {
        let mut pos = [1.0f32, 0.0, 0.0, 3.0, 0.0, 0.0];
        let mass = [1.0f32, 1.0];
        let mut sim = integrator(1.0);
        sim.apply(&mut pos, &[0.0f32; 6], &mass, [0.0; 3]);
        assert_eq!(sim.speed, 1.0);
    }

    #[test]
    fn the_speed_is_clamped_into_the_reference_band() {
        let mass = [1.0f32];
        let mut sim = Integrator::new(1, 1.0, 1.0);
        // A huge field and then a tiny one: the 50%-per-step cap must hold both ways, and
        // the floor must keep it off zero.
        let mut pos = [0.0f32; 3];
        sim.apply(&mut pos, &[1.0e9, 0.0, 0.0], &mass, [0.0; 3]);
        let after_big = sim.speed;
        assert!((SPEED_MIN..=SPEED_MAX).contains(&after_big), "{after_big}");
        sim.apply(&mut pos, &[1.0e-9, 0.0, 0.0], &mass, [0.0; 3]);
        assert!(sim.speed <= after_big * 1.5 + 1e-12, "{}", sim.speed);
        assert!(sim.speed >= SPEED_MIN);
    }
}

impl Integrator {
    /// Scratch accessor for the probe test.
    pub(super) fn k_of(&self) -> f64 {
        self.k
    }
}
