//! The warm start (`create_warm`): a session seeded on a layout's drawn centres rather than on
//! the spiral, and its two refusals.

use super::super::{Engine, create, create_warm, reset};
use super::fixture::{bits, model, params, wire_of};
use crate::errors::Code;
use graph_contract::geometry::NodeGeometry;
use graph_core::Geometry;
use graph_core::post::centres;
use graph_core::registry::LAYOUTS;

fn ran(layout: &str, nodes: u32) -> Geometry {
    let entry = LAYOUTS
        .iter()
        .find(|entry| entry.id == layout)
        .expect("registered");
    (entry.run)(&model(3, nodes)).expect("the layout runs")
}

fn widened(column: &[f32]) -> Vec<f64> {
    column.iter().map(|&value| f64::from(value)).collect()
}

/// The session starts exactly where the layout drew each node: both columns, bit for bit, for
/// both engines.
#[test]
fn a_warm_session_starts_on_the_layouts_centres() {
    let geometry = ran("layout.grid", 24);
    let (xs, ys) = centres(&geometry.nodes);
    for engine in [Engine::BarnesHut, Engine::ParticleMesh] {
        reset();
        let id = create_warm(&model(3, 24), Some(&geometry), params(), engine).expect("warm");
        assert_eq!(bits(&wire_of(id, 0)), bits(&widened(xs)), "{engine:?} x");
        assert_eq!(bits(&wire_of(id, 1)), bits(&widened(ys)), "{engine:?} y");
    }
}

/// The defect this exists for: a cold session's start ignores the layout, so two layouts gave
/// the live view one picture. Two warm sessions over two layouts start on two pictures.
#[test]
fn two_layouts_seed_two_different_sessions() {
    reset();
    let topology = model(3, 24);
    let cold = create(&topology, params(), Engine::BarnesHut).expect("cold");
    let grid = create_warm(
        &topology,
        Some(&ran("layout.grid", 24)),
        params(),
        Engine::BarnesHut,
    );
    let circle = ran("layout.circular.ring", 24);
    let circle = create_warm(&topology, Some(&circle), params(), Engine::BarnesHut);
    let (grid, circle) = (grid.expect("grid"), circle.expect("circle"));
    assert_ne!(bits(&wire_of(grid, 0)), bits(&wire_of(circle, 0)));
    assert_ne!(bits(&wire_of(cold, 0)), bits(&wire_of(grid, 0)));
}

#[test]
fn a_graph_with_no_run_has_nothing_to_seed_from() {
    reset();
    let refused = create_warm(&model(3, 8), None, params(), Engine::BarnesHut);
    assert_eq!(refused, Err(Code::NoGeometryYet));
}

/// A centre that is not finite, or a column that is not one per node, is geometry the motor
/// did not write: refused by name, and no session is created.
#[test]
fn a_tampered_centre_is_refused() {
    reset();
    let mut geometry = ran("layout.grid", 8);
    let (NodeGeometry::Point { x, .. }
    | NodeGeometry::Circle { x, .. }
    | NodeGeometry::Box { x, .. }) = &mut geometry.nodes;
    x[0] = f32::NAN;
    let refused = create_warm(&model(3, 8), Some(&geometry), params(), Engine::BarnesHut);
    assert_eq!(refused, Err(Code::TamperedGeometry));
    let other = ran("layout.grid", 9);
    let refused = create_warm(&model(3, 8), Some(&other), params(), Engine::BarnesHut);
    assert_eq!(
        refused,
        Err(Code::TamperedGeometry),
        "nine centres for eight nodes"
    );
}
