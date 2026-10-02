//! What comes out: each edge's drawn path, the contract's rows, and the two ink figures. The
//! only place the geometry is rounded to the contract's `f32`.

use super::P;
use super::level::dist;
use graph_contract::geometry::Paths;

/// Every edge's drawn path, endpoints included: the two endpoints and the meeting points it
/// rode, source side outward then target side back. An edge that never merged has no hops and
/// so draws the straight segment it started as.
pub fn polygons(ends: &[(P, P)], chain: &[Vec<[P; 2]>]) -> Vec<Vec<P>> {
    ends.iter()
        .zip(chain)
        .map(|(&(a, b), hops)| {
            let mut poly = Vec::with_capacity(2 * hops.len() + 2);
            poly.push(a);
            poly.extend(hops.iter().map(|h| h[0]));
            poly.extend(hops.iter().rev().map(|h| h[1]));
            poly.push(b);
            poly
        })
        .collect()
}

/// The contract's `Polyline` rows: interior points only, since the endpoints are the node
/// geometry's. This is the rounding to `f32`, once, on the way out.
pub fn paths(polys: &[Vec<P>]) -> Paths {
    let mut offsets = Vec::with_capacity(polys.len() + 1);
    let mut pts = Vec::new();
    offsets.push(0);
    for poly in polys {
        for p in &poly[1..poly.len() - 1] {
            pts.push(p[0] as f32);
            pts.push(p[1] as f32);
        }
        // Offsets count points, `pts` two coordinates each: the contract's own shape.
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

/// The ink actually drawn, deduplicated: every member of a bundle carries the shared trunk in
/// its own path, so summing per-edge lengths counts it once per member and reports that
/// bundling made the drawing longer. Exact equality, not a tolerance: a shared trunk is the
/// same `f64` in every member's path.
///
/// Measured on the `f64` polygons, before [`paths`] rounds them to the wire's `f32`. Two
/// segments a few `f64` ulps apart are two here and may be one drawn segment, so the figure
/// can exceed what the `f32` rows draw by such a segment's length; relative error ~2⁻²⁴.
pub fn drawn_ink(polys: &[Vec<P>]) -> f64 {
    let mut segs: Vec<[f64; 4]> = polys
        .iter()
        .flat_map(|poly| poly.windows(2).map(|w| canonical(w[0], w[1])))
        .collect();
    // `f64` is not `Ord`, so the key is its total order: the signed zeroes and NaN each sort
    // to a fixed place, which is what makes the deduplication itself total (D5).
    segs.sort_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then(a[1].total_cmp(&b[1]))
            .then(a[2].total_cmp(&b[2]))
            .then(a[3].total_cmp(&b[3]))
    });
    // Deduplicated on bits, not on `==`: `0.0` and `-0.0` compare equal and would merge two
    // segments that are not the same bits.
    segs.dedup_by(|a, b| {
        a[0].to_bits() == b[0].to_bits()
            && a[1].to_bits() == b[1].to_bits()
            && a[2].to_bits() == b[2].to_bits()
            && a[3].to_bits() == b[3].to_bits()
    });
    let mut sum = 0.0;
    for s in &segs {
        sum += dist([s[0], s[1]], [s[2], s[3]]);
    }
    sum
}

/// A segment with its ends in lexicographic order, so the two directions of one drawn segment
/// are one segment however many edges drew it.
fn canonical(a: P, b: P) -> [f64; 4] {
    if (a[0], a[1]) <= (b[0], b[1]) {
        [a[0], a[1], b[0], b[1]]
    } else {
        [b[0], b[1], a[0], a[1]]
    }
}
