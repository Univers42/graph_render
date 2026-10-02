//! Liang–Barsky clipping of one segment to one axis-aligned box, as the pure numbers the
//! raster walks with: no geometry type, no cell, no state. A segment that misses the box
//! entirely is `None`; one that leaves it is the part between its two crossings, so the
//! walk after it is bounded by the box's diagonal rather than by the caller's coordinates.
//!
//! Every bound is inclusive and every division is by a non-zero `p` or skipped, so the
//! routine is total: no `NaN`, no `inf`, and the same answer on every target.

/// The part of `a`–`b` inside `[lo, hi]`, as `(t0, t1)`: the segment walked from `a` at
/// fraction `t0` to `b` at `t1`, with `t0 <= t1`. `None` when the segment misses the box.
pub fn clip(a: (f32, f32), b: (f32, f32), lo: (f32, f32), hi: (f32, f32)) -> Option<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, a.0 - lo.0),
        (dx, hi.0 - a.0),
        (-dy, a.1 - lo.1),
        (dy, hi.1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            t0 = t0.max(r);
        } else {
            t1 = t1.min(r);
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

/// The two ends of the clipped part: `at(a, b, t0)` and `at(a, b, t1)` for [`clip`]'s
/// fractions. Kept beside it so the walk interpolates exactly the way `clip` measured.
pub fn ends(a: (f32, f32), b: (f32, f32), ts: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let at = |t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    (at(ts.0), at(ts.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The box's own corner is inside it: a segment that starts and ends on the boundary is
    /// not outside, which is what keeps a diagonal of the box from being dropped.
    #[test]
    fn a_segment_touching_only_the_boundary_is_kept_whole() {
        let square = ((0.0, 0.0), (1.0, 1.0));
        assert_eq!(
            clip((0.0, 0.0), (1.0, 1.0), square.0, square.1),
            Some((0.0, 1.0))
        );
    }

    /// A segment outside a side is missed; one crossing the box is cut at both crossings.
    #[test]
    fn a_segment_outside_is_none_and_a_crossing_is_cut_at_both_ends() {
        let lo = (0.0, 0.0);
        let hi = (1.0, 1.0);
        assert_eq!(clip((2.0, 2.0), (3.0, 3.0), lo, hi), None);
        // From (-1, -1) to (2, 2) the box's near corner is at a third of the way and its far
        // corner at two thirds, not at the ends.
        assert_eq!(
            clip((-1.0, -1.0), (2.0, 2.0), lo, hi),
            Some((1.0 / 3.0, 2.0 / 3.0))
        );
        // Entering and leaving through the same side is a graze: no interior, so None.
        assert_eq!(clip((-1.0, 0.25), (-2.0, 0.25), lo, hi), None);
    }

    /// A segment parallel to an axis has `p == 0` on the other one, and the bound decides:
    /// inside it passes, outside it is missed. This is the branch that would divide by zero.
    #[test]
    fn a_parallel_segment_is_decided_by_its_offset_not_by_a_division() {
        let lo = (0.0, 0.0);
        let hi = (1.0, 1.0);
        // Up the middle of the box from y = -1 to y = 2: the interior is a third to two
        // thirds of the way, and the walk is that part, not the three units of travel.
        assert_eq!(
            clip((0.25, -1.0), (0.25, 2.0), lo, hi),
            Some((1.0 / 3.0, 2.0 / 3.0))
        );
        assert_eq!(clip((-1.0, 2.0), (2.0, 2.0), lo, hi), None);
        // A zero-length segment inside the box is kept, and marks the cell it is in.
        assert_eq!(clip((0.5, 0.5), (0.5, 0.5), lo, hi), Some((0.0, 1.0)));
    }
}
