//! The adaptive-speed half of one FA2 iteration: the swing/traction pair, networkx's own
//! `estimate_factor`, and the step that applies the force. Split out of `state.rs` for the
//! house's 300-line limit, the same split `state/barnes_hut.rs` uses.
//!
//! **A pure move.** Every expression, every reduction order and every constant is the one
//! `state.rs` held; only the file they are written in. They travel together because they
//! are the three quantities networkx carries *cumulatively* across iterations — the
//! swing and the traction that feed the speed, and the step that spends it.
//!
//! `attraction`, `repulsion` and `gravity` — the forces themselves — stay in `state.rs`,
//! next to the pair loop they belong to.

use super::{Fa2State, MAX_DIM, norm2};

impl Fa2State {
    pub(super) fn swing_and_traction(&self) -> (f64, f64) {
        let (mut swing, mut traction) = (0.0, 0.0);
        let dim = self.dim;
        for i in 0..self.p.len() {
            let mut back = [0.0; MAX_DIM];
            let mut fwd = [0.0; MAX_DIM];
            for a in 0..dim {
                back[a] = self.p[i][a] - self.u[i][a];
                fwd[a] = self.p[i][a] + self.u[i][a];
            }
            swing += self.mass[i] * f64::sqrt(norm2(&back, dim));
            traction += 0.5 * self.mass[i] * f64::sqrt(norm2(&fwd, dim));
        }
        (swing, traction)
    }

    /// networkx's own `estimate_factor` helper, verbatim.
    pub(super) fn estimate_factor(&mut self, swing: f64, traction: f64) {
        let n = self.p.len() as f64;
        let jt = self.params.jitter_tolerance;
        let opt_jitter = 0.05 * f64::sqrt(n);
        let min_jitter = f64::sqrt(opt_jitter);
        let min_speed_efficiency = 0.05;
        let other = f64::min(10.0, opt_jitter * traction / (n * n));
        let mut jitter = jt * f64::max(min_jitter, other);
        if swing / traction > 2.0 {
            if self.speed_efficiency > min_speed_efficiency {
                self.speed_efficiency *= 0.5;
            }
            jitter = f64::max(jitter, jt);
        }
        let target_speed = if swing == 0.0 {
            f64::INFINITY
        } else {
            jitter * self.speed_efficiency * traction / swing
        };
        if swing > jitter * traction {
            if self.speed_efficiency > min_speed_efficiency {
                self.speed_efficiency *= 0.7;
            }
        } else if self.speed < 1000.0 {
            self.speed_efficiency *= 1.3;
        }
        self.speed += f64::min(target_speed - self.speed, 0.5 * self.speed);
    }

    pub(super) fn apply_update(&mut self) -> f64 {
        let mut moved = 0.0;
        let dim = self.dim;
        for i in 0..self.p.len() {
            let norm = f64::sqrt(norm2(&self.u[i], dim));
            let factor = self.speed / (1.0 + f64::sqrt(self.speed * self.mass[i] * norm));
            let mut step = 0.0;
            for a in 0..dim {
                let d = self.u[i][a] * factor;
                self.p[i][a] += d;
                step += f64::abs(d);
            }
            moved += step;
        }
        moved
    }
}
