//! What the pass refuses, and what it is registered as.
//!
//! Split out of the parent so neither file outgrows the house 300-line limit. These are the
//! boundaries: the z column, one case per parameter, the empty graph, a node count that
//! disagrees with the topology, and the registration the hash gate and the ledger name.

use super::*;
use crate::post::{PostRun, find};

/// **The z refusal, no new error variant.** A 3D geometry is refused rather than damaged,
/// which is what keeps `contract-3d-verdict.md` condition 6 unamended.
#[test]
fn a_geometry_with_a_z_column_is_refused() {
    let n = 10;
    let topology = path_topology(n);
    let input = Geometry::in_space(
        NodeGeometry::Circle {
            x: vec![0.0; n],
            y: vec![0.0; n],
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
        vec![0.0; n],
    );
    let err = run(&topology, &input).expect_err("refused");
    assert_eq!(
        err,
        StageError::Param {
            name: "geometry.z",
            rule: "must be absent"
        }
    );
}

/// Refused rather than clamped, one case per parameter.
#[test]
fn bad_parameters_are_refused() {
    let topology = path_topology(4);
    let input = stacked(4, 1.0);
    for (params, name) in [
        (
            SeparateParams {
                margin: -1.0,
                ..SeparateParams::default()
            },
            "margin",
        ),
        (
            SeparateParams {
                max_iterations: 0,
                ..SeparateParams::default()
            },
            "max_iterations",
        ),
        (
            SeparateParams {
                point_radius: -1.0,
                ..SeparateParams::default()
            },
            "point_radius",
        ),
        (
            SeparateParams {
                margin: f64::NAN,
                ..SeparateParams::default()
            },
            "margin",
        ),
    ] {
        let err = separate(&topology, &input, &params).expect_err("refused");
        match err {
            StageError::Param { name: got, .. } => assert_eq!(got, name),
            other => panic!("{name}: expected Param, got {other}"),
        }
    }
}

/// An empty graph is not an error, the same rule the routing grid follows: no nodes, no
/// discs, no work. The topology is genuinely empty, so this is the "no nodes" case rather
/// than a count mismatch.
#[test]
fn an_empty_layout_is_not_an_error() {
    let empty: Topology = crate::index::index_model(&[], &[]).expect("fits");
    let bundled = run(&empty, &stacked(0, 1.0)).expect("runs");
    assert_eq!(bundled.pairs, 0);
    assert_eq!(bundled.unbundled, 0);
}

/// **A geometry whose node count disagrees with the topology is refused**, not drawn: the
/// columns would index past the edge endpoints they have to match, and a mismatch here means
/// the caller wired two different graphs together.
#[test]
fn a_node_count_mismatch_is_refused() {
    let topology = path_topology(9);
    let err = run(&topology, &stacked(4, 1.0)).expect_err("refused");
    assert_eq!(
        err,
        StageError::Param {
            name: "node count",
            rule: "the topology's own",
        }
    );
}
/// The pass is registered, so the hash gate and the ledger can name it.
#[test]
fn the_pass_is_registered_and_declares_it_moves_nodes() {
    let cap = find(ID).expect("registered");
    assert!(cap.meta.moves_nodes, "this pass moves nodes and says so");
    // `fn_addr_eq` rather than `assert_eq!` on the pointers: two functions can share an
    // address after merging, so pointer equality is not a statement about identity (clippy's
    // `unpredictable_function_pointer_comparisons`). This is what makes the check meaningful.
    assert!(
        std::ptr::fn_addr_eq(cap.run, run as PostRun),
        "the registry's `run` is not this module's `run`"
    );
}
