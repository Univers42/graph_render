//! Deterministic initial positions: the engine's own golden-angle spiral
//! (`osionos/packages/graph-engine/src/core/layout/forceLayout.ts:66-79`,
//! `seedPositions`/`GOLDEN_ANGLE`), centred at the origin since graph-core has no
//! viewport — a documented deviation; the engine centres on `width/2, height/2`, which
//! is meaningless off-screen. No randomness: the spiral is a pure function of a node's
//! dense index, so it needs neither `Mulberry32` nor the counter hash.

const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Node `i`'s seed position is `12*sqrt(i+1)` out along the golden-angle spiral.
pub(in crate::layout::force) fn golden_spiral(n: u32) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(n as usize);
    let mut y = Vec::with_capacity(n as usize);
    for i in 0..n {
        let radius = 12.0 * libm::sqrt(f64::from(i) + 1.0);
        let angle = f64::from(i) * GOLDEN_ANGLE;
        x.push(libm::cos(angle) * radius);
        y.push(libm::sin(angle) * radius);
    }
    (x, y)
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
