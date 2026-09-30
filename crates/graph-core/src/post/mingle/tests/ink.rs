use super::*;

#[test]
fn parameters_are_clamped_to_the_references_own_limits() {
    assert_eq!(
        Params {
            neighbors: 0,
            rounds: 99,
            min_gain: -1.0
        }
        .clamped(),
        Params {
            neighbors: 1,
            rounds: MAX_ROUNDS,
            min_gain: 0.0
        },
        "neighbors floored at 1, rounds capped, min_gain never negative"
    );
    assert_eq!(
        Params {
            neighbors: 99,
            rounds: 0,
            min_gain: f64::NAN
        }
        .clamped(),
        Params {
            neighbors: MAX_NEIGHBORS,
            rounds: 0,
            min_gain: 0.0
        },
        "a NaN gain takes 0 rather than poisoning every comparison"
    );
    // Zero rounds is a legal request: nothing is bundled, and the paths are the input's
    // straight edges.
    let (topology, g) = laid_out();
    let out = over(
        &topology,
        &g,
        &Params {
            rounds: 0,
            ..Params::default()
        },
    )
    .expect("runs");
    assert_eq!(out.merges, 0);
    assert_eq!(out.depth, vec![0; topology.edge_count() as usize]);
    assert_eq!(
        out.paths.offsets.iter().sum::<u32>(),
        0,
        "no interior points at all"
    );
    assert_eq!(out.ink_after, out.ink_before, "the drawing is untouched");
}

#[test]
fn a_min_gain_above_one_refuses_every_merge() {
    let (topology, g) = laid_out();
    let out = over(
        &topology,
        &g,
        &Params {
            min_gain: 2.0,
            ..Params::default()
        },
    )
    .expect("runs");
    assert_eq!(out.merges, 0, "no gain can be twice the pair's own ink");
    assert!(out.depth.iter().all(|depth| *depth == 0));
}

#[test]
fn the_ink_a_merge_is_scored_on_is_not_the_ink_a_viewer_sees() {
    // Three copies of one edge plus a fourth leaving the same node. The score counts one
    // line as several, so it sees a large gain; the deduplicated drawing is longer than the
    // straight one, because the copies' arcs are not shared the way a trunk is. This is the
    // Ponytail marker the module doc owes, pinned as an exact figure.
    let pos = [[0.0, 0.0], [4.0, 0.0], [0.0, 1.0]];
    let src = [0, 0, 0, 0];
    let dst = [1, 1, 1, 2];
    let out = bundle(&pos, &src, &dst, &Params::default()).expect("runs");
    assert!(out.merges > 0, "the copies did merge");
    assert!(
        out.ink_after > out.ink_before,
        "the deduplicated drawing grew: {} -> {}",
        out.ink_before,
        out.ink_after
    );
    assert_eq!(out.unbundled(), 0, "the fourth edge rides the bundle too");
}

#[test]
fn an_endpoint_past_the_positions_is_refused_rather_than_read_as_zero() {
    let pos = [[0.0, 0.0], [1.0, 1.0]];
    let err = bundle(&pos, &[0], &[2], &Params::default()).expect_err("out of range");
    assert!(err.to_string().contains("dense node index"), "{err}");
    // The same positions and the same indices in range run, which is what makes the refusal
    // above about the index and not about the geometry.
    assert!(bundle(&pos, &[0], &[1], &Params::default()).is_ok());
}

#[test]
fn the_registered_run_hands_back_a_polyline_and_the_layouts_own_nodes() {
    let (topology, g) = laid_out();
    let bundled = run(&topology, &g).expect("runs");
    assert_eq!(
        bundled.geometry.nodes, g.nodes,
        "a post pass may not move a node"
    );
    assert_eq!(bundled.geometry.notes, g.notes);
    let EdgeGeometry::Polyline(paths) = &bundled.geometry.edges else {
        panic!("mingle emits polylines");
    };
    assert_eq!(paths.offsets.len(), topology.edge_count() as usize + 1);
    assert!(bundled.pairs > 0, "the registered default merges something");
    paths
        .check(topology.edge_count())
        .expect("the paths fit the topology");
}

/// The level: what one bundle costs, and what a fused one costs. A singleton's ink is its own
/// length; a fused one's is the two arcs plus the trunk between the meeting points, and the
/// meeting points are the solve's — not the endpoints', which is what makes a bundle shorter
/// than the two edges it replaces.
#[test]
fn a_singletons_ink_is_its_own_length_and_a_fused_ones_is_arcs_plus_trunk() {
    use super::level;
    let ends = [([0.0, 0.0], [4.0, 0.0]), ([0.0, 0.1], [4.0, 0.1])];
    let seeded = level::seed(&ends);
    assert_eq!(seeded.len(), 2);
    assert!(
        (seeded[0].ink - 4.0).abs() < 1e-12,
        "one straight edge is its own length"
    );
    let eps = 1e-4 * level::diagonal(&[ends[0].0, ends[0].1, ends[1].0, ends[1].1]);
    let scored = pass::score(&seeded[0], &seeded[1], eps);
    assert!(
        scored.gain > 0.0,
        "two near-coincident edges are worth bundling"
    );
    // The meeting points sit between the two rows, not on either of them.
    let [_, q_y] = scored.p;
    assert!(
        q_y > 0.0 && q_y < 0.1,
        "the head meets inside the gap, got {q_y}"
    );
    let (level, remap, rev) = pass::fused(&seeded, &[(0, 1)], &[0], &[scored]);
    assert_eq!(level.len(), 1, "one bundle out of two");
    assert_eq!(remap, vec![0, 0], "both edges ride it");
    assert!(!rev[1], "an unflipped pairing reverses nothing");
}

/// The reference's own worked example for the greedy order: three bundles where the best
/// pair is not the first candidate. The pass must take the best gain whatever its position,
/// and must then refuse a second merge that shares a bundle — a matching, not a chain.
#[test]
fn a_pass_takes_the_best_gain_first_and_then_refuses_a_second_share() {
    use super::level;
    let ends = [
        ([0.0, 0.0], [4.0, 0.0]),
        ([0.0, 0.1], [4.0, 0.1]),
        ([0.0, 0.2], [4.0, 0.2]),
    ];
    let seeded = level::seed(&ends);
    let eps = 1e-4
        * level::diagonal(&[
            ends[0].0, ends[0].1, ends[1].0, ends[1].1, ends[2].0, ends[2].1,
        ]);
    let cand = pass::candidates(&seeded, 2);
    assert!(cand.len() >= 2, "candidates found: {cand:?}");
    let scores: Vec<pass::Score> = cand
        .iter()
        .map(|&(u, v)| pass::score(&seeded[u as usize], &seeded[v as usize], eps))
        .collect();
    let picks = pass::matching(&seeded, &cand, &scores, 0.0);
    let best = (0..scores.len())
        .max_by(|&a, &b| scores[a].gain.total_cmp(&scores[b].gain).then(b.cmp(&a)))
        .expect("at least one candidate");
    assert_eq!(picks[0], best, "the best gain is taken first");
    let mut used = std::collections::BTreeSet::new();
    for &i in &picks {
        let (u, v) = cand[i];
        assert!(used.insert(u), "bundle {u} used twice");
        assert!(used.insert(v), "bundle {v} used twice");
    }
}
