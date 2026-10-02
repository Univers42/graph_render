//! The opening temperature: the one number a start position fixes before the first gather,
//! and the one place `layout.py` reads two columns and not `dim` of them.

use super::{Field, opening};

/// Three columns whose widest span is the **third** one: the case that separates the
/// reference's rule from "the widest of the columns the port has".
///
/// `layout.py:687` (dense) and `layout.py:776` (sparse) both read exactly two spans out of
/// `pos` — `pos.T[0]` and `pos.T[1]` — at every `dim`, `dim = 3` included. So a start whose
/// `z` column is 100 wide and whose `x` is 1 wide still opens at `0.1 * 1`, not at
/// `0.1 * 100`.
#[test]
fn a_three_column_start_opens_on_its_x_and_y_spans_and_not_on_its_z() {
    let mut field = Field::<3>::zeros(3);
    field.c[0] = vec![0.0, 1.0, 0.5];
    field.c[1] = vec![0.0, 0.0, 0.0];
    field.c[2] = vec![0.0, 100.0, 50.0];
    assert_eq!(
        opening(&field).to_bits(),
        0.1_f64.to_bits(),
        "x span 1, y span 0, z span 100: the third column does not open the run"
    );
}

/// The same number whichever way the two read columns are swapped, and the same number again
/// when `z` moves — so the answer is `max(xspan, yspan)`, not an order-dependent accident.
#[test]
fn the_opening_temperature_is_the_larger_of_the_first_two_spans() {
    let mut field = Field::<3>::zeros(2);
    field.c[0] = vec![0.0, 2.0];
    field.c[1] = vec![7.0, 0.0];
    assert_eq!(
        opening(&field).to_bits(),
        (7.0_f64 * 0.1).to_bits(),
        "y is the wider of the two read columns"
    );
    field.c[1] = vec![0.0, 20.0];
    assert_eq!(
        opening(&field).to_bits(),
        (20.0_f64 * 0.1).to_bits(),
        "and the answer follows whichever column is wider"
    );
    field.c[2] = vec![-1000.0, 1000.0];
    assert_eq!(
        opening(&field).to_bits(),
        (20.0_f64 * 0.1).to_bits(),
        "moving the third column moves nothing"
    );
}

/// `D = 2` reads all the columns there are, so the two-dimensional stage's opening
/// temperature is what it always was — the numbers the goldens in
/// [`super::super::tests::golden`] pin.
#[test]
fn a_two_column_start_still_opens_on_its_only_two_spans() {
    let mut field = Field::<2>::zeros(2);
    field.c[0] = vec![0.0, 4.0];
    field.c[1] = vec![1.0, 1.0];
    assert_eq!(opening(&field).to_bits(), (4.0_f64 * 0.1).to_bits());
}