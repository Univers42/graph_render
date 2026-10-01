//! Registry-facing entry points for `layout.spectral` and `layout.mds.pivot`: the
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

/// `layout.mds.pivot` at its defaults.
pub fn pivot_mds(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds::run(topology)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

/// `layout.spectral.3d`: [`spectral::run_at`] at the reference's `dims = 3`.
pub fn spectral_3d(topology: &Topology) -> Result<Geometry, StageError> {
    spectral::run_at(topology, spectral::DIMS_3D)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

/// `layout.mds.pivot.3d`: [`pivot_mds::run_at`] at the reference's `dims = 3`.
pub fn pivot_mds_3d(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds::run_at(topology, spectral::DIMS_3D)
        .map(|(geometry, _)| geometry)
        .map_err(|_| NOTHING_SOLVED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
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

    #[test]
    fn the_three_d_entry_points_agree_with_the_layouts_they_wrap() {
        let (nodes, edges) = seeded_model(3, gate_node_count(3), REFERENCE_DEGREE);
        let run = |id, f: fn(&Topology) -> Result<Geometry, StageError>| {
            run_with(&nodes, &edges, id, f).expect("runs").snapshot
        };
        for (id, entry) in [
            ("layout.spectral.3d", spectral_3d as fn(&Topology) -> _),
            ("layout.mds.pivot.3d", pivot_mds_3d as fn(&Topology) -> _),
        ] {
            let snapshot = run(id, entry);
            assert_eq!(
                snapshot.header().dim,
                graph_contract::snapshot::Dim::D3,
                "{id} must be labelled 3D"
            );
            assert_ne!(
                snapshot.parts().z.as_ref().map(Vec::len),
                None,
                "{id} must carry a z column"
            );
        }
    }

    #[test]
    fn a_2d_and_a_3d_run_of_the_same_layout_differ() {
        let (nodes, edges) = seeded_model(3, gate_node_count(3), REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("fits");
        let (flat, _) = spectral::run(&topology).expect("solves");
        let (spaced, _) = spectral::run_at(&topology, 3).expect("solves");
        assert_eq!(flat.dim(), graph_contract::snapshot::Dim::D2);
        assert_eq!(spaced.dim(), graph_contract::snapshot::Dim::D3);
        assert_eq!(flat.z, None);
        assert!(spaced.z.is_some());
    }
}
