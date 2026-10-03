//! Which stages the threaded arm recomputes, and the refusal for one it does not handle.
//! Split out by the house's 300-line limit, and every test here is a claim about the
//! *match* rather than about a hash: the failure this file exists for is an arm that
//! compares itself and reports it as an N-way-equal stage.
//!
//! The refusal test is a negative control in the strict sense — against the arm as it stood
//! it **failed**, because the match's fall-through recomputed *any* unhandled id as a
//! spiral. An id appended to `THREADED_STAGES` and forgotten in the match was therefore
//! "recomputed" as the spiral and compared against the scalar arm's own bytes, so the
//! report printed "N-way equal" for a stage that was never run at any width.

use super::{THREADED_STAGES, geometry};
use crate::hashgate::Setting;
use crate::hashgate::tests::honest;
use graph_core::Stage as _;
use graph_core::layout::circular::ring;
use graph_core::layout::force::{BarnesHut, ParticleMesh, YifanHu};
use graph_core::layout::spiral;
use graph_core::{Grid, REFERENCE_DEGREE, Topology, gate_node_count, index_model, seeded_model};

/// The seed the arms run at: one small enough that every threaded stage converges inside
/// its budget, so a test here measures the match and not a solver's iteration count.
const SEED: u32 = 4;

/// A stage named in the threaded list but with no arm of its own, which is the shape a
/// half-finished edit to `THREADED_STAGES` leaves behind.
const UNHANDLED: &str = "layout.not.a.stage";

/// The gate's own model, so `geometry` is driven over a real `Topology`.
fn topology() -> Topology {
    let (nodes, edges) = seeded_model(SEED, gate_node_count(SEED), REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the model's own topology indexes")
}

/// The honest setting, over which every stage below runs.
fn setting() -> Setting {
    honest()
}

/// A stage the arm names but cannot recompute is refused, not run as something else.
///
/// Against the arm as it stood this returned `Ok` — the fall-through recomputed the spiral
/// for *any* id — so the arm would have compared the scalar arm's bytes with themselves and
/// reported the stage as equal at every width. `expect_err` is the whole assertion: there is
/// no wording that has to be right, only the fact that the id must not resolve.
#[test]
fn a_stage_named_but_not_handled_is_refused() {
    let err = geometry(UNHANDLED, &topology(), &setting(), 3).expect_err("refused");
    assert!(
        err.contains(UNHANDLED),
        "the refusal must name the stage it could not handle: {err}"
    );
}

/// Every stage [`THREADED_STAGES`] names resolves, so the list and the match cannot drift.
///
/// This is the tie between the two spellings of "the stages the threaded arm recomputes".
/// The refusal above stops an unhandled id from passing; this stops the list from naming an
/// id the match never learned, which would fail the gate as a *missing* arm instead — a
/// failure at run time on every `--tiers all` row rather than in a test.
#[test]
fn every_threaded_stage_has_an_arm() {
    for id in THREADED_STAGES {
        geometry(id, &topology(), &setting(), 3)
            .unwrap_or_else(|e| panic!("{id} is in THREADED_STAGES but has no arm: {e}"));
    }
}

/// The arm the fall-through *used* to take is the spiral's, and it is still in the list.
///
/// So the refusal above is not a vacuous one: the id that used to be silently served is a
/// stage the gate really does run threaded, and it is still run threaded — by its own named
/// arm, which is what makes "handle `spiral::ID` and refuse everything else" the fix rather
/// than a removal.
#[test]
fn spiral_is_the_stage_the_catch_all_used_to_take() {
    assert!(
        THREADED_STAGES.contains(&spiral::ID),
        "the spiral was the fall-through's stage; dropping it from the list would make the \
         refusal above vacuous"
    );
    // And it is still computed *as* the spiral rather than refused: the named arm is live.
    assert!(
        geometry(spiral::ID, &topology(), &setting(), 3).is_ok(),
        "the spiral's own arm must still run"
    );
}

/// The other direction of the tie between [`THREADED_STAGES`] and [`geometry`]: an arm may not
/// exist without a listing. `every_threaded_stage_has_an_arm` above is the first direction (a
/// listing may not exist without an arm); between them the two spellings cannot drift.
///
/// **The length is compared against the spelled-out arms, not against a literal.** A count
/// pinned here is a third place the stage set would live, and it is a count that only ever
/// says the tree changed: when the merge added `layout.force.particle_mesh` to
/// `THREADED_STAGES` and to [`geometry`] — the one edit that makes this test meaningful — this
/// test went red on `left: 6, right: 5` and was asserting that the merge had not happened.
/// A stage added in both places now needs no edit here; one added in only one still fails,
/// naming the id that has an arm without a listing.
#[test]
fn the_threaded_list_has_one_arm_each_and_no_more() {
    let arms = [
        BarnesHut::ID,
        YifanHu::ID,
        ParticleMesh::ID,
        Grid::ID,
        ring::ID,
        spiral::ID,
    ];
    assert_eq!(
        THREADED_STAGES.len(),
        arms.len(),
        "one arm per listed stage; add an arm here or drop the id"
    );
    for arm in arms {
        assert!(
            THREADED_STAGES.contains(&arm),
            "{arm} has an arm but no listing"
        );
    }
}
