//! The opening threshold and the centre of mass a Barnes-Hut cell is summarised by.
//! Split from `charge.rs` for the house 300-line cap (`step.rs` and `charge/tests.rs` are
//! the precedent this module already follows).

use super::Body;

/// A cell's opening threshold `w² / θ²`, `θ2` already squared by the caller.
///
/// The walk reads `open >= l`, so a `NaN` here reads as "approximate this cell as one
/// blob" — the opposite of what `θ = 0` asks for, and reachable as `0/0` on a
/// zero-width cell or `inf/inf` on a cell as wide as `|θ|`. `θ = 0` opens nothing, and
/// the squared ratio is only formed where the quotient overflowed, so the value is the
/// bit-identical one `w * w / θ2` produced everywhere it was already finite.
///
/// Ponytail: it is the squared ratio that decides, so a cell exactly at the opening angle
/// opens on `>=` and a zero-width cell at `θ = 0` descends all the way to the exact
/// terms — the `O(N²)` the caller asked for. Escape hatch: a caller that wants an
/// approximation must pass `θ > 0`.
pub(crate) fn opening_threshold(w: f64, theta: f64, theta2: f64) -> f64 {
    if theta == 0.0 {
        return f64::INFINITY;
    }
    let naive = w * w / theta2;
    if !naive.is_nan() {
        return naive;
    }
    // `inf / inf` on the way in means the ratio itself is `NaN` too, so the square root of
    // the ratio is taken as `(w / θ)²` where that is finite and the cell is left to be
    // descended; `theta` itself infinite opens nothing, which is the same answer `θ = 0`
    // gives.
    if !theta.is_finite() {
        return f64::INFINITY;
    }
    let ratio = w / theta;
    if ratio.is_finite() {
        ratio * ratio
    } else {
        f64::INFINITY
    }
}

/// The mass-weighted centre of the children in `first..skip`, in slot order.
///
/// `m` is the child's own `count` summed: `centre` is only reached for an internal cell,
/// which `insert_leaf` fills a slot of before it can become one, so `m > 0` and the
/// division is exact. `quadtree`'s
/// `every_internal_cell_has_a_child_so_the_centre_never_divides_by_zero` is what pins it.
pub(crate) fn centre(bodies: &[Body], first: u32, skip: u32) -> (f64, f64) {
    let (mut m, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let mut c = first;
    while c < skip {
        let child = &bodies[c as usize];
        let cm = f64::from(child.count);
        m += cm;
        cx += cm * child.comx;
        cy += cm * child.comy;
        c = child.skip;
    }
    (cx / m, cy / m)
}
