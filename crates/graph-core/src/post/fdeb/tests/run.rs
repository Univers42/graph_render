use super::*;

#[test]
fn one_iteration_moves_an_interior_point_and_pins_both_endpoints() {
    // Two coincident edges on one line and a third five units above: the displacement of each
    // interior point is then the reference's arithmetic written out, with nothing else in
    // the sum. step = 0.6, strength = 0.8, spring = 0 on a straight row (its two neighbours
    // are symmetric about the point, so the Laplacian is exactly 0), and the softening
    // (SOFTEN_FRAC · RADIUS_FRAC · diag)² with diag = hypot(1, 5) over the four positions.
    let topology = three_edges();
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 5.0), (1.0, 5.0)];
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let params = FdebParams {
        threshold: 0.0,
        visibility: false,
        ..FdebParams::default()
    };
    let list = pairs::PairList::of(&frames, &params);
    let soften = (0.01 * 0.05 * libm::hypotf(1.0, 5.0)) * (0.01 * 0.05 * libm::hypotf(1.0, 5.0));

    let before = Points::of(&topology, &geometry, (x, y), 3);
    let mut after = Points::empty(3, 3);
    before.step_into(&mut after, &list, &frames, params.strength, 0.6, soften);

    // The endpoints never move: an edge has to touch its two nodes.
    assert_eq!(after.at(0, 0), (0.0, 0.0));
    assert_eq!(after.at(0, 2), (1.0, 0.0));
    // Edge 0 and edge 1 are the same segment, so the interior point's attractor is edge 1's
    // coincident point: the displacement is zero, softened but zero.
    let (px, py) = after.at(0, 1);
    assert_eq!(px, 0.5, "the pull is along the perpendicular only");
    assert!(
        py > 0.0 && py < 1e-6,
        "the coincident partner pulls nowhere, the far one barely: {py}"
    );
    // Edge 2 sits five units above edges 0 and 1, which both attract it fully: the attractor
    // is 5 below, so it moves step · strength · 5 = 0.6 · 0.8 · 5 = 2.4 down.
    let (_, y) = after.at(2, 1);
    assert!(
        (5.0 - y - 2.4).abs() < 1e-5,
        "moved toward the pair, y = {y}"
    );
}

#[test]
fn resampling_keeps_the_endpoints_and_spaces_the_row_evenly() {
    let mut points = Points::empty(1, 2);
    points.write_row(0, &[(0.0, 0.0), (4.0, 0.0)]);
    points.resample(6);
    let row: Vec<(f32, f32)> = (0..6).map(|p| points.at(0, p)).collect();
    assert_eq!(row.first(), Some(&(0.0, 0.0)), "the row's own first point");
    assert_eq!(row.last(), Some(&(4.0, 0.0)), "and its own last");
    let gaps: Vec<f32> = row.windows(2).map(|w| w[1].0 - w[0].0).collect();
    for gap in &gaps {
        assert!((gap - 0.8).abs() < 1e-6, "even arc length, got {gap}");
    }
    // A row of one point cannot be resampled and is left where it is, rather than divided by
    // zero: the guard the reference has for `k_in == 1`.
    let mut single = Points::empty(1, 2);
    single.write_row(0, &[(0.0, 0.0), (0.0, 0.0)]);
    single.resample(1);
    assert_eq!(single.shape(), (1, 1));
    assert_eq!(single.at(0, 0), (0.0, 0.0));
}

#[test]
fn bundling_a_layouts_own_polylines_composes_rather_than_replacing_them() {
    // A `Polyline` input whose bend is off the straight line: the pass bundles from the
    // drawn path, so the bend survives where no force opposes it. A pass that assumed a
    // `Line` input would start from the chord and lose it, which is what this asserts.
    let topology = three_edges();
    let geometry = Geometry::planar(
        NodeGeometry::Point {
            x: vec![0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 5.0, 5.0],
        },
        EdgeGeometry::Polyline(Paths {
            offsets: vec![0, 1, 2, 3],
            pts: vec![0.5, 3.0, 0.5, 3.0, 0.5, 3.0],
        }),
        Vec::new(),
    );
    let bundled = bundle(&topology, &geometry, &FdebParams::default()).expect("runs");
    let EdgeGeometry::Polyline(paths) = &bundled.geometry.edges else {
        panic!("bundling emits polylines");
    };
    assert_eq!(paths.offsets.len(), topology.edge_count() as usize + 1);
    for row in 0..topology.edge_count() as usize {
        let (from, to) = (paths.offsets[row] as usize, paths.offsets[row + 1] as usize);
        assert!(to > from, "edge {row} has interior points");
    }
}

#[test]
fn a_parameter_outside_the_accepted_range_is_refused_rather_than_clipped() {
    let topology = three_edges();
    let geometry = points(4, &[(0.0, 0.0), (1.0, 0.0), (0.0, 5.0), (1.0, 5.0)]);
    for params in [
        FdebParams {
            strength: 4.0,
            ..FdebParams::default()
        },
        FdebParams {
            threshold: -0.5,
            ..FdebParams::default()
        },
        FdebParams {
            threshold: f32::NAN,
            ..FdebParams::default()
        },
    ] {
        let err = bundle(&topology, &geometry, &params).expect_err("refused");
        assert!(
            matches!(err, crate::stage::StageError::Param { .. }),
            "{err}"
        );
    }
    // The same graph at the defaults runs, which is what makes the refusals above about the
    // parameter and not about the geometry.
    assert!(bundle(&topology, &geometry, &FdebParams::default()).is_ok());
}

