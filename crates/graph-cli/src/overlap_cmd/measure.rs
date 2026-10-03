//! The instruments `graph-cli overlap` measures with, kept apart from the command that drives
//! them so neither file outgrows the house 300-line limit.
//!
//! **These are instruments, not capability.** `overlapping` is the exhaustive `O(n^2)` check
//! that makes the invariant exact rather than "usually"; the other three turn a pass into the
//! numbers the job's quality bar names. Nothing in `graph-core` calls any of them.
//!
//! **One `O(n^2)` and no more.** `overlapping` is the only exhaustive scan here and the command
//! refuses it above `BRUTE_FORCE_CEILING` for exactly that reason. The two scale ratios — the
//! displacement normaliser and the stress ratio — are `O(sample · n)` instead, and both say so
//! in the field they fill: a measurement command that secretly costs `O(n^2)` is measuring
//! itself at the sizes this pass exists for.

use graph_core::Geometry;
use graph_core::Topology;
use graph_core::post::{self, separate::sweep};

/// How many of a drawing's nodes [`mean_neighbour_distance`] measures.
///
/// **A sample, and here is what it gives up.** The mean over every node would be an exact
/// `O(n^2)` scan, which at `n = 100 000` is the whole cost of the command — and it was, until
/// this function existed: the normaliser used to take the drawing's single closest pair, so
/// every "relative" number it produced was that one pair's distance, not the drawing's spacing.
/// Taking every `stride`-th node in index order is fixed and RNG-free (D2, D3) and spreads over
/// the drawing for both layouts measured here, because index order is spatial order on a grid.
pub const SCALE_SAMPLE: usize = 256;

/// The stress ratio's edge sample size, and the same trade with the same reasoning.
pub const STRESS_SAMPLE: usize = 2_000;

/// The exhaustive `O(n^2)` count of pairs closer than `ri + rj + 2 · margin`.
///
/// The authority the gate row checks against, and the reason `--no-scan` prints a *different*
/// number rather than nothing: the pass's own grid count and this count bracket one quantity,
/// and their agreeing at small `n` is what makes the large-`n` grid count worth reading.
pub fn overlapping(geometry: &Geometry, radii: &[f32], margin: f64) -> u32 {
    let graph_contract::geometry::NodeGeometry::Circle { x, y, .. } = &geometry.nodes else {
        return 0;
    };
    let mut count = 0u32;
    for i in 0..x.len() {
        for j in i + 1..x.len() {
            let (dx, dy) = (x[i] - x[j], y[i] - y[j]);
            let need = radii[i] + radii[j] + 2.0 * margin as f32 - sweep::TOLERANCE;
            if libm::sqrtf(dx * dx + dy * dy) < need {
                count += 1;
            }
        }
    }
    count
}

/// The mean distance a node moved, in layout units.
pub fn mean_displacement(before: &Geometry, after: &Geometry) -> f64 {
    let (bx, by) = post::centres(&before.nodes);
    let (ax, ay) = post::centres(&after.nodes);
    if bx.is_empty() {
        return 0.0;
    }
    let total: f64 = (0..bx.len())
        .map(|i| {
            let (dx, dy) = (f64::from(ax[i] - bx[i]), f64::from(ay[i] - by[i]));
            libm::sqrt(dx * dx + dy * dy)
        })
        .sum();
    total / bx.len() as f64
}

/// The drawing's own mean nearest-neighbour distance: the scale a displacement is read against.
///
/// Without it a raw displacement is not comparable across sizes — 14 units means something quite
/// different on a 32-unit drawing than on a 320-unit one. One node, or none, has no spacing to
/// report and gives 1.0 rather than a number about nothing.
pub fn mean_neighbour_distance(geometry: &Geometry) -> f64 {
    let (x, y) = post::centres(&geometry.nodes);
    if x.len() < 2 {
        return 1.0;
    }
    let stride = (x.len() / SCALE_SAMPLE).max(1);
    let mut total = 0.0f64;
    let mut counted = 0u32;
    for i in (0..x.len()).step_by(stride) {
        let mut near = f64::MAX;
        for j in 0..x.len() {
            if i == j {
                continue;
            }
            let (dx, dy) = (f64::from(x[i] - x[j]), f64::from(y[i] - y[j]));
            near = near.min(libm::sqrt(dx * dx + dy * dy));
        }
        total += near;
        counted += 1;
    }
    if counted == 0 {
        return 1.0;
    }
    (total / f64::from(counted)).max(1e-9)
}

