//! The packing oracle held to the states a real packing can and cannot be in: an exact
//! planar packing, a fallback packing, and the numbers this file chose itself.

use super::*;

/// The exact path really runs on a planar wheel, and the branch really passes there.
#[test]
fn an_exact_packing_of_a_planar_wheel_passes_its_own_tangency_promise() {
    let exact = exact_packing();
    let p = exact.parts();
    assert_eq!(p.nodes.kind(), NodeGeometryKind::Circle);
    assert!(
        !p.notes
            .code
            .contains(&(NoteCode::PackingApproximate as u32)),
        "a planar wheel is packed on the exact path: no note 3"
    );
    assert_eq!(
        p.source.len(),
        2 * WHEEL_RIM as usize,
        "the wheel's 12 edges"
    );
    let worst = worst_error(&exact);
    println!("packing oracle: a wheel's worst tangency error = {worst:e}");
    assert!(worst < TANGENCY, "worst observed tangency error {worst}");
    assert_eq!(
        super::packing(&exact),
        Ok(()),
        "the tangency branch ran and passed"
    );
}

/// The negative control the branch is owed: move one circle of an exact packing and the
/// oracle goes red, naming the edge it broke. Without this a packing oracle that returned
/// `Ok(())` for everything would still pass every gate seed. The message is pinned to what
/// the wheel above packs to, measured by running it (2026-09-28, branch p3).
#[test]
fn moving_one_circle_of_an_exact_packing_turns_the_oracle_red() {
    let exact = exact_packing();
    let moved = moved(&exact, 2, (0.5, 0.0));
    let err = super::packing(&moved).expect_err("a moved circle is not tangent");
    assert_eq!(
        err,
        "edge 1 (0, 2): centres 1.8027756 apart, radii sum to 1.5"
    );
    // The same geometry is fine the moment the fallback flags it: what the promise hangs
    // on is the note, not the coordinates.
    assert_eq!(super::packing(&renoted(&moved, &approximate())), Ok(()));
}

/// A fallback packing is held to finiteness and nothing else, from both sides: the broken
/// geometry passes because note 3 is there, and fails once it is gone.
#[test]
fn a_fallback_packing_is_held_to_nothing_but_the_note_that_excuses_it() {
    let fallback = fallback_packing();
    assert_eq!(
        fallback.parts().notes.code,
        vec![NoteCode::PackingApproximate as u32],
        "the gate's model is non-planar: flagged, and flagged with nothing else"
    );
    assert_eq!(super::packing(&fallback), Ok(()));
    let broken = moved(&fallback, 1, (7.0, -3.0));
    assert_eq!(
        super::packing(&broken),
        Ok(()),
        "note 3: no tangency promised"
    );
    let relabelled = renoted(&broken, &[]);
    assert!(
        super::packing(&relabelled)
            .expect_err("no note 3, so tangency is claimed after all")
            .starts_with("edge "),
        "a non-tangent packing claiming to be exact is caught"
    );
}

/// The bound itself, on circles this file placed: relative to the radii summed, never
/// tighter than an absolute thousandth, and refused once it is five thousandths out.
#[test]
fn tangency_is_held_to_a_thousandth_of_the_radii_summed() {
    assert_eq!(
        super::packing(&pair(1.0, 0.5, 0.0)),
        Ok(()),
        "exactly tangent"
    );
    assert_eq!(
        super::packing(&pair(1.0, 0.5, 0.0005)),
        Ok(()),
        "half a thousandth"
    );
    assert_eq!(
        super::packing(&pair(0.5006, 0.25, 0.0)),
        Ok(()),
        "small radii"
    );
    assert_eq!(super::packing(&pair(2.0015, 1.0, 0.0)), Ok(()), "big radii");
    for out in [pair(1.0, 0.5, 0.005), pair(0.5006, 0.25, 0.005)] {
        assert!(
            super::packing(&out)
                .expect_err("five thousandths out")
                .starts_with("edge 0 (0, 1): centres "),
            "five thousandths out is refused"
        );
    }
    let err = super::packing(&pair(1.0, 0.5, 0.25)).expect_err("a quarter out");
    assert_eq!(err, "edge 0 (0, 1): centres 1 apart, radii sum to 1.25");
}

/// A radius of zero is a legal circle: the contract refuses only `r < 0.0`
/// (`graph_contract::geometry::NodeGeometry::check`), so the oracle holds no stricter
/// promise than the wire does, and two zero circles at one point are tangent.
#[test]
fn a_radius_of_zero_is_a_legal_circle_and_a_self_loop_is_no_claim() {
    assert_eq!(
        super::packing(&circles(&[(0.0, 0.0), (0.0, 0.0)], &[0.0, 0.0], &[(0, 1)])),
        Ok(())
    );
    let self_loop = circles(&[(0.0, 0.0), (0.0, 0.0)], &[0.5, 0.5], &[(0, 0)]);
    assert_eq!(
        super::packing(&self_loop),
        Ok(()),
        "no two circles to place"
    );
}
