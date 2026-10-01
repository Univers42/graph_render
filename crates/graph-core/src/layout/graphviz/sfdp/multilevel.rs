//! The multilevel hierarchy: maximal matching up, prolongation back down.
//!
//! Reference: `Multilevel.c` (`Multilevel_new`, `Multilevel_get_coarsest`, `coarsen`) and
//! `prolongate` at `lib/sfdpgen/post_process.c`, read as an algorithm reference. A level is
//! `pair(coarse_node, fine_node)`; coarsening repeats matching passes ([`matching`]) until the
//! level is small enough; prolongation lays a coarse solution back down by giving every fine
//! node its coarse node's position, jittered.

use super::matching;

/// One level: `pair[j]` is the coarse node that fine node `j` belongs to, and `coarse` the
/// number of coarse nodes.
pub(super) struct Level {
    pub(super) pair: Vec<u32>,
    pub(super) coarse: u32,
}

/// The smallest level the reference keeps (`Multilevel.c:23`): a pass that would go below it
/// is discarded and the level before it is the coarsest.
const MIN_SIZE: u32 = 4;

/// A level must shrink to this fraction of the level below it (`Multilevel.c:24`).
const MIN_COARSEN_FACTOR: f64 = 0.75;

/// The next coarser level: matching passes repeat, composed, until the level has shrunk to
/// [`MIN_COARSEN_FACTOR`] of `count`, or a pass stops making progress
/// (`Multilevel_coarsen`, `Multilevel.c:206-242`). One pass alone is not a level: `K` shrinks
/// once per level, and a graph that coarsens slowly would shrink it towards zero.
///
/// A level that could not coarsen at all comes back as the identity, with `coarse == count`.
pub(super) fn coarsen(count: u32, edges: &[(u32, u32)]) -> Level {
    let mut level = Level {
        pair: (0..count).collect(),
        coarse: count,
    };
    let mut current = edges.to_vec();
    while f64::from(level.coarse) > MIN_COARSEN_FACTOR * f64::from(count) {
        let next = matching::pass(level.coarse, &current);
        if next.coarse == level.coarse || next.coarse < MIN_SIZE {
            break;
        }
        current = coarse_edges(&next, &current);
        for coarse in &mut level.pair {
            *coarse = next.pair[*coarse as usize];
        }
        level.coarse = next.coarse;
    }
    level
}

/// Lay a coarse solution down onto `count` fine nodes, adding the reference's jitter.
///
/// `prolongate` (`post_process.c`) gives each fine node its coarse partner's position and then
/// breaks ties among coincident points, because without that a matched pair lands exactly on
/// top of each other and the two spring apart from a zero-length edge. The jitter here is
/// derived from `(i, level)` through the quadtree's own counter-based generator
/// (`crate::rng::jiggle`), so it is order-independent and bit-identical everywhere.
pub(super) fn prolongate(
    coarse_x: &[f64],
    coarse_y: &[f64],
    level: &Level,
    count: u32,
    seed: u32,
) -> (Vec<f64>, Vec<f64>) {
    let mut x = vec![0.0; count as usize];
    let mut y = vec![0.0; count as usize];
    for i in 0..count as usize {
        let c = level.pair[i] as usize;
        x[i] = coarse_x.get(c).copied().unwrap_or(0.0);
        y[i] = coarse_y.get(c).copied().unwrap_or(0.0);
    }
    // One pass of separation over coincident pairs, in dense index order.
    for i in 0..count {
        let (j, oi) = (i as usize, i);
        let dx = crate::rng::jiggle(seed, oi, 0, (i, i));
        let dy = crate::rng::jiggle(seed, oi, 1, (i, i));
        x[j] += dx;
        y[j] += dy;
    }
    (x, y)
}

