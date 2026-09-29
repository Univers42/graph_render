//! The contract around the generators rather than the geometry inside them: the kind and
//! degree each style stamps, the parameters it refuses, the node geometry it will not
//! accept, and the registration each one answers to. Split from the parent, which holds
//! the shared case builders.

use super::*;

#[test]
fn a_straight_style_emits_line_and_refuses_a_gap_it_cannot_store() {
    let centres = [(0.0, 0.0), (4.0, 0.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let edges = style_edges(
        &t,
        &points(&centres),
        &StyleParams::for_style(Style::Straight),
    );
    assert_eq!(
        edges.expect("the pinned default is legal"),
        EdgeGeometry::Line
    );
    // A non-zero gap on a style that stores no interior point is refused, not ignored.
    let mut p = StyleParams::for_style(Style::Straight);
    p.parallel_offset = 0.05;
    assert_eq!(
        style_edges(&t, &points(&centres), &p),
        Err(StageError::Param {
            name: "parallel_offset",
            rule: "must be 0.0 for post.style.straight: Line stores no interior points, so an offset has nowhere to go — use post.style.orthogonal, .quadratic or .bezier"
        })
    );
}

#[test]
fn every_parameter_rule_is_refused_by_name_before_a_point_is_written() {
    let centres = [(0.0, 0.0), (4.0, 0.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let named = |field, p: StyleParams| {
        let err = style_edges(&t, &points(&centres), &p).expect_err("refused");
        match err {
            StageError::Param { name, .. } => assert_eq!(name, field, "the refused field"),
            other => panic!("{field}: refused with {other:?}, not a parameter error"),
        }
    };
    let mut p = params(Style::Bezier);
    p.curvature = -0.5;
    named("curvature", p);
    p = params(Style::Bezier);
    p.curvature = f32::NAN;
    named("curvature", p);
    p = params(Style::Bezier);
    p.parallel_offset = f32::INFINITY;
    named("parallel_offset", p);
    p = params(Style::Bezier);
    p.self_loop_radius = 0.0;
    named("self_loop_radius", p);
    p = params(Style::Bezier);
    p.self_loop_segments = 2;
    named("self_loop_segments", p);
    // A curvature of exactly 0 is legal and draws the chord itself: the bulge, not the
    // parameter, is what bends an edge.
    p = params(Style::Bezier);
    p.curvature = 0.0;
    p.parallel_offset = 0.0;
    expect(&t, &centres, &p, &[vec![(1.0, 0.0), (3.0, 0.0)]]);
}

#[test]
fn node_geometry_that_does_not_fit_the_topology_is_refused_before_anything_is_written() {
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let short = NodeGeometry::Point {
        x: vec![0.0],
        y: vec![0.0],
    };
    assert_eq!(
        style_edges(&t, &short, &params(Style::Bezier)),
        Err(StageError::Snapshot(SnapshotError::Length {
            column: "node.x",
            expected: 2,
            found: 1
        }))
    );
    // D9: a non-finite centre is caught here, before it can reach the wire, whose NaN
    // bit pattern wasm32 does not pin.
    let nan = NodeGeometry::Point {
        x: vec![0.0, f32::NAN],
        y: vec![0.0, 0.0],
    };
    assert_eq!(
        style_edges(&t, &nan, &params(Style::Bezier)),
        Err(StageError::Snapshot(SnapshotError::NonFinite {
            column: "node.x",
            index: 1
        }))
    );
}

#[test]
fn a_curve_stamps_its_own_degree_and_a_polyline_stamps_none() {
    let centres = [(0.0, 0.0), (4.0, 0.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let nodes = points(&centres);
    for (style, degree) in [(Style::Quadratic, 2), (Style::Bezier, 3)] {
        let edges = style_edges(&t, &nodes, &params(style)).expect("legal");
        assert_eq!(
            edges,
            EdgeGeometry::Curve {
                degree,
                paths: paths(&t, &centres, &params(style))
            },
            "{style:?} stamps degree {degree}"
        );
    }
    let edges = style_edges(&t, &nodes, &params(Style::Orthogonal)).expect("legal");
    assert_eq!(edges.kind(), EdgeGeometryKind::Polyline);
}

#[test]
fn every_style_id_names_one_kind_one_degree_and_one_registration() {
    assert_eq!(Style::ALL.len(), STYLES.len(), "one capability per style");
    let mut seen: Vec<&str> = Vec::new();
    for style in Style::ALL {
        assert_eq!(Style::from_id(style.id()), Some(style), "{}", style.id());
        let capability = find(style.id()).unwrap_or_else(|| panic!("{}", style.id()));
        assert_eq!(capability.id, style.id());
        assert_eq!(capability.meta.edges, style.kind(), "{}", style.id());
        assert_eq!(capability.meta.stage, "post");
        assert!(!seen.contains(&capability.id), "{} twice", style.id());
        seen.push(capability.id);
    }
    assert!(find("post.style.ribbon").is_none());
    assert!(find("layout.grid").is_none());
    assert_eq!(Style::Quadratic.degree(), 2);
    assert_eq!(Style::Bezier.degree(), 3);
    assert_eq!(
        (Style::Straight.degree(), Style::Orthogonal.degree()),
        (0, 0)
    );
}
