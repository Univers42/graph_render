//! The gather/scatter equivalence proof for [`super::refine_tangency`] (D10) and the
//! boundary behaviour the gather must keep: a pair too close to have a direction
//! contributes nothing, and the loop stops on the reference's own two conditions. The
//! placement walk's own cases are in [`walk`].
//!
//! [`walk`]: walk

mod walk;

use super::{
    LEARNING_RATE, apply_gradients, edge_pull, gather_gradients, incidence_lists, refine_tangency,
};

/// The scatter `refine_tangency` had before D10: one loop over the edges, each writing
/// into both endpoints' accumulators. Test-only reference — the production path is the
/// gather, and this exists to prove the two agree bit for bit.
fn scatter_refine(
    mut positions: Vec<(f64, f64)>,
    radii: &[f64],
    edges: &[(u32, u32)],
    iterations: u32,
) -> Vec<(f64, f64)> {
    if edges.is_empty() {
        return positions;
    }
    let target: Vec<f64> = edges
        .iter()
        .map(|&(u, v)| radii[u as usize] + radii[v as usize])
        .collect();
    for _ in 0..iterations {
        let mut gradients = vec![(0.0, 0.0); positions.len()];
        let mut worst = 0.0_f64;
        for (i, &edge) in edges.iter().enumerate() {
            let Some(((sx, sy), error)) = edge_pull(&positions, edge, target[i]) else {
                continue;
            };
            let (u, v) = edge;
            gradients[u as usize].0 += sx;
            gradients[u as usize].1 += sy;
            gradients[v as usize].0 -= sx;
            gradients[v as usize].1 -= sy;
            worst = worst.max(error);
        }
        if worst < 1e-9 || !apply_gradients(&mut positions, &gradients) {
            break;
        }
    }
    positions
}

pub(super) fn bits(positions: &[(f64, f64)]) -> Vec<u64> {
    positions
        .iter()
        .flat_map(|&(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

/// mulberry32, the same generator `crate::synthetic` uses: a seeded, target-independent
/// stream, so these cases are the same bits on every run and every target.
struct Rng(u32);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        t ^ (t >> 14)
    }

    fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4_294_967_296.0
    }
}

/// One seeded case: `n` nodes at seeded positions, seeded radii, `m` edge slots, and
/// `duplicates` extra copies of edges already in the list.
type Case = (Vec<(f64, f64)>, Vec<f64>, Vec<(u32, u32)>);

fn seeded_case(seed: u32, n: u32, m: u32, duplicates: u32) -> Case {
    let mut rng = Rng(seed);
    let positions: Vec<(f64, f64)> = (0..n)
        .map(|_| (rng.next_f64() * 4.0 - 2.0, rng.next_f64() * 4.0 - 2.0))
        .collect();
    let radii: Vec<f64> = (0..n).map(|_| rng.next_f64() * 2.0 + 0.1).collect();
    let mut edges = Vec::new();
    for _ in 0..m {
        let u = rng.next_u32() % n;
        let mut v = rng.next_u32() % n;
        if v == u {
            v = (v + 1) % n;
        }
        edges.push((u, v));
    }
    for _ in 0..duplicates {
        if !edges.is_empty() {
            edges.push(edges[(rng.next_u32() as usize) % edges.len()]);
        }
    }
    (positions, radii, edges)
}

#[test]
fn the_gather_is_bit_identical_to_the_scatter_over_seeded_random_cases() {
    for seed in 0..64u32 {
        // Seven nodes with five edge slots: from case 3 on, some nodes carry no edge at
        // all, and their gathered gradient must be exactly the `(0.0, 0.0)` the scatter
        // left them at. `duplicates` also puts a repeated edge in the list, so an
        // incidence list built by set or by sort instead of by edge order is caught.
        let (positions, radii, edges) = seeded_case(0x00C1_C5E0 + seed, 7, 5, seed % 4);
        assert!(
            !edges.is_empty(),
            "seed {seed}: a case with no edges proves nothing"
        );
        let gather = refine_tangency(positions.clone(), &radii, &edges, 8);
        let scatter = scatter_refine(positions, &radii, &edges, 8);
        assert_eq!(
            bits(&gather),
            bits(&scatter),
            "seed {seed}: the gather must reproduce the scatter exactly"
        );
    }
}

