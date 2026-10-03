//! The layout against measured answers.
//!
//! **Every constant here is measured, not fitted.** The port was written from
//! `SciGraphs/core/scigraphs_core/mesh/layouts/{simulation,forceatlas}.py` and the numpy
//! 2.3.3 sources under `$GM_SCRATCH/refs/numpy-2.3.3`; the vectors were then pasted out of
//! `ge-python-oracle` (`default_rng`, `ForceSim`) and out of the conformance arm's
//! `ref/FORCEATLAS2.f64`. `docs/measurements/sg-fa2-forcesim.md` records the derivation and
//! the measurement.
//!
//! **The `f32` position vectors come from the transliteration, not from
//! `ref/FORCEATLAS2.f64`,** because this port uses fixed-order reductions where the
//! reference uses BLAS and cannot land on the same bytes. The three constants below — `k`,
//! `repulsion`, `gravity` — are *exactly* the reference's (`sim.k`, `sim.repulsion`,
//! `sim.gravity` compared equal in the oracle), and the distance from the reference's
//! finished layout is a median mean absolute `1.0e-6` with a worst coordinate of `6.2e-4`.

use super::*;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};

/// A topology of `n` nodes labelled `n0..n{n}` and the given index pairs — what
/// `emit-conformance-fixtures` writes and both conformance arms read.
fn graph(n: usize, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .map(|&(a, b)| edge(&format!("e{a}_{b}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// `(x, y, z)` column bits, in node order, for the default parameters.
fn columns(n: usize, pairs: &[(u32, u32)]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let params = Fa2ForceSimParams::default();
    let layout = ForceAtlas2ForceSim::run(&graph(n, pairs), &params).expect("finite");
    let (x, y) = match &layout.nodes {
        NodeGeometry::Point { x, y } => (x.clone(), y.clone()),
        other => panic!("expected points, got {other:?}"),
    };
    (bits(&x), bits(&y), bits(layout.z.as_deref().expect("a z column")))
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

fn flat(values: &[u32]) -> Vec<u32> {
    values.to_vec()
}

/// `dag-diamond`: 4 nodes, the diamond `(0,1) (0,2) (1,3) (2,3)`, the smallest fixture with
/// a real force field.
const DIAMOND: &[(u32, u32)] = &[(0, 1), (0, 2), (1, 3), (2, 3)];

/// The finished `dag-diamond` layout, x column, `f32` bits.
const DIAMOND_X: [u32; 4] = [0x4082_FC14, 0xBD9B_F065, 0x3D9B_F04D, 0xC020_A903];
/// …y column.
const DIAMOND_Y: [u32; 4] = [0xBFD4_B204, 0x40A0_0000, 0xC0A0_0000, 0xC082_FC14];
/// …z column.
const DIAMOND_Z: [u32; 4] = [0x405B_C46D, 0x4020_A903, 0xC05B_C46E, 0x3FD4_B204];

#[test]
fn the_diamond_layout_is_the_measured_one_in_all_three_columns() {
    let (x, y, z) = columns(4, DIAMOND);
    assert_eq!(x, flat(&DIAMOND_X));
    assert_eq!(y, flat(&DIAMOND_Y));
    assert_eq!(z, flat(&DIAMOND_Z));
}

/// `tree-balanced`: 15 nodes, 15 edges — enough for `coeff.sum(axis=1)` to be pairwise and
/// for the `bincount` accumulator to see a node with several edges.
const TREE_X: [u32; 15] = [
    0xBF31_B6C9, 0xC00A_F28C, 0xC03C_E39C, 0xC002_5F77, 0xBEDC_AE74, 0xC082_6F7E, 0xC096_6DAB,
    0xC0A0_0000, 0xBFC3_324D, 0xBE18_BCB2, 0xBF18_2E5D, 0xBFAF_023C, 0x4028_C1E9, 0x4079_E8CC,
    0xC060_54AF,
];

/// `tree-balanced`'s edge list, in the fixture's own order. `simple_graph` reorders it to
/// `lo < hi` per pair, which `_attraction` is indifferent to.
const TREE: &[(u32, u32)] = &[
    (14, 7),
    (11, 7),
    (7, 8),
    (14, 0),
    (0, 1),
    (4, 0),
    (1, 3),
    (1, 2),
    (4, 5),
    (6, 4),
    (8, 9),
    (8, 10),
    (3, 9),
    (11, 12),
    (13, 11),
];

#[test]
fn the_tree_layout_x_column_is_the_measured_one() {
    let (x, _, _) = columns(15, TREE);
    assert_eq!(x, flat(&TREE_X));
}

/// `gate-04`: 6 nodes and 6 edges after `simple_graph` collapses the fixture's 9 raw edges
/// — two of them are the same pair and two are self-loops, so this is the fixture that
/// says the deduplication matches `G.edges()`.
const GATE04_X: [u32; 6] = [
    0xBBC1_C7FB, 0x3E18_749E, 0xBF4E_0B13, 0xBF00_F81B, 0x3F33_F82B, 0xC04E_C319,
];

#[test]
fn the_gate_layout_x_column_is_the_measured_one() {
    let (x, _, _) = columns(6, &[(1, 0), (1, 0), (2, 1), (2, 0), (3, 0), (4, 0)]);
    assert_eq!(x, flat(&GATE04_X));
}

#[test]
fn the_layout_is_bit_identical_twice_over() {
    let once = columns(4, DIAMOND);
    let twice = columns(4, DIAMOND);
    assert_eq!(once, twice);
}

/// The negative control. Without it, a layout that ignored `params.seed` and hard-coded the
/// default would satisfy every vector above.
#[test]
fn a_neighbouring_seed_moves_every_coordinate() {
    let params = Fa2ForceSimParams {
        seed: 1_767_573_728,
        ..Fa2ForceSimParams::default()
    };
    let moved = ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &params).expect("finite");
    let baseline = ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &Fa2ForceSimParams::default())
        .expect("finite");
    assert_ne!(moved.nodes, baseline.nodes);
}

/// `iterations = 0` still runs one step — `max(1, int(iterations))` is the reference's own
/// floor (`simulation.py:1069`), so zero is not "no work".
#[test]
fn zero_iterations_still_runs_the_references_one_step_floor() {
    let none = Fa2ForceSimParams {
        iterations: 0,
        ..Fa2ForceSimParams::default()
    };
    let one = Fa2ForceSimParams {
        iterations: 1,
        ..Fa2ForceSimParams::default()
    };
    assert_eq!(
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &none)
            .expect("finite")
            .nodes,
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &one)
            .expect("finite")
            .nodes
    );
    assert_ne!(
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &none)
            .expect("finite")
            .nodes,
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &Fa2ForceSimParams::default())
            .expect("finite")
            .nodes
    );
}

