//! The session's own columns, read without copying: the simple graph the tick integrates
//! from, and the two velocity columns it accumulates into.
//!
//! # Why these are `&self` and not a snapshot
//!
//! [`ForceSession`] owns one `Sim`, and every column below is that `Sim`'s own `Vec`, so a
//! reader that wants the *live* state — a device preparing a tick, a host drawing a frame —
//! can have it with no copy and no second implementation to keep honest. The price is the
//! borrow: each slice is good until the session's next `&mut` call, which for a
//! particle-mesh session is the next [`step`](ForceSession::step) (a mesh tick swaps the
//! position and velocity columns with its scratch, `particle_mesh/motion.rs`) and for every
//! session the next [`grow`](ForceSession::grow) (which appends rows and may move the
//! storage). A caller that needs the bytes past that copies them.
//!
//! No `&mut` method and no new type: these are the three reads the live input needs, and a
//! writer here would be a second path into the same `Sim`.

use super::ForceSession;

impl ForceSession {
    /// The simple graph the tick integrates from: `(lo, hi, strength)`, one entry per simple
    /// edge in edge order — the same three columns [`mesh_probe`](ForceSession::mesh_probe)
    /// clones.
    ///
    /// Borrows `self.sim.graph` for the slices' lifetime. Good until the next `&mut` call on
    /// this session: a [`grow`](ForceSession::grow) appends edges and may move the storage.
    pub fn simple_edges(&self) -> (&[u32], &[u32], &[f64]) {
        let graph = &self.sim.graph;
        (&graph.lo, &graph.hi, &graph.strength)
    }

    /// The `vx` column, one per node in row order — the velocity the next
    /// [`step`](ForceSession::step) integrates `x` from.
    ///
    /// Borrows `self.sim.vx`. Good until the next `&mut` call: a particle-mesh
    /// [`step`](ForceSession::step) swaps it with the tick's scratch, and a
    /// [`grow`](ForceSession::grow) appends a row per node.
    pub fn vxs(&self) -> &[f64] {
        &self.sim.vx
    }

    /// The `vy` column, one per node in row order — the velocity the next
    /// [`step`](ForceSession::step) integrates `y` from.
    ///
    /// Borrows `self.sim.vy`. Good until the next `&mut` call: a particle-mesh
    /// [`step`](ForceSession::step) swaps it with the tick's scratch, and a
    /// [`grow`](ForceSession::grow) appends a row per node.
    pub fn vys(&self) -> &[f64] {
        &self.sim.vy
    }
}

#[cfg(test)]
#[path = "columns/tests.rs"]
mod tests;
