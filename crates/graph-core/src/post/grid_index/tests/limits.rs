//! What the grid refuses, and how a zero-extent axis is measured (review findings R2, R10,
//! M21).

use super::*;
use graph_contract::snapshot::SnapshotError;

fn points(at: &[(f32, f32)]) -> NodeGeometry {
    NodeGeometry::Point {
        x: at.iter().map(|p| p.0).collect(),
        y: at.iter().map(|p| p.1).collect(),
    }
}

fn refusal(nodes: &NodeGeometry, params: &GridParams) -> StageError {
    GridIndex::new()
        .build(nodes, params)
        .expect_err("refused, not built")
}

#[test]
fn a_resolution_whose_cell_count_overflows_u32_is_refused() {
    let params = GridParams {
        resolution: 65_536,
        margin: 2,
        clearance: 0.0,
    };
    let err = refusal(&points(&[(0.0, 0.0), (1.0, 1.0)]), &params);
    assert!(
        matches!(
            err,
            StageError::Param {
                name: "resolution",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_margin_past_half_of_u32_is_refused_rather_than_overflowed() {
    let params = GridParams {
        margin: u32::MAX,
        ..small()
    };
    let err = refusal(&points(&[(0.0, 0.0), (1.0, 1.0)]), &params);
    assert!(
        matches!(
            err,
            StageError::Param {
                name: "resolution",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn the_ceiling_is_u32_max_over_8_cells() {
    // A square span of `r` makes the cell exactly 1 and the grid exactly r × r.
    let square = |r: u32| {
        let side = f64::from(r);
        let params = GridParams {
            resolution: r,
            ..small()
        };
        build::axes(&[(0.0, side, 0.0, side)], &params)
    };
    assert_eq!(
        square(23_170),
        Ok((1.0, 23_170, 23_170)),
        "536 848 900 cells"
    );
    assert!(square(23_171).is_err(), "536 895 241 cells > u32::MAX / 8");
}

#[test]
fn a_zero_extent_axis_is_measured_over_the_references_floor_span() {
    // `routed.py:55` floors each axis span at 1e-9 before dividing, so the flat y axis is
    // one cell wide plus the margin on both sides: 5, not 4.
    let mut grid = GridIndex::new();
    let nodes = points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
    grid.build(&nodes, &GridParams::default()).expect("builds");
    assert_eq!(grid.shape(), (132, 5));
}

#[test]
fn a_negative_size_is_refused_rather_than_marking_nothing() {
    let nodes = NodeGeometry::Box {
        x: vec![0.0, 4.0, 8.0],
        y: vec![0.0, 4.0, 8.0],
        w: vec![0.0, -2.0, 0.0],
        h: vec![0.0, 1.0, 0.0],
    };
    let err = refusal(&nodes, &small());
    assert!(
        matches!(
            err,
            StageError::Snapshot(SnapshotError::Negative { index: 1, .. })
        ),
        "{err:?}"
    );
}

#[test]
fn a_negative_or_non_finite_clearance_is_refused() {
    let nodes = points(&[(0.0, 0.0), (8.0, 8.0)]);
    for clearance in [-0.5, f64::NAN, f64::INFINITY] {
        let params = GridParams {
            clearance,
            ..small()
        };
        let err = refusal(&nodes, &params);
        assert!(
            matches!(
                err,
                StageError::Param {
                    name: "clearance",
                    ..
                }
            ),
            "{err:?}"
        );
    }
}
