use super::*;

/// **A control that cannot bite must refuse the run, not pass it.** Collide's own control
/// moves nothing below five seeds — at two or three nodes link and many-body have already
/// pushed every pair past `2 * collideRadius`, so the deltas it would split are all zero
/// (measured in `barnes_hut/tests/kernels.rs`, where collide first bites at seed 4). A row
/// run at two seeds would exit 0 having corrupted nothing: a **vacuous pass**, which is
/// worse than a failure because it reads as evidence. So the gate refuses the run instead,
/// and the refusal is exit 2, "could not run" — never exit 0.
#[test]
fn a_control_that_cannot_bite_at_this_seed_count_refuses_rather_than_passing() {
    for split in [Split::Collide, Split::All] {
        for seeds in 0..5 {
            let err = refuse_a_vacuous_control(seeds, split).expect_err("refused");
            assert!(
                err.contains("collide") && err.contains(&format!("{seeds}")),
                "the refusal must name the pass and the count: {err:?}"
            );
        }
        assert!(
            refuse_a_vacuous_control(5, split).is_ok(),
            "{split:?} bites at five seeds and must run"
        );
        assert!(
            refuse_a_vacuous_control(8, split).is_ok(),
            "{split:?} at the phase gate's eight seeds must run"
        );
    }
    // The other two bites at the first seed, and an honest run sets no control at all: a
    // floor on those would refuse rows the gate has always run and needs.
    for split in [Split::None, Split::Charge, Split::Link] {
        assert!(
            refuse_a_vacuous_control(1, split).is_ok(),
            "{split:?} bites at one seed"
        );
    }
    assert!(
        refuse_a_vacuous_control(2, Split::None).is_ok(),
        "an honest run has no control to be vacuous"
    );
}

/// The floor is the model's, not a constant invented beside the gate: graph-core measures
/// which seed each pass's control first bites at, and the gate reads the same number.
#[test]
fn the_collide_controls_floor_is_the_one_graph_core_measures() {
    assert_eq!(
        Split::Collide.min_seeds(),
        5,
        "collide first bites at seed 4, so five seeds is the floor"
    );
    assert_eq!(Split::All.min_seeds(), Split::Collide.min_seeds());
    assert_eq!(Split::Charge.min_seeds(), 1);
    assert_eq!(Split::Link.min_seeds(), 1);
    assert_eq!(Split::None.min_seeds(), 1);
}

/// A force layout's own negative control must move that stage and *only* that stage: a
/// control that also moved the topology would back every stage at once and prove nothing
/// about the stage it is filed under.
pub(super) fn only_stage_moved(
    base: &[(&'static str, Vec<u8>)],
    moved: &[(&'static str, Vec<u8>)],
    stage: &str,
) {
    for ((id, a), (_, b)) in base.iter().zip(moved) {
        assert_eq!(
            a == b,
            *id != stage,
            "only {stage} may move, but {id} did not"
        );
    }
}

#[test]
fn each_force_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(FORCE_SEED, &honest()).expect("runs");
    let theta = setting(env(vec![("GM_MUTATE_FORCE_THETA", "0.5")])).expect("parses");
    assert_eq!(theta.control, Some(Knob::ForceTheta));
    assert_eq!(theta.force.theta, 0.5);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &theta).expect("runs"),
        BarnesHut::ID,
    );
    let scaling = setting(env(vec![("GM_MUTATE_FA2_SCALING_RATIO", "3")])).expect("parses");
    assert_eq!(scaling.control, Some(Knob::Fa2ScalingRatio));
    assert_eq!(scaling.fa2.scaling_ratio, 3.0);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &scaling).expect("runs"),
        ForceAtlas2::ID,
    );
    let both = env(vec![
        ("GM_MUTATE_FORCE_THETA", "0.5"),
        ("GM_MUTATE_FA2_SCALING_RATIO", "3"),
    ]);
    assert!(
        setting(both)
            .expect_err("one at a time")
            .ends_with("one control at a time")
    );
    // A typo in either new variable must be refused, not fall back to the default and
    // let the control pass as green.
    let bad: [&'static [(&str, &str)]; 2] = [
        &[("GM_MUTATE_FORCE_THETA", "wide")],
        &[("GM_MUTATE_FA2_SCALING_RATIO", "")],
    ];
    for pairs in bad {
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
}

/// The two p12-t2 controls, each moving its own stage and no other.
///
/// One is a parameter and one re-draws a model, and the difference is the layouts': spring
/// publishes [`graph_core::layout::force::spring::SpringParams`], while SciGraphs' closed
/// form takes none at all (its `scale` is the dispatcher's own constant), so the graph is the
/// only thing a control can move for it. Both claims are the same claim the four Phase 3
/// controls make, and the test is the same test.
#[test]
fn each_p12_t2_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    let iterations = setting(env(vec![("GM_MUTATE_SPRING_ITERATIONS", "3")])).expect("parses");
    assert_eq!(iterations.control, Some(Knob::SpringIterations));
    assert_eq!(iterations.spring.iterations, 3);
    only_stage_moved(
        &base,
        &stage_bytes(P3_SEED, &iterations).expect("runs"),
        Spring::ID,
    );
    let nodes = setting(env(vec![("GM_MUTATE_CIRCULAR_HIERARCHY_NODES", "1")])).expect("parses");
    assert_eq!(nodes.control, Some(Knob::CircularHierarchyNodes));
    assert_eq!(nodes.stage_nodes, Some((circular::hierarchy::ID, 1)));
    assert_eq!(nodes.extra_nodes, 0, "the gate's own model is untouched");
    only_stage_moved(
        &base,
        &stage_bytes(P3_SEED, &nodes).expect("runs"),
        circular::hierarchy::ID,
    );
    let both = env(vec![
        ("GM_MUTATE_SPRING_ITERATIONS", "3"),
        ("GM_MUTATE_CIRCULAR_HIERARCHY_NODES", "1"),
    ]);
    assert!(
        setting(both)
            .expect_err("one at a time")
            .ends_with("one control at a time")
    );
    // A typo, a negative budget and a zero count are all refused rather than falling back
    // to the default, which is what a control that moves nothing looks like.
    let bad: [&'static [(&str, &str)]; 3] = [
        &[("GM_MUTATE_SPRING_ITERATIONS", "three")],
        &[("GM_MUTATE_SPRING_ITERATIONS", "-1")],
        &[("GM_MUTATE_CIRCULAR_HIERARCHY_NODES", "0")],
    ];
    for pairs in bad {
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
}

/// The seed whose model is large enough that a theta change reaches the quadtree's
/// opening test. At the gate's smallest models every cell is already inside theta and
/// the two values coincide, which would make the control vacuous.
const FORCE_SEED: u32 = 30;
