//! `layout.force.spring3d`: D9, determinism, and the snapshot the hash is taken from.
//!
//! Split from the sibling half so neither file crosses the house line cap. Together with
//! [`tests`](super::tests) these are the only 3D assertions; the algorithm itself is
//! asserted by the 2D tests, which run over the same code.

use super::tests::{columns, path_edges, run};
use super::{Spring3D, SpringParams};
use crate::layout::coords::probe::graph;
use crate::layout::snapshot;
use crate::stage::Stage;
use graph_contract::snapshot::{Dim, label_for};

/// D9 covers z. `first_non_finite` is the check both stages run, and it is reached with a
/// field whose third column is bad — asserted on the helper directly, because the kernel
/// cannot produce one (the 0.01 clip and the rescale's single reciprocal multiply keep it
/// finite), so the refusal is only observable at the check itself.
///
/// The negative control is the same field with the bad value in x, which must name `x`.
#[test]
fn a_non_finite_z_is_refused_under_node_z() {
    use crate::layout::force::spring::forces::{Field, first_non_finite};
    let mut bad_z: Field<3> = Field::zeros(2);
    bad_z.c[2][1] = f64::NAN;
    assert_eq!(first_non_finite(&bad_z), Some("node.z"));

    let mut bad_x: Field<3> = Field::zeros(2);
    bad_x.c[0][0] = f64::INFINITY;
    assert_eq!(
        first_non_finite(&bad_x),
        Some("node.x"),
        "the axis names itself"
    );

    let mut clean: Field<3> = Field::zeros(2);
    clean.c[2][0] = 1.5;
    assert_eq!(
        first_non_finite(&clean),
        None,
        "a finite field is not refused"
    );
    // And the stage itself does not refuse a run that is entirely finite.
    assert_eq!(
        run(&graph(1, &[])),
        Spring3D::run(&graph(1, &[]), &SpringParams::default()).expect("runs")
    );
}

/// The stage's own determinism at 3D: the same topology and parameters give the same three
/// columns, bit for bit — the property the hash gate checks across targets, on the arm that
/// will be hashed.
#[test]
fn the_3d_stage_is_bit_identical_twice_over() {
    let t = graph(14, &path_edges(14));
    assert_eq!(columns(&run(&t)), columns(&run(&t)));
}

/// The `iterations` knob reaches the 3D arm too, which is what "same kernel, new dimension"
/// means: the parameter that perturbs `layout.force.spring` perturbs this.
#[test]
fn the_iteration_parameter_moves_the_3d_layout() {
    let t = graph(12, &path_edges(12));
    let none = Spring3D::run(
        &t,
        &SpringParams {
            iterations: 0,
            ..SpringParams::default()
        },
    )
    .expect("runs");
    assert_ne!(
        columns(&none),
        columns(&run(&t)),
        "iterations is not reaching the 3D kernel"
    );
    let (_, _, z) = columns(&none);
    assert!(
        z.iter().all(|v| v.is_finite()),
        "an unsettled layout is still finite"
    );
}

/// z is in the **snapshot**, not only in the geometry — a `Geometry::z` that never reached
/// `SnapshotParts` would pass every test above and produce a 2D-labelled 0.4 file. Asserted
/// three ways: the label, the header's `dim`, and header byte 14, which is where a reader
/// dispatches on.
#[test]
fn a_snapshot_of_the_3d_geometry_carries_the_z_column_and_a_04_label() {
    let t = graph(5, &path_edges(5));
    let snap = snapshot(&t, run(&t)).expect("a 3D snapshot round-trips");
    assert_eq!(
        snap.parts().version,
        label_for(Dim::D3),
        "labelled with the 3D minor"
    );
    assert_eq!(snap.header().dim, Dim::D3);
    assert_eq!(
        snap.parts().z.as_ref().map(Vec::len),
        Some(5),
        "z is in the snapshot, not just the geometry"
    );
    assert_eq!(snap.to_bytes()[14], 1, "header byte 14 is the dim");
}

/// The 2D arm of the same topology stays 0.3 with no z. This is the pair of assertions that
/// says the two stages cannot be confused downstream, because their bytes cannot be.
#[test]
fn a_snapshot_of_the_2d_geometry_stays_labelled_03() {
    use crate::layout::force::spring::Spring;
    let t = graph(5, &path_edges(5));
    let g = Spring::run(&t, &SpringParams::default()).expect("runs");
    let snap = snapshot(&t, g).expect("a 2D snapshot round-trips");
    assert_eq!(snap.parts().version, label_for(Dim::D2));
    assert_eq!(snap.header().dim, Dim::D2);
    assert_eq!(snap.parts().z, None);
    assert_eq!(snap.to_bytes()[14], 0, "header byte 14 is the dim");
}

/// The two dimensions are told apart by the column alone: same node count, same shape, one
/// carrying z. Nothing is shared mutably between the two runs — they are pure functions of
/// the topology, so the two columns are independent values.
#[test]
fn the_two_dimensions_are_told_apart_by_the_column() {
    let t = graph(6, &path_edges(6));
    let flat =
        crate::layout::force::spring::Spring::run(&t, &SpringParams::default()).expect("runs");
    let solid = run(&t);
    assert_ne!(flat.dim(), solid.dim());
    let (solid_x, solid_y, solid_z) = columns(&solid);
    assert_eq!(solid_x.len(), solid_y.len());
    assert_eq!(solid_z.len(), solid_x.len(), "z is one value per node");
}

/// `Stage` is implemented over the crate's one error type, so a 3D run's refusal is a
/// `StageError` a caller already handles — and it names a real column when it prints.
#[test]
fn the_3d_stage_error_type_is_the_shared_one() {
    fn accepts<S: Stage<Params = SpringParams>>(_: S) {}
    accepts(Spring3D);
    let err = crate::stage::StageError::NonFinite { column: "node.z" };
    assert!(err.to_string().contains("node.z"), "{err}");
}
