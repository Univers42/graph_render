use super::*;

#[test]
fn the_schedule_is_the_references_own_doubling_and_halving() {
    let params = FdebParams {
        cycles: 6,
        iterations: 8,
        segments: 12,
        ..FdebParams::default()
    };
    let sched = schedule(&params);
    assert_eq!(
        sched,
        vec![
            (1, 0.6, 8),
            (2, 0.3, 5),
            (4, 0.15, 3),
            (8, 0.075, 2),
            (12, 0.0375, 1),
            (12, 0.01875, 1)
        ],
        "points double to the cap, the step halves, iterations fall to two thirds rounded up"
    );
    // Degenerate parameters are floored, never obeyed: zero cycles still runs one.
    let floored = schedule(&FdebParams {
        cycles: 0,
        iterations: 0,
        segments: 0,
        ..FdebParams::default()
    });
    assert_eq!(floored, vec![(1, 0.6, 1)]);
}

#[test]
fn the_four_compatibility_terms_are_the_references_arithmetic() {
    // A square of unit side: four unit edges, two parallel pairs and two perpendicular
    // pairs, so every term can be checked against a value worked out by hand.
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "c", "d"),
        edge("e2", "a", "c"),
        edge("e3", "b", "d"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");

    // Ca: |cos| of the angle between the unit directions. Parallel 1, perpendicular 0.
    assert_eq!(frames.angle(0, 1), 1.0);
    assert_eq!(frames.angle(0, 2), 0.0);
    // Cs: equal lengths score exactly 1.
    assert_eq!(frames.scale(0, 1), 1.0);
    // Cp: midpoints a unit apart, so 1/(1+1) = 1/2 for the two horizontal edges; the
    // same for the two vertical ones.
    assert_eq!(frames.position(0, 1), 0.5);
    assert_eq!(frames.position(2, 3), 0.5);
    // A perpendicular pair has a zero-width shadow, so Cv sends it to 0 and the product is
    // 0 whatever the other terms say.
    assert_eq!(frames.visibility(0, 2), 0.0);
    assert_eq!(frames.compatibility(0, 2, true), 0.0);
    assert_eq!(frames.compatibility(0, 1, true), 0.5);
    // Without the visibility term the perpendicular pair is scored on three terms alone.
    assert_eq!(frames.compatibility(0, 2, false), 0.0, "Ca is 0 anyway");
    // A self-loop has no direction and no length. Its unit direction is the reference's
    // `(0, 0) / max(0, LEN_EPS)`, so the angle term is 0 and the product is 0 rather than a
    // NaN from a division by zero — the length guard is what makes the degenerate edge score
    // "no compatibility" instead of poisoning the whole pair list.
    let loop_nodes = [node("a", ""), node("b", "")];
    let loops = index_model(&loop_nodes, [edge("l", "a", "a")].as_slice()).expect("fits");
    let loop_geometry = points(2, &[(0.0, 0.0), (5.0, 5.0)]);
    let (lx, ly) = centres(&loop_geometry.nodes);
    let loop_edges = loops.edges();
    let loop_frames =
        compat::Frames::of(lx, ly, &loop_edges.source, &loop_edges.target).expect("frames");
    assert_eq!(loop_frames.len(), 1);
    assert_eq!(
        loop_frames.ends(0),
        ((0.0, 0.0), (0.0, 0.0)),
        "both ends are node a"
    );
    assert_eq!(
        loop_frames.compatibility(0, 0, true),
        0.0,
        "no length, no angle, no NaN"
    );
    assert_eq!(loop_frames.compatibility(0, 0, false), 0.0);
}

