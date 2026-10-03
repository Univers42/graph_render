//! The layout against measured answers.
//!
//! **Every constant here is measured, not fitted.** The port was written from
//! `SciGraphs/core/scigraphs_core/mesh/layouts/{simulation,forceatlas}.py` and the numpy
//! 2.3.3 sources under `$GM_SCRATCH/refs/numpy-2.3.3`; the vectors were then pasted out of
//! `ge-python-oracle` (`default_rng`, `ForceSim`). `docs/measurements/sg-fa2-forcesim.md`
//! records the derivation, the probes and the numbers.
//!
//! **The vectors come from the transliteration, not from the conformance arm's
//! `ref/FORCEATLAS2.f64`,** because this port uses fixed-order reductions where the
//! reference uses BLAS and cannot land on the same bytes. What *is* exact against the
//! reference is everything upstream of them: `k`, `repulsion` and `gravity` compare equal
//! to `sim.k`, `sim.repulsion` and `sim.gravity` in the oracle, and so does the start
//! (`state::Sim` prints them). The distance from the reference's finished layout is a median
//! mean absolute `1.0e-6` over the 24 fixtures with a worst coordinate of `6.2e-4`.

use super::*;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};

/// A topology of `n` nodes labelled `n{i:02}` and the given index pairs — what
/// `emit-conformance-fixtures` writes and both conformance arms read.
///
/// Zero-padded labels on purpose: `index_model` admits nodes in the order given, and
/// `n10` sorts before `n2` in byte order, so an unpadded label would quietly reorder a
/// fifteen-node fixture against the reference.
fn graph(n: usize, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i:02}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| {
            edge(&format!("e{i:03}"), &format!("n{a:02}"), &format!("n{b:02}"))
        })
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
    (
        bits(&x),
        bits(&y),
        bits(layout.z.as_deref().expect("a z column")),
    )
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// `dag-diamond`: 4 nodes, the diamond `(0,1) (0,2) (1,3) (2,3)` — the smallest fixture
/// with a real force field.
const DIAMOND: &[(u32, u32)] = &[(0, 1), (0, 2), (1, 3), (2, 3)];

/// …x column, `f32` bits.
const DIAMOND_X: [u32; 4] = [0x4082_FC14, 0xBD9B_F065, 0x3D9B_F04D, 0xC082_FC14];
/// …y column. The widest axis of the finished layout, so `±5.0` exactly.
const DIAMOND_Y: [u32; 4] = [0xBFD4_B204, 0x40A0_0000, 0xC0A0_0000, 0x3FD4_B204];
/// …z column.
const DIAMOND_Z: [u32; 4] = [0x405B_C46D, 0x4020_A903, 0xC020_A903, 0xC05B_C46E];

#[test]
fn the_diamond_layout_is_the_measured_one_in_all_three_columns() {
    let (x, y, z) = columns(4, DIAMOND);
    assert_eq!(x, DIAMOND_X.to_vec());
    assert_eq!(y, DIAMOND_Y.to_vec());
    assert_eq!(z, DIAMOND_Z.to_vec());
}

/// `tree-balanced`'s edge list, in the fixture's own order. `simple_graph` reorders each
/// pair to `lo < hi`, which `_attraction` is indifferent to.
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

const TREE_X: [u32; 15] = [
    0xBF31_B6C9, 0xC00A_F28C, 0xC03C_E39C, 0xC002_5F77, 0xBEDC_AE74, 0xBB47_04E5, 0xBF2B_75BB,
    0x3F89_7283, 0xBE18_BCE2, 0xBFAF_023C, 0xBDC6_76AF, 0x402E_25C4, 0x4068_8EA7, 0x4037_1CAC,
    0x3E8F_F4C8,
];

#[test]
fn the_tree_layout_x_column_is_the_measured_one() {
    let (x, _, _) = columns(15, TREE);
    assert_eq!(x, TREE_X.to_vec());
}

/// `gate-04`'s **raw** edge list: nine edges over six nodes, of which two are the same pair
/// and one is a self-loop, so six simple edges survive. This is the fixture that says the
/// deduplication matches `G.edges()` — the reference builds a `networkx.Graph`, which
/// collapses parallel edges and keeps self-loops for `_as_edges` to drop
/// (`simulation.py:107`), and `simple_graph` collapses both.
const GATE04_RAW: &[(u32, u32)] = &[
    (1, 0),
    (1, 0), // parallel
    (2, 1),
    (2, 0),
    (3, 0),
    (4, 0),
    (4, 0), // parallel
    (5, 1),
    (5, 1), // parallel
];

