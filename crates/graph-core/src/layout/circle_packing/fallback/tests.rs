//! The fallback's entry point: the degree-proportional starting radii, the seed budget
//! and the coincident-pair substitute, and the whole packing pinned bit for bit so the
//! relaxation's own tests have something to be pinned against.

use super::seed::{GOLDEN_ANGLE, start_positions};
use super::{initial_radii, nudge, pack, seed_iterations, separated};

/// `get_layout_seed()` with no pipeline seed set: `derive_seed(42, "layout")`
/// (`repro/determinism.py:124-129`) — the seed SciGraphs passes to the fallback's
/// `nx.spring_layout` (`circle_packing.py:428`).
const LAYOUT_SEED: u32 = 981_798_123;

/// `np.random.RandomState(981798123).rand(2, 2)`, row-major, as bits (numpy 2.3.3): node 0
/// takes draws 0 and 1, node 1 draws 2 and 3. The reference's own stream, bit for bit —
/// a tolerance here would accept a generator that is merely *near* it.
const RAND_2_2: [u64; 4] = [
    0x3FED_2B61_1EED_9D7B,
    0x3FE2_38BF_EACB_3589,
    0x3FD7_C4D8_0207_BEAE,
    0x3FC9_03DC_489C_5238,
];

#[test]
fn a_seeded_start_is_numpys_random_sample_bit_for_bit() {
    let got = start_positions(2, Some(LAYOUT_SEED));
    let bits: Vec<u64> = got
        .iter()
        .flat_map(|&(x, y)| [x.to_bits(), y.to_bits()])
        .collect();
    assert_eq!(bits, RAND_2_2.to_vec());
}

/// The control: a `start_positions` that ignored its argument and hard-coded the layout seed
/// would pass the test above.
#[test]
fn a_neighbouring_seed_moves_every_start_coordinate() {
    assert_ne!(
        start_positions(2, Some(LAYOUT_SEED + 1)),
        start_positions(2, Some(LAYOUT_SEED))
    );
}

/// `None` is still the golden-angle spiral, and this is the registered
/// `layout.packing.circle`'s path: `sqrt((i + 1) / n)` out at `i * GOLDEN_ANGLE`. Pinned as
/// the closed form rather than as bits so the claim is readable, and so a default that moved
/// would show up as a wrong radius or a wrong angle and not only as a changed number.
#[test]
fn the_unseeded_start_is_still_the_golden_angle_spiral() {
    let got = start_positions(2, None);
    assert_eq!(got[0].0.to_bits(), f64::sqrt(0.5).to_bits());
    assert_eq!(got[0].1.to_bits(), 0.0_f64.to_bits());
    assert_eq!(got[1].0.to_bits(), libm::cos(GOLDEN_ANGLE).to_bits());
    assert_eq!(got[1].1.to_bits(), libm::sin(GOLDEN_ANGLE).to_bits());
    assert_ne!(
        start_positions(2, None),
        start_positions(2, Some(LAYOUT_SEED)),
        "the two streams must not be the same generator"
    );
}

/// The seed reaches the whole packing, not only `start_positions`: at SciGraphs' own two
/// numbers the two starts relax to different centres.
#[test]
fn the_seed_parameter_reaches_the_packing() {
    let edges: Vec<(u32, u32)> = (0..5)
        .flat_map(|i| ((i + 1)..5).map(move |j| (i, j)))
        .collect();
    let at = |seed| {
        pack(
            5,
            &edges,
            &[0; 5],
            &super::CirclePackingParams {
                iterations: 500,
                scale: 5.0,
                seed,
            },
        )
    };
    let seeded = at(Some(LAYOUT_SEED));
    let spiral = at(None);
    assert_ne!(
        (seeded.x.clone(), seeded.y.clone()),
        (spiral.x.clone(), spiral.y.clone()),
        "the seed is not reaching the packing"
    );
    for packed in [&seeded, &spiral] {
        assert!(packed.approximate);
        assert!(
            packed.x.iter().chain(&packed.y).all(|v| v.is_finite()),
            "a seeded start must still relax to finite centres"
        );
    }
}

