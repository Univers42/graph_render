use super::*;
use crate::exec::{Runner, Serial};

/// The one-thread loop the kernels replace, over the whole `side²` mesh.
fn reference(frame: &Frame, side: usize, order: &[u32], (x, y): (&[f64], &[f64])) -> Vec<C> {
    let mut density = vec![C::default(); side * side];
    for &i in order {
        let i = i as usize;
        let Some(((cx, cy), (fx, fy))) = frame::stencil(frame, (x[i], y[i])) else {
            continue;
        };
        let at = cy * side + cx;
        for (cell, w) in [at, at + 1, at + side, at + side + 1]
            .into_iter()
            .zip(weights(fx, fy))
        {
            density[cell].re += w;
        }
    }
    density
}

/// Crowded and sparse regions, the far corner, and two non-finite nodes.
fn positions(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x: Vec<f64> = (0..n).map(|i| libm::sin(i as f64 * 0.37) * 900.0).collect();
    let mut y: Vec<f64> = (0..n)
        .map(|i| libm::cos(i as f64 * 0.11) * libm::sin(i as f64 * 0.05) * 400.0)
        .collect();
    (x[3], y[7]) = (f64::NAN, f64::INFINITY);
    (x[n - 1], y[n - 1]) = (900.0, 400.0);
    (x, y)
}

#[test]
fn every_division_of_the_cells_is_the_one_thread_loop_bit_for_bit() {
    let n = 5000;
    let (x, y) = positions(n);
    // A stride permutation, so grid order is not node order.
    let order: Vec<u32> = (0..n as u32).map(|k| (k * 7919) % n as u32).collect();
    for side in [128, 256] {
        let frame = frame::place_over((&x, &y), side, 520.0).expect("finite");
        let want = reference(&frame, side, &order, (&x, &y));
        assert!(
            want[frame.cells * side..]
                .iter()
                .all(|c| *c == C::default())
        );
        let stencils = Stencils {
            frame: &frame,
            side,
            order: &order,
            xy: (&x, &y),
        };
        // 4096 cuts most rows into pieces, so a range starts and ends mid-row.
        for workers in [1, 2, 3, 7, 64, 4096] {
            let (mut at, mut got) = (Vec::new(), Vec::new());
            Serial.run(&stencils, workers, &mut at);
            let mut rows = Rows::new(side, n as u32);
            rows.sort(&at, side);
            let deposit = Deposit {
                stencils: &stencils,
                at: &at,
                rows: &rows,
            };
            Serial.run(&deposit, workers, &mut got);
            assert_eq!(got.len(), frame.cells * side);
            for (cell, (g, w)) in got.iter().zip(&want).enumerate() {
                let bits = |c: &C| (c.re.to_bits(), c.im.to_bits());
                assert_eq!(
                    bits(g),
                    bits(w),
                    "side {side}, workers {workers}, cell {cell}"
                );
            }
        }
    }
}

#[test]
fn rows_hold_every_finite_slot_once_ascending_in_its_row() {
    let side = 8;
    let at = [17, NONE, 3, 63, 16, 0, NONE, 18, 2];
    let mut rows = Rows::new(side, at.len() as u32);
    rows.sort(&at, side);
    let got: Vec<&[u32]> = (0..side).map(|row| rows.of(row)).collect();
    let want: [&[u32]; 8] = [&[2, 5, 8], &[], &[0, 4, 7], &[], &[], &[], &[], &[3]];
    assert_eq!(got, want);
    // A second sort over other cells reuses the buffers and leaves nothing behind.
    rows.sort(&[9, 9, NONE, 9, NONE, NONE, NONE, NONE, NONE], side);
    assert_eq!(rows.of(1), [0, 1, 3]);
    assert!(
        (0..side)
            .filter(|&row| row != 1)
            .all(|row| rows.of(row).is_empty())
    );
}
