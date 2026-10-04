use super::*;

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

/// The seed the rescale-merge control's rows run at, and the two stage ids it may move.
const TIER_SEED: u32 = 31;
const RING: &str = graph_core::layout::circular::ring::ID;
const SPIRAL: &str = graph_core::layout::spiral::ID;