#[test]
fn a_separated_coincident_pair_gets_a_repeatable_direction() {
    // The reference jitters this case with `rng.rand() - 0.5`; graph-core substitutes
    // `nudge`, a fixed index-derived direction. Two different pairs must not all split
    // along the same axis, and the same pair must always split the same way.
    let (u, v) = (2, 5);
    let first = separated((0.0, 0.0), u, v);
    assert_eq!(first.1, 1.0, "a coincident pair is given unit distance");
    assert_eq!(first.0, nudge(u, v));
    assert_eq!(separated((0.0, 0.0), u, v), first, "repeatable");
    assert_ne!(
        nudge(0, 1),
        nudge(1, 0),
        "index order matters, not just the pair"
    );
    // A pair that is not coincident is left exactly as measured.
    let (dx, dy) = (3.0, 4.0);
    assert_eq!(separated((dx, dy), 1, 2), ((dx, dy), 5.0));
}

#[test]
fn nudge_is_the_golden_angle_direction_of_the_pair() {
    let angle = f64::from(3) * GOLDEN_ANGLE + f64::from(4) * core::f64::consts::PI;
    let want = (libm::cos(angle), libm::sin(angle));
    assert_eq!(nudge(3, 4).0.to_bits(), want.0.to_bits());
    assert_eq!(nudge(3, 4).1.to_bits(), want.1.to_bits());
}

#[test]
fn the_seed_budget_is_twenty_thousand_over_n_clamped_to_ten_and_fifty() {
    // SciGraphs' own `max(10, min(50, 20000 // max(num_nodes, 1)))`
    // (`circle_packing.py:427`). Above `n = 2000` the quotient is under 10 and the floor
    // takes over; under `n = 400` the ceiling does.
    for (n, want) in [
        (1u32, 50u32),
        (3, 50),
        (399, 50),
        (400, 50),
        (401, 49),
        (2000, 10),
        (4000, 10),
    ] {
        assert_eq!(seed_iterations(n), want, "n = {n}");
    }
    // And a zero node count cannot divide by zero.
    assert_eq!(seed_iterations(0), 50);
}

#[test]
fn initial_radii_are_degree_proportional_and_bounded_by_the_frame() {
    // SciGraphs' own rule (`circle_packing.py:420-425`): `0.3 + 0.7 * degree/max`, then
    // scaled so the squares sum to 0.35 of the frame's area.
    let edges = [(0, 1), (0, 2), (0, 3), (1, 2)];
    let radii = initial_radii(4, &edges, &[0; 4], 5.0);
    let degrees = [3.0, 2.0, 2.0, 1.0];
    let raw: Vec<f64> = degrees.iter().map(|&d| 0.3 + 0.7 * (d / 3.0)).collect();
    let sum_sq: f64 = raw.iter().map(|r| r * r).sum();
    let factor = f64::sqrt(0.35 * 2.25 * 2.25 / sum_sq);
    for (got, want) in radii.iter().zip(&raw) {
        assert_eq!(got.to_bits(), (want * factor).to_bits());
    }
    // A node with no edges still gets a positive radius, so no circle is ever zero-sized.
    let isolated = initial_radii(2, &[(0, 1)], &[0; 2], 5.0);
    assert!(isolated.iter().all(|&r| r > 0.0), "{isolated:?}");
}

#[test]
fn a_nodes_radius_follows_its_own_degree_not_its_index() {
    // The degree is counted per incident edge, on the simple graph `simple_pairs` hands
    // over — so a hub is the biggest circle and an isolated node the smallest. A degree
    // taken from the dense index, or a hub counted once, would break this.
    let edges = [(0, 1), (0, 2), (0, 3), (0, 4), (1, 2), (1, 3)];
    let radii = initial_radii(5, &edges, &[0; 5], 5.0);
    assert!(
        radii[0] > radii[1] && radii[1] > radii[4],
        "degrees 4, 3, 0: {radii:?}"
    );
    // The 0.7 weight means the ratio is not linear in the degree, but it is monotone and
    // the floor keeps an isolated node well above zero.
    assert!(radii[4] > 0.0, "an isolated node still gets a real circle");
    // And the frame normalisation is global, not per node: every packing's squares sum to
    // the same 0.35 of the frame, so a graph with one hub packs very differently from a
    // graph where every node has that degree.
    let hub = initial_radii(5, &edges, &[0; 5], 5.0);
    let flat: Vec<(u32, u32)> = (0..4).map(|i| (i, i + 1)).collect();
    let even = initial_radii(5, &flat, &[0; 5], 5.0);
    assert!(
        hub[0] > even[0],
        "a hub beats an even spread: {hub:?} vs {even:?}"
    );
    for radii in [&hub, &even] {
        let sum_sq: f64 = radii.iter().map(|r| r * r).sum();
        assert!((sum_sq - 0.35 * 2.25 * 2.25).abs() < 1e-12, "sum {sum_sq}");
    }
}

