//! The solver's own per-corner angle (`solver_angle`), against the placement helper
//! (`packing_angle`) it must not be confused with.

use super::{Flat, Rng, reference_solver_angle};
use crate::layout::circle_packing::radii::{angle_sums, packing_angle, solver_angle};
use core::f64::consts::PI;

#[test]
fn the_solver_clamps_its_denominator_where_placement_does_not() {
    // A flower of three equal circles of radius 1e-7: `2ab = 8e-14`, well under the
    // 1e-12 the solver clamps to, so the cosine is `(a^2 + b^2 - c^2) / 1e-12 = 0.04`
    // and the corner opens to about 1.53 rad. `_packing_angle` answers `pi/3` here,
    // because it *guards* the same denominator instead of clamping it
    // (`circle_packing.py:99-101`) — the two differ by nearly half a radian.
    let (r_i, r_j, r_k) = (1e-7, 1e-7, 1e-7);
    let solver = solver_angle(r_i, r_j, r_k);
    assert_eq!(
        solver.to_bits(),
        reference_solver_angle(r_i, r_j, r_k).to_bits(),
        "the clamped acos, exactly"
    );
    assert!(
        (solver - PI / 2.0).abs() < 0.1 && (solver - PI / 3.0).abs() > 0.1,
        "expected a wide corner, not pi/3: {solver}"
    );
    assert_eq!(
        packing_angle(r_i, r_j, r_k).to_bits(),
        (PI / 3.0).to_bits(),
        "placement keeps the pi/3 branch the solver does not have"
    );
    assert_ne!(solver.to_bits(), packing_angle(r_i, r_j, r_k).to_bits());
}

#[test]
fn a_collapsed_corner_answers_a_right_angle_where_placement_answers_sixty() {
    // `r_i = 0`, `r_j = 0`, `r_k = 1e-7`: the two sides out of the corner are `0` and
    // `1e-7`, the side opposite is `1e-7`, and with the denominator clamped the numerator
    // cancels exactly — the cosine is `0.0` and the corner is a right angle. Under the
    // `pi/3` branch this would be 1.0472 against 1.5708, a third of a radian on every
    // corner of the flower, and those corners are what the sweep sums.
    let (r_i, r_j, r_k) = (0.0, 0.0, 1e-7);
    let solver = solver_angle(r_i, r_j, r_k);
    assert_eq!(solver.to_bits(), (PI / 2.0).to_bits(), "acos(0.0)");
    assert_eq!(
        reference_solver_angle(r_i, r_j, r_k).to_bits(),
        (PI / 2.0).to_bits()
    );
    assert_eq!(packing_angle(r_i, r_j, r_k).to_bits(), (PI / 3.0).to_bits());
}

