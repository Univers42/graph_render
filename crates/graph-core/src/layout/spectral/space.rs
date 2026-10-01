//! Where solved eigenpairs become placed nodes: the peak-normalised scatter, the lattice
//! packing at `dims` dimensions, and the `f32` geometry both emit.
//!
//! Split out of `spectral.rs` to stay under the house line cap, and because these three are
//! one concern — "an `f64` coordinate buffer laid out per component, at some dimension" —
//! that the eigensolve above does not share. They are also exactly the parts
//! `layout::pivot_mds` borrows, so keeping them together is what lets that module say
//! "components, adjacency and packing are `layout::spectral`'s" and mean one file.

use super::Geometry;
use crate::linalg::EigBlock;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// `_COMPONENT_SPACING` (reference constant).
pub(super) const SPACING: f64 = 2.5;

/// Writes one solved component's (already sign-pinned, peak-normalised) coordinates
/// into the shared `coords` buffer, filling any dimension past `dims_eff` with the last
/// solved column (`n_c = 2`'s "copy the last column into y", generalised). `pub(crate)`:
/// `layout::pivot_mds` scatters its own projected coordinates the same way.
pub(crate) fn scatter(coords: &mut [f64], members: &[u32], eig: &EigBlock, dims: usize) {
    let peak = eig.vectors.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    for d in 0..dims {
        let source = d.min(eig.k - 1);
        for (li, &g) in members.iter().enumerate() {
            let value = eig.column(source)[li];
            coords[g as usize * dims + d] = if peak > 0.0 { value / peak } else { value };
        }
    }
}

/// The lattice packing, at `dims` dimensions.
///
/// **`dims = 2` is the house adaptation and stays byte-identical**: side
/// `ceil(sqrt(components))`, per-component scale `sqrt(n_c / biggest)`, a two-cell
/// coordinate (`docs/decisions/eigensolver.md:242-243`, "component packing adapted to 2D
/// as the devil describes"). **`dims = 3` is the reference's own** `_pack_component_blocks`
/// (`networkx_layouts.py:218-236`): a *cubic* lattice, `side = ceil(cbrt(components))`, and
/// a per-component scale of `(n_c / biggest)^(1/3)` — a cube root, not a square root,
/// because a 3D cell has to hold a block scaled in three axes. Reusing the 2D rule here
/// would pack 3D blocks onto a flat lattice and let components overlap along z.
///
/// Stable sort on `(-size, min_index)` in both arms, so the packing order is a function of
/// the components and not of their discovery order (D2). `pub(crate)`: shared with
/// `layout::pivot_mds`, which packs the same shape of per-component blocks.
pub(crate) fn pack_components(coords: &mut [f64], components: &[Vec<u32>], dims: usize) {
    if components.len() < 2 {
        return;
    }
    let mut order: Vec<usize> = (0..components.len()).collect();
    order.sort_by(|&a, &b| {
        components[b]
            .len()
            .cmp(&components[a].len())
            .then(components[a][0].cmp(&components[b][0]))
    });
    let biggest = components.iter().map(Vec::len).max().unwrap_or(1) as f64;
    let count = components.len() as f64;
    let side = lattice_side(count, dims);
    let offset = (side as f64 - 1.0) * SPACING * 0.5;
    let original = coords.to_vec();
    for (slot, &c) in order.iter().enumerate() {
        let ratio = components[c].len() as f64 / biggest;
        let scale = block_scale(ratio, dims);
        let cell = cell_of(slot, side, dims);
        for &g in &components[c] {
            let base = g as usize * dims;
            for (d, &axis) in cell.iter().enumerate().take(dims) {
                coords[base + d] = original[base + d] * scale + axis * SPACING - offset;
            }
        }
    }
}

/// Lattice side length: `ceil(n^(1/dims))` over the component count, at least 1.
///
/// Written as the `dims`-th root rather than as two branches so the 2D arm's
/// `sqrt(...).ceil()` and the 3D arm's `cbrt(...).ceil()` are one rule. For `dims = 2`
/// this is the exact expression the 2D arm used, so its bytes do not move.
fn lattice_side(count: f64, dims: usize) -> usize {
    let root = libm::pow(count, 1.0 / dims as f64);
    libm::ceil(root).max(1.0) as usize
}

/// A block's per-axis scale, `(n_c / biggest)^(1/dims)` — `sqrt` at 2, `cbrt` at 3
/// (`networkx_layouts.py:232`).
fn block_scale(ratio: f64, dims: usize) -> f64 {
    libm::pow(ratio, 1.0 / dims as f64)
}

/// The lattice cell of packing slot `slot`, as one coordinate per dimension.
///
/// At 2: `(slot % side, slot / side)`, the arm that was there before. At 3:
/// `(slot % side, (slot / side) % side, slot / side^2)`, the reference's
/// `networkx_layouts.py:233-234` — the `% side` on the middle term is what makes it a
/// lattice rather than a skew.
fn cell_of(slot: usize, side: usize, dims: usize) -> [f64; 3] {
    let index = slot as f64;
    let width = side as f64;
    let mut cell = [0.0; 3];
    for (d, axis) in cell.iter_mut().enumerate().take(dims) {
        // Integer division by the running stride, one axis per dimension: at 2 this is
        // `(slot % side, slot / side)` and at 3 `(slot % side, (slot / side) % side,
        // slot / side^2)`, which is the reference's own `cell`
        // (`networkx_layouts.py:233-234`) written once.
        let mut stride = 1.0_f64;
        for _ in 0..d {
            stride *= width;
        }
        *axis = libm::floor(index / stride) % width;
    }
    cell
}

/// The packed `f64` buffer as the layout's geometry, narrowed to `f32`.
///
/// `pub(crate)`: `layout::pivot_mds` packs its own buffer into the same geometry.
///
/// The z column exists only at `dims = 3`, and its presence is what labels the snapshot
/// 0.4 rather than 0.3 (`Geometry::in_space`), so this is the one place the two arms part.
pub(crate) fn to_geometry(coords: &[f64], n: usize, dims: usize) -> Geometry {
    let column = |d: usize| {
        (0..n)
            .map(|i| coords[i * dims + d] as f32)
            .collect::<Vec<f32>>()
    };
    let nodes = NodeGeometry::Point {
        x: column(0),
        y: column(1),
    };
    if dims >= 3 {
        Geometry::in_space(nodes, EdgeGeometry::Line, Vec::new(), column(2))
    } else {
        Geometry::planar(nodes, EdgeGeometry::Line, Vec::new())
    }
}

#[cfg(test)]
mod tests;