/// SciGraphs reads `G.degree(n)` on the graph `_build_networkx_graph` handed it, and that
/// graph **keeps its self-loops**: `G.add_edges_from(edge_indices)` at `common.py:297` has
/// no `u != v` filter, unlike `_planar_triangulation`'s own `simple` copy
/// (`circle_packing.py:61`). networkx counts a self-loop twice — `DegreeView.__getitem__`
/// is `len(nbrs) + (n in nbrs)` (`classes/reportviews.py:526`) — so a node whose only edge
/// is a self-loop has degree 2, not 0.
///
/// Four nodes, one edge and one self-loop, in the exact shape the fallback receives them:
/// `edges` is what `simple_pairs` leaves (loop gone), `loops` what `loop_counts` counts
/// alongside it. Node 3's only edge is its own loop, so networkx gives it degree 2 — the
/// largest here — where dropping the loop left it on the `0.3` isolated floor.
#[test]
fn a_self_loop_counts_twice_in_the_starting_radii_as_it_does_in_networkx() {
    let edges = [(1, 2)];
    let loops = [0u32, 0, 0, 1];
    let radii = initial_radii(4, &edges, &loops, 5.0);
    let degrees = [0.0, 1.0, 1.0, 2.0]; // networkx `G.degree`: the loop at 3 counts twice
    let raw: Vec<f64> = degrees.iter().map(|&d| 0.3 + 0.7 * (d / 2.0)).collect();
    let sum_sq: f64 = raw.iter().map(|r| r * r).sum();
    let factor = f64::sqrt(0.35 * 2.25 * 2.25 / sum_sq);
    for (got, want) in radii.iter().zip(&raw) {
        assert_eq!(
            got.to_bits(),
            (want * factor).to_bits(),
            "degrees {degrees:?}"
        );
    }
    // And the self-loop's own node is *not* left on the isolated floor: this is the whole
    // defect, since `simple_pairs` reduces the loop away before the fallback ever sees it.
    let without = initial_radii(4, &edges, &[0; 4], 5.0);
    assert_ne!(
        radii[3].to_bits(),
        without[3].to_bits(),
        "dropping the loop changes the radius networkx gives it"
    );
    // The loops are the *only* thing `loops` contributes: the same graph without the loop
    // is untouched by the argument, so nothing else in the reduction moved.
    let isolated = initial_radii(4, &edges, &loops, 5.0);
    assert_eq!(radii[0].to_bits(), isolated[0].to_bits());
}

#[test]
fn the_fallbacks_own_packing_is_pinned_bit_for_bit() {
    // Golden values produced by running this code after the D10 gather and the multi-edge
    // fixes: K5, the module's named failing input, through the whole fallback at the
    // default parameters. On top of `tests::fallback::the_fallback_still_produces_finite_
    // non_negative_circle_geometry`, which is the behavioural half of this pair.
    let edges: Vec<(u32, u32)> = (0..5)
        .flat_map(|i| ((i + 1)..5).map(move |j| (i, j)))
        .collect();
    let packed = pack(
        5,
        &edges,
        &[0; 5],
        &super::CirclePackingParams {
            iterations: 500,
            scale: 5.0,
            ..super::CirclePackingParams::default()
        },
    );
    let got: Vec<u64> = packed
        .x
        .iter()
        .chain(&packed.y)
        .chain(&packed.r)
        .map(|v| v.to_bits())
        .collect();
    assert_eq!(got.len(), 15);
    assert!(
        packed.approximate,
        "the fallback always reports approximate"
    );
    assert_eq!(
        got,
        vec![
            0x3FF5731389BB0297,
            0xBFEA0FCBDD41D5BA,
            0xBF975EB34FE92311,
            0x3FEB3E4D90C56DC3,
            0xBFF5ACD9963D2A10,
            0xBFDD69CBFFA132F5,
            0x3FF28E81D0881122,
            0xBFF6AC0860051DAD,
            0x3FF2209D784A6355,
            0xBFDAA28FA3942833,
            0x3FEAA7EF3FF5C4A5,
            0x3FEAA7EF3FF5C4A5,
            0x3FEAA7EF3FF5C4A5,
            0x3FEAA7EF3FF5C4A5,
            0x3FEAA7EF3FF5C4A5,
        ],
        "K5 fallback packing"
    );
}
