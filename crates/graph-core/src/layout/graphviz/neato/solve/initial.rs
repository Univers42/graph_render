//! The placement the stress iteration starts from: `initLayout` (`stress.c:131-160`).

use crate::layout::graphviz::neato::matrix::centre_f64;
use crate::layout::graphviz::neato::rng::Drand48;

/// `initLayout` (`stress.c:131-160`): two `drand48` draws per node, then each column
/// centred in `double`.
///
/// The centring is `double` here and `float` inside the iteration, in that order, because
/// the reference centres before it narrows (`stress.c:897` narrows, `:907` is the last
/// `orthog1` before it). Centring after the narrowing would be a different drawing.
pub(super) fn initial_placement(count: usize, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut rng = Drand48::seeded(seed);
    let (mut x, mut y) = (Vec::with_capacity(count), Vec::with_capacity(count));
    for _ in 0..count {
        x.push(rng.next());
        y.push(rng.next());
    }
    centre_f64(&mut x);
    centre_f64(&mut y);
    (x, y)
}
