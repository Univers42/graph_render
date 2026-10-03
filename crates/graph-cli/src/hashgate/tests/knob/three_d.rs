//! The seven natively 3D layout controls, each moving its own stage and no other.
//!
//! p12-t3 appended `layout.basic3d.{sphere,helix,cube}`, `layout.hierarchical3d` and
//! `layout.force.spring3d` to `LAYOUTS` with no control reaching any one of them alone, so
//! all five were covered only by the shared `GM_MUTATE_NODE_COUNT` — which moves every
//! stage at once and names none of them. These five are the per-stage controls that close
//! the gap, the same re-drawn-model probe the igraph family uses
//! ([`crate::hashgate::knobs::THREE_D_LAYOUT_STAGES`]), and for `sphere`, `helix` and `cube`
//! it is not merely the available probe but the *only* one: those three read the node count
//! and no edge at all, so one more node is precisely their whole input.

use super::controls::only_stages_moved;
use super::p3::stage_of;
use super::*;
use graph_core::layout::basic_3d::{bipartite_3d, cube, helix, sphere, spiral};
use graph_core::layout::force::spring::{self, ID_3D as SPRING_3D};
use graph_core::layout::hierarchical_3d;

/// Every control but one more node, the value each must be set by. The variable's own
/// environment and the stage it is filed under: one slice per control because [`env`] reads
/// a slice of pairs.
const NODES: [(&str, &str); 7] = [
    ("GM_MUTATE_BASIC3D_SPHERE_NODES", "1"),
    ("GM_MUTATE_BASIC3D_HELIX_NODES", "1"),
    ("GM_MUTATE_BASIC3D_CUBE_NODES", "1"),
    ("GM_MUTATE_HIERARCHICAL3D_NODES", "1"),
    ("GM_MUTATE_FORCE_SPRING3D_NODES", "1"),
    // knobs-3d-new: the two layouts p12-t3's list predates.
    ("GM_MUTATE_BASIC3D_SPIRAL_NODES", "1"),
    ("GM_MUTATE_BIPARTITE_3D_NODES", "1"),
];

/// The same seven pairs beside the stage each names, so a permutation of one list cannot
/// pass against the other. The ids are graph-core's own constants.
const STAGES: [&str; 7] = [
    sphere::ID,
    helix::ID,
    cube::ID,
    hierarchical_3d::ID,
    SPRING_3D,
    spiral::ID,
    bipartite_3d::ID,
];

/// Each of the seven turns the gate red for its own stage and no other — the property a
/// negative control exists for, and the one p12-t3's shared node count could not give.
#[test]
fn each_three_d_layout_has_its_own_control_that_moves_only_its_stage() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    for (pairs, stage) in NODES.iter().zip(STAGES) {
        let moved = setting(env(vec![*pairs])).expect("parses");
        assert_eq!(moved.control, Some(knob_named(pairs.0)));
        only_stage_moved(&base, &stage_bytes(P3_SEED, &moved).expect("runs"), stage);
    }
}

/// The five name a *stage*, not a count, and leave the gate's own model alone: the shared
/// `GM_MUTATE_NODE_COUNT` would grow the one model every stage is a function of, which is
/// the ambiguity the per-stage controls exist to remove.
#[test]
fn the_three_d_controls_re_draw_one_stages_model_and_not_the_gate_s_own() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    for (pairs, stage) in NODES.iter().zip(STAGES) {
        let moved = setting(env(vec![(pairs.0, "3")])).expect("parses");
        assert_eq!(moved.stage_nodes, Some((stage, 3)), "{}", pairs.0);
        assert_eq!(
            moved.extra_nodes, 0,
            "{}: the gate's model is untouched",
            pairs.0
        );
        let bytes = stage_bytes(P3_SEED, &moved).expect("runs");
        only_stage_moved(&base, &bytes, stage);
        assert_eq!(
            stage_of(&base, "topology"),
            stage_of(&bytes, "topology"),
            "{}: a stage-scoped control must not move the transport stage's topology",
            pairs.0
        );
    }
}

/// A typo, a negative count, a fractional one and a zero are all refused: a control that
/// falls back to the default, or perturbs by nothing, would pass as green.
#[test]
fn the_three_d_controls_are_refused_rather_than_defaulting() {
    for (pairs, _) in NODES.iter().zip(STAGES) {
        for value in ["one", "1.5", "-1", "0", ""] {
            let err = setting(env(vec![(pairs.0, value)])).expect_err("refused");
            assert!(err.starts_with(pairs.0), "{}={value:?}: {err}", pairs.0);
        }
    }
    let both = setting(env(vec![(NODES[0].0, "1"), (NODES[4].0, "1")])).expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// **`layout.force.spring3d` is `layout.force.spring` at `D = 3`** — one kernel, one
/// `SpringParams`, one `Solver::settle` (`force/spring.rs:163`, `force/spring3d.rs:45`) —
/// so the *parameter* control reaches both dimensions and the per-stage control reaches
/// only the 3D one. Both claims are asserted here, because the second is the reason the
/// first is not a substitute for it: a knob that moved two stages could not name which
/// divergence came from where.
#[test]
fn the_spring_iteration_control_reaches_both_dimensions_and_the_node_control_only_the_3d() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    let iterations = setting(env(vec![("GM_MUTATE_SPRING_ITERATIONS", "3")])).expect("parses");
    only_stages_moved(
        &base,
        &stage_bytes(P3_SEED, &iterations).expect("runs"),
        &[spring::ID, SPRING_3D],
    );
    let nodes = setting(env(vec![("GM_MUTATE_FORCE_SPRING3D_NODES", "1")])).expect("parses");
    only_stage_moved(
        &base,
        &stage_bytes(P3_SEED, &nodes).expect("runs"),
        SPRING_3D,
    );
    // And the two are separate stages, or "one kernel, two ids" would be a claim about a
    // single hashed row: a drawing labelled 0.3 and carrying no z hashes differently from
    // the same drawing at 0.4 with one, which is why the ids differ at all.
    assert_ne!(
        stage_of(&base, spring::ID),
        stage_of(&base, SPRING_3D),
        "the two dimensions are one row, so one control could name them"
    );
}

/// The one knob that reads `env`: the same lookup the knob table's own helper makes, kept
/// here so a control that parses is also the arm it claims to be.
fn knob_named(env: &'static str) -> Knob {
    *Knob::ALL
        .iter()
        .find(|knob| knob.env() == env)
        .unwrap_or_else(|| panic!("{env} is read"))
}
