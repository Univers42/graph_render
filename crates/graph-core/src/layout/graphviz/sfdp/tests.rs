//! What the sfdp port is pinned on, and what it is deliberately **not** pinned on.
//!
//! The reference is an iterative force layout whose output depends on glibc's `rand()` and on
//! the order its multilevel matchings are drawn, so the port is NOT pinned node-for-node
//! against Graphviz — see `docs/measurements/p13-gv2-sfdp.md`, which records the measured
//! seed-to-seed spread of the oracle itself (292 points at `-Gstart` 1 against 7). What is
//! pinned here is the set of properties a *correct* implementation must have and that a broken
//! one loses: determinism, finiteness, direction-independence, and the two closed cases the
//! engine really does answer exactly.

use super::*;
use crate::layout::coords::probe;

/// A geometry's point columns as bit pairs, so "the same drawing" is an exact comparison.
fn bits(geometry: &crate::layout::Geometry) -> Vec<[u32; 2]> {
    probe::points(geometry)
        .into_iter()
        .map(|(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

/// The oracle's own answer for one node, which is the single case sfdp answers independently
/// of its seed: `0.375, 0.25` inch, the centre of the default 0.75 x 0.5 node box
/// (`docs/measurements/p13-gv2-sfdp.md` records it identical at `-Gstart` 1, 7 and 99).
#[test]
fn one_node_lands_on_the_node_box_centre() {
    let points = probe::points(&run(&probe::graph(1, &[])).expect("one node lays out"));
    assert_eq!(points.len(), 1);
    assert!(
        (points[0].0 - 27.0).abs() < 1e-2 && (points[0].1 - 18.0).abs() < 1e-2,
        "one node at {:?}, want the 27x18 point box centre",
        points[0]
    );
}

/// Two nodes stay apart, finite, and inside the drawn frame.
///
/// The separation is the property, not the direction. The attraction along the single edge and
/// the repulsion between the two nodes both point along the line joining them, so the pair
/// never collapses; the residual rotation the model does allow is named in the module's
/// `Ponytail` note, because the two nodes sit in Barnes-Hut cells with different centres of
/// mass, so the two forces are only nearly antiparallel.
///
/// The oracle's own two-node answer is seed-*dependent*, so there is no closed answer to pin:
/// `docs/measurements/p13-gv2-sfdp.md` records three different separations for `-Gstart` 1, 7
/// and 99. Asserting they lie on the x-axis would assert something the oracle itself refutes.
#[test]
fn two_nodes_stay_apart_and_finite() {
    let points = probe::points(&run(&probe::graph(2, &[(0, 1)])).expect("two nodes lay out"));
    assert_eq!(points.len(), 2);
    for (i, (x, y)) in points.iter().enumerate() {
        assert!(x.is_finite() && y.is_finite(), "node {i} at ({x}, {y})");
    }
    let (dx, dy) = (points[1].0 - points[0].0, points[1].1 - points[0].1);
    assert!(
        f64::sqrt((dx * dx + dy * dy) as f64) > 1e-3,
        "the two nodes collapsed onto each other: {points:?}"
    );
    // The rendering translates the drawing's lower-left node-box corner to the origin, so
    // every node's coordinates are positive.
    assert!(
        points.iter().all(|p| p.0 > 0.0 && p.1 > 0.0),
        "a node landed outside the drawn frame: {points:?}"
    );
}

/// Determinism (D1-D10): the same graph twice is bit-identical, and a different seed moves
/// the drawing. Without the second half this would also pass for a layout that ignores its
/// seed, which is a different and wrong layout.
#[test]
fn run_twice_is_bit_identical_and_the_seed_moves_it() {
    let graph = probe::graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)]);
    let once = run(&graph).expect("lays out");
    let twice = run(&graph).expect("lays out");
    let other = run_seeded(&graph, 7).expect("lays out");
    let moved = bits(&once)
        .iter()
        .zip(bits(&other).iter())
        .any(|(a, b)| a != b);
    assert_eq!(bits(&once), bits(&twice), "two runs of one input differed");
    assert!(moved, "seed 7 placed every node exactly where seed 1 did");
}

