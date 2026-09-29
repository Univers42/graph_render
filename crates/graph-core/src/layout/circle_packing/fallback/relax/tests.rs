//! The springs and the edge keys, pinned by something sharper than "it still produces a
//! plausible circle": the edge-pull gather against the scatter it replaced, bit for bit
//! over seeded cases, and the key scheme that tells a graph edge from a near miss. The
//! per-pair forces are in [`forces`].
//!
//! [`forces`]: forces

mod forces;

use super::super::RelaxParams;
use super::{RelaxField, edge_key, edge_key_set, edge_pull, incidence_lists, relax, spring};
use crate::synthetic::Mulberry32;

pub(super) fn bits(forces: &[(f64, f64)]) -> Vec<u64> {
    forces
        .iter()
        .flat_map(|&(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

/// The scatter `edge_pull` had before D10: one loop over the edges, each writing into both
/// endpoints' accumulators. Test-only reference — the production path is the gather, and
/// this exists to prove the two agree bit for bit.
fn scatter_edge_pull(
    positions: &[(f64, f64)],
    field: &RelaxField,
    edges: &[(u32, u32)],
) -> Vec<(f64, f64)> {
    let mut forces = vec![(0.0, 0.0); field.n as usize];
    for &(u, v) in edges {
        let (dx, dy) = spring(positions, field, (u, v));
        forces[u as usize].0 += dx;
        forces[u as usize].1 += dy;
        forces[v as usize].0 -= dx;
        forces[v as usize].1 -= dy;
    }
    forces
}

/// A [`RelaxField`] over an already-built key set, the way `relax` holds it.
pub(super) fn field_for<'a>(
    keys: &'a [u64],
    n: u32,
    radii: &'a [f64],
    cutoff: f64,
) -> RelaxField<'a> {
    RelaxField {
        radii,
        edge_keys: keys,
        cutoff,
        n,
    }
}

/// Positions, radii and an edge list over a seeded stream, so a case is the same bits on
/// every run and every target (the same generator `crate::synthetic` draws the synthetic
/// models with).
fn seeded_case(seed: u32, n: u32, m: u32, duplicates: u32) -> Seeded {
    let mut rng = Mulberry32::new(seed);
    let positions: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.next_f64() * 4.0 - 2.0, rng.next_f64() * 4.0 - 2.0))
        .collect();
    let radii: Vec<f64> = (0..n).map(|_| rng.next_f64() * 0.5 + 0.05).collect();
    let mut edges = Vec::new();
    for _ in 0..m {
        let u = rng.pick(n as usize) as u32;
        let mut v = rng.pick(n as usize) as u32;
        if v == u {
            v = (v + 1) % n;
        }
        edges.push((u, v));
    }
    for _ in 0..duplicates {
        if !edges.is_empty() {
            edges.push(edges[rng.pick(edges.len())]);
        }
    }
    (positions, radii, edges)
}

/// Positions, radii and edges: one seeded case.
type Seeded = (Vec<(f64, f64)>, Vec<f64>, Vec<(u32, u32)>);

#[test]
fn the_edge_pull_gather_is_bit_identical_to_the_scatter_over_seeded_cases() {
    for seed in 0..64u32 {
        // Seven nodes, five edge slots: from case 3 on, some nodes carry no edge at all,
        // and their gathered force must be the exact `(0.0, 0.0)` the scatter left them
        // at. `duplicates` also puts a repeated edge in the list, so an incidence list
        // built by set or by sort instead of by edge order is caught.
        let (positions, radii, edges) = seeded_case(0x00ED_6E01 + seed, 7, 5, seed % 4);
        assert!(
            !edges.is_empty(),
            "seed {seed}: a case with no edges proves nothing"
        );
        let keys = edge_key_set(7, &edges);
        let field = field_for(&keys, 7, &radii, 1e9);
        assert_eq!(
            bits(&edge_pull(&positions, &field, &edges)),
            bits(&scatter_edge_pull(&positions, &field, &edges)),
            "seed {seed}: the gather must reproduce the scatter exactly"
        );
    }
}

#[test]
fn an_edge_pull_force_is_the_sum_of_that_nodes_edges_in_edge_order() {
    // A star plus a chord: node 1 is on three edges, and the order they are summed in is
    // the edge list's own order, not the node's.
    let positions = [
        (0.0, 0.0),
        (1.0, 0.5),
        (2.0, -1.0),
        (-1.5, 0.25),
        (0.75, 0.75),
    ];
    let radii = [0.3, 0.4, 0.5, 0.2, 0.6];
    let edges = [(0, 1), (0, 2), (1, 2), (1, 3)];
    let keys = edge_key_set(5, &edges);
    let field = field_for(&keys, 5, &radii, 1e9);
    let forces = edge_pull(&positions, &field, &edges);

    let incidence = incidence_lists(&edges, positions.len());
    let mut want = vec![(0.0, 0.0); positions.len()];
    for (i, incident) in incidence.iter().enumerate() {
        for &e in incident {
            let (dx, dy) = spring(&positions, &field, edges[e]);
            if edges[e].0 as usize == i {
                want[i].0 += dx;
                want[i].1 += dy;
            } else {
                want[i].0 -= dx;
                want[i].1 -= dy;
            }
        }
    }
    assert_eq!(bits(&forces), bits(&want));
    // The one node no edge touches keeps the exact zero it started at.
    assert_eq!(forces[4], (0.0, 0.0));
}

