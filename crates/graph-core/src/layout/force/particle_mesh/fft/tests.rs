use super::*;
use crate::exec::Serial;

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
            assert!(
                gap < 1e-9 * side as f64,
                "side {side} inverse {inverse}: {gap}"
            );
        }
    }
}

/// The 2D transform as it was before the passes: rows in place, transpose, rows. The
/// passes must give its bytes.
fn reference(src: &mut [C], dst: &mut [C], plan: &Plan, inverse: bool) {
    let side = plan.side();
    for row in src.chunks_exact_mut(side) {
        plan.line(row, inverse);
    }
    transpose(src, dst, side);
    for row in dst.chunks_exact_mut(side) {
        plan.line(row, inverse);
    }
}

fn transpose(src: &[C], dst: &mut [C], side: usize) {
    for y in 0..side {
        for x in 0..side {
            dst[x * side + y] = src[y * side + x];
        }
    }
}

fn bits(a: &[C]) -> Vec<(u64, u64)> {
    a.iter().map(|c| (c.re.to_bits(), c.im.to_bits())).collect()
}

/// `samples`, with rows from `live` on +0, as the deposit leaves them.
fn deposited(side: usize, live: usize) -> Vec<C> {
    let mut a = samples(side * side);
    a[live * side..].fill(C::default());
    a
}

fn forward(plan: &Plan, a: &mut Vec<C>, live: usize, workers: u32) {
    let fft = Fft {
        plan,
        runner: &Serial,
        workers,
    };
    fft.forward((a, &mut Vec::new()), live);
}

#[test]
fn a_line_of_positive_zeros_stays_positive_zero() {
    let plan = Plan::new(MAX_SIDE);
    for inverse in [false, true] {
        let mut a = vec![C::default(); MAX_SIDE];
        plan.line(&mut a, inverse);
        assert!(bits(&a).iter().all(|&b| b == (0, 0)), "inverse {inverse}");
    }
}

// Workers 3 and 7 split rows across ranges, so the partial-row path is in the comparison.
#[test]
fn the_passes_are_the_reference_bit_for_bit() {
    for (side, live) in [(8, 8), (8, 3), (64, 64), (64, 37)] {
        let plan = Plan::new(side);
        let input = deposited(side, live);
        let gain = samples(side * side);
        let (mut want, mut spectrum) = (input.clone(), vec![C::default(); side * side]);
        reference(&mut want, &mut spectrum, &plan, false);
        let mut product: Vec<C> = spectrum.iter().zip(&gain).map(|(&s, &g)| s * g).collect();
        reference(&mut product, &mut want, &plan, true);
        for workers in [1, 2, 3, 7] {
            let fft = Fft {
                plan: &plan,
                runner: &Serial,
                workers,
            };
            let (mut a, mut b) = (input.clone(), Vec::new());
            fft.forward((&mut a, &mut b), live);
            assert_eq!(bits(&a), bits(&spectrum), "forward {side} {live} {workers}");
            fft.inverse((&mut a, &mut b), &gain, live);
            let kept = live * side;
            assert_eq!(
                bits(&a[..kept]),
                bits(&want[..kept]),
                "inverse {side} {live} {workers}"
            );
            assert!(bits(&a[kept..]).iter().all(|&b| b == (0, 0)));
        }
    }
}

#[test]
fn forward_then_inverse_is_side_squared_times_the_input() {
    let side = 32;
    let plan = Plan::new(side);
    let input = samples(side * side);
    let ones = vec![C { re: 1.0, im: 0.0 }; side * side];
    let fft = Fft {
        plan: &plan,
        runner: &Serial,
        workers: 3,
    };
    let (mut a, mut b) = (input.clone(), Vec::new());
    fft.forward((&mut a, &mut b), side);
    fft.inverse((&mut a, &mut b), &ones, side);
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
    let mut a = input.clone();
    forward(&plan, &mut a, side, 2);
    let rows: Vec<C> = input.chunks(side).flat_map(|r| naive(r, false)).collect();
    let mut columns = vec![C::default(); side * side];
    transpose(&rows, &mut columns, side);
    let want: Vec<C> = columns.chunks(side).flat_map(|r| naive(r, false)).collect();
    assert!(max_gap(&a, &want) < 1e-9);
}