/// Every coordinate is finite across the sizes that matter. A spring model that loses a node
/// to `inf`/`NaN` is the classic failure, and the kernel rejects it rather than publishing it
/// (D10: gather form, so one bad node does not take its neighbours with it).
#[test]
fn every_coordinate_is_finite_over_the_gate_sizes() {
    for n in [2usize, 3, 17, 64, 257] {
        let count = n as u32;
        let mut edges: Vec<(u32, u32)> = (0..count).map(|i| (i, (i + 1) % count)).collect();
        edges.extend((0..count / 2).map(|i| (i, (i + count / 3) % count)));
        let layout = run(&probe::graph(count, &edges)).expect("a cycle lays out");
        for (i, (x, y)) in probe::points(&layout).iter().enumerate() {
            assert!(
                x.is_finite() && y.is_finite(),
                "n={n} node {i} at ({x}, {y})"
            );
        }
    }
}

/// D10 where it bites: node `i`'s position must depend on its neighbours and on nothing else,
/// not on which nodes are visited first. The spring model is undirected and the port
/// symmetrises, so reversing every edge must leave the drawing bit-identical.
#[test]
fn edge_direction_does_not_change_the_drawing() {
    let forward = probe::graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]);
    let backward = probe::graph(4, &[(1, 0), (2, 1), (3, 2), (0, 3), (2, 0)]);
    assert_eq!(
        bits(&run(&forward).expect("lays out")),
        bits(&run(&backward).expect("lays out")),
        "reversing every edge moved the drawing"
    );
}

/// The layout does not collapse. A spring-electrical model whose step control is wrong puts
/// every node on one point: finite, deterministic, direction-independent, and all four tests
/// above would still pass. The mean nearest-neighbour distance has to stay a real fraction of
/// the drawing's own width.
#[test]
fn the_layout_does_not_collapse_to_a_point() {
    let n = 40u32;
    let edges: Vec<(u32, u32)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
    let points = probe::points(&run(&probe::graph(n, &edges)).expect("lays out"));
    let width = points.iter().map(|p| p.0).fold(f32::MIN, f32::max)
        - points.iter().map(|p| p.0).fold(f32::MAX, f32::min);
    let mean: f32 = points
        .iter()
        .enumerate()
        .map(|(i, a)| {
            points
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| (a.0 - b.0).hypot(a.1 - b.1))
                .fold(f32::MAX, f32::min)
        })
        .sum::<f32>()
        / points.len() as f32;
    assert!(width > 0.0, "the drawing has zero width: {points:?}");
    assert!(
        mean > width * 0.01,
        "mean nearest-neighbour distance {mean} against width {width}: the layout collapsed"
    );
}

/// The degenerate ends of the model — one node, and a graph whose edges are all that exist —
/// must still produce finite positions rather than dividing by an empty edge set.
#[test]
fn a_lone_node_and_a_path_are_both_finite() {
    let single = probe::points(&run(&probe::graph(1, &[])).expect("lays out"));
    assert_eq!(single.len(), 1);
    assert!(single[0].0.is_finite() && single[0].1.is_finite());
    let path = probe::points(&run(&probe::graph(3, &[(0, 1), (1, 2)])).expect("lays out"));
    assert_eq!(path.len(), 3);
    for (i, (x, y)) in path.iter().enumerate() {
        assert!(x.is_finite() && y.is_finite(), "node {i} at ({x}, {y})");
    }
}

/// A 400-node random graph, two edges per node to an earlier one: the studio's default size.
fn random_graph(count: u32) -> Vec<(u32, u32)> {
    let mut state = 12_345u64;
    let mut edges = Vec::new();
    for i in 1..count {
        for _ in 0..2 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            edges.push((i, ((state >> 33) % u64::from(i)) as u32));
        }
    }
    edges
}

/// The drawing spreads over both axes and keeps its nodes apart. On 2026-10-01 a node's own
/// quadtree leaf repelled it, so 400 nodes fell onto 11 distinct x values in a strip 0.07 tall
/// while one node flew 180 units away.
#[test]
fn a_400_node_graph_spreads_in_two_dimensions() {
    let points = probe::points(&run(&probe::graph(400, &random_graph(400))).expect("lays out"));
    let span = |axis: fn(&(f32, f32)) -> f32| {
        let values: Vec<f32> = points.iter().map(axis).collect();
        values.iter().copied().fold(f32::MIN, f32::max)
            - values.iter().copied().fold(f32::MAX, f32::min)
    };
    let (wide, tall) = (span(|p| p.0), span(|p| p.1));
    assert!(
        tall > 0.3 * wide && wide > 0.3 * tall,
        "a strip: {wide} x {tall}"
    );
    let cell = wide.max(tall) * 1e-3;
    let mut seen: Vec<(i64, i64)> = points
        .iter()
        .map(|p| ((p.0 / cell) as i64, (p.1 / cell) as i64))
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert!(
        seen.len() >= 380,
        "only {} of 400 positions are distinct",
        seen.len()
    );
}
