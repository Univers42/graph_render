use super::{SpiralParams, run, run_under, run_with};
use crate::exec::Serial;
use crate::layout::coords::probe::{assert_close, graph, points};

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
