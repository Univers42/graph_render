use super::{run, run_with};
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
    let got = points(&run_with(&graph(3, &[]), 0.5, false).unwrap());
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
