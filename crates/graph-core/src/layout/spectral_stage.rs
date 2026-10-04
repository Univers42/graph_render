//! Registry-facing entry points for `layout.spectral`, `layout.mds.pivot` and their two
//! 3D siblings: the layouts' own `run` returns per-component reports, the registry wants a
//! bare [`Geometry`]. A run in which every attempted component failed the residual gate is
//! refused (C12), never turned into a random picture.

use super::{Geometry, pivot_mds, spectral};
use crate::index::Topology;
use crate::stage::StageError;

const NOTHING_SOLVED: StageError = StageError::Param {
    name: "topology",
    rule: "no component passed the eigensolver's residual and orthonormality gate",
};

/// `layout.spectral` at its defaults.
pub fn spectral(topology: &Topology) -> Result<Geometry, StageError> {
    spectral::run(topology)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

/// `layout.mds.pivot` at its defaults.
pub fn pivot_mds(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds::run(topology)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

/// `layout.spectral3d` at its defaults: the reference's `_spectral_layout_3d`, whose `n < 4`
/// branch draws [`spectral::DEFAULT_SEED`] rather than a layout seed.
pub fn spectral_3d(topology: &Topology) -> Result<Geometry, StageError> {
    spectral_3d_seeded(topology, spectral::DEFAULT_SEED)
}

/// `layout.spectral3d` at an explicit layout seed, which is the arm the conformance harness
/// runs: below four nodes `_spectral_layout_3d:257-258` is `_random_layout`, and that draws
/// `RandomState(get_layout_seed())`. The same shape as `sfdp::run_seeded`.
pub fn spectral_3d_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    spectral::run_3d(topology, seed)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

/// `layout.mds.pivot3d` at its defaults.
pub fn pivot_mds_3d(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds_3d_seeded(topology, spectral::DEFAULT_SEED)
}

/// `layout.mds.pivot3d` at an explicit layout seed — see [`spectral_3d_seeded`], whose
/// `n < 4` guard is the same line of the reference (`_mds_layout_3d:283-284`).
pub fn pivot_mds_3d_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    pivot_mds::run_3d(topology, seed)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::{gate_node_count, run_with, seeded_model};
    use crate::weights::REFERENCE_DEGREE;

    #[test]
    fn both_entry_points_agree_with_the_layouts_they_wrap() {
        let (nodes, edges) = seeded_model(3, gate_node_count(3), REFERENCE_DEGREE);
        let run = |id, f: fn(&Topology) -> Result<Geometry, StageError>| {
            run_with(&nodes, &edges, id, f).expect("runs").snapshot
        };
        let s = run("layout.spectral", spectral);
        let m = run("layout.mds.pivot", pivot_mds);
        assert_ne!(s.to_bytes(), m.to_bytes(), "two layouts, two pictures");
    }
}