#[test]
fn a_whole_flower_whose_denominators_all_underflow_sums_60_degrees_never() {
    // The bug as the solver actually meets it: a scale at which `2ab` underflows on
    // *every* corner. The underflow needs all three radii small — `2ab` is
    // `2 (r_i + r_j)(r_i + r_k)`, so one tiny radius among large ones leaves `a` and `b`
    // near their large values and the denominator is fine. Here the three radii are
    // 1e-7, 2e-7, 3e-7, which put every corner's `2ab` between 2.4e-13 and 4e-13.
    //
    // The three clamped angles come out as `acos(0)`, `acos(0.18)` and `acos(0.32)` —
    // three distinct values, none of them `pi/3`. A `pi/3` branch would answer 1.0472 for
    // all three, so every slot here is wrong by between a fifth and a half of a radian.
    let flat = Flat::from_flowers(vec![vec![(1, 2)], vec![(0, 2)], vec![(0, 1)]]);
    let radii = [1e-7, 2e-7, 3e-7];
    for (i, &r_i) in radii.iter().enumerate() {
        let denom = 2.0 * (r_i + radii[1]) * (r_i + radii[2]);
        assert!(denom < 1e-12, "vertex {i} must underflow, got {denom}");
    }
    let sums = angle_sums(&flat.at, &radii);
    for (i, &r_i) in radii.iter().enumerate() {
        // Each vertex's own corner names its own neighbours, so the reference value is
        // read off the flower rather than assumed.
        let &(l, r) = &flat.at[i][0];
        let want = reference_solver_angle(r_i, radii[l as usize], radii[r as usize]);
        assert_eq!(sums[i].to_bits(), want.to_bits(), "vertex {i}");
        assert_ne!(
            sums[i].to_bits(),
            (PI / 3.0).to_bits(),
            "vertex {i}: a pi/3 branch is the bug, got {}",
            sums[i]
        );
    }
    // And the three answers really do differ, so this is not one case checked three times.
    assert_ne!(sums[0].to_bits(), sums[1].to_bits());
    assert_ne!(sums[1].to_bits(), sums[2].to_bits());
    // The triangle no longer closes to pi under the clamp — by construction: the clamp
    // is a fixed floor on the denominator, not a limit of the corner formula, so it
    // changes the geometry rather than approximating it. That is the reference's own
    // behaviour and this port reproduces it; it is not a mistake to assert away.
    let total: f64 = (0..3)
        .map(|i| {
            let &(l, r) = &flat.at[i][0];
            reference_solver_angle(radii[i], radii[l as usize], radii[r as usize])
        })
        .sum();
    assert!(
        (total - PI).abs() > 0.01,
        "expected the clamp to break the closure, got {total}"
    );
}

#[test]
fn the_two_angle_functions_agree_exactly_while_the_denominator_is_real() {
    // The fix must not become a second, differently-rounded copy of the same formula:
    // above the clamp the two are the identical expression, so the bit patterns must
    // match corner for corner across a seeded sweep. Only the `denom < 1e-12` region may
    // differ, and that is the case the two tests above pin.
    let mut rng = Rng(0x5EED_0A11);
    let mut checked = 0;
    for _ in 0..4096 {
        // Radii across the eight decades, so the sweep really does straddle the clamp
        // rather than sitting above it for all 4096 cases.
        let decade = |rng: &mut Rng| rng.unit() * super::DECADES[(rng.next_u32() % 8) as usize];
        let (r_i, r_j, r_k) = (decade(&mut rng), decade(&mut rng), decade(&mut rng));
        if 2.0 * (r_i + r_j) * (r_i + r_k) < 1e-12 {
            continue;
        }
        checked += 1;
        assert_eq!(
            solver_angle(r_i, r_j, r_k).to_bits(),
            packing_angle(r_i, r_j, r_k).to_bits(),
            "above the clamp the two forms are the same expression"
        );
        assert_eq!(
            solver_angle(r_i, r_j, r_k).to_bits(),
            reference_solver_angle(r_i, r_j, r_k).to_bits()
        );
    }
    // ...and the sweep must have reached enough of the region above the clamp, or the
    // assertions above prove nothing.
    assert!(checked > 100, "only {checked} cases cleared the clamp");
}

#[test]
fn a_collapsed_sweep_sums_the_clamped_angles_and_not_sixty_degrees() {
    // The kernel itself, on a real flower: the single corner `(0, 1, 2)` seen from
    // vertex 0. All three radii are 1e-7, so the corner underflows and the sweep must
    // read the clamped angle back.
    let flat = Flat::from_flowers(vec![vec![(1, 2)], vec![], vec![]]);
    let radii = [1e-7; 3];
    let want = reference_solver_angle(1e-7, 1e-7, 1e-7);
    let sums = angle_sums(&flat.at, &radii);
    assert_eq!(sums[0].to_bits(), want.to_bits(), "the clamped acos");
    assert_ne!(
        sums[0].to_bits(),
        (PI / 3.0).to_bits(),
        "a pi/3 branch here is the bug: {}",
        sums[0]
    );
    // The other two vertices carry no corner, so the gather still writes their slots — as
    // exact zeros, which is what the scatter's `vec![0.0]` left there too.
    assert_eq!(sums[1].to_bits(), 0.0_f64.to_bits());
    assert_eq!(sums[2].to_bits(), 0.0_f64.to_bits());
}