#[test]
fn an_edge_pull_is_the_same_magnitude_at_both_ends_and_opposite_in_sign() {
    // Newton's third law, which is what makes the `v -= pull` term the negation of the
    // `u += pull` one rather than a second computation.
    let positions = [(0.0, 0.0), (1.5, 0.5), (-0.5, 2.0)];
    let radii = [0.25, 0.4, 0.6];
    let edges = [(0, 1), (0, 2)];
    let keys = edge_key_set(3, &edges);
    let field = field_for(&keys, 3, &radii, 1e9);
    let forces = edge_pull(&positions, &field, &edges);
    let toward_1 = spring(&positions, &field, (0, 1));
    let toward_2 = spring(&positions, &field, (0, 2));
    assert_eq!(forces[0].0.to_bits(), (toward_1.0 + toward_2.0).to_bits());
    assert_eq!(forces[1].0.to_bits(), (-toward_1.0).to_bits());
    assert_eq!(forces[2].0.to_bits(), (-toward_2.0).to_bits());
}

#[test]
fn relax_leaves_a_pack_with_no_edges_exactly_where_it_started() {
    // With no edges there is no spring pull, and `overlap_and_repel` only moves a pair
    // that overlaps, so two well-separated circles must not move at all.
    let positions = vec![(-1.0, 0.0), (1.0, 0.0)];
    let radii = vec![0.25, 0.25];
    let p = RelaxParams {
        iterations: 5,
        scale: 5.0,
    };
    let (moved, _) = relax(positions.clone(), radii, &[], &p);
    assert_eq!(bits(&moved), bits(&positions));
}

#[test]
fn edge_keys_are_order_insensitive_and_collision_free_within_a_graph() {
    // `min(i, j) * n + max(i, j)` is a total order on unordered pairs, so the same pair
    // keys the same whichever way round the edge list spells it.
    assert_eq!(edge_key(6, 2, 5), edge_key(6, 5, 2));
    assert!(
        edge_key(6, 0, 1) < edge_key(6, 2, 5),
        "ascending pair order"
    );
    let edges = [(2, 5), (5, 2), (0, 3), (0, 3)];
    let keys = edge_key_set(6, &edges);
    assert_eq!(keys, vec![edge_key(6, 0, 3), edge_key(6, 2, 5)]);
    // A duplicate that is *not* adjacent in the list still goes: dedup only removes
    // neighbours, so the list has to be sorted before it is deduped, not after.
    let spread = edge_key_set(6, &[(0, 3), (2, 5), (0, 3), (5, 2)]);
    assert_eq!(
        spread, keys,
        "a non-adjacent duplicate is still removed: {spread:?}"
    );
    // Every edge of the list is still recognised, whichever way round it was written.
    let radii = [0.5; 6];
    let field = field_for(&keys, 6, &radii, 1e9);
    assert!(field.is_edge(5, 2) && field.is_edge(3, 0));
    assert!(!field.is_edge(0, 1) && !field.is_edge(4, 5));
}

#[test]
fn an_edge_key_does_not_collide_with_a_different_pair() {
    // `min * n + max` is injective on the pairs below `n`; a plain `min + max` is not, so
    // `(0, 3)` and `(1, 2)` would answer for each other and a graph with one of them
    // would behave as if it had the other. That is the whole reason the key multiplies.
    assert_ne!(edge_key(6, 0, 3), edge_key(6, 1, 2));
    assert_ne!(edge_key(6, 0, 5), edge_key(6, 5, 0) + 1);
    // And `is_edge` agrees with the edge list on the colliding case: an edge (0, 3) must
    // not make `(1, 2)` an edge too.
    let keys = edge_key_set(6, &[(0, 3)]);
    let field = field_for(&keys, 6, &[0.5; 6], 1e9);
    assert!(field.is_edge(0, 3) && field.is_edge(3, 0));
    assert!(!field.is_edge(1, 2) && !field.is_edge(2, 1));
    // A wider node range keeps the keys apart too, which the `n` factor is for.
    for n in [2u32, 3, 8, 64, 4096] {
        let mut seen: Vec<u64> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                seen.push(edge_key(n, i, j));
            }
        }
        seen.sort_unstable();
        assert!(
            seen.windows(2).all(|w| w[0] != w[1]),
            "n = {n}: keys collide"
        );
    }
}
