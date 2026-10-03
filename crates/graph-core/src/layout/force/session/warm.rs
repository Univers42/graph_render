//! The warm start: a session placed at positions a caller already has.

use super::{ForceSession, LiveParams, SessionError};
use crate::index::Topology;

impl ForceSession {
    /// A session that starts from `xs`/`ys` rather than from the spiral: the warm start a
    /// caller needs to nudge a picture it already has, or to continue one it stored.
    ///
    /// Both columns must hold exactly one value per node and be finite; the velocities
    /// start at rest, because a position is not a velocity and inventing one here would
    /// make the result depend on what the session happened to be before.
    pub fn from_positions(
        topology: &Topology,
        params: LiveParams,
        xs: &[f64],
        ys: &[f64],
    ) -> Result<Self, SessionError> {
        let mut session = Self::new(topology, params)?;
        session.set_positions(xs, ys)?;
        Ok(session)
    }

    /// Replaces both position columns, and the velocities with zeros.
    fn set_positions(&mut self, xs: &[f64], ys: &[f64]) -> Result<(), SessionError> {
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
}
