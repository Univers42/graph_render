use super::{Archimedean, SpiralParams, run, run_under, run_with};
use crate::exec::{Serial, StepRange};
use crate::layout::coords::probe::{assert_close, graph, points};
use crate::stage::StageError;

/// The one refusal `SpiralParams::resolution` can draw, named as `grid.rs` names a bad
/// `spacing` (`StageError::Param`), so a caller reads which parameter and which rule.
const BAD_RESOLUTION: StageError = StageError::Param {
    name: "resolution",
    rule: "finite and above 0",
};

#[test]
fn empty_graph_has_no_points_and_one_node_is_the_centre() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), vec![(0.0, 0.0)]);
}

#[test]
fn two_to_five_nodes_match_networkx_spiral_layout() {
    let want: [&[(f32, f32)]; 4] = [
        &[(-1.0, -0.3650285), (1.0, 0.3650285)],
        &[
            (-1.0, -0.660711),
            (0.1413742, -0.2440769),
            (0.8586258, 0.9047879),
        ],
        &[
            (-0.6415328, -0.6855509),
            (-0.0330791, -0.463448),
            (0.3492795, 0.1489988),
            (0.3253324, 1.0),
        ],
        &[
            (-0.4024452, -0.7088339),
            (0.0047882, -0.5601821),
            (0.2606979, -0.1502762),
            (0.2446702, 0.4192923),
            (-0.1077111, 1.0),
        ],
    ];
    for (offset, want) in want.iter().enumerate() {
        let got = points(&run(&graph(offset as u32 + 2, &[(0, 1)])).unwrap());
        assert_close(&got, want, 2e-6);
    }
}

#[test]
fn the_non_equidistant_branch_is_the_archimedean_spiral() {
    // networkx: dist = arange(n); angle = resolution * dist; pos = dist * (cos, sin).
    let params = SpiralParams {
        resolution: 0.5,
        equidistant: false,
    };
    let got = points(&run_with(&graph(3, &[]), &params, &Serial, 1).unwrap());
    let raw = [
        (0.0, 0.0),
        (0.877582_f64, 0.479425_f64),
        (1.080605, 1.682941),
    ];
    let (mx, my) = (
        raw.iter().map(|p| p.0).sum::<f64>() / 3.0,
        raw.iter().map(|p| p.1).sum::<f64>() / 3.0,
    );
    let limit = raw
        .iter()
        .flat_map(|p| [(p.0 - mx).abs(), (p.1 - my).abs()])
        .fold(0.0, f64::max);
    let want: Vec<(f32, f32)> = raw
        .iter()
        .map(|p| (((p.0 - mx) / limit) as f32, ((p.1 - my) / limit) as f32))
        .collect();
    assert_close(&got, &want, 1e-5);
}

#[test]
fn a_disconnected_graph_is_laid_out_like_an_edgeless_one() {
    let a = points(&run(&graph(4, &[])).unwrap());
    assert_eq!(a, points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap()));
}

/// Every width writes the same archimedean spiral: a per-node gather above a serial merge,
/// so the division of the outputs cannot move a byte.
#[test]
fn the_archimedean_branch_is_the_same_bytes_at_every_worker_count() {
    for n in [2, 3, 5, 16, 17, 64, 1000] {
        let want = points(&run(&graph(n, &[])).unwrap());
        for workers in [1, 2, 3, 4, 7] {
            let got = run_with(&graph(n, &[]), &SpiralParams::default(), &Serial, workers).unwrap();
            assert_eq!(points(&got), want, "n = {n}, workers = {workers}");
        }
    }
}

/// The equidistant branch carries `theta` across iterations, so it is the same bytes at
/// every width **because it never reaches a runner** — the one `if` in `run_under` is the
/// whole claim, and this holds it from outside.
#[test]
fn the_equidistant_branch_is_serial_and_unchanged_by_the_width() {
    let params = SpiralParams {
        resolution: 0.5,
        equidistant: true,
    };
    let want = points(&run_with(&graph(7, &[]), &params, &Serial, 1).unwrap());
    for workers in [1, 2, 3, 4, 7] {
        let got = run_with(&graph(7, &[]), &params, &Serial, workers).unwrap();
        assert_eq!(points(&got), want, "workers = {workers}");
    }
    // And it is a different drawing from the archimedean one, so the branch really is taken.
    assert_ne!(want, points(&run(&graph(7, &[])).unwrap()));
}

