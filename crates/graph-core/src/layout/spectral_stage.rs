//! Registry-facing entry points for `layout.spectral` and `layout.pivot_mds`: the
//! layouts' own `run` returns per-component reports, the registry wants a bare
//! [`Geometry`]. A run in which every attempted component failed the residual gate is
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

/// `layout.pivot_mds` at its defaults.
pub fn pivot_mds(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds::run(topology)
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
        let m = run("layout.pivot_mds", pivot_mds);
        assert_ne!(s.to_bytes(), m.to_bytes(), "two layouts, two pictures");
    }
}
