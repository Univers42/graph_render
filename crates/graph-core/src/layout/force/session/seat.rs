//! Where a session's rows stand: seated on a picture a host is drawing, or sent back to the
//! spiral a new session starts on. Both keep the pins and copy into the columns in place, so
//! an address a host holds into them stays valid.

use super::ForceSession;
use super::error::SessionError;
use crate::layout::force::barnes_hut::seed::golden_spiral;

impl ForceSession {
    /// Replaces both position columns, and the velocities with zeros; pins and alpha are kept.
    ///
    /// Public so a host can seat a live session on the picture it is drawing: on 2026-10-01 the
    /// studio's session started from its own spiral, so the first drag on a finished layout
    /// replaced the whole drawing with it. The columns are copied in place, never resized, so an
    /// address a host holds into them stays valid.
    pub fn set_positions(&mut self, xs: &[f64], ys: &[f64]) -> Result<(), SessionError> {
        self.check_column("xs", xs)?;
        self.check_column("ys", ys)?;
        self.sim.x.copy_from_slice(xs);
        self.sim.y.copy_from_slice(ys);
        self.sim.vx.iter_mut().for_each(|v| *v = 0.0);
        self.sim.vy.iter_mut().for_each(|v| *v = 0.0);
        Ok(())
    }

    /// One warm-start column: the right length, and finite (D9 — wasm32 does not pin a
    /// NaN's bits).
    fn check_column(&self, column: &'static str, values: &[f64]) -> Result<(), SessionError> {
        let nodes = self.rows();
        if values.len() as u64 != u64::from(nodes) {
            return Err(SessionError::ColumnLength {
                column,
                got: values.len() as u64,
                nodes,
            });
        }
        if let Some(_bad) = values.iter().position(|v| !v.is_finite()) {
            return Err(SessionError::NonFinite { field: column });
        }
        Ok(())
    }

    /// Every row back on the golden-angle spiral [`new`](Self::new) seeds on, at rest, with
    /// the tick count and alpha a new session starts at; parameters, pins and the alpha
    /// target are kept.
    ///
    /// The studio's "Animate" restarts here. On 2026-10-01 it restarted from the random
    /// layout's unit square instead: 400 nodes inside `distance_min` of each other flew out to
    /// 55 000 units in three ticks. A restart replays a new session bit for bit
    /// (`session/tests/warm.rs`), so under the frozen parameters it settles into the batch
    /// layout (`session/tests/m1b.rs`).
    pub fn restart(&mut self) {
        let (xs, ys) = golden_spiral(self.rows());
        let sim = &mut self.sim;
        sim.x.copy_from_slice(&xs);
        sim.y.copy_from_slice(&ys);
        sim.vx.iter_mut().for_each(|v| *v = 0.0);
        sim.vy.iter_mut().for_each(|v| *v = 0.0);
        sim.tick_no = 0;
        sim.alpha = sim.params.initial_alpha;
    }
}