/// The control reaches the **merge**: the spiral's angles are untouched and the centroid is
/// stolen, so a threaded arm that ran the merge is red.
#[test]
fn the_control_diverges_the_spiral_from_the_honest_run() {
    let honest = points(&run(&graph(5, &[])).unwrap());
    let stolen =
        points(&run_under(&graph(5, &[]), &SpiralParams::default(), &Serial, 3, true).unwrap());
    assert_ne!(honest, stolen, "the shared merge did not move");
}

/// The equidistant arm against networkx's own numbers, at a stated `resolution` — the
/// only pin on `CHORD` and `STEP` (`layout.py:1329-1336` run on a 7-node graph), without
/// which a mutation of either constant keeps every other test here green.
#[test]
fn the_equidistant_branch_matches_networkxs_equidistant_spiral() {
    // spiral_layout(nx.path_graph(7), resolution=0.5, equidistant=True).
    let want: [(f32, f32); 7] = [
        (-0.6617275, -0.7290234),
        (-0.2897382, -0.650238),
        (0.0212332, -0.4288537),
        (0.2293105, -0.1074977),
        (0.3140252, 0.2667393),
        (0.2723615, 0.6488734),
        (0.1145354, 1.0),
    ];
    let params = SpiralParams {
        resolution: 0.5,
        equidistant: true,
    };
    assert_close(
        &points(&run_with(&graph(7, &[]), &params, &Serial, 1).unwrap()),
        &want,
        2e-6,
    );
    // **The pin bites**, shown here rather than claimed: `resolution` is the one input that
    // moves `CHORD / (STEP * theta)`, so these same seven numbers are what a mutation of
    // `CHORD` or `STEP` would have to keep. One step of the arm from `0.5`, the drawing is
    // a different one and `assert_close` above would fail — that is the mutation this test
    // exists to catch, run as its own negative control inside the test that holds the pin.
    let mutated = SpiralParams {
        resolution: 0.6,
        equidistant: true,
    };
    let off = points(&run_with(&graph(7, &[]), &mutated, &Serial, 1).unwrap());
    assert!(
        off.iter()
            .zip(&want)
            .any(|(g, w)| (g.0 - w.0).abs() > 2e-6 || (g.1 - w.1).abs() > 2e-6),
        "a mutated step stayed inside the tolerance, so these values pin nothing: {off:?}"
    );
}

/// A `resolution` that cannot produce finite geometry is refused with the parameter named,
/// in **both** arms and at **every** node count — never clamped, never returned as `Ok`
/// holding NaN, and not waved through by the 0- and 1-node short-circuit.
#[test]
fn a_resolution_that_cannot_produce_finite_geometry_is_refused() {
    let bad = [
        (0.0, true),
        (-0.35, true),
        (f64::NAN, true),
        (f64::INFINITY, true),
        (0.0, false),
        (-0.35, false),
        (f64::NAN, false),
        (f64::INFINITY, false),
    ];
    for &(resolution, equidistant) in &bad {
        let params = SpiralParams {
            resolution,
            equidistant,
        };
        for n in [0, 1, 2, 4] {
            assert_eq!(
                run_with(&graph(n, &[]), &params, &Serial, 1),
                Err(BAD_RESOLUTION),
                "resolution {resolution}, equidistant {equidistant}, n {n}"
            );
        }
    }
}

/// The default is inside the rule, so the guard cannot have moved a registered layout.
#[test]
fn the_default_resolution_is_accepted_by_the_rule_that_refuses_the_rest() {
    assert!(SpiralParams::default().resolution > 0.0);
    assert!(SpiralParams::default().resolution.is_finite());
}

/// `out` is the range's own sub-column (`StepRange`'s stated contract), so a runner handing
/// a short one is loud about it instead of leaving the tail at the default `(0.0, 0.0)`,
/// which would stack real nodes on the origin.
#[test]
#[should_panic(expected = "out is the range's own sub-column")]
fn a_runner_that_hands_a_short_out_is_refused_rather_than_truncated() {
    let kernel = Archimedean {
        count: 4,
        resolution: 0.35,
    };
    let mut out = [(0.0, 0.0); 2];
    kernel.step_range(0..4, &mut out);
}
