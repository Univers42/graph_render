//! One matching pass of the coarsening: modules first, then a maximal matching.
//!
//! Reference: `maximal_independent_edge_set_heavest_edge_pernode_supernodes_first` at
//! `lib/sfdpgen/Multilevel.c:58-146`, read as an algorithm reference. Nodes with exactly the
//! same neighbours (a *module*, `SparseMatrix_decompose_to_supervariables`) are grouped first,
//! up to [`MAX_CLUSTER_SIZE`] per coarse node; every node left over is matched to its first
//! unmatched neighbour, or stays a singleton.
//!
//! The module step is what lets a hub's leaves coarsen together. Without it a star of `n`
//! leaves loses one node per pass, since every leaf can only match the hub.
//!
//! Deterministic (D2): modules are grouped by sorting the neighbour sets, ties broken by node
//! index, and the matching visits nodes in index order. The reference visits them in a random
//! permutation (`gv_permutation`); the parent module's `Ponytail` note records that.

use super::multilevel::Level;

/// The most fine nodes one module cluster may hold (`Multilevel.h:29`).
const MAX_CLUSTER_SIZE: usize = 4;

/// A fine node not yet given a coarse node.
const UNSET: u32 = u32::MAX;

/// One pass over `count` nodes: each fine node's coarse node, numbered densely.
pub(super) fn pass(count: u32, edges: &[(u32, u32)]) -> Level {
    let rows = neighbour_sets(count, edges);
    let mut pair = vec![UNSET; count as usize];
    let mut coarse = modules(&rows, &mut pair);
    for i in 0..count as usize {
        if pair[i] != UNSET {
            continue;
        }
        pair[i] = coarse;
        if let Some(&j) = rows[i].iter().find(|&&j| pair[j as usize] == UNSET) {
            pair[j as usize] = coarse;
        }
        coarse += 1;
    }
    Level { pair, coarse }
}

/// Each node's neighbours, sorted, without duplicates or itself.
fn neighbour_sets(count: u32, edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut rows: Vec<Vec<u32>> = vec![Vec::new(); count as usize];
    for &(a, b) in edges.iter().filter(|&&(a, b)| a != b) {
        rows[a as usize].push(b);
        rows[b as usize].push(a);
    }
    for row in &mut rows {
        row.sort_unstable();
        row.dedup();
    }
    rows
}

/// Groups the nodes that share a neighbour set into clusters of up to [`MAX_CLUSTER_SIZE`],
/// writing each cluster's coarse id into `pair`. Returns the number of clusters made.
fn modules(rows: &[Vec<u32>], pair: &mut [u32]) -> u32 {
    let mut order: Vec<u32> = (0..rows.len() as u32).collect();
    order.sort_by(|&a, &b| rows[a as usize].cmp(&rows[b as usize]).then(a.cmp(&b)));
    let mut coarse = 0;
    for group in order.chunk_by(|&a, &b| rows[a as usize] == rows[b as usize]) {
        if group.len() < 2 {
            continue;
        }
        for cluster in group.chunks(MAX_CLUSTER_SIZE) {
            for &node in cluster {
                pair[node as usize] = coarse;
            }
            coarse += 1;
        }
    }
    coarse
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A star's leaves share the neighbour set `{hub}`, so one pass folds them four at a time.
    /// On 2026-10-01 they matched the hub one per pass, and a 400-node graph coarsened through
    /// about 390 levels, which shrank `K` by 0.75 each time until the drawing collapsed.
    #[test]
    fn a_stars_leaves_coarsen_four_at_a_time() {
        let edges: Vec<(u32, u32)> = (1..9).map(|leaf| (0, leaf)).collect();
        let level = pass(9, &edges);
        assert_eq!(level.coarse, 3, "pairs {:?}", level.pair);
        assert_eq!(&level.pair[1..5], &[0, 0, 0, 0]);
        assert_eq!(&level.pair[5..9], &[1, 1, 1, 1]);
        assert_eq!(level.pair[0], 2, "the hub has no unmatched neighbour left");
    }

    #[test]
    fn a_path_matches_neighbours_in_index_order() {
        let level = pass(4, &[(0, 1), (1, 2), (2, 3)]);
        assert_eq!(level.pair, vec![0, 0, 1, 1]);
    }

    #[test]
    fn isolated_nodes_form_one_module() {
        let level = pass(3, &[]);
        assert_eq!(level.pair, vec![0, 0, 0]);
        assert_eq!(level.coarse, 1);
    }
}
