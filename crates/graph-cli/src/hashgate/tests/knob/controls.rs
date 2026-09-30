use super::*;
use crate::hashgate::stage_bytes_threaded;

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

/// **`GM_MUTATE_SPLIT_RESCALE` must reach the merge, not the gather, and must move only
/// the threaded arms of the stages that end in that merge.** The negative control for the
/// `equal` column and for the hash gate's per-width arms: without it, a threaded arm that
/// reused the scalar column would compare equal forever and "10-way equal" would be a
/// statement about nothing.
///
/// Run through [`stage_bytes_threaded`] at a width above one, because that is the arm the
/// gate actually compares — the *scalar* arms are hashed from the honest run and must not
/// move, or the gate would go red for a reason that has nothing to do with threading.
#[test]
fn the_rescale_control_moves_only_the_three_closed_form_layouts_of_the_threaded_arms() {
    let base = stage_bytes_threaded(TIER_SEED, &honest(), 3).expect("runs");
    let split = setting(env(vec![("GM_MUTATE_SPLIT_RESCALE", "1")])).expect("parses");
    assert_eq!(split.control, Some(Knob::SplitRescale));
    let moved = stage_bytes_threaded(TIER_SEED, &split, 3).expect("runs");
    for ((id, a), (_, b)) in base.iter().zip(&moved) {
        let threads_rescale = matches!(*id, RING | SPIRAL);
        assert_eq!(a == b, !threads_rescale, "{id} moved or did not, wrongly");
    }
    // The grid is in the threaded match and does **not** move: it has no reduction to
    // split, so a control that moved it would be reaching its gather. This is the property
    // that tells the two controls apart — `GM_MUTATE_GRID_SPACING` is the grid's pass
    // control and moves every arm alike, this one does not touch it at all.
    assert!(
        base.iter().any(|(id, _)| *id == ring::ID) && base.iter().any(|(id, _)| *id == spiral::ID),
        "the two rescale stages are hashed, or the control proves nothing"
    );
    // And the force stage is untouched: `split_sum` is Barnes-Hut's, and a rescale control
    // that reached it would be the same knob doing two jobs.
    assert_eq!(
        base.iter()
            .find(|(id, _)| *id == BarnesHut::ID)
            .map(|(_, b)| b),
        moved
            .iter()
            .find(|(id, _)| *id == BarnesHut::ID)
            .map(|(_, b)| b),
        "the force stage moved under a rescale control"
    );
}

/// At `workers = 1` the threaded arm is the scalar arm, so a control that reaches the
/// threaded path is still red — the floor of the row, and the reason a `--tiers all` run
/// with one width is not the trivial case.
#[test]
fn the_rescale_control_bites_at_every_width_including_one() {
    let split = setting(env(vec![("GM_MUTATE_SPLIT_RESCALE", "1")])).expect("parses");
    for workers in [1, 2, 3, 4, 7] {
        let base = stage_bytes_threaded(TIER_SEED, &honest(), workers).expect("runs");
        let moved = stage_bytes_threaded(TIER_SEED, &split, workers).expect("runs");
        let changed = base
            .iter()
            .zip(&moved)
            .filter(|((id, a), (_, b))| matches!(*id, RING | SPIRAL) && a != b)
            .count();
        assert_eq!(changed, 2, "workers = {workers}: the control did not bite");
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

/// The seed whose model is large enough that a theta change reaches the quadtree's
/// opening test. At the gate's smallest models every cell is already inside theta and
/// the two values coincide, which would make the control vacuous.
const FORCE_SEED: u32 = 30;

/// The seed the rescale-merge control's rows run at, and the two stage ids it may move.
const TIER_SEED: u32 = 31;
const RING: &str = graph_core::layout::circular::ring::ID;
const SPIRAL: &str = graph_core::layout::spiral::ID;
