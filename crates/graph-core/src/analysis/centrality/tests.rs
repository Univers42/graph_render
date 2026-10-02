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

/// An undirected edge carrying `weight` as its `strength`.
fn weighted(id: &str, a: &str, b: &str, weight: f64) -> crate::records::EdgeRecord {
    let mut e = edge(id, a, b);
    e.strength = weight;
    e
}

/// R5 (`docs/reviews/review-core-post.md`): a peer reached at distance 0 is reached.
/// networkx 3.6 `closeness.py:127-133` keeps it in both `len(sp) - 1` and `totsp`; the
/// values are networkx's own output (`closeness_centrality(G, distance="weight")` in
/// `ge-python-oracle`: a 2.0, b 2.0, c 1.0).
#[test]
fn closeness_counts_a_zero_distance_peer_as_reached_like_networkx() {
    let nodes = [node("a", ""), node("b", ""), node("c", "")];
    let edges = [weighted("ab", "a", "b", 0.0), weighted("ac", "a", "c", 1.0)];
    let t = index_model(&nodes, &edges).expect("fits");
    assert_eq!(closeness(&t), vec![2.0, 2.0, 1.0]);
}

/// R6 (i): two nodes 1e-40 apart have closeness 1e40, past `f32::MAX`. D9: the value
/// written is defined (finite), never `inf`.
#[test]
fn closeness_past_f32_range_is_finite() {
    let nodes = [node("a", ""), node("b", "")];
    let t = index_model(&nodes, &[weighted("ab", "a", "b", 1e-40)]).expect("fits");
    let scores = closeness(&t);
    assert!(scores.iter().all(|v| v.is_finite()), "{scores:?}");
}

/// R6 (ii): two parallel edges of strength 1.7e308 overflow one power-iteration step
/// to `inf`, and `inf / inf` is NaN. The answer is finite and does not claim convergence.
#[test]
fn eigenvector_whose_step_overflows_f64_is_finite_and_unconverged() {
    let nodes = [node("a", ""), node("b", "")];
    let edges = [
        weighted("ab1", "a", "b", 1.7e308),
        weighted("ab2", "a", "b", 1.7e308),
    ];
    let (scores, converged) = eigenvector(&index_model(&nodes, &edges).expect("fits"));
    assert!(scores.iter().all(|v| v.is_finite()), "{scores:?}");
    assert!(!converged, "an overflowed iterate is not a converged one");
}

/// `layers` layers of two nodes, every node joined to both nodes of the next layer:
/// layer `i` is reached by `2^i` shortest paths from layer 0, past `f64::MAX` at 1024.
fn doubling_ladder(layers: u32) -> Topology {
    let ids: Vec<String> = (0..layers)
        .flat_map(|i| [format!("x{i}"), format!("y{i}")])
        .collect();
    let nodes: Vec<_> = ids.iter().map(|id| node(id, "")).collect();
    let mut edges = Vec::new();
    for i in 0..layers as usize - 1 {
        for (a, b) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let (from, to) = (&ids[2 * i + a], &ids[2 * i + 2 + b]);
            edges.push(directed(&format!("{from}-{to}"), from, to, 1.0));
        }
    }
    index_model(&nodes, &edges).expect("fits")
}

/// R6 (iii), found while repairing R6: Brandes' path counts overflow `f64` on 3k
/// nodes, inside the declared 20,000-node ceiling, and `inf / inf` writes NaN. A node
/// of layer `i` of `L` carries half of the `2i * 2(L-1-i)` pairs across it.
#[test]
fn betweenness_past_f64_path_counts_is_finite_and_exact() {
    const LAYERS: u32 = 1_030;
    let scores = betweenness(&doubling_ladder(LAYERS));
    for (v, &score) in scores.iter().enumerate() {
        let i = v as u32 / 2;
        let want = 2.0 * f64::from(i) * f64::from(LAYERS - 1 - i);
        assert_eq!(score, want as f32, "node {v} (layer {i})");
    }
}

/// R18: Brandes needs strictly positive weights. A zero-weight edge relaxed after its
/// head was settled drops a predecessor (`a`'s share here: the definition gives
/// `[0, 2, 1, 0]`, the code `[0, 2, 0, 0]`, as does networkx 3.6), and an undirected
/// zero-weight edge is a zero-length cycle with no path count at all. igraph 0.11.9
/// refuses the input (`src/centrality/betweenness.c:436-437`, "Weight vector must be
/// positive"); this refuses it in debug, the module's discipline for negative weights.
#[test]
#[should_panic(expected = "strictly positive")]
fn betweenness_panics_in_debug_on_a_zero_weight_edge() {
    let nodes = [node("s", ""), node("b", ""), node("a", ""), node("t", "")];
    let edges = [
        directed("sb", "s", "b", 1.0),
        directed("sa", "s", "a", 1.0),
        directed("ab", "a", "b", 0.0),
        directed("bt", "b", "t", 1.0),
    ];
    let _ = betweenness(&index_model(&nodes, &edges).expect("fits"));
}

/// R19: eigenvector centrality is a Perron vector, defined for non-negative weights
/// only. On `a <-> b` at strength -1 the iteration settles on the `lambda = -1`
/// vector and reports it converged.
#[test]
#[should_panic(expected = "non-negative")]
fn eigenvector_panics_in_debug_on_a_negative_weight_graph() {
    let nodes = [node("a", ""), node("b", "")];
    let edges = [
        directed("ab", "a", "b", -1.0),
        directed("ba", "b", "a", -1.0),
    ];
    let _ = eigenvector(&index_model(&nodes, &edges).expect("fits"));
}
