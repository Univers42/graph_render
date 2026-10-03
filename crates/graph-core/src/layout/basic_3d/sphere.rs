//! `_sphere_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-34`): a
//! Fibonacci sphere, ported formula for formula. Closed form, no stream, no graph.
//!
//! The construction, as the reference writes it:
//!
//! ```text
//! i     = 0, 1, ..., n-1
//! y     = 1 - 2*(i + 0.5)/n          the band midpoint, not the edge
//! r     = sqrt(1 - y*y)             the radius at that latitude
//! theta = pi*(3 - sqrt(5))*i        the golden angle
//! x     = cos(theta)*r*scale
//! z     = sin(theta)*r*scale
//! ```
//!
//! **The `+0.5` and the golden angle are both load-bearing and both are the oracle.**
//! The band midpoint rather than the band edge is what keeps the poles from over-packing
//! (`basic.py:24-26`, citing Gonzalez 2010 and Marques et al. 2013); drop the `0.5` and
//! the two extreme nodes land exactly on the poles, `sqrt(1 - 1*1) = 0`, and every other
//! node shifts by half a band. `pi*(3 - sqrt(5))` is `2*pi/phi` to the last bit the
//! `f64` holds — it is not `2*pi/1.6180339887`, and the difference is visible in node 3.
//! So the differential is on these constants and this shape, not on "roughly spherical":
//! a reader who wants to check the port can run the six lines above against
//! `harness/oracle-basic-3d.py --function sphere`.
//!
//! **Axis order is the reference's and is not interchangeable**: `np.column_stack` puts
//! `cos(theta)*r` on x, `y` on **y**, and `sin(theta)*r` on z (`basic.py:34`). Swapping
//! the two trig columns is a rotation of the drawing that looks fine and hashes
//! differently, so `the_trig_columns_are_x_and_z_in_that_order` pins it.

use super::{SCALE, in_space};
use crate::layout::Geometry;
use crate::stage::StageError;

/// `pi * (3 - sqrt(5))` (`basic.py:32`), the golden angle `2*pi/phi`.
///
/// A named function rather than a `const`, because `f64::sqrt` is not a `const fn` and a `lazy` cell or
/// a `OnceLock` would cost more than it saves for one multiply. It is computed **once per
/// run**, before the loop, and every node's `theta` is that one value times the node's
/// index — never a running sum, so node `i`'s angle cannot inherit node `i - 1`'s
/// rounding (D10).
fn golden_angle() -> f64 {
    core::f64::consts::PI * (3.0 - f64::sqrt(5.0))
}

/// The `SPHERE` capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.basic3d.sphere";

/// `_sphere_layout(n, scale)` (`basic.py:22-34`) over the node count.
pub(super) fn run(n: u32) -> Result<Geometry, StageError> {
    let (x, y, z) = columns(n);
    Ok(in_space(&x, &y, &z))
}

/// The three `f64` columns at `n`, before any narrowing, in the reference's order.
///
/// One `f64` accumulation per node and no cross-node state, so this is a legal gather
/// (D10) and could be handed to a runner unchanged; it is not threaded today, for the
/// reason `layout.random` gives (`layout/random.rs:11-18`): the layout *is* the loop, and
/// a threaded arm that recomputed nothing would print "equal" for a stage no arm computed.
///
/// **Empty is not special-cased, and the reference does not special-case it either.**
/// `n = 0` makes every column empty and the division by `n` never happens because the
/// loop body never runs — the same reason `np.arange(0)` yields nothing to divide. So the
/// empty sphere is an empty *3D* geometry, not a 2D one: the column's presence is the
/// dimension.
fn columns(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let total = f64::from(n);
    let angle = golden_angle();
    let (mut x, mut y, mut z) = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    for i in 0..n {
        let index = f64::from(i);
        // `1 - 2*(i + 0.5)/n`, the reference's own operand order: the multiply happens
        // before the divide, and hoisting the reciprocal out of the loop would divide
        // once where the reference divides per node.
        let latitude = 1.0 - 2.0 * (index + 0.5) / total;
        let radius = f64::sqrt(1.0 - latitude * latitude);
        let theta = angle * index;
        x.push(radius * libm::cos(theta) * SCALE);
        y.push(latitude * SCALE);
        z.push(radius * libm::sin(theta) * SCALE);
    }
    (x, y, z)
}

#[cfg(test)]
mod tests;
