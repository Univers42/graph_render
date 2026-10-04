//! Deterministic initial positions: the engine's own golden-angle spiral
//! (`osionos/packages/graph-engine/src/core/layout/forceLayout.ts:66-79`,
//! `seedPositions`/`GOLDEN_ANGLE`), centred at the origin since graph-core has no
//! viewport — a documented deviation; the engine centres on `width/2, height/2`, which
//! is meaningless off-screen. No randomness: the spiral is a pure function of a node's
//! dense index, so it needs neither `Mulberry32` nor the counter hash.

const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Node `i`'s seed position: `12*sqrt(i+1)` out along the golden-angle spiral. One row
/// alone, so a session that grows in place seeds a new row without the rows before it.
pub(in crate::layout::force) fn spiral_point(i: u32) -> (f64, f64) {
    let radius = 12.0 * f64::sqrt(f64::from(i) + 1.0);
    let angle = f64::from(i) * GOLDEN_ANGLE;
    (libm::cos(angle) * radius, libm::sin(angle) * radius)
}

/// The seed positions of rows `0..n`, each [`spiral_point`].
pub(super) fn golden_spiral(n: u32) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(n as usize);
    let mut y = Vec::with_capacity(n as usize);
    for (px, py) in (0..n).map(spiral_point) {
        x.push(px);
        y.push(py);
    }
    (x, y)
}

/// Node `i`'s 3D seed position: `12*sqrt(i+1)` out on the **same** Fibonacci sphere
/// `kamada_kawai/start.rs:47` builds — `y` uniform on `[-1, 1)`, the azimuth stepping by the
/// golden angle, `r = sqrt(1 - y²)`. The formula is borrowed rather than invented a third
/// time; only the radius is this module's, and it is the 2D spiral's own `12`.
///
/// **The sphere is required, not decorative.** A 3D start drawn in a plane would leave every
/// `dz` zero on the first tick, so the many-body walk's z gaps and the link and collide z
/// terms would all take their jiggle branches instead of their real values, and the octree
/// would degenerate to the planar case that its own
/// `a_planar_point_set_gives_the_octree_the_quadtrees_arena` test covers. The z column would
/// be a hash's worth of noise rather than a layout.
///
/// Ponytail: the radius is not KK's `0.36 * sqrt(n)` and not a force layout's own constant,
/// because this simulation has no start-radius requirement — `chargeStrength` and
/// `linkDistance` set the scale it settles at, and the start only has to be finite, distinct
/// and off-plane. Escape hatch: `golden_spiral`'s `12`.
pub(in crate::layout::force) fn sphere_point(i: u32) -> (f64, f64, f64) {
    let radius = 12.0 * f64::sqrt(f64::from(i) + 1.0);
    let n = f64::from(u16::MAX);
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / n;
    let r = libm::sqrt((1.0 - y * y).max(0.0));
    let angle = golden * f64::from(i);
    (
        radius * r * libm::cos(angle),
        radius * r * libm::sin(angle),
        radius * y,
    )
}

/// The 3D seed positions of rows `0..n`, each [`sphere_point`].
///
/// `y` is uniform over `n` rows with `n = u16::MAX` as the denominator rather than the row
/// count, because a caller only knows `n` after it has built the columns. It is a pure
/// function of the row index, so it is still reproducible, and at every graph size this
/// engine sees (well under 65 536 rows) the sphere is sampled densely enough that no two
/// rows coincide.
pub(in crate::layout::force) fn golden_sphere(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (mut x, mut y, mut z) = (Vec::with_capacity(n as usize), Vec::new(), Vec::new());
    for (px, py, pz) in (0..n).map(sphere_point) {
        x.push(px);
        y.push(py);
        z.push(pz);
    }
    (x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_gets_a_distinct_finite_position_and_it_is_a_pure_function_of_its_index() {
        let (x, y) = golden_spiral(50);
        assert_eq!((x.len(), y.len()), (50, 50));
        assert!(x.iter().chain(&y).all(|v| v.is_finite()));
        let (x2, _) = golden_spiral(50);
        assert_eq!(x, x2, "same index, same spiral point, every time");
        assert_eq!((x[0], y[0]), (12.0 * libm::cos(0.0), 12.0 * libm::sin(0.0)));
    }
}