#[test]
fn the_pair_list_is_sorted_by_edge_then_edge_and_names_the_flipped_rows() {
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.1), (1.0, 0.1)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "c", "d"),
        edge("e2", "b", "a"),
        edge("e3", "a", "b"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let params = FdebParams {
        threshold: 0.0,
        ..FdebParams::default()
    };
    let list = pairs::PairList::of(&frames, &params);

    // Every row ascending in its partner, and the partner back-pointer agreeing: that is
    // what "sorted by (edge, edge)" means in the CSR.
    for e in 0..4 {
        let row = list.row(e);
        assert!(
            row.windows(2).all(|w| w[0].partner < w[1].partner),
            "edge {e}: row ascending"
        );
        for entry in row {
            let back = list
                .row(entry.partner)
                .iter()
                .find(|other| other.partner == e)
                .expect("the pair is in both rows");
            assert_eq!(back.compat, entry.compat, "compatibility is symmetric");
            assert_eq!(back.flipped, entry.flipped, "the flip is symmetric too");
        }
    }
    // e0 and e2 run opposite ways down the same line: the antiparallel pair, and the only
    // one the reference flips so the bundle does not open into an X.
    let flipped = list
        .row(0)
        .iter()
        .find(|entry| entry.partner == 2)
        .expect("the antiparallel pair survives a zero threshold");
    assert!(flipped.flipped, "e0 and e2 point opposite ways");
    assert!(
        list.row(0)
            .iter()
            .find(|entry| entry.partner == 1)
            .is_some_and(|entry| !entry.flipped),
        "a parallel pair is not flipped"
    );
    // Six pairs over four edges, and the count is of pairs, not of rows.
    assert_eq!(list.total(), 6);
    assert_eq!(list.total() as usize, list.row(0).len() + 3);
    assert!(
        list.unbundled().is_empty(),
        "a zero threshold leaves nobody out"
    );
}

#[test]
fn a_threshold_above_one_leaves_every_edge_unbundled() {
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.1), (1.0, 0.1)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [edge("e0", "a", "b"), edge("e1", "c", "d")];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let list = pairs::PairList::of(
        &frames,
        &FdebParams {
            threshold: 1.0,
            ..FdebParams::default()
        },
    );
    assert_eq!(list.total(), 0);
    assert_eq!(list.unbundled(), &[0, 1], "both edges drawn unbundled");
}

#[test]
fn at_a_zero_threshold_a_pair_scoring_exactly_zero_is_kept_as_the_reference_keeps_it() {
    // M2. The reference prunes with `live = cm >= thresh` (`fdeb.py:286`), so a finite
    // compatibility of exactly 0 survives a threshold of 0. A perpendicular pair scores
    // Ca = 0; so does a pair of zero-length edges here, where the reference's 0/0 is NaN and
    // fails the test — the drawing agrees (a zero weight pulls nothing), the count does not.
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.5, -0.5), (0.5, 0.5)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "c", "d"),
        edge("e2", "a", "a"),
        edge("e3", "a", "a"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    assert_eq!(frames.compatibility(0, 1, true), 0.0, "perpendicular");
    assert_eq!(
        frames.compatibility(2, 3, true),
        0.0,
        "two zero-length edges"
    );
    let at = |threshold| {
        pairs::PairList::of(
            &frames,
            &FdebParams {
                threshold,
                ..FdebParams::default()
            },
        )
    };
    let kept = at(0.0);
    assert!(
        kept.row(0)
            .iter()
            .any(|entry| entry.partner == 1 && entry.compat == 0.0)
    );
    assert!(kept.row(2).iter().any(|entry| entry.partner == 3));
    assert!(
        kept.unbundled().is_empty(),
        "every edge has a 0-scoring partner"
    );
    let pruned = at(f32::MIN_POSITIVE);
    assert_eq!(pruned.total(), 0, "nothing here scores above zero");
    // The bundled drawing is the same either way: a zero weight pulls nothing.
    let drawn = |threshold| {
        let params = FdebParams {
            threshold,
            ..FdebParams::default()
        };
        bundle(&topology, &geometry, &params)
            .expect("runs")
            .geometry
    };
    assert_eq!(drawn(0.0), drawn(f32::MIN_POSITIVE));
}

#[test]
fn the_resample_lerp_is_the_references_algebra_and_not_its_bits() {
    // M6 / U20. `at_arc` writes a + (b − a)·f; `fdeb.py:171` writes a·(1 − f) + b·f. One
    // input where the two f32 results differ, by one ulp: the reason META's oracle row is a
    // hand oracle and not a byte-for-byte one.
    let (a, b, f) = (0.1_f32, 0.7_f32, 1.0_f32 / 3.0);
    let port = a + (b - a) * f;
    let reference = a * (1.0 - f) + b * f;
    assert_ne!(port.to_bits(), reference.to_bits());
    assert_eq!(
        port.to_bits().abs_diff(reference.to_bits()),
        1,
        "{port} {reference}"
    );
}