/// Fewer than two nodes: `step` breaks out at once, so the answer is the seeded start,
/// rescaled. The geometry must still be three columns wide.
#[test]
fn a_graph_too_small_to_step_still_returns_three_columns() {
    for n in [0usize, 1] {
        let params = Fa2ForceSimParams::default();
        let layout = ForceAtlas2ForceSim::run(&graph(n, &[]), &params).expect("finite");
        let (x, y) = match &layout.nodes {
            NodeGeometry::Point { x, y } => (x.len(), y.len()),
            other => panic!("expected points, got {other:?}"),
        };
        assert_eq!((x, y), (n, n));
        assert_eq!(layout.z.as_deref().map(<[f32]>::len), Some(n));
    }
}

/// `scale` reaches `_fa2_rescale` and nothing else: it changes the width of the answer and
/// not its shape. `self.k` comes from `_FA2_SIM_SCALE`, a constant.
#[test]
fn the_output_scale_resizes_without_rearranging() {
    let params = Fa2ForceSimParams {
        scale: 50.0,
        ..Fa2ForceSimParams::default()
    };
    let x = {
        let layout = ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &params).expect("finite");
        let NodeGeometry::Point { x, .. } = &layout.nodes else {
            panic!("expected points")
        };
        bits(x)
    };
    let (small, _, _) = columns(4, DIAMOND);
    assert_eq!(x.len(), small.len());
    for (wide, narrow) in x.iter().zip(&small) {
        let ratio = f64::from(f32::from_bits(*wide)) / f64::from(f32::from_bits(*narrow));
        assert!((ratio - 10.0).abs() < 1e-5, "ratio {ratio}");
    }
}

#[test]
fn the_defaults_are_the_dispatcher_s_and_the_reference_s_own_seed() {
    let p = Fa2ForceSimParams::default();
    assert_eq!(p.iterations, 50);
    assert_eq!(p.scale, 5.0);
    assert_eq!(p.scaling_ratio, 2.0);
    assert_eq!(p.gravity, 1.0);
    assert_eq!(p.jitter_tolerance, 1.0);
    assert_eq!(p.seed, 1_767_573_729);
    assert_eq!(ForceAtlas2ForceSim::ID, "layout.forceatlas2.forcesim");
}
