//! The generators, pinned value by value over exactly representable inputs, so a swapped
//! operator, a wrong constant or a wrong sign fails a test. Split from the parent, which
//! holds the shared case builders.

use super::*;

#[test]
fn a_cubic_puts_two_control_points_at_the_quarters_pushed_off_by_a_quarter_bulge() {
    // The same chord: push = (0, -4) * (-0.5 / 4) = (0, 0.5), so a quarter and three
    // quarters along the chord, each 0.5 up.
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    expect(
        &t,
        &[(0.0, 0.0), (4.0, 0.0)],
        &params(Style::Bezier),
        &[vec![(1.0, 0.5), (3.0, 0.5)]],
    );
}

#[test]
fn an_l_elbow_is_one_bend_and_a_z_elbow_brackets_the_midpoint() {
    // Chord (0,0) -> (4,3): the endpoints are not level, so neither elbow collapses.
    let centres = [(0.0, 0.0), (4.0, 3.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let mut l = params(Style::Orthogonal);
    l.orthogonal = Orthogonal::L;
    expect(&t, &centres, &l, &[vec![(4.0, 0.0)]]);
    expect(
        &t,
        &centres,
        &params(Style::Orthogonal),
        &[vec![(2.0, 0.0), (2.0, 3.0)]],
    );
    // Level endpoints: the two Z bends land on the chord itself, which is exact and is
    // the row a renderer draws as a straight line.
    let flat = [(0.0, 0.0), (4.0, 0.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    expect(
        &t,
        &flat,
        &params(Style::Orthogonal),
        &[vec![(2.0, 0.0), (2.0, 0.0)]],
    );
}

#[test]
fn a_reversed_pair_is_one_group_fanned_along_one_direction() {
    // The case the fan's direction has to get right: a -> b and b -> a are one group, so
    // they get opposite amounts — and the perpendicular must be keyed to the group
    // (lower index to higher), not to each edge's own direction, or the two amounts
    // cancel and both edges land on one line. The two rows are then the same shape read
    // end for end, one exactly `parallel_offset` above the other.
    let centres = [(0.0, 0.0), (4.0, 0.0)];
    let gap = f64::from(GAP);
    for style in [Style::Orthogonal, Style::Quadratic, Style::Bezier] {
        let t = topology(&["a", "b"], &[("e0", "a", "b"), ("e1", "b", "a")]);
        let got = paths(&t, &centres, &params(style));
        let (first, second) = (row(&got, 0), row(&got, 1));
        assert_eq!(
            first.len(),
            second.len(),
            "{style:?}: the two rows differ in length"
        );
        assert_ne!(first, second, "{style:?}: a reversed pair overlaid");
        for (i, high) in first.iter().enumerate() {
            let low = &second[first.len() - 1 - i];
            assert!(
                (high.0 - low.0).abs() <= EXACT && (high.1 - low.1 - gap).abs() <= EXACT,
                "{style:?} point {i}: {high:?} is not one gap above {low:?}"
            );
        }
    }
}

#[test]
fn a_group_of_one_takes_no_offset_at_all() {
    let centres = [(0.0, 0.0), (4.0, 0.0), (8.0, 0.0)];
    let t = topology(&["a", "b", "c"], &[("e0", "a", "b"), ("e1", "a", "c")]);
    // The second chord is twice as long, so its perpendicular is twice as long too and
    // its control point lands at (4, 2) — the bulge scales with the chord, not with the
    // gap, and a group of one has no gap to begin with.
    expect(
        &t,
        &centres,
        &params(Style::Quadratic),
        &[vec![(2.0, 1.0)], vec![(4.0, 2.0)]],
    );
}

#[test]
fn a_self_loop_is_a_regular_octagon_centred_half_a_radius_above_the_node() {
    // Centre (4, 0.125): half of RADIUS = 0.25, and a lone loop is a group of one, so no
    // fan displacement. Vertex 0 is at angle 0, where cos and sin are exactly 1 and 0.
    let t = topology(&["a"], &[("e0", "a", "a")]);
    let got = paths(&t, &[(4.0, 0.0)], &params(Style::Bezier));
    let loop_row = row(&got, 0);
    assert_eq!(loop_row.len(), 8, "eight vertices, as pinned");
    assert_eq!(loop_row[0], (4.25, 0.125), "vertex 0 is exact at angle 0");
    assert!(
        (loop_row[4].0 - 3.75).abs() <= EXACT,
        "vertex 4 is opposite: {:?}",
        loop_row[4]
    );
    let centre = (4.0f64, 0.125f64);
    for (i, v) in loop_row.iter().enumerate() {
        let (dx, dy) = (v.0 - centre.0, v.1 - centre.1);
        let d = f64::sqrt(dx * dx + dy * dy);
        assert!(
            (d - f64::from(RADIUS)).abs() <= LOOP,
            "vertex {i} is off the circle: {d}"
        );
        assert_ne!(*v, (4.0, 0.0), "vertex {i} sits on the node centre");
    }
    let (n, (mx, my)) = loop_row
        .iter()
        .fold((0.0f64, (0.0f64, 0.0f64)), |(n, (sx, sy)), v| {
            (n + 1.0, (sx + v.0, sy + v.1))
        });
    assert!(
        (mx / n - centre.0).abs() <= LOOP,
        "the vertices are not centred on x"
    );
    assert!(
        (my / n - centre.1).abs() <= LOOP,
        "the vertices are not centred on y"
    );
}

#[test]
fn two_self_loops_on_one_node_are_a_parallel_group_and_slide_apart() {
    // A zero-length chord has no perpendicular, so the group slides the two centres
    // along y instead. GAP = 1.0 over a group of two is +-0.5, and half of RADIUS is
    // 0.125, so the centres are (4, -0.375) and (4, 0.625) — a full diameter apart.
    let t = topology(&["a"], &[("e0", "a", "a"), ("e1", "a", "a")]);
    let mut p = params(Style::Quadratic);
    p.parallel_offset = 1.0;
    let got = paths(&t, &[(4.0, 0.0)], &p);
    // Vertex 0 is at angle 0, where cos and sin are exactly 1 and 0, so the two centres
    // are readable straight off the first point of each row.
    assert_eq!(row(&got, 0)[0], (4.25, -0.375), "the lower loop's centre");
    assert_eq!(
        row(&got, 1)[0],
        (4.25, 0.625),
        "the upper loop's centre, one gap up"
    );
}

#[test]
fn two_distinct_nodes_at_one_position_take_the_loop_shape_and_their_own_slide() {
    // The positions decide, not the endpoint identities: a layout that put two nodes on
    // one spot gets the same loop a self-loop would, and the fan's zero-length chord
    // slides the two loops apart along y rather than turning a perpendicular.
    let t = topology(&["a", "b"], &[("e0", "a", "b"), ("e1", "b", "a")]);
    let mut p = params(Style::Orthogonal);
    p.parallel_offset = 1.0;
    let got = paths(&t, &[(0.0, 0.0), (0.0, 0.0)], &p);
    assert_eq!(row(&got, 0).len(), 8, "the loop shape, not an elbow");
    assert_eq!(row(&got, 0)[0], (0.25, -0.375), "the lower loop's centre");
    assert_eq!(
        row(&got, 1)[0],
        (0.25, 0.625),
        "the upper loop's centre, one gap up"
    );
}

#[test]
fn the_fan_gap_is_exact_for_every_group_size_up_to_five() {
    // The fan is `base * (i - (k-1)/2)`, so a group of k has offsets
    // base * {-(k-1)/2, ..., 0, ..., (k-1)/2} — integers or half-integers of the base,
    // never anything else. k = 1..5 covers the odd and even spans.
    let names = ["e0", "e1", "e2", "e3", "e4"];
    for k in 1..=5u32 {
        let edges: Vec<(&str, &str, &str)> = names[..k as usize]
            .iter()
            .map(|&id| (id, "a", "b"))
            .collect();
        let t = topology(&["a", "b"], &edges);
        let got = fan(&t, 1.0).expect("fits");
        let span = f64::from(k - 1) * 0.5;
        let want: Vec<f64> = (0..k).map(|i| f64::from(i) - span).collect();
        assert_eq!(got, want, "a group of {k}");
    }
}

#[test]
fn a_chord_loops_only_at_exactly_zero_length_and_its_fan_gap_never_shrinks() {
    // R14, recorded false. The reference loops what `np.allclose(p0, p1, atol=1e-6)` calls
    // equal (`edge_styles.py:458`) and zeroes a fan under `length < 1e-10` (`:117`): absolute
    // tolerances in its mesh units. Centres here carry no unit, so the convention is exact
    // zero (`shapes.rs` module doc); pinned so that changing it is a decision.
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    let near = paths(&t, &[(0.0, 0.0), (5e-7, 0.0)], &params(Style::Orthogonal));
    let half = f64::from(5e-7_f32) * 0.5;
    assert_eq!(row(&near, 0), vec![(half, 0.0); 2], "drawn, not looped");
    // A 1e-11 chord in a parallel pair keeps the whole gap: the fan is a distance in node
    // units, not a fraction of the chord.
    let t = topology(&["a", "b"], &[("e0", "a", "b"), ("e1", "a", "b")]);
    let pair = paths(&t, &[(0.0, 0.0), (1e-11, 0.0)], &params(Style::Orthogonal));
    let gap = row(&pair, 0)[0].1 - row(&pair, 1)[0].1;
    assert!((gap.abs() - f64::from(GAP)).abs() <= LOOP, "gap {gap}");
}

#[test]
fn a_fanned_l_elbow_tilts_its_legs_where_a_fanned_z_keeps_its_right_angles() {
    // M24, the documented exception. Chord (0,0) -> (4,3), a parallel pair: an `L` has one
    // interior point, so the fan moves the elbow off the source's level; a `Z` moves both
    // bends together and its middle leg stays vertical.
    let centres = [(0.0, 0.0), (4.0, 3.0)];
    let t = topology(&["a", "b"], &[("e0", "a", "b"), ("e1", "a", "b")]);
    let mut l = params(Style::Orthogonal);
    l.orthogonal = Orthogonal::L;
    let got = paths(&t, &centres, &l);
    for e in 0..2 {
        assert_ne!(
            row(&got, e)[0].1,
            0.0,
            "edge {e}: the first leg is not horizontal"
        );
    }
    let got = paths(&t, &centres, &params(Style::Orthogonal));
    for e in 0..2 {
        let bends = row(&got, e);
        assert_eq!(
            bends[0].0, bends[1].0,
            "edge {e}: the middle leg is vertical"
        );
    }
}
