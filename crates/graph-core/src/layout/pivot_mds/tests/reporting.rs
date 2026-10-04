//! `run`-level pins: the pure-function statement, the expected pivot count per shape, and
//! multi-component reporting.

use super::*;

/// The old negative control, kept as the plain purity statement it always was, with the
/// fixture `LF-26` names as the one that cannot see the defect: a `6x10` grid's ties are in
/// the Gram's null space, and a null direction projects to zero however it is rotated — which
/// is exactly why `run(&t) == run(&t)` held before and after LF-11, and why it is not the
/// pin that proves the rule.
#[test]
fn is_deterministic_run_twice() {
    let t = topology(60, &grid_pairs(6, 10));
    assert_eq!(run(&t), run(&t), "same input bits, same output bits");
}

#[test]
fn common_shapes_solve_with_the_expected_pivot_count() {
    for (n, pairs) in [
        (30usize, path_pairs(30)),
        (30, cycle_pairs(30)),
        (30, star_pairs(30)),
        (20, complete_pairs(20)),
    ] {
        let t = topology(n, &pairs);
        let (_, reports) = run(&t).expect("solves");
        assert_eq!(reports.len(), 1, "n={n}");
        assert!(reports[0].solved, "n={n}");
        assert_eq!(
            reports[0].pivots, n as u32,
            "n={n}: pivots = min(100, n) = n"
        );
    }
    // Past MAX_PIVOTS, the pivot count caps at 100 rather than following n.
    let big = topology(150, &cycle_pairs(150));
    let (_, reports) = run(&big).expect("solves");
    assert_eq!(reports[0].pivots, MAX_PIVOTS as u32);
}

#[test]
fn disconnected_graph_reports_only_attempted_components() {
    let mut nodes: Vec<_> = (0..5).map(|i| node(&format!("p{i}"), "")).collect();
    nodes.push(node("iso", ""));
    nodes.extend((0..4).map(|i| node(&format!("c{i}"), "")));
    let mut edges: Vec<_> = (0..4)
        .map(|i| edge(&format!("pe{i}"), &format!("p{i}"), &format!("p{}", i + 1)))
        .collect();
    edges.extend((0..4).map(|i| {
        edge(
            &format!("ce{i}"),
            &format!("c{i}"),
            &format!("c{}", (i + 1) % 4),
        )
    }));
    let t = index_model(&nodes, &edges).expect("fits");

    let (geometry, reports) = run(&t).expect("both size >= 2 components solve");
    assert_eq!(
        reports.len(),
        2,
        "the isolated node gets no report: nothing was attempted for it"
    );
    assert!(reports.iter().all(|r| r.solved));
    assert_eq!(
        reports.iter().map(|r| r.size).collect::<Vec<_>>(),
        vec![5, 4]
    );

    let (x, y) = points(&geometry);
    assert_eq!(x.len(), 10);
    assert!(x.iter().chain(y).all(|v| v.is_finite()));
}