#[test]
fn a_schedule_past_the_references_own_ceilings_is_refused_rather_than_overflowed() {
    // R3. The oracle the registry names is SciGraphs' FDEB, whose own panel bounds both
    // knobs: `edge_segments` max=32 (`SciGraphs/properties/edge_style_properties.py:78-85`)
    // and `edge_fdeb_cycles` max=10 (`:286-297`). Past them, 33 doubling cycles took the
    // point count to u32::MAX and `subdivisions + 2` overflowed, on an empty graph.
    let empty = index_model(&[], &[]).expect("fits");
    let nothing = points(0, &[]);
    for (params, names) in [
        (
            FdebParams {
                cycles: 33,
                segments: u32::MAX,
                ..FdebParams::default()
            },
            ["segments", "cycles"],
        ),
        (
            FdebParams {
                segments: 33,
                ..FdebParams::default()
            },
            ["segments", "segments"],
        ),
        (
            FdebParams {
                cycles: 11,
                ..FdebParams::default()
            },
            ["cycles", "cycles"],
        ),
    ] {
        let err = bundle(&empty, &nothing, &params).expect_err("refused");
        assert!(
            matches!(err, crate::stage::StageError::Param { name, .. } if names.contains(&name)),
            "{err}"
        );
    }
    // The ceilings themselves are accepted.
    let geometry = points(4, &[(0.0, 0.0), (1.0, 0.0), (0.0, 5.0), (1.0, 5.0)]);
    let at_ceiling = FdebParams {
        cycles: 10,
        segments: 32,
        ..FdebParams::default()
    };
    assert!(bundle(&three_edges(), &geometry, &at_ceiling).is_ok());
}

#[test]
fn a_drawing_too_small_to_soften_still_bundles_to_finite_points() {
    // M1. Two parallel edges 1e-22 long: the softening (0.01 · 0.05 · 1e-22)² underflows to
    // 0 in f32, every interior point collapses onto the row's start, and the coincident
    // partner's weight compat / 0 is +inf. The reference (`fdeb.py:290-298`) carries the
    // same blind spot and writes NaN; a geometry column must not.
    let nodes = ["a", "b"].map(|id| node(id, ""));
    let edges = [edge("e0", "a", "b"), edge("e1", "a", "b")];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(2, &[(0.0, 0.0), (1e-22, 0.0)]);
    let params = FdebParams {
        threshold: 0.0,
        ..FdebParams::default()
    };
    let bundled = bundle(&topology, &geometry, &params).expect("runs");
    assert_eq!(
        bundled.pairs, 1,
        "the pair is admitted, so the weight is used"
    );
    let EdgeGeometry::Polyline(paths) = &bundled.geometry.edges else {
        panic!("bundling emits polylines");
    };
    assert!(
        paths.pts.iter().all(|v| v.is_finite()),
        "{:?}",
        &paths.pts[..4]
    );
}

#[test]
fn the_committed_hairball_is_exactly_what_the_generator_builds() {
    let (from_file, file_edges) = load("hairball").expect("the fixture is committed");
    let (from_formula, formula_edges) = hairball(42);
    assert_eq!(from_file, from_formula, "the file and the formula agree");
    assert_eq!(
        file_edges, formula_edges,
        "edge for edge, in the same order"
    );
    assert_eq!(from_file.len(), 42, "two clusters of 21");
    // 84 cross edges (21 A nodes x 4 strides), then per cluster 20 path + 21 cycle + 6 fan
    // edges: the edge count every ink figure in docs/measurements/phase08-ink.md was taken
    // at, restated here so a fixture that grew would move this number.
    assert_eq!(file_edges.len(), 84 + 2 * (20 + 21 + 6));
    assert_eq!(file_edges.len(), 178);
    // The cross edges come first and in stride order, which is what puts the four
    // near-parallel families at the front of the pair list.
    assert_eq!(file_edges[0].source, "a0");
    assert_eq!(file_edges[0].target, "b0");
    assert_eq!(file_edges[3].target, "b9", "stride 17, k = 3");
    assert_eq!(file_edges[4].source, "a1");
    assert_eq!(
        file_edges[83].source, "a20",
        "84 cross edges, then A's own path"
    );
    assert_eq!(
        (&file_edges[84].source, &file_edges[84].target),
        (&"a0".into(), &"a1".into())
    );
    // No self-loop and no duplicate id, which the indexer would otherwise refuse.
    let mut ids: Vec<&str> = file_edges.iter().map(|e| e.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "unique edge ids");
}

#[test]
fn a_fixture_that_is_not_there_is_refused_by_name() {
    assert!(load("nope").is_err());
    assert!(
        load("long-span").is_ok(),
        "the second fixture is committed too"
    );
    // A generator run is the same graph at every size, which is what makes the scale sweep
    // the same graph as the fixture rather than a different one that only looks like it.
    let (small, small_edges) = hairball(10);
    let (large, large_edges) = hairball(100);
    assert_eq!(small.len(), 10);
    assert_eq!(large.len(), 100);
    assert!(small_edges.len() < large_edges.len());
    assert!(
        hairball(2).0.len() == 2,
        "the smallest graph still has two clusters"
    );
}