const GATE04_X: [u32; 6] = [
    0xBBC1_C7FB,
    0x3E18_749E,
    0xBF4E_0B13,
    0xBF00_F81B,
    0x3F33_F82B,
    0x3EEC_E2D6,
];

#[test]
fn the_gate_layout_x_column_is_the_measured_one() {
    let (x, _, _) = columns(6, GATE04_RAW);
    assert_eq!(x, GATE04_X.to_vec());
}

/// Parallel edges and self-loops must not reach the kernel twice: the same graph written
/// with them and without them has to lay out identically, because `simple_graph` collapses
/// them before the port sees them.
#[test]
fn parallel_edges_and_self_loops_do_not_change_the_layout() {
    let plain: Vec<(u32, u32)> = vec![(0, 1), (1, 2), (2, 0)];
    let noisy: Vec<(u32, u32)> = vec![
        (0, 1),
        (1, 0),
        (1, 2),
        (2, 2), // self-loop
        (2, 0),
        (1, 2),
    ];
    assert_eq!(columns(3, &plain), columns(3, &noisy));
}

#[test]
fn the_layout_is_bit_identical_twice_over() {
    assert_eq!(columns(4, DIAMOND), columns(4, DIAMOND));
}

/// The negative control. Without it, a layout that ignored `params.seed` and hard-coded the
/// default would satisfy every vector above.
#[test]
fn a_neighbouring_seed_moves_every_coordinate() {
    let moved = Fa2ForceSimParams {
        seed: 1_767_573_728,
        ..Fa2ForceSimParams::default()
    };
    assert_ne!(
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &moved)
            .expect("finite")
            .nodes,
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &Fa2ForceSimParams::default())
            .expect("finite")
            .nodes
    );
}

/// `iterations = 0` still runs one step: `max(1, int(iterations))` is the reference's own
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
    let at = |p: &Fa2ForceSimParams| {
        ForceAtlas2ForceSim::run(&graph(4, DIAMOND), p)
            .expect("finite")
            .nodes
    };
    assert_eq!(at(&none), at(&one));
    assert_ne!(at(&none), at(&Fa2ForceSimParams::default()));
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

/// `scale` reaches `_fa2_rescale` and nothing else: it changes the width of the answer, not
/// its shape. `self.k` comes from `_FA2_SIM_SCALE`, a constant.
#[test]
fn the_output_scale_resizes_without_rearranging() {
    let params = Fa2ForceSimParams {
        scale: 50.0,
        ..Fa2ForceSimParams::default()
    };
    let wide = {
        let layout = ForceAtlas2ForceSim::run(&graph(4, DIAMOND), &params).expect("finite");
        let NodeGeometry::Point { x, .. } = &layout.nodes else {
            panic!("expected points")
        };
        bits(x)
    };
    let narrow = columns(4, DIAMOND).0;
    assert_eq!(wide.len(), narrow.len());
    for (w, n) in wide.iter().zip(&narrow) {
        let ratio = f64::from(f32::from_bits(*w)) / f64::from(f32::from_bits(*n));
        assert!((ratio - 10.0).abs() < 1e-4, "ratio {ratio}");
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

#[test]
fn scratch_prints_the_constants_and_the_rescaled_state() {
    let n: usize = std::env::var("GM_FA2_N").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let pairs: Vec<(u32, u32)> = TREE.to_vec();
    let g = graph(n, &pairs);
    let simple = crate::layout::force::simple_graph(&g);
    let mut sim = state::Sim::new(n, &simple, Fa2ForceSimParams::default());
    eprintln!("k        {:.17}", sim.k_debug());
    eprintln!("mass     {:?}", sim.mass_debug().iter().map(|x| format!("{x}")).collect::<Vec<_>>());
    eprintln!("soften 0x{:08X}", sim.soften_debug().to_bits());
    let (r, a, g) = sim.terms_debug();
    for (name, v) in [("rep", r), ("att", a), ("gra", g)] {
        eprintln!("{name}  {:?}", v.iter().map(|x| format!("0x{:08X}", x.to_bits())).collect::<Vec<_>>());
    }
    for step in 1..=50u32 {
        sim.step(1);
        eprintln!("pos{step}  {:?}", sim.pos_debug().iter().map(|x| format!("0x{:08X}", x.to_bits())).collect::<Vec<_>>());
    }
}
