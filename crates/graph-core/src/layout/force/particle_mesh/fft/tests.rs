use super::*;

fn naive(a: &[C], inverse: bool) -> Vec<C> {
    let n = a.len();
    let sign = if inverse { 1.0 } else { -1.0 };
    (0..n)
        .map(|k| {
            a.iter().enumerate().fold(C::default(), |acc, (j, &x)| {
                let angle = sign * core::f64::consts::TAU * ((j * k) % n) as f64 / n as f64;
                acc + x * C {
                    re: libm::cos(angle),
                    im: libm::sin(angle),
                }
            })
        })
        .collect()
}

fn samples(n: usize) -> Vec<C> {
    (0..n)
        .map(|i| C {
            re: libm::sin(i as f64 * 0.7) + 0.25,
            im: libm::cos(i as f64 * 1.3) - 0.5,
        })
        .collect()
}

fn max_gap(a: &[C], b: &[C]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::max((x.re - y.re).abs(), (x.im - y.im).abs()))
        .fold(0.0, f64::max)
}

#[test]
fn a_line_matches_the_naive_dft_both_ways() {
    for side in [2, 4, 8, 64, 256] {
        let plan = Plan::new(side);
        for inverse in [false, true] {
            let mut a = samples(side);
            let want = naive(&a, inverse);
            plan.line(&mut a, inverse);
            let gap = max_gap(&a, &want);
            assert!(gap < 1e-9 * side as f64, "side {side} inverse {inverse}: {gap}");
        }
    }
}

#[test]
fn forward_then_inverse_is_side_squared_times_the_input() {
    let side = 32;
    let plan = Plan::new(side);
    let input = samples(side * side);
    let (mut a, mut b) = (input.clone(), vec![C::default(); side * side]);
    plan.fft2(&mut a, &mut b, false);
    plan.fft2(&mut b, &mut a, true);
    let scale = (side * side) as f64;
    let back: Vec<C> = a
        .iter()
        .map(|c| C {
            re: c.re / scale,
            im: c.im / scale,
        })
        .collect();
    assert!(max_gap(&back, &input) < 1e-12);
}

#[test]
fn the_2d_transform_is_the_naive_one_transposed() {
    let side = 8;
    let plan = Plan::new(side);
    let input = samples(side * side);
    let (mut a, mut b) = (input.clone(), vec![C::default(); side * side]);
    plan.fft2(&mut a, &mut b, false);
    let rows: Vec<C> = input.chunks(side).flat_map(|r| naive(r, false)).collect();
    let mut columns = vec![C::default(); side * side];
    transpose(&rows, &mut columns, side);
    let want: Vec<C> = columns.chunks(side).flat_map(|r| naive(r, false)).collect();
    assert!(max_gap(&b, &want) < 1e-9);
}

#[test]
fn transpose_is_its_own_inverse_over_a_ragged_tile() {
    let side = 2 * TILE + 5;
    let input = samples(side * side);
    let (mut once, mut twice) = (vec![C::default(); side * side], vec![C::default(); side * side]);
    transpose(&input, &mut once, side);
    assert_eq!(once[side + 2], input[2 * side + 1]);
    transpose(&once, &mut twice, side);
    assert_eq!(twice, input);
}
