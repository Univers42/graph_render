//! The stage encoder's own tests: determinism, the seeded `remix` draw, and the
//! refusal of a non-finite float rather than hashing it. Split out of `topology.rs`
//! for the house line limit.

use super::*;
use crate::edgekind::EdgeKind;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::weights::REFERENCE_DEGREE;

fn topology_stage(seed: u32, reference_degree: u32) -> Result<Vec<u8>, StageError> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), reference_degree);
    let mut out = Vec::new();
    encode(
        &index_model(&nodes, &edges).map_err(StageError::Capacity)?,
        &mut out,
    )?;
    Ok(out)
}

#[test]
fn the_stage_is_deterministic_and_seed_and_reference_reach_it() {
    let a = topology_stage(5, REFERENCE_DEGREE).expect("fits");
    assert!(!a.is_empty());
    assert_eq!(a, topology_stage(5, REFERENCE_DEGREE).expect("fits"));
    assert_ne!(a, topology_stage(6, REFERENCE_DEGREE).expect("fits"));
    assert_ne!(a, topology_stage(5, REFERENCE_DEGREE + 1).expect("fits"));
}

/// `remix` draws each hierarchy edge's `child_first` from the seed, and the flag
/// decides which end of that edge is the parent — so a sign slip in the draw
/// (`== 1` for `!= 1`) inverts real subtrees in every seeded model, silently, with
/// nothing downstream able to notice. This pins the draw to the property that
/// actually matters: over a spread of seeds, hierarchy edges come out **both**
/// ways, and non-hierarchy edges never child-first at all.
#[test]
fn child_first_is_drawn_from_the_seed_and_only_on_hierarchy_edges() {
    let mut seen_true = 0;
    let mut seen_false = 0;
    for seed in 0..24u32 {
        let (_nodes, edges) = seeded_model(seed, 40, REFERENCE_DEGREE);
        for e in &edges {
            if e.kind != EdgeKind::Hierarchy {
                assert!(
                    !e.child_first,
                    "seed {seed}: a non-hierarchy edge is never child-first"
                );
                continue;
            }
            if e.child_first {
                seen_true += 1;
            } else {
                seen_false += 1;
            }
        }
    }
    assert!(
        seen_true > 0,
        "no seeded hierarchy edge came out child-first"
    );
    assert!(
        seen_false > 0,
        "no seeded hierarchy edge came out parent-first"
    );
}

/// The `child_first` draw, pinned per edge for three seeds.
///
/// The test above proves only that *both* outcomes occur, which a sign slip in the
/// draw (`== 1` for `!= 1`) still satisfies — it just swaps which draw lands child
/// first. This pins the actual per-edge pattern instead, which is what the flag means:
/// one entry per edge, `true` only where the edge is a hierarchy edge drawn child-first.
///
/// The values were produced by running the generator above and printing them, not by
/// working out the PRNG by hand. A golden is the right shape here: the draw is seeded
/// and deterministic (D8), and the alternative — asserting the Mulberry32 stream —
/// would restate the generator rather than pin its output.
#[test]
fn the_child_first_draw_is_pinned_per_edge() {
    let want: [(u32, &[usize]); 3] = [
        (0, &[4, 21, 26, 32, 47]),
        (1, &[10, 14, 16, 20, 21, 34, 55]),
        (2, &[1, 6, 10, 14, 17, 24, 55]),
    ];
    for (seed, at) in want {
        let (_nodes, edges) = seeded_model(seed, 40, REFERENCE_DEGREE);
        for (i, e) in edges.iter().enumerate() {
            assert_eq!(
                e.child_first,
                at.contains(&i),
                "seed {seed}, edge {i}: child_first disagrees with the pinned draw"
            );
        }
    }
}