#[test]
fn a_gathered_gradient_is_the_sum_of_that_nodes_edges_in_edge_order() {
    // A star plus a chord: node 1 is on three edges, and the order they are summed in is
    // the edge list's own order, not the node's.
    let positions = vec![(0.0, 0.0), (1.0, 0.5), (2.0, -1.0), (-1.5, 0.25)];
    let radii = [0.4; 4];
    let edges = [(0, 1), (0, 2), (1, 2), (1, 3)];
    let target: Vec<f64> = edges
        .iter()
        .map(|&(u, v)| radii[u as usize] + radii[v as usize])
        .collect();
    let incidence = incidence_lists(&edges, positions.len());
    let (pulls, _) = super::edge_pulls(&positions, &edges, &target);
    let gradients = gather_gradients(&pulls, &incidence, &edges);

    // By hand, from the same pulls in the same order.
    let mut want = [(0.0, 0.0); 4];
    for (i, incident) in incidence.iter().enumerate() {
        for &e in incident {
            let (sx, sy) = pulls[e].expect("no coincident pair in this case");
            if edges[e].0 as usize == i {
                want[i].0 += sx;
                want[i].1 += sy;
            } else {
                want[i].0 -= sx;
                want[i].1 -= sy;
            }
        }
    }
    let got: Vec<u64> = gradients
        .iter()
        .flat_map(|&(x, y)| [x, y])
        .map(f64::to_bits)
        .collect();
    let expected: Vec<u64> = want
        .iter()
        .flat_map(|&(x, y)| [x, y])
        .map(f64::to_bits)
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn a_coincident_pair_contributes_no_pull_and_no_error() {
    // `edge_pull` returns `None` at or below 1e-12, so a node pair that has collapsed onto
    // one point neither moves nor is counted in the worst error.
    let at = (0.0, 0.0);
    assert!(edge_pull(&[at, at], (0, 1), 0.5).is_none(), "coincident");
    assert!(
        edge_pull(&[at, (1e-12, 0.0)], (0, 1), 0.5).is_none(),
        "exactly at the 1e-12 guard"
    );
    assert!(
        edge_pull(&[at, (2e-12, 0.0)], (0, 1), 0.5).is_some(),
        "past the guard"
    );
}

#[test]
fn refine_tangency_stops_on_a_negligible_step_norm() {
    // Two circles already tangent: the error is 0, so the loop breaks before applying.
    let positions = vec![(0.0, 0.0), (1.0, 0.0)];
    let radii = [0.5, 0.5];
    assert_eq!(
        bits(&refine_tangency(positions.clone(), &radii, &[(0, 1)], 5)),
        bits(&positions),
        "an already-tangent packing is left alone"
    );
}

#[test]
fn refine_tangency_pulls_an_over_long_pair_back_together() {
    let positions = vec![(0.0, 0.0), (4.0, 0.0)];
    let radii = [0.5, 0.5];
    let refined = refine_tangency(positions, &radii, &[(0, 1)], 200);
    let (dx, dy) = (refined[0].0 - refined[1].0, refined[0].1 - refined[1].1);
    let dist = libm::hypot(dx, dy);
    assert!(
        (dist - 1.0).abs() < 1e-6,
        "tangency error {} after refinement",
        dist - 1.0
    );
    // The pair closes from both ends by the same amount, so the midpoint holds: `u`'s
    // gathered gradient is the negation of `v`'s, so neither end outruns the other.
    let mid = (refined[0].0 + refined[1].0) / 2.0;
    assert!((mid - 2.0).abs() < 1e-9, "midpoint drifted to {mid}");
}

#[test]
fn refine_tangency_leaves_an_empty_edge_list_alone() {
    let positions = vec![(1.0, 2.0), (3.0, 4.0)];
    assert_eq!(
        bits(&refine_tangency(positions.clone(), &[1.0, 1.0], &[], 5)),
        bits(&positions)
    );
}

#[test]
fn apply_gradients_scales_by_the_learning_rate_over_the_damped_norm() {
    let mut positions = [(0.0, 0.0), (0.0, 0.0)];
    let gradients = [(3.0, 4.0), (0.0, 0.0)];
    assert!(apply_gradients(&mut positions, &gradients));
    // norm 5, so scale = LEARNING_RATE / (1 + 0.1 * 5) and the move is that times (3, 4).
    let want = LEARNING_RATE / (1.0 + 0.1 * 5.0);
    assert_eq!(positions[0].0.to_bits(), (-want * 3.0).to_bits());
    assert_eq!(positions[0].1.to_bits(), (-want * 4.0).to_bits());
    assert_eq!(positions[1].0.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn apply_gradients_refuses_a_step_whose_whole_norm_is_negligible() {
    let mut positions = [(1.0, 1.0), (2.0, 2.0)];
    assert!(!apply_gradients(&mut positions, &[(0.0, 0.0), (0.0, 0.0)]));
    assert_eq!(positions[0].0, 1.0);
    assert_eq!(positions[1].1, 2.0);
}
