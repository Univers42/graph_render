//! The multilevel hierarchy: maximal matching up.
//!
//! Reference: `Multilevel.c` (`Multilevel_new`, `Multilevel_get_coarsest`, `Multilevel_coarsen`,
//! `maximal_independent_edge_set_heaviest_edge_pernode_supernodes_first`), read as an algorithm
//! reference. A level is `pair(fine_node) -> coarse_node`; coarsening repeats matching passes
//! ([`matching`]) until the level has shrunk to `min_coarsen_factor` of the one below or a pass
//! stops making progress. Laying a coarse solution back down is [`super::prolongation`].
//!
//! **Graphviz's `sfdp` never reaches this module at its defaults.** `sfdp`'s `levels` attribute
//! defaults to `0` (`sfdpinit.c:213`) and `Multilevel_establish` returns at
//! `grid->level >= ctrl.maxlevel - 1` (`Multilevel.c:163`), so the engine runs a single level.
//! This port always coarsens; `docs/measurements/sg-sfdp-collapse.md` measures what that costs
//! and `sg-sfdp-step` owns closing it.

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

    #[test]
    fn k_decays_by_three_quarters() {
        assert!((decay_k(4.0) - 3.0).abs() < 1e-12);
        assert!((K_DECAY - 0.75).abs() < 1e-12);
    }
}
