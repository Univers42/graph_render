//! The packing lattice and the `dims`-dependent geometry, tested as the units they are —
//! `spectral.rs` keeps the per-component solve and the C12 rule.

use super::*;
use graph_contract::snapshot::Dim;

/// Components of the given sizes, each a run of its own dense indices.
fn components(sizes: &[usize]) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut next = 0u32;
    for &size in sizes {
        out.push((next..next + size as u32).collect());
        next += size as u32;
    }
    out
}

#[test]
fn the_lattice_side_is_the_dims_th_root_of_the_component_count() {
    // 2D keeps the historical ceil(sqrt(n)); 3D is the reference's ceil(cbrt(n))
    // (networkx_layouts.py:226).
    assert_eq!(lattice_side(4.0, 2), 2);
    assert_eq!(lattice_side(5.0, 2), 3);
    assert_eq!(lattice_side(8.0, 2), 3);
    assert_eq!(lattice_side(9.0, 2), 3);
    assert_eq!(lattice_side(8.0, 3), 2);
    assert_eq!(lattice_side(27.0, 3), 3);
    assert_eq!(lattice_side(28.0, 3), 4);
    assert_eq!(lattice_side(1.0, 2), 1);
    assert_eq!(lattice_side(1.0, 3), 1);
}

#[test]
fn the_3d_cell_is_cubic_and_the_2d_cell_is_the_flat_one_it_was() {
    // networkx_layouts.py:233-234 -- the middle term's `% side` is what makes it a lattice
    // and not a skew.
    assert_eq!(cell_of(0, 2, 3), [0.0, 0.0, 0.0]);
    assert_eq!(cell_of(1, 2, 3), [1.0, 0.0, 0.0]);
    assert_eq!(cell_of(2, 2, 3), [0.0, 1.0, 0.0]);
    assert_eq!(cell_of(3, 2, 3), [1.0, 1.0, 0.0]);
    assert_eq!(cell_of(4, 2, 3), [0.0, 0.0, 1.0]);
    // 2D: (slot % side, slot / side), the arm that was already there.
    assert_eq!(&cell_of(0, 2, 2)[..2], &[0.0, 0.0]);
    assert_eq!(&cell_of(1, 2, 2)[..2], &[1.0, 0.0]);
    assert_eq!(&cell_of(2, 2, 2)[..2], &[0.0, 1.0]);
    assert_eq!(&cell_of(3, 2, 2)[..2], &[1.0, 1.0]);
}

#[test]
fn a_block_scales_by_its_own_dims_th_root_of_the_size_ratio() {
    // networkx_layouts.py:232 -- a cube root at 3, because a 3D cell holds a block scaled
    // in three axes. A square root here would shrink big blocks wrongly.
    assert!((block_scale(1.0, 2) - 1.0).abs() < 1e-12);
    assert!((block_scale(0.25, 2) - 0.5).abs() < 1e-12);
    assert!((block_scale(0.125, 3) - 0.5).abs() < 1e-12);
}

#[test]
fn packing_separates_components_along_every_axis_it_has() {
    // The property the 2D-adapted lattice would have failed in 3D: with 8 components the
    // 2D rule puts slots 0..3 in row 0, so components 0 and 4 would share a cell's x and y
    // and differ only in z. Every component must land on its own lattice point.
    let comps = components(&[2; 8]);
    let mut coords = vec![0.0_f64; 16 * 3];
    for (i, c) in comps.iter().enumerate() {
        for &g in c {
            coords[g as usize * 3] = i as f64 * 0.01;
        }
    }
    pack_components(&mut coords, &comps, 3);
    let cells: Vec<(f64, f64)> = comps
        .iter()
        .map(|c| {
            let base = c[0] as usize * 3;
            (coords[base], coords[base + 1])
        })
        .collect();
    for (i, a) in cells.iter().enumerate() {
        for (j, b) in cells.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "components {i} and {j} share a cell");
            }
        }
    }
}

#[test]
fn one_component_is_left_where_it_is_at_both_dimensions() {
    // `pack_components` returns early under two components (networkx_layouts.py:221-222).
    let comps = components(&[4]);
    let mut coords: Vec<f64> = (0..12).map(|i| i as f64 * 0.5).collect();
    let before = coords.clone();
    pack_components(&mut coords, &comps, 3);
    assert_eq!(coords, before, "a lone component is not moved");
}

#[test]
fn to_geometry_labels_a_three_d_snapshot_and_leaves_a_2d_one_flat() {
    let coords = [0.1_f64, 0.2, 0.3, 0.4, 0.5, 0.6];
    let flat = to_geometry(&coords, 2, 2);
    assert_eq!(flat.dim(), Dim::D2);
    assert_eq!(flat.z, None);
    let spaced = to_geometry(&coords, 2, 3);
    assert_eq!(spaced.dim(), Dim::D3);
    assert_eq!(spaced.z, Some(vec![0.3_f32, 0.6_f32]));
    // x and y are read with the same stride either way, so the 2D columns are the first two
    // of the 3D ones -- the same arithmetic, not a re-derivation.
    let NodeGeometry::Point { x, y } = &spaced.nodes else {
        panic!("point nodes");
    };
    assert_eq!((x[0], y[0]), (0.1, 0.2));
    assert_eq!((x[1], y[1]), (0.4, 0.5));
}
