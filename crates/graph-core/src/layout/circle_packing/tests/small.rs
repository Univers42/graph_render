//! `n < 3` (`circle_packing.py:295-303`, exact, no note) and parameter validation.

use super::support::topology;
use crate::layout::circle_packing::{CirclePackingParams, run, run_with};
use crate::stage::StageError;
use graph_contract::geometry::NodeGeometry;

#[test]
fn default_params_match_scigraphs() {
    assert_eq!(
        CirclePackingParams::default(),
        CirclePackingParams {
            iterations: 500,
            scale: 5.0
        }
    );
}

#[test]
fn zero_nodes_is_empty_and_exact() {
    let geometry = run(&topology(0, &[])).expect("runs");
    assert_eq!(
        geometry.nodes,
        NodeGeometry::Circle {
            x: vec![],
            y: vec![],
            r: vec![]
        }
    );
    assert!(geometry.notes.is_empty());
}

#[test]
fn one_node_is_a_single_circle_at_the_origin() {
    let geometry = run(&topology(1, &[])).expect("runs");
    let want = NodeGeometry::Circle {
        x: vec![0.0],
        y: vec![0.0],
        r: vec![2.5],
    };
    assert_eq!(geometry.nodes, want);
    assert!(geometry.notes.is_empty());
}

#[test]
fn two_nodes_are_two_tangent_circles_either_side_of_the_origin() {
    let geometry = run(&topology(2, &[(0, 1)])).expect("runs");
    let want = NodeGeometry::Circle {
        x: vec![-1.25, 1.25],
        y: vec![0.0, 0.0],
        r: vec![1.25, 1.25],
    };
    assert_eq!(geometry.nodes, want);
    assert!(geometry.notes.is_empty());
}

#[test]
fn a_scale_that_is_not_finite_and_positive_is_refused() {
    let params_at = |scale| CirclePackingParams {
        iterations: 10,
        scale,
    };
    for scale in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
        let err = run_with(&topology(3, &[(0, 1)]), &params_at(scale)).expect_err("refused");
        assert_eq!(
            err,
            StageError::Param {
                name: "scale",
                rule: "finite and above 0"
            },
            "{scale}"
        );
    }
}