#[test]
fn a_non_finite_float_is_refused_not_hashed() {
    let mut nan = node("a", "");
    nan.weight = f64::NAN;
    let mut out = Vec::new();
    let topology = index_model(&[nan], &[]).expect("fits");
    let err = encode(&topology, &mut out).expect_err("NaN weight");
    assert_eq!(err, StageError::NonFinite { column: "weight" });
    assert_eq!(err.to_string(), "non-finite value in column weight");
    let mut far = edge("e", "a", "a");
    far.strength = f64::INFINITY;
    let topology = index_model(&[node("a", "")], &[far]).expect("fits");
    let err = encode(&topology, &mut Vec::new()).expect_err("infinite strength");
    assert_eq!(err, StageError::NonFinite { column: "strength" });
}

#[test]
fn the_remix_populates_every_edge_kind_and_crosses_256_groups() {
    let (mut nodes, mut edges) = synthetic_records(600);
    remix(599, &mut nodes, &mut edges);
    let topology = index_model(&nodes, &edges).expect("fits");
    assert!(topology.nodes().group.iter().any(|&g| g > 255));
    for kind in EdgeKind::ALL {
        assert!(edges.iter().any(|e| e.kind == kind), "{kind:?}");
    }
    assert!(!topology.hierarchy().is_empty());
    let hierarchy = || edges.iter().filter(|e| e.kind == EdgeKind::Hierarchy);
    assert!(hierarchy().any(|e| e.child_first) && hierarchy().any(|e| !e.child_first));
    assert!(
        edges
            .iter()
            .all(|e| !e.child_first || e.kind == EdgeKind::Hierarchy)
    );
}

#[test]
fn the_child_first_flag_reaches_the_bytes() {
    let nodes = [node("a", ""), node("b", "")];
    let mut parent_of = edge("h", "a", "b");
    parent_of.kind = EdgeKind::Hierarchy;
    let child_of = EdgeRecord {
        child_first: true,
        ..parent_of.clone()
    };
    let bytes = |e: EdgeRecord| {
        let mut out = Vec::new();
        encode(&index_model(&nodes, &[e]).expect("fits"), &mut out).expect("finite");
        out
    };
    assert_ne!(bytes(parent_of), bytes(child_of));
}

#[test]
fn the_layout_is_pinned_for_a_tiny_topology() {
    let mut out = Vec::new();
    encode(&index_model(&[], &[]).expect("fits"), &mut out).expect("finite");
    assert_eq!(out, [0u8; 16], "four zero counts, nothing else");
    let mut opt = Vec::new();
    put_opt(&mut opt, Some("ab"));
    put_opt(&mut opt, None);
    assert_eq!(opt, [1, 2, 0, 0, 0, b'a', b'b', 0]);
    let mut list = Vec::new();
    put_u32s(&mut list, &[7, 0x0102_0304]);
    assert_eq!(list, [2, 0, 0, 0, 7, 0, 0, 0, 4, 3, 2, 1]);
}

#[test]
fn the_seed_sizes_the_graph_at_two_plus_seed_mod_600_nodes() {
    for (seed, nodes) in [(0, 2), (5, 7), (599, 601), (600, 2), (1205, 7)] {
        let bytes = topology_stage(seed, REFERENCE_DEGREE).expect("fits");
        assert_eq!(bytes[..4], u32::to_le_bytes(nodes), "seed {seed}");
    }
}

#[test]
fn the_adjacency_and_database_members_reach_the_bytes() {
    let nodes = [node("a", "db"), node("b", "db")];
    let encoded = |edges: &[EdgeRecord]| {
        let mut out = Vec::new();
        encode(&index_model(&nodes, edges).expect("fits"), &mut out).expect("finite");
        out
    };
    let (bare, linked) = (encoded(&[]), encoded(&[edge("e", "a", "b")]));
    // by_database: "db" then members [0, 1] closes both encodings.
    let tail = [2, 0, 0, 0, b'd', b'b', 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
    assert!(bare.ends_with(&tail) && linked.ends_with(&tail));
    // Before it: b's in-row [0], then the two empty hierarchy rows.
    let rows = |out: &[u8]| out[..out.len() - tail.len()].to_vec();
    let last = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(rows(&linked).ends_with(&last));
    assert!(rows(&bare).ends_with(&[0; 16]));
}
