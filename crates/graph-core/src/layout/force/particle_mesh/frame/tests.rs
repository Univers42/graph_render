use super::*;

#[test]
fn the_chosen_rung_is_the_smallest_that_fits() {
    let x = [-300.0, 0.0, 700.0];
    let y = [10.0, -50.0, 20.0];
    let frame = place(&x, &y, 128, 520.0).expect("finite");
    assert!(frame.cells + frame.reach <= 128);
    assert!(fit(frame.step - 1, 1000.0, 128, 520.0).is_none());
    assert!(frame.origin.0 <= -300.0 && frame.origin.0 > -300.0 - frame.h);
}

#[test]
fn coincident_nodes_and_no_nodes() {
    let frame = place(&[5.0, 5.0], &[5.0, 5.0], 128, 520.0).expect("finite");
    assert_eq!(frame.step, STEP_MIN);
    assert_eq!(frame.cells, 3);
    assert!(place(&[f64::NAN], &[0.0], 128, 520.0).is_none());
    assert!(place(&[], &[], 128, 520.0).is_none());
}

#[test]
fn every_finite_position_lands_inside_the_occupied_cells() {
    let x: Vec<f64> = (0..200).map(|i| libm::sin(i as f64) * 12345.678).collect();
    let y: Vec<f64> = (0..200).map(|i| libm::cos(i as f64 * 0.3) * 77.0).collect();
    let frame = place(&x, &y, 256, 520.0).expect("finite");
    for (&px, &py) in x.iter().zip(&y) {
        let ((cx, cy), (fx, fy)) = stencil(&frame, (px, py)).expect("finite");
        assert!(cx + 1 < frame.cells && cy + 1 < frame.cells);
        let back = frame.origin.0 + (cx as f64 + fx) * frame.h;
        assert!((back - px).abs() < 1e-9 * px.abs().max(1.0));
        assert!((0.0..=1.0).contains(&fy));
    }
}
