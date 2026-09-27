use super::*;

#[test]
fn every_node_kind_round_trips_through_its_tag() {
    for kind in [
        NodeGeometryKind::Point,
        NodeGeometryKind::Circle,
        NodeGeometryKind::Box,
    ] {
        assert_eq!(NodeGeometryKind::from_tag(kind.tag()), Ok(kind));
    }
}

#[test]
fn every_edge_kind_round_trips_through_its_tag() {
    for kind in [
        EdgeGeometryKind::Line,
        EdgeGeometryKind::Polyline,
        EdgeGeometryKind::Curve,
    ] {
        assert_eq!(EdgeGeometryKind::from_tag(kind.tag()), Ok(kind));
    }
}

#[test]
fn reserved_edge_tags_are_refused_as_reserved_not_unknown() {
    assert_eq!(
        EdgeGeometryKind::from_tag(RIBBON_TAG),
        Err(TagError::Reserved(3))
    );
    assert_eq!(
        EdgeGeometryKind::from_tag(ARC_TAG),
        Err(TagError::Reserved(4))
    );
    assert_eq!(EdgeGeometryKind::from_tag(5), Err(TagError::Unknown(5)));
    assert_eq!(NodeGeometryKind::from_tag(3), Err(TagError::Unknown(3)));
}

fn point(n: usize) -> NodeGeometry {
    NodeGeometry::Point {
        x: vec![0.5; n],
        y: vec![-0.5; n],
    }
}

fn paths(offsets: &[u32], points: usize) -> Paths {
    Paths {
        offsets: offsets.to_vec(),
        pts: vec![1.0; 2 * points],
    }
}

#[test]
fn every_value_carries_its_kind_and_lists_its_columns_in_wire_order() {
    let circle = NodeGeometry::Circle {
        x: vec![],
        y: vec![],
        r: vec![],
    };
    let boxes = NodeGeometry::Box {
        x: vec![],
        y: vec![],
        w: vec![],
        h: vec![],
    };
    let names = |g: &NodeGeometry| g.columns().iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(point(0).kind(), NodeGeometryKind::Point);
    assert_eq!(circle.kind(), NodeGeometryKind::Circle);
    assert_eq!(boxes.kind(), NodeGeometryKind::Box);
    assert_eq!(names(&point(0)), ["x", "y"]);
    assert_eq!(names(&circle), ["x", "y", "r"]);
    assert_eq!(names(&boxes), ["x", "y", "w", "h"]);
    let curve = EdgeGeometry::Curve {
        degree: 2,
        paths: Paths::default(),
    };
    assert_eq!(EdgeGeometry::Line.kind(), EdgeGeometryKind::Line);
    assert_eq!(
        EdgeGeometry::Polyline(Paths::default()).kind(),
        EdgeGeometryKind::Polyline
    );
    assert_eq!(curve.kind(), EdgeGeometryKind::Curve);
}

#[test]
fn node_columns_must_match_the_count_be_finite_and_sizes_not_negative() {
    assert_eq!(point(3).check(3), Ok(()));
    let short = SnapshotError::Length {
        column: "node.x",
        expected: 4,
        found: 3,
    };
    assert_eq!(point(3).check(4), Err(short));
    let nan = NodeGeometry::Point {
        x: vec![0.0],
        y: vec![f32::NAN],
    };
    let at = |column, index| SnapshotError::NonFinite { column, index };
    assert_eq!(nan.check(1), Err(at("node.y", 0)));
    let circle = |r: f32| NodeGeometry::Circle {
        x: vec![0.0; 2],
        y: vec![0.0; 2],
        r: vec![0.0, r],
    };
    assert_eq!(
        circle(-0.0).check(2),
        Ok(()),
        "negative zero is not negative"
    );
    let negative = |column| SnapshotError::Negative { column, index: 1 };
    assert_eq!(circle(-1e-30).check(2), Err(negative("node.r")));
    assert_eq!(circle(f32::INFINITY).check(2), Err(at("node.r", 1)));
    let boxes = |w: f32, h: f32| NodeGeometry::Box {
        x: vec![0.0; 2],
        y: vec![0.0; 2],
        w: vec![1.0, w],
        h: vec![1.0, h],
    };
    assert_eq!(boxes(0.0, 0.0).check(2), Ok(()));
    assert_eq!(boxes(-1.0, 1.0).check(2), Err(negative("node.w")));
    assert_eq!(boxes(1.0, -1.0).check(2), Err(negative("node.h")));
}

#[test]
fn paths_need_m_plus_one_offsets_from_zero_never_decreasing() {
    let polyline = |p| EdgeGeometry::Polyline(p);
    assert_eq!(EdgeGeometry::Line.check(7), Ok(()));
    assert_eq!(polyline(paths(&[0, 0, 2, 3], 3)).check(3), Ok(()));
    assert_eq!(polyline(paths(&[0], 0)).check(0), Ok(()));
    let length = |column, expected, found| SnapshotError::Length {
        column,
        expected,
        found,
    };
    assert_eq!(
        polyline(paths(&[0, 1], 1)).check(2),
        Err(length("edge.offsets", 3, 2))
    );
    assert_eq!(
        polyline(Paths::default()).check(0),
        Err(length("edge.offsets", 1, 0))
    );
    let offsets = |index| SnapshotError::Offsets {
        column: "edge.offsets",
        index,
    };
    assert_eq!(polyline(paths(&[1, 1], 1)).check(1), Err(offsets(0)));
    assert_eq!(polyline(paths(&[0, 2, 1], 1)).check(2), Err(offsets(2)));
    assert_eq!(
        polyline(paths(&[0, 2], 1)).check(1),
        Err(length("edge.pts", 4, 2))
    );
    let mut bad = paths(&[0, 1], 1);
    bad.pts[1] = f32::NEG_INFINITY;
    assert_eq!(
        polyline(bad).check(1),
        Err(SnapshotError::NonFinite {
            column: "edge.pts",
            index: 1
        })
    );
}

#[test]
fn a_curve_needs_a_degree_and_well_formed_paths() {
    let curve = |degree, p| EdgeGeometry::Curve { degree, paths: p };
    assert_eq!(curve(3, paths(&[0, 2], 2)).check(1), Ok(()));
    assert_eq!(curve(1, paths(&[0, 0], 0)).check(1), Ok(()));
    assert_eq!(
        curve(0, paths(&[0, 2], 2)).check(1),
        Err(SnapshotError::CurveDegree)
    );
    assert_eq!(
        curve(2, paths(&[1, 2], 2)).check(1),
        Err(SnapshotError::Offsets {
            column: "edge.offsets",
            index: 0
        })
    );
}

#[test]
fn positions_past_u32_saturate_and_lengths_compare_exactly() {
    assert_eq!(index_u32(7), 7);
    assert_eq!(index_u32(u32::MAX as usize), u32::MAX);
    assert_eq!(check_len("c", 2, 2), Ok(()));
    assert!(check_len("c", 2, 3).is_err());
    assert_eq!(
        check_finite("c", &[0.0, -0.0, f32::MAX, f32::MIN_POSITIVE]),
        Ok(())
    );
}