/// The stress ratio: mean graph distance over mean layout distance, over a fixed sample of
/// edges. Below 1 means the drawing compresses edges; above 1 means it stretches them.
///
/// The graph term is the expensive one, so the sample is the first [`STRESS_SAMPLE`] edges in
/// topology order — fixed, no clock, no RNG (D1–D3) — and the command says it is a sample
/// rather than letting it look exhaustive.
pub fn stress(topology: &Topology, geometry: &Geometry) -> f64 {
    let (x, y) = post::centres(&geometry.nodes);
    let edges = topology.edges();
    let take = edges.source.len().min(STRESS_SAMPLE);
    if take == 0 {
        return 1.0;
    }
    let mut graph_total = 0.0f64;
    let mut layout_total = 0.0f64;
    for e in 0..take {
        let (s, t) = (edges.source[e] as usize, edges.target[e] as usize);
        graph_total += 1.0;
        let (dx, dy) = (f64::from(x[s] - x[t]), f64::from(y[s] - y[t]));
        layout_total += libm::sqrt(dx * dx + dy * dy);
    }
    if layout_total == 0.0 {
        return f64::INFINITY;
    }
    graph_total / layout_total
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
    use graph_core::{REFERENCE_DEGREE, index_model, seeded_model};

    /// A `Circle` drawing from `points`, all at `radius`.
    fn drawing(points: &[(f32, f32)], radius: f32) -> Geometry {
        let x: Vec<f32> = points.iter().map(|p| p.0).collect();
        let y: Vec<f32> = points.iter().map(|p| p.1).collect();
        graph_core::layout::Geometry::planar(
            NodeGeometry::Circle {
                x,
                y,
                r: vec![radius; points.len()],
            },
            EdgeGeometry::Line,
            Vec::new(),
        )
    }

    fn radii(n: usize, radius: f32) -> Vec<f32> {
        vec![radius; n]
    }

    /// The mean over **every** node, computed the slow way: the exact answer the sampled one
    /// approximates, used here as the oracle rather than a hand-written constant.
    fn exact_mean_neighbour_distance(points: &[(f32, f32)]) -> f64 {
        let mut total = 0.0f64;
        for (i, a) in points.iter().enumerate() {
            let near = points
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| {
                    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
                    libm::sqrt(dx * dx + dy * dy)
                })
                .fold(f64::MAX, f64::min);
            total += near;
        }
        total / points.len() as f64
    }

    #[test]
    fn mean_neighbour_distance_is_the_mean_and_not_the_closest_pair() {
        // Nearest distances 2, 2, 4 — a mean of 8/3, where the closest pair alone says 2.
        let points = [(0.0f32, 0.0f32), (2.0, 0.0), (6.0, 0.0)];
        let got = mean_neighbour_distance(&drawing(&points, 1.0));
        assert!(
            (got - 8.0 / 3.0).abs() < 1e-9,
            "mean nearest-neighbour distance is {got}, want {} (the closest pair is 2)",
            8.0 / 3.0
        );
        assert!((got - exact_mean_neighbour_distance(&points)).abs() < 1e-9);
    }

    #[test]
    fn mean_neighbour_distance_of_a_drawing_too_small_to_have_spacing_is_one() {
        assert_eq!(mean_neighbour_distance(&drawing(&[], 1.0)), 1.0);
        assert_eq!(mean_neighbour_distance(&drawing(&[(4.0, 4.0)], 1.0)), 1.0);
    }

    #[test]
    fn overlapping_counts_every_pair_closer_than_the_two_radii() {
        // Centres 1.5 apart with radius 1: one pair. The same pair at 2.2: none, because
        // 2.2 > 1 + 1 - TOLERANCE.
        let close = drawing(&[(0.0, 0.0), (1.5, 0.0)], 1.0);
        assert_eq!(overlapping(&close, &radii(2, 1.0), 0.0), 1);
        let apart = drawing(&[(0.0, 0.0), (2.2, 0.0)], 1.0);
        assert_eq!(overlapping(&apart, &radii(2, 1.0), 0.0), 0);
        // A margin widens the required separation to 2 + 2 · 0.25, so it can only add pairs.
        assert_eq!(overlapping(&apart, &radii(2, 1.0), 0.25), 1);
    }

    #[test]
    fn overlapping_is_zero_on_a_layout_with_no_extent() {
        // A `Point` layout has no radius, so there is nothing to overlap: the count that the
        // no-op case rests on.
        let points = Geometry::planar(
            NodeGeometry::Point {
                x: vec![0.0, 0.5],
                y: vec![0.0, 0.0],
            },
            EdgeGeometry::Line,
            Vec::new(),
        );
        assert_eq!(overlapping(&points, &radii(2, 0.0), 0.0), 0);
    }

    #[test]
    fn mean_displacement_is_the_mean_path_length() {
        let before = drawing(&[(0.0, 0.0), (10.0, 0.0)], 1.0);
        let after = drawing(&[(3.0, 4.0), (10.0, 0.0)], 1.0);
        assert!((mean_displacement(&before, &after) - 2.5).abs() < 1e-9);
        assert_eq!(mean_displacement(&before, &before), 0.0);
        assert_eq!(mean_displacement(&drawing(&[], 1.0), &before), 0.0);
    }

    #[test]
    fn stress_is_graph_distance_over_layout_distance() {
        let (nodes, edges) = seeded_model(1, 8, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("seeded model indexes");
        let points: Vec<(f32, f32)> = (0..8).map(|i| (i as f32, 0.0)).collect();
        let geometry = drawing(&points, 1.0);
        let edges = topology.edges();
        let layout_total: f64 = (0..edges.source.len())
            .map(|e| (edges.source[e] as f64 - edges.target[e] as f64).abs())
            .sum();
        let want = edges.source.len() as f64 / layout_total;
        assert!((stress(&topology, &geometry) - want).abs() < 1e-9, "{want}");
    }

    #[test]
    fn stress_of_an_edgeless_graph_is_one_and_of_a_collapsed_one_is_infinite() {
        let (nodes, edges) = seeded_model(1, 4, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("seeded model indexes");
        let collapsed: Vec<(f32, f32)> = (0..4).map(|_| (2.0, 2.0)).collect();
        assert_eq!(stress(&topology, &drawing(&collapsed, 1.0)), f64::INFINITY);
    }
}
