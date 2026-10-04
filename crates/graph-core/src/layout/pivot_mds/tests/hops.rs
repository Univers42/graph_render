//! The pivot-selection cross-check: the in-place walks against a straightforward BFS
//! reference implementation, on a grid plus a seeded random tail so the graph has more than
//! one component.

use super::*;

/// The selection as first ported: one BFS per pivot over the whole graph, gathered through
/// `members`, the minimum kept as `f64` with each chosen pivot marked `-1`.
fn reference_hops(neighbors: &Neighbors, members: &[u32], k: usize) -> Vec<u32> {
    let n = members.len();
    let (mut hops, mut covered, mut chosen) = (Vec::new(), vec![f64::INFINITY; n], 0);
    for _ in 0..k {
        let mut dist = vec![u32::MAX; neighbors.len()];
        dist[members[chosen] as usize] = 0;
        let mut queue = std::collections::VecDeque::from([members[chosen]]);
        while let Some(v) = queue.pop_front() {
            for &w in neighbors.row(v) {
                if dist[w as usize] == u32::MAX {
                    dist[w as usize] = dist[v as usize] + 1;
                    queue.push_back(w);
                }
            }
        }
        for (i, &g) in members.iter().enumerate() {
            hops.push(dist[g as usize]);
            covered[i] = covered[i].min(f64::from(dist[g as usize]));
        }
        covered[chosen] = -1.0;
        chosen = (0..n).fold(
            0,
            |best, i| if covered[i] > covered[best] { i } else { best },
        );
    }
    hops
}

#[test]
fn in_place_walks_choose_the_reference_pivots_and_hops() {
    let (grid, n) = (9 * 13, 9 * 13 + 150);
    let mut pairs = grid_pairs(9, 13);
    let mut state = 7_u32;
    for i in 0..300 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        pairs.push((grid + i % 150, grid + (state >> 8) as usize % 150));
    }
    let neighbors = simple_neighbors(&topology(n, &pairs));
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let mut checked = 0;
    for members in components.iter().filter(|m| m.len() > 1) {
        let k = MAX_PIVOTS.min(members.len());
        let local = neighbors.component(members, &local_of);
        assert_eq!(
            pivot_hops(&local, k),
            reference_hops(&neighbors, members, k)
        );
        checked += 1;
    }
    assert!(
        checked >= 2,
        "the grid and the random part are separate components"
    );
}
