use super::p3::stage_of;
use super::*;
use crate::hashgate::stage_bytes_threaded;
mod rescale;

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
    // The other two bite at the first seed, and an honest run sets no control at all: a
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

/// **The predicate above can only be trusted because these bite — measured, not asserted.**
///
/// `refuse_a_vacuous_control(1, split).is_ok()` says nothing on its own: a control that had
/// stopped moving anything would satisfy it just as well as one that bites, which is the
/// condition the whole vacuity guard exists to catch. So the guard's silence is read next to
/// a measurement of the bytes, through the **threaded** arm — `split_sum` reaches a gathered
/// pass's merge only under `run_under(.., &Threads, ..)`, so the scalar arm is the arm that
/// cannot see this control at all (RG-48). If a control stopped biting, this goes red and the
/// `is_ok()` above is no longer being read as permission.
#[test]
fn the_floorless_controls_bite_at_one_seed_which_is_what_their_floorlessness_means() {
    let bites = |seed: u32, setting: &Setting| {
        stage_of(
            &stage_bytes_threaded(seed, &honest(), 1).expect("runs"),
            BarnesHut::ID,
        ) != stage_of(
            &stage_bytes_threaded(seed, setting, 1).expect("runs"),
            BarnesHut::ID,
        )
    };
    for word in ["charge", "link"] {
        let split = setting(env(vec![("GM_MUTATE_SPLIT_SUM", word)])).expect("parses");
        assert!(
            bites(FORCE_SEED, &split),
            "GM_MUTATE_SPLIT_SUM={word} must move Barnes-Hut's stage, or its floor of one seed \
             is a claim nothing checks"
        );
    }
    // The collide controls are the other half: their floor is a *measured* seed count, and
    // this is the measurement that makes `min_seeds` a fact rather than a constant beside
    // the gate — collide first bites at seed 4, so five seeds is the floor and not one less.
    let collide = setting(env(vec![("GM_MUTATE_SPLIT_SUM", "collide")])).expect("parses");
    let mut first = None;
    for seed in 0..8 {
        if bites(seed, &collide) {
            first = Some(seed);
            break;
        }
    }
    assert_eq!(first, Some(4), "collide first bites at seed 4");
    assert_eq!(
        Split::Collide.min_seeds(),
        first.expect("bites") + 1,
        "the floor is one past the first seed that moves, which is what it is for"
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
    only_stages_moved(base, moved, &[stage]);
}

/// Every stage in `allowed` must move and every other must be byte-identical. The
/// many-stage sibling of [`only_stage_moved`], for a control that legitimately reaches more
/// than one — `GM_MUTATE_SPRING_ITERATIONS` moves the spring kernel at both `D = 2` and
/// `D = 3`, and that is a claim to be asserted rather than a gap to be tolerated.
pub(super) fn only_stages_moved(
    base: &[(&'static str, Vec<u8>)],
    moved: &[(&'static str, Vec<u8>)],
    allowed: &[&str],
) {
    // **Both lists must be the whole stage list, and every `allowed` id must be in it**
    // (RG-48). The `zip` below stops at the shorter side, so a `moved` list that lost a stage
    // — a control that reached fewer stages, or a run that refused one — satisfied "only
    // these moved" by never looking at what it dropped. An `allowed` id that is not in the
    // list was the same silence: the control moved nothing and the assertion passed.
    assert_eq!(
        base.len(),
        moved.len(),
        "the perturbed run produced {} stages against {base:?} names",
        moved.len()
    );
    for id in base.iter().map(|(id, _)| *id).collect::<Vec<_>>() {
        assert_eq!(
            base.iter().filter(|(seen, _)| *seen == id).count(),
            1,
            "{id} appears twice in the base list, so 'only {allowed:?} moved' is ambiguous"
        );
    }
    for id in allowed {
        assert!(
            base.iter().any(|(seen, _)| seen == id),
            "{id} is allowed to move but is not one of the {} stages that ran",
            base.len()
        );
    }
    for ((id, a), (_, b)) in base.iter().zip(moved) {
        assert_eq!(
            a == b,
            !allowed.contains(id),
            "only {allowed:?} may move, but {id} did not"
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
    // Both ForceAtlas2 stages and no other: the tree layout's far cells and leaf pairs
    // read the same `scaling_ratio` the dense pair loop does.
    only_stages_moved(
        &base,
        &stage_bytes(FORCE_SEED, &scaling).expect("runs"),
        &[ForceAtlas2::ID, ForceAtlas2BarnesHut::ID],
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
    // Both spring stages and no other. `layout.force.spring3d` is the same kernel at
    // `D = 3` over the same `SpringParams`, so the budget is one parameter and this control
    // reaches both — which is exactly why it is a *parameter* control and not spring3d's
    // own: it proves the shared kernel is compared at both dimensions, and
    // `GM_MUTATE_FORCE_SPRING3D_NODES` is what names the 3D stage alone
    // (`three_d::the_spring_iteration_control_reaches_both_dimensions_and_the_node_control_only_the_3d`).
    only_stages_moved(
        &base,
        &stage_bytes(P3_SEED, &iterations).expect("runs"),
        &[Spring::ID, graph_core::layout::force::spring::ID_3D],
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
