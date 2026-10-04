//! `_igraph_fit_positions` on the motor arm: the arithmetic, and the five ids it applies to.
//!
//! Two things could be wrong with the fit and neither would show up as a wrong coordinate: it
//! could be applied to the wrong rows, or it could squash per axis instead of uniformly. The
//! second is invisible in a shape comparison — a squash preserves a shape's *topology* and
//! destroys its proportions — so the fixture below is chosen so a per-axis extent gives a
//! different answer from a shared one.

use super::{FITTED, fit};
use crate::oracle_python::conformance::fixtures::all;
use crate::oracle_python::conformance::motor::{columns, run};
use crate::oracle_python::conformance::{ROWS, SCALE};
use graph_core::{registry, run_with};

/// A drawing whose extent is 0 — every coordinate equal — is returned untouched: `extent > 0`
/// is the reference's own guard (`igraph_layouts.py:39`), and dividing by a zero extent would
/// hand the judge a column of NaN, which compares as "different" on every coordinate and says
/// nothing about which arm is right.
#[test]
fn a_flat_drawing_is_left_alone_rather_than_divided_by_zero() {
    let mut points = [[2.0_f64, 2.0, 2.0]; 3];
    fit(&mut points, SCALE);
    assert_eq!(points, [[0.0, 0.0, 0.0]; 3]);
    assert!(points.iter().all(|p| p.iter().all(|v| v.is_finite())));
}

/// The fit centres each axis on its own mean and scales by one factor shared by all three.
///
/// The numbers are chosen so every step is exact in `f64`: mean `(2, -1, 0)`, centred
/// `(-1, -2, 0)` and `(1, 2, 0)`, extent 2, factor `5 / 2`.
#[test]
fn the_fit_centres_every_axis_and_lands_exactly_on_the_scale() {
    let mut points = [[1.0_f64, -3.0, 0.0], [3.0, 1.0, 0.0]];
    fit(&mut points, SCALE);
    assert_eq!(
        points[0],
        [-2.5, -5.0, 0.0],
        "x or y was not centred then scaled"
    );
    assert_eq!(
        points[1],
        [2.5, 5.0, 0.0],
        "x or y was not centred then scaled"
    );
}

/// **One factor over all three axes, not one per axis.** A per-axis squash would centre this
/// drawing on itself and scale each axis to `+/-5`; the reference scales by the largest
/// magnitude anywhere, so `z` — already centred and small — keeps its proportion.
#[test]
fn the_extent_is_the_largest_magnitude_over_every_axis() {
    let mut points = [[0.0_f64, 0.0, 3.0], [10.0, 0.0, -1.0]];
    fit(&mut points, SCALE);
    // mean `(5, 0, 1)`; centred z spans `2` to `-2`, x spans `-5` to `5`; extent is 5.
    assert_eq!(points[0][2], 2.0, "z was squashed to the full scale");
    assert_eq!(points[1][2], -2.0, "z was squashed to the full scale");
    assert_eq!(points[0][0], -5.0);
    assert_eq!(points[1][0], 5.0);
}

/// The fit is the reference's convention and applies to the igraph rows only. Every other row's
/// bytes must be untouched by it, or a change meant for five rows has silently moved the rest.
#[test]
fn the_fit_reaches_the_igraph_rows_and_nothing_else() {
    assert_eq!(FITTED.len(), 5, "{FITTED:?}");
    for id in FITTED {
        assert!(registry::find(id).is_some(), "{id} is not registered");
    }
    // `davidson_harel` and `graphopt` go through the same reference helper and are deliberately
    // **not** listed: they are another job's rows. Named here so the omission is a decision.
    for id in ["layout.force.davidson_harel", "layout.force.graphopt"] {
        assert!(!FITTED.contains(&id), "{id} was added to this job's list");
    }
    // The rows this job owns all name a fitted id, or the fit is dead code.
    for name in [
        "IGRAPH_FR",
        "IGRAPH_KK",
        "IGRAPH_DRL",
        "IGRAPH_DRL_2D",
        "IGRAPH_LGL",
    ] {
        let row = ROWS.iter().find(|r| r.name == name).expect(name);
        let id = row.motor.expect("a motor layout");
        assert!(FITTED.contains(&id), "{name} -> {id} is not fitted");
    }
}

/// The arm really runs the fit: a row's coordinates come out centred and at `scale`, where the
/// raw layout does not. This is the end-to-end half of the two tests above.
#[test]
fn an_igraph_row_comes_back_centred_and_at_the_scale() {
    let fixture = all()
        .expect("the fixture set")
        .into_iter()
        .find(|f| f.name == "bipartite")
        .expect("the bipartite fixture");
    let points = run("layout.force.fruchterman_reingold.3d", &fixture).expect("FR ran");
    let largest = points
        .iter()
        .flat_map(|p| p.iter().map(|v| v.abs()))
        .fold(0.0_f64, f64::max);
    assert!(
        (largest - SCALE).abs() < 1e-9,
        "largest magnitude {largest} != {SCALE}"
    );
    for axis in 0..3 {
        let mean = points.iter().map(|p| p[axis]).sum::<f64>() / points.len() as f64;
        assert!(mean.abs() < 1e-9, "axis {axis} mean is {mean}");
    }
    // …and the un-fitted arm over the same fixture does not.
    let layout = registry::find("layout.force.fruchterman_reingold.3d").expect("registered");
    let raw = run_with(&fixture.nodes, &fixture.edges, layout.id, layout.run)
        .expect("FR ran")
        .snapshot
        .into_parts();
    let raw = columns(&raw, fixture.nodes.len()).expect("three columns");
    let raw_largest = raw
        .iter()
        .flat_map(|p| p.iter().map(|v| v.abs()))
        .fold(0.0_f64, f64::max);
    assert!(
        (raw_largest - SCALE).abs() > 1e-9,
        "the raw layout already sits at the scale, so the test cannot see the fit"
    );
}
