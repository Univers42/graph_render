//! The small shared pieces that do not belong to either packing path: the `n < 3` special
//! cases (`circle_packing.py:296-303`), centring and final scaling
//! (`circle_packing.py:365-371`, `:496-501`), and the cast into [`Geometry`].

use super::CirclePackingParams;
use crate::layout::Geometry;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::{Note, NoteCode, SNAPSHOT_WIDE};

/// One packing's result in `f64`, before the final cast to the wire's `f32`.
pub(super) struct Packed {
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    pub(super) r: Vec<f64>,
    /// Whether this came from [`super::fallback`]: flagged as note code 3.
    pub(super) approximate: bool,
}

impl Packed {
    fn empty() -> Self {
        Self {
            x: Vec::new(),
            y: Vec::new(),
            r: Vec::new(),
            approximate: false,
        }
    }
}

/// `n == 0`: nothing to place (`circle_packing.py:296-297`).
pub(super) fn empty() -> Packed {
    Packed::empty()
}

/// `n == 1`: one circle at the origin, radius half the scale (`circle_packing.py:299-300`).
pub(super) fn single(scale: f32) -> Packed {
    Packed {
        x: vec![0.0],
        y: vec![0.0],
        r: vec![f64::from(scale) * 0.5],
        approximate: false,
    }
}

/// `n == 2`: two tangent circles either side of the origin (`circle_packing.py:302-303`).
pub(super) fn pair(scale: f32) -> Packed {
    let r = f64::from(scale) * 0.25;
    Packed {
        x: vec![-r, r],
        y: vec![0.0, 0.0],
        r: vec![r, r],
        approximate: false,
    }
}

/// Centres `positions` on their own mean (`circle_packing.py:365-366`, `:496-497`).
pub(super) fn center(positions: &mut [(f64, f64)]) {
    let n = positions.len() as f64;
    if n == 0.0 {
        return;
    }
    let (sx, sy) = positions
        .iter()
        .fold((0.0, 0.0), |(ax, ay), &(x, y)| (ax + x, ay + y));
    let (mx, my) = (sx / n, sy / n);
    for p in positions.iter_mut() {
        p.0 -= mx;
        p.1 -= my;
    }
}

/// Scales `positions` and `radii` together so the packing's full extent (its furthest
/// centre coordinate plus its largest radius) becomes `0.45` of `scale`
/// (`circle_packing.py:368-371`, `:498-501`); a no-op on a packing too small to measure.
pub(super) fn fit_to_scale(positions: &mut [(f64, f64)], radii: &mut [f64], scale: f64) {
    let max_pos = positions
        .iter()
        .fold(0.0_f64, |m, &(x, y)| m.max(x.abs()).max(y.abs()));
    let max_r = radii.iter().cloned().fold(0.0_f64, f64::max);
    let extent = max_pos + max_r;
    if extent > 1e-6 {
        let factor = (scale * 0.45) / extent;
        for p in positions.iter_mut() {
            p.0 *= factor;
            p.1 *= factor;
        }
        for r in radii.iter_mut() {
            *r *= factor;
        }
    }
}

/// The exact path's finishing touch: centre, then fit to `params.scale`.
pub(super) fn finish(
    mut positions: Vec<(f64, f64)>,
    mut radii: Vec<f64>,
    params: &CirclePackingParams,
    approximate: bool,
) -> Packed {
    center(&mut positions);
    fit_to_scale(&mut positions, &mut radii, f64::from(params.scale));
    let (x, y) = positions.into_iter().unzip();
    Packed {
        x,
        y,
        r: radii,
        approximate,
    }
}

/// `packed`'s `f64` columns cast to the wire's `f32`, as [`crate::layout::Geometry`]:
/// `Circle` nodes, `Line` edges (endpoints come from the centres), and a code-3 note when
/// the packing came from the fallback.
pub(super) fn to_geometry(packed: Packed) -> Geometry {
    let cast = |v: Vec<f64>| v.into_iter().map(|f| f as f32).collect();
    let notes = if packed.approximate {
        vec![Note {
            code: NoteCode::PackingApproximate,
            index: SNAPSHOT_WIDE,
        }]
    } else {
        Vec::new()
    };
    Geometry {
        nodes: NodeGeometry::Circle {
            x: cast(packed.x),
            y: cast(packed.y),
            r: cast(packed.r),
        },
        edges: EdgeGeometry::Line,
        notes,
    }
}