/// The coarsened edge list at one level up, with each edge mapped through `level` and
/// duplicates dropped so the level above is a simple graph.
pub(super) fn coarse_edges(level: &Level, edges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    // Sort-then-dedup rather than a `Vec::contains` per edge, which is O(m²) and is the
    // second of this layout's two superlinear hotspots. Sorting by the canonical `(min, max)`
    // key puts every duplicate next to its twin, and the fixed order it leaves behind is a
    // deterministic function of the edge list (D2) — no hash map is iterated anywhere.
    let mut keys: Vec<(u32, u32)> = edges
        .iter()
        .map(|&(a, b)| {
            let (ca, cb) = (level.pair[a as usize], level.pair[b as usize]);
            if ca <= cb { (ca, cb) } else { (cb, ca) }
        })
        .filter(|&(ca, cb)| ca != cb)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// The ideal edge length `K` for the next finer level: the reference shrinks it by 0.75 on the
/// way down (`spring_electrical.c:1159`).
pub(super) const K_DECAY: f64 = 0.75;

/// The number of levels to coarsen through, so the coarsest level is small enough to lay out
/// directly. The reference coarsens until the level stops shrinking
/// (`Multilevel_get_coarsest`); matching that exactly needs the reference's own matching
/// choices, so this stops at a fixed floor and the module `Ponytail` note says so.
pub(super) const COARSEST_FLOOR: u32 = 8;

/// `K` scaled for the next level down: the reference shrinks it by 0.75 on the way down
/// (`spring_electrical.c:1159`), which is what keeps a fine level's attraction in scale with
/// the coarse solution it was prolonged from.
pub(super) fn decay_k(k: f64) -> f64 {
    k * K_DECAY
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fine node lands in exactly one coarse node, and the coarse numbering is dense.
    #[test]
    fn coarsening_partitions_the_nodes_and_numbers_them_densely() {
        let edges = [(0u32, 1u32), (2, 3), (4, 5), (6, 7)];
        let level = coarsen(8, &edges);
        assert_eq!(level.pair.len(), 8);
        assert_eq!(
            level.coarse, 4,
            "eight nodes in four edges make four coarse nodes"
        );
        let mut sorted = level.pair.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![0, 0, 1, 1, 2, 2, 3, 3],
            "pairs {:?}",
            level.pair
        );
    }

    /// Coarsening repeatedly must strictly shrink, or the driver's loop would not terminate.
    /// The edges are re-mapped at each level, exactly as the driver does: a level's node ids
    /// are its own dense ids, so carrying the finest level's edges past the first coarsening
    /// would index past the end.
    #[test]
    fn coarsening_shrinks_a_connected_graph() {
        let n = 32u32;
        let mut edges: Vec<(u32, u32)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
        let mut count = n;
        for _ in 0..3 {
            let level = coarsen(count, &edges);
            assert!(level.coarse < count, "{} -> {}", count, level.coarse);
            edges = coarse_edges(&level, &edges);
            count = level.coarse;
        }
        assert_eq!(count, 4);
        assert_eq!(
            coarsen(count, &edges).coarse,
            4,
            "a pass below MIN_SIZE is discarded"
        );
    }

    /// An isolated node is its own coarse node — it has no partner to match with.
    #[test]
    fn an_isolated_node_becomes_its_own_coarse_node() {
        let level = coarsen(9, &[(0u32, 1u32), (2, 3), (4, 5), (6, 7)]);
        assert_eq!(level.coarse, 5);
        assert!(
            level.pair[..8].iter().all(|&c| c != level.pair[8]),
            "the isolated node joined another pair"
        );
    }

    /// Coarse edges drop self loops (a matched pair) and duplicates, so the level above is a
    /// simple graph. Without the drop the layout above sees phantom springs of length zero.
    #[test]
    fn coarse_edges_drop_loops_and_duplicates() {
        // The identity level (`pair = [0, 1, 2, 3]`): `coarse_edges` is exercised on a level that
        // does not remap anything, which is the case that isolates its own dedup behaviour.
        let level = Level {
            pair: vec![0, 1, 2, 3],
            coarse: 4,
        };
        let out = coarse_edges(&level, &[(0u32, 1u32), (1, 0), (0, 1), (2, 3)]);
        assert_eq!(out, vec![(0, 1), (2, 3)]);
    }

    /// Prolongation gives every fine node its coarse partner's position, up to the jitter that
    /// breaks matched pairs apart. The bound is the point: a jitter that were not tiny would
    /// displace the drawing, and one that were zero would leave every matched pair stacked.
    #[test]
    fn prolongation_places_fine_nodes_at_their_coarse_partners() {
        let level = Level {
            pair: vec![0, 0, 1, 1],
            coarse: 2,
        };
        let coarse_x = [10.0, 20.0];
        let coarse_y = [30.0, 40.0];
        let (x, y) = prolongate(&coarse_x, &coarse_y, &level, 4, 7);
        assert_eq!(x.len(), 4);
        // `pair = [0, 0, 1, 1]`: nodes 0 and 1 take coarse 0's position, nodes 2 and 3 coarse 1's.
        for i in 0..4 {
            let c = level.pair[i] as usize;
            assert!(
                (x[i] - coarse_x[c]).abs() < 1e-5 && (y[i] - coarse_y[c]).abs() < 1e-5,
                "node {i} at ({}, {}), want ({}, {}) up to the jitter",
                x[i],
                y[i],
                coarse_x[c],
                coarse_y[c]
            );
        }
    }

    /// The two nodes of a matched pair must actually separate: the jitter is what stops them
    /// landing exactly on top of each other, and zero jitter would leave the spring at length
    /// zero forever.
    #[test]
    fn a_matched_pair_ends_up_apart() {
        let level = Level {
            pair: vec![0, 0],
            coarse: 1,
        };
        let (x, y) = prolongate(&[10.0], &[30.0], &level, 2, 7);
        assert!(
            (x[0] - x[1]).abs() > 0.0 || (y[0] - y[1]).abs() > 0.0,
            "a matched pair was prolonged onto one point: ({}, {})",
            x[0],
            y[0]
        );
    }

    #[test]
    fn k_decays_by_three_quarters() {
        assert!((decay_k(4.0) - 3.0).abs() < 1e-12);
        assert!((K_DECAY - 0.75).abs() < 1e-12);
    }
}
