use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

/// A 4-point star: `center` is the sole cut vertex on every leaf-leaf path.
fn star(leaves: u32) -> Topology {
    let mut nodes = vec![node("center", "")];
    let mut edges = Vec::new();
    for i in 0..leaves {
        let leaf = format!("leaf{i}");
        nodes.push(node(&leaf, ""));
        edges.push(edge(&format!("e{i}"), "center", &leaf));
    }
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn degree_is_the_topology_column_not_a_recomputation() {
    let t = star(3);
    assert_eq!(degree(&t), t.nodes().degree.as_slice());
    assert_eq!(degree(&t)[0], 3, "the center touches every leaf");
}

#[test]
fn on_a_star_only_the_center_has_positive_betweenness() {
    let t = star(4);
    let b = betweenness(&t);
    assert!(
        b[0] > 0.0,
        "every leaf pair's shortest path crosses the center"
    );
    assert!(
        b[1..].iter().all(|&v| v == 0.0),
        "leaves sit on no one else's path"
    );
}

#[test]
fn the_center_of_a_star_is_closer_to_everyone_than_any_leaf() {
    let t = star(3);
    let c = closeness(&t);
    assert!(c[0] > c[1], "center: {}, leaf: {}", c[0], c[1]);
}

/// hub-x, hub-y, x-y (a triangle: an odd cycle, so this graph is *not* bipartite)
/// plus hub-leaf. Power iteration on a bipartite graph oscillates between the two
/// eigenvectors of `+lambda_max` and `-lambda_max` forever (equal magnitude, so
/// neither dominates) rather than converging — [`star`] below is exactly that case,
/// which is why convergence is checked against this graph instead.
fn triangle_with_pendant() -> Topology {
    let nodes = [
        node("hub", ""),
        node("x", ""),
        node("y", ""),
        node("leaf", ""),
    ];
    let edges = [
        edge("hx", "hub", "x"),
        edge("hy", "hub", "y"),
        edge("xy", "x", "y"),
        edge("hl", "hub", "leaf"),
    ];
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn eigenvector_converges_and_favours_the_higher_degree_node() {
    let (scores, converged) = eigenvector(&triangle_with_pendant());
    assert!(converged);
    assert!(scores[0] > scores[3], "the hub outranks the pendant leaf");
    assert!(scores[0] > 0.0);
}

#[test]
fn eigenvector_on_a_bipartite_star_reports_non_convergence_not_a_plausible_lie() {
    // Ponytail (module doc): a bipartite graph's two extremal eigenvalues tie in
    // magnitude, so plain power iteration never settles. The star is bipartite
    // (center vs. leaves) by construction — this must come back `false`, not a
    // number that merely looks like an answer.
    let (_, converged) = eigenvector(&star(3));
    assert!(
        !converged,
        "a star is bipartite: this must not silently claim convergence"
    );
}

#[test]
fn repeated_runs_agree_bit_for_bit() {
    let t = star(3);
    assert_eq!(closeness(&t), closeness(&t));
    assert_eq!(betweenness(&t), betweenness(&t));
    assert_eq!(eigenvector(&t), eigenvector(&t));
}

/// A→B(2), A→C(3), C→B(−2) — the exact fixture `paths::tests` proves Dijkstra wrong
/// on (`fixtures/analysis/negative-weight.json`'s `defect`). Local to this module the
/// same way `paths.rs`'s own `directed` test helper is.
fn directed(id: &str, a: &str, b: &str, weight: f64) -> crate::records::EdgeRecord {
    let mut e = edge(id, a, b);
    e.directed = true;
    e.strength = weight;
    e
}

fn negative_weight_defect() -> Topology {
    let nodes = [node("a", ""), node("b", ""), node("c", "")];
    let edges = [
        directed("ab", "a", "b", 2.0),
        directed("ac", "a", "c", 3.0),
        directed("cb", "c", "b", -2.0),
    ];
    index_model(&nodes, &edges).expect("fits")
}

/// Regression for the MAJOR finding (phase-07 review): `closeness` carried no guard
/// against the negative-weight input class `paths.rs`'s own precondition names, so it
/// silently returned a wrong number instead of failing loudly. This is the guard.
#[test]
#[should_panic(expected = "non-negative")]
fn closeness_panics_in_debug_on_a_negative_weight_graph() {
    let _ = closeness(&negative_weight_defect());
}

/// Same regression, for `betweenness` (its own hand-rolled Dijkstra, `shortest_path_dag`).
#[test]
#[should_panic(expected = "non-negative")]
fn betweenness_panics_in_debug_on_a_negative_weight_graph() {
    let _ = betweenness(&negative_weight_defect());
}

/// The "wrong-vs-correct" evidence itself, the way `paths.rs` shows Dijkstra vs
/// Bellman-Ford: `closeness_of` (the pure per-source arithmetic `closeness` calls,
/// with no topology to guard) fed Dijkstra's wrong distance for this graph gives a
/// different value than fed Bellman-Ford's correct one — proof the wrongness is real
/// arithmetic, not just a hypothetical the guard above forecloses.
#[test]
fn closeness_of_diverges_between_dijkstras_wrong_distance_and_bellman_fords_correct_one() {
    use crate::analysis::paths::{ShortestPaths, bellman_ford, dijkstra_distances};
    let t = negative_weight_defect();
    let wrong = dijkstra_distances(&t, 0);
    let ShortestPaths::Distances(correct) = bellman_ford(&t, 0) else {
        panic!("this graph has no negative cycle");
    };
    assert_ne!(
        wrong, correct,
        "the fixture must actually exercise the defect"
    );
    assert_ne!(
        closeness_of(wrong, 3),
        closeness_of(correct, 3),
        "closeness computed from Dijkstra's wrong distance silently diverges from the \
         Bellman-Ford-correct value on this input"
    );
}
