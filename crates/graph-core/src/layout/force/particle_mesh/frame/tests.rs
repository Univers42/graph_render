use super::*;

#[test]
fn the_chosen_rung_is_the_smallest_that_fits() {
    let x = [-300.0, 0.0, 700.0];
    let y = [10.0, -50.0, 20.0];
    let frame = place_over((&x, &y), 128, 520.0).expect("finite");
    assert!(frame.cells + frame.reach <= 128);
    assert!(fit(frame.step - 1, 1000.0, 128, 520.0).is_none());
    assert!(frame.origin.0 <= -300.0 && frame.origin.0 > -300.0 - frame.h);
}

#[test]
fn coincident_nodes_and_no_nodes() {
    let frame = place_over((&[5.0, 5.0], &[5.0, 5.0]), 128, 520.0).expect("finite");
    assert_eq!(frame.step, STEP_MIN);
    assert_eq!(frame.cells, 3);
    assert!(place_over((&[f64::NAN], &[0.0]), 128, 520.0).is_none());
    assert!(place_over((&[], &[]), 128, 520.0).is_none());
}

#[test]
fn every_finite_position_lands_inside_the_occupied_cells() {
    let x: Vec<f64> = (0..200).map(|i| libm::sin(i as f64) * 12345.678).collect();
    let y: Vec<f64> = (0..200).map(|i| libm::cos(i as f64 * 0.3) * 77.0).collect();
    let frame = place_over((&x, &y), 256, 520.0).expect("finite");
    for (&px, &py) in x.iter().zip(&y) {
        let ((cx, cy), (fx, fy)) = stencil(&frame, (px, py)).expect("finite");
        assert!(cx + 1 < frame.cells && cy + 1 < frame.cells);
        let back = frame.origin.0 + (cx as f64 + fx) * frame.h;
        assert!((back - px).abs() < 1e-9 * px.abs().max(1.0));
        assert!((0.0..=1.0).contains(&fy));
    }
}

/// Every node of `x`/`y` in one fold, the strict comparisons [`widen`] makes.
fn one_fold(x: &[f64], y: &[f64]) -> Bounds {
    let mut acc = EMPTY;
    for (&px, &py) in x.iter().zip(y) {
        if px.is_finite() && py.is_finite() {
            acc = widen(acc, ((px, py), (px, py)));
        }
    }
    acc
}

#[test]
fn the_blocked_bounds_are_one_fold_at_every_worker_count() {
    let n = 3 * BLOCK as usize + 17;
    let mut x: Vec<f64> = (0..n).map(|i| libm::sin(i as f64 * 0.7) * 50.0).collect();
    let mut y: Vec<f64> = (0..n).map(|i| libm::cos(i as f64 * 0.3) * 20.0).collect();
    // Signed-zero ties on every end, one in each of two blocks, and non-finite nodes that
    // would win every comparison they took part in.
    x.iter_mut()
        .chain(y.iter_mut())
        .for_each(|v| *v = v.abs() + 1.0);
    (x[5], y[5], x[BLOCK as usize + 9], y[BLOCK as usize + 9]) = (0.0, -0.0, -0.0, 0.0);
    (x[7], y[8], x[9]) = (f64::NAN, f64::NEG_INFINITY, f64::INFINITY);
    let want = one_fold(&x, &y);
    let bits = |b: Bounds| [b.0.0, b.0.1, b.1.0, b.1.1].map(f64::to_bits);
    assert_eq!(bits(want)[..2], [0.0f64.to_bits(), (-0.0f64).to_bits()]);
    for workers in [1, 2, 3, 7, 64] {
        let got = bounds((&x, &y), &crate::exec::Serial, workers, &mut Vec::new());
        assert_eq!(got.map(bits), Some(bits(want)), "workers {workers}");
    }
    let nothing = [f64::NAN; 5];
    assert!(
        bounds(
            (&nothing, &nothing),
            &crate::exec::Serial,
            2,
            &mut Vec::new()
        )
        .is_none()
    );
}
