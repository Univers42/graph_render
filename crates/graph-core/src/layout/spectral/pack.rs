//! The two component lattices and the final rescale: the last steps of every spectral-family
//! layout, and the only place a per-component block stops being its own coordinate system.
//!
//! **Two lattices, not one parameterised one.** `_pack_component_blocks`
//! (`networkx_layouts.py:218-236`) is cubic: side `ceil(k^(1/3))`, block scale
//! `(n_c / biggest)^(1/3)`, three cell axes. `layout.spectral` and `layout.mds.pivot` are
//! pinned to the 2D adaptation of it (`docs/decisions/eigensolver.md`, decision 4): side
//! `ceil(sqrt(k))`, scale `sqrt(n_c / biggest)`, two axes. Both are one primitive each here,
//! because the 2D one is a *frozen byte contract* and the 3D one is the reference — fusing
//! them would put a square-root/cube-root switch in the middle of the pinned bytes.

use super::width::Width;
use super::{COMPONENT_SPACING, DIMS_3D};

/// The exponent the reference's cube root is written with (`** (1.0 / 3.0)`, `:226` and
/// `:232`). `libm::powf` and not `cbrt`, because `pow(x, 1/3)` is what the reference's `**`
/// evaluates to and the two differ in the last bit for the perfect-cube component counts a
/// lattice size is most often derived from.
const THIRD: f64 = 1.0 / 3.0;

/// Components in descending size, ties by lowest first member.
///
/// `sorted(range(len(components)), key=lambda c: -len(components[c]))` (`:224`) is a
/// **stable** sort over `nx.connected_components` order, and each component's index array
/// is sorted ascending at `:43`, so a tie keeps ascending-first-index order. The explicit
/// `then` is that rule written down rather than inherited from the discovery order.
fn component_order(components: &[Vec<u32>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..components.len()).collect();
    order.sort_by(|&a, &b| {
        components[b]
            .len()
            .cmp(&components[a].len())
            .then(components[a][0].cmp(&components[b][0]))
    });
    order
}

/// The lattice offset `(side - 1) * 2.5 * 0.5` (`:227`), the half a cell of slack that
/// centres the block inside its lattice.
fn offset(side: usize) -> f64 {
    (side as f64 - 1.0) * COMPONENT_SPACING * 0.5
}

/// `layout.spectral`/`layout.mds.pivot`'s 2D lattice: side `ceil(sqrt(k))`, per-component
/// scale `sqrt(n_c / biggest)`, cells on two axes. Byte for byte what `spectral.rs` carried
/// before the 3D arm existed — the reason it is its own function and not a `match`.
pub(crate) fn pack_components(coords: &mut [f64], components: &[Vec<u32>], width: Width) {
    debug_assert!(!width.in_space(), "the 2D lattice is the 2D arm's own");
    if components.len() < 2 {
        return;
    }
    let biggest = components.iter().map(Vec::len).max().unwrap_or(1) as f64;
    let side = (components.len() as f64).sqrt().ceil().max(1.0) as usize;
    let offset = offset(side);
    let original = coords.to_vec();
    for (slot, &c) in component_order(components).iter().enumerate() {
        let scale = (components[c].len() as f64 / biggest).sqrt();
        let cell = [f64::from((slot % side) as u32), f64::from((slot / side) as u32)];
        let dims = width.dims();
        for &g in &components[c] {
            let base = g as usize * dims;
            for d in 0..dims {
                coords[base + d] = original[base + d] * scale + cell[d] * COMPONENT_SPACING - offset;
            }
        }
    }
}

/// `_pack_component_blocks` (`networkx_layouts.py:218-236`), the reference's own cubic
/// lattice: side `ceil(k^(1/3))`, block scale `(n_c / biggest)^(1/3)`, cell
/// `(slot % side, (slot // side) % side, slot // (side * side))` with x fastest. `coords`
/// carries `dims` per node, so this is also the shape `_pack_component_blocks` sees when it
/// is handed a 2D block — the reference packs components the same way in both cases.
pub(crate) fn pack_component_blocks_3d(coords: &mut [f64], components: &[Vec<u32>]) {
    if components.len() < 2 {
        return;
    }
    let biggest = components.iter().map(Vec::len).max().unwrap_or(1) as f64;
    let side = libm::powf(components.len() as f64, THIRD).ceil().max(1.0) as usize;
    let offset = offset(side);
    let square = side * side;
    let original = coords.to_vec();
    for (slot, &c) in component_order(components).iter().enumerate() {
        let scale = libm::powf(components[c].len() as f64 / biggest, THIRD);
        let cell = [
            (slot % side) as f64,
            ((slot / side) % side) as f64,
            (slot / square) as f64,
        ];
        for &g in &components[c] {
            let base = g as usize * DIMS_3D;
            for d in 0..DIMS_3D {
                coords[base + d] = original[base + d] * scale + cell[d] * COMPONENT_SPACING - offset;
            }
        }
    }
}

/// `_rescale_positions` (`:238-247`): subtract the per-axis mean, divide by the largest
/// absolute coordinate left, multiply by `scale`. Also igraph's `fit_positions` formula,
/// which is why it lives here as one primitive rather than inside either 3D layout.
///
/// **Two documented differences from the reference, neither of which moves a coordinate by
/// more than the last bit.** `mean(axis=0)` is numpy's pairwise sum; this is a fixed-order
/// ascending sum (D5), which differs in the low bits of a mean. And `peak` is taken over
/// `coords` *after* centring, as the reference does, so `peak > 0` and the divide always run
/// unless the whole drawing is one point.
pub(crate) fn rescale_to_scale(coords: &mut [f64], dims: usize, scale: f64) {
    let n = coords.len() / dims;
    if n == 0 {
        return;
    }
    for d in 0..dims {
        let mut sum = 0.0_f64;
        for value in coords.chunks_exact(dims) {
            sum += value[d];
        }
        let mean = sum / n as f64;
        for value in coords.chunks_exact_mut(dims) {
            value[d] -= mean;
        }
    }
    let peak = coords.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if peak > 0.0 {
        for value in coords.iter_mut() {
            *value = *value / peak * scale;
        }
    } else {
        for value in coords.iter_mut() {
            *value *= scale;
        }
    }
}