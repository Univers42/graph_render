//! The `n = 0` divergence and the shape properties that follow from it.
//!
//! **Split out of [`structure`](super::structure) for the house line cap**, and because these
//! two tests are the only place in the tree where this layout knowingly disagrees with its
//! reference: at `n = 0` SciGraphs hands back a `(1, 3)` array for a graph with no nodes and
//! `apply_graph_layout` then returns `False`, while this port returns an empty 3D geometry.
//! Everything here is either that disagreement or a property of the drawing's extent.

use crate::layout::basic_3d::spiral;
use crate::layout::basic_3d::tests::{bare, space};

/// `n = 0` is the one input where this port and the reference **disagree on purpose**, and
/// this is the test that holds the port's side of that bargain.
///
/// The reference returns shape `(1, 3)` for a zero-node graph — `basic.py:52`'s guard is
/// `if num_nodes > 1`, so `n = 0` takes the `n = 1` branch and gets the same single point —
/// and `apply_graph_layout` then rejects it in `_check_positions`
/// (`layouts/common.py:175-185`), returning `False`. Measured in `ge-python-oracle`:
///
/// ```text
/// >>> _spiral_layout_3d(0, 5.0).shape
/// (1, 3)
/// >>> apply_graph_layout(None, 'SPIRAL_3D', iterations=50, scale=5.0)
/// False
/// ```
///
/// This port returns three empty columns and a 3D geometry. Asserted rather than left
/// undocumented: it is the only place the layout knowingly differs from the reference, and
/// `basic_3d`'s contract is that all four of its layouts are total.
#[test]
fn the_empty_graph_is_an_empty_three_d_geometry_where_the_reference_refuses() {
    let (x, y, z) = space(&spiral(&bare(0)).expect("runs"));
    assert!(
        x.is_empty() && y.is_empty() && z.is_empty(),
        "n=0: three empty columns"
    );
    assert_eq!(super::super::columns(0).0.len(), 0, "no f64 column either");
}

/// `n = 1` and `n = 0` must NOT coincide, which is the whole of the divergence: the reference
/// hands both the same point, and this port does not.
///
/// If a future change ever made `n = 0` return the `n = 1` point, this fails — which is the
/// intended direction, since the port's contract is totality rather than fidelity at an input
/// the reference itself rejects.
#[test]
fn one_node_and_no_nodes_are_not_the_same_drawing() {
    let (one, _, _) = space(&spiral(&bare(1)).expect("runs"));
    let (none, _, _) = space(&spiral(&bare(0)).expect("runs"));
    assert_eq!(one.len(), 1, "n=1 is the single reference point");
    assert!(
        none.is_empty(),
        "n=0 is empty here and refused in the reference"
    );
}

/// Every coordinate is inside the reference's own extent: `r <= scale` and
/// `-scale <= z <= scale`, for every node at every size tried.
#[test]
fn every_node_is_inside_the_cone_the_reference_draws() {
    for n in [1u32, 2, 7, 77, 256] {
        let (x, y, z) = super::super::columns(n);
        for i in 0..n as usize {
            let r = libm::sqrt(x[i] * x[i] + y[i] * y[i]);
            assert!(r <= 5.0 + 1e-12, "n={n} node {i}: radius {r}");
            assert!(
                z[i] >= -5.0 - 1e-12 && z[i] <= 5.0 + 1e-12,
                "n={n} node {i}: z {}",
                z[i]
            );
        }
    }
}

/// Two nodes never coincide, at any size tried: the spiral climbs as it turns, so `z` is
/// strictly increasing over `t` and a repeated point would mean the arc-length inversion
/// had collapsed.
#[test]
fn no_two_nodes_coincide() {
    for n in [2u32, 3, 7, 77, 512] {
        let (x, y, z) = super::super::columns(n);
        for i in 0..n as usize {
            for j in i + 1..n as usize {
                let apart = (x[i] - x[j]).abs() + (y[i] - y[j]).abs() + (z[i] - z[j]).abs();
                assert!(apart > 0.0, "n={n}: nodes {i} and {j} coincide");
            }
        }
    }
}

/// The layout is published under the id the ledger, the wasm exports and the conformance
/// row all use, spelled out so a rename breaks here first.
#[test]
fn the_layout_is_named_by_its_module() {
    assert_eq!(spiral::ID, "layout.basic3d.spiral", "SciGraphs SPIRAL_3D");
    assert!(spiral::ID.starts_with("layout."), "no layout. prefix");
}
