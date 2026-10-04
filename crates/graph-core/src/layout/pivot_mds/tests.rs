//! `layout::pivot_mds::run` tests: a hand-verified closed-form embedding for the
//! smallest nontrivial case (`P_3`), **LF-11's hand-computed orientations for `C4` and
//! `C8`**, multi-component C12 reporting, and the expected pivot count including the
//! `k = 100` cap.
//!
//! The `P_3` case is worked by hand: with `n = 3`, `k = min(100, 3) = 3`, so every node
//! becomes a pivot. Farthest-point selection picks local pivots `0, 2, 1` in that order
//! (traced against `pivot_hops`' own tie rule); squaring and double-centering the
//! resulting `3x3` hop-distance matrix gives the exact Gram matrix
//! `[[2,-2,0],[-2,2,0],[0,0,0]]`, whose eigenvalues are `{4, 0, 0}`. The eigenvalue-4
//! eigenvector is simple and projects (up to sign) to `(sqrt2, 0, -sqrt2)`; **both**
//! eigenvalue-0 eigenvectors project to exactly `(0, 0, 0)` regardless of which
//! orthonormal basis the solver picks for that degenerate subspace — `Gv = 0` implies
//! `‖Bv‖² = vᵀBᵀBv = vᵀGv = 0`, so `Bv = 0` for *any* `v` in `G`'s null space, not just
//! the one the solver happens to return. So the result is fully determined: after
//! sign-pinning (index 0 wins the `|sqrt2|` tie) and peak-normalisation, node 1 lands
//! exactly at the origin, symmetric between the two endpoints.
//!
//! **`P_3` cannot see LF-11 and the file says so.** Every tie there is at the eigenvalue
//! `0`, and a null-space direction projects to zero however it is rotated — that is why the
//! header can claim "fully determined" and why `is_deterministic_run_twice` over a 6x10 grid
//! (the old negative control, LF-26) proved nothing: it compared two runs of a *pure*
//! function and the grid it chose has its ties in the null space too. LF-11 is about a tie at
//! a **non-zero** eigenvalue, where the projection is `Bv ≠ 0` and the rotation reaches the
//! output. `c4_and_c8_land_on_their_hand_computed_orientation` is that fixture.

use super::*;
use crate::index::{Topology, index_model};
use crate::linalg::dense_sym::eigh;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

fn topology(n: usize, pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| edge(&format!("e{k}"), &a.to_string(), &b.to_string()))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn path_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n - 1).map(|i| (i, i + 1)).collect()
}

fn cycle_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n).map(|i| (i, (i + 1) % n)).collect()
}

fn star_pairs(n: usize) -> Vec<(usize, usize)> {
    (1..n).map(|i| (0, i)).collect()
}

fn complete_pairs(n: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            v.push((i, j));
        }
    }
    v
}

fn grid_pairs(rows: usize, cols: usize) -> Vec<(usize, usize)> {
    let idx = |r: usize, c: usize| r * cols + c;
    let mut v = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                v.push((idx(r, c), idx(r, c + 1)));
            }
            if r + 1 < rows {
                v.push((idx(r, c), idx(r + 1, c)));
            }
        }
    }
    v
}

fn points(geometry: &Geometry) -> (&[f32], &[f32]) {
    match &geometry.nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("expected Point geometry, got {other:?}"),
    }
}

#[test]
fn a_three_node_path_lands_on_three_exactly_spaced_points() {
    let t = topology(3, &path_pairs(3));
    let (geometry, reports) = run(&t).expect("solves");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(reports[0].pivots, 3);

    let (x, y) = points(&geometry);
    let close = |got: f32, want: f32| (got - want).abs() < 1e-6;
    assert!(close(x[0], 1.0) && close(y[0], 0.0), "{:?}", (x[0], y[0]));
    assert!(close(x[1], 0.0) && close(y[1], 0.0), "{:?}", (x[1], y[1]));
    assert!(close(x[2], -1.0) && close(y[2], 0.0), "{:?}", (x[2], y[2]));
}

/// **LF-11's RED and GREEN, and LF-26's replacement for `is_deterministic_run_twice`.**
///
/// `C4` is the smallest fixture with a tie at a **non-zero** eigenvalue, so it is the only
/// kind that can see the defect. Worked by hand:
///
/// - `k = min(100, 4) = 4`, every node a pivot. Squared hop distances on a 4-cycle are
///   `[[0,1,4,1],[1,0,1,4],[4,1,0,1],[1,4,1,0]]`; every row and column sums to `6` and the
///   grand mean is `1.5`, so the centred matrix is `C = -0.5 (D² - 1.5)` entrywise.
/// - `C`'s eigenvectors are the 4-cycle's own — the constant `(1,1,1,1)`, the alternating
///   `(1,-1,1,-1)`, and the pair `(1,0,-1,0)`, `(0,1,0,-1)` — and the last two carry **equal**
///   eigenvalues. Measured through the module's own pipeline, `G = CᵀC`'s spectrum is
///   `{0, 1, 4, 4}`: **the two largest are equal**, so the 2-D arm's entire selection is one
///   tied group and its orientation was `tred2`/`tql2`'s to pick.
/// - `tied.rs`'s rule reads the **projector** `P` of that 2-plane, whose rows are
///   `P e_i / ‖P e_i‖` after Gram-Schmidt in ascending index. `P e_0 / ‖·‖ = (1,0,-1,0)/√2`
///   and `P e_1 / ‖·‖ = (0,1,0,-1)/√2`, both by the closed form above — so the canonical first
///   axis is `(1,0,-1,0)/√2` and the second `(0,1,0,-1)/√2`, and projecting the centred
///   distances onto them, sign-pinning and peak-normalising, puts the four nodes at
///   `(1,0), (0,1), (-1,0), (0,-1)`: **a square standing on its corner**, axis-aligned.
/// - What `tred2`/`tql2` returned instead, and what the rule exists to replace:
///   `(+0, +0, -0.7071, +0.7071)` and `(-0.7071, +0.7071, +0, +0)` — the same square, turned
///   by whatever angle the reduction happened to land on.
///
/// `C8` is the negative control for the *rule being present at all*: its tied pair sits at
/// `186.5097`, and the same closed form puts node 0 at `(1, 0)` with the other seven on the
/// `45°` steps of the unit circle. A no-op canonicalisation leaves `C8` at the solver's
/// rotation and fails this just as it fails `C4`.
#[test]
fn c4_and_c8_land_on_their_hand_computed_orientation() {
    for (n, want_lambda) in [(4usize, 4.0_f64), (8, 186.509_667_991_878_08)] {
        let t = topology(n, &cycle_pairs(n));
        let (geometry, reports) = run(&t).expect("one component");
        assert!(reports[0].solved);
        assert!(
            reports[0].peak_residual < 1e-9,
            "n={n}: the dense solve's own residual, {:?}",
            reports[0].peak_residual
        );
        let largest = largest_eigenvalue(n);
        assert!(
            (largest - want_lambda).abs() / want_lambda < 1e-12,
            "n={n}: the Gram eigenvalue the tie sits at, got {largest} want {want_lambda}"
        );
        let (x, y) = points(&geometry);
        for i in 0..n {
            // The regular `n`-gon the closed form gives, node `i` at angle `2*pi*i/n`.
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            let (wx, wy) = (libm::cos(angle), libm::sin(angle));
            assert!(
                (f64::from(x[i]) - wx).abs() < 1e-6 && (f64::from(y[i]) - wy).abs() < 1e-6,
                "n={n}: node {i} at ({}, {}), the canonical form is ({wx}, {wy})",
                x[i],
                y[i]
            );
        }
    }
}

/// The largest eigenvalue of `C_n`'s Gram matrix, through the module's own pipeline, so the
/// tie the pin above depends on is *measured* here rather than asserted in prose.
fn largest_eigenvalue(n: usize) -> f64 {
    let t = topology(n, &cycle_pairs(n));
    let neighbors = simple_neighbors(&t);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let members = &components[0];
    let component = neighbors.component(members, &local_of);
    let k = MAX_PIVOTS.min(n);
    let centered = Centered::new(pivot_hops(&component, k), n, k);
    eigh(&gram(&centered), k).values[k - 1]
}

/// The tie is only a defect if the *solver's* choice actually moves the drawing, so this is
/// the check that separates LF-11 from a no-op: a `C4` whose tied block is rotated by 30° must
/// produce the same canonical coordinates, because the rule reads the row space and the row
/// space does not see a rotation.
///
/// A canonicalisation that read the columns instead of the rows would move every coordinate
/// here and be caught; one that did nothing at all would pass the pins above and be caught by
/// the rotation they already exclude.
#[test]
fn a_rotated_tied_basis_gives_the_same_canonical_coordinates() {
    let t = topology(4, &cycle_pairs(4));
    let neighbors = simple_neighbors(&t);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, 4);
    let members = &components[0];
    let component = neighbors.component(members, &local_of);
    let centered = Centered::new(pivot_hops(&component, 4), 4, 4);
    let full = eigh(&gram(&centered), 4);
    let straight = top_eigenpairs(&full, 2);

    // 30 degrees, exactly representable in neither, so the rotation is a genuine one.
    let (sin, cos) = (0.5_f64, 0.866_025_403_784_438_6_f64);
    let mut rotated = straight.clone();
    for r in 0..4 {
        let (a, b) = (straight.column(0)[r], straight.column(1)[r]);
        rotated.column_mut(0)[r] = cos * a - sin * b;
        rotated.column_mut(1)[r] = sin * a + cos * b;
    }
    assert_ne!(
        rotated.vectors, straight.vectors,
        "the negative control: the rotation must actually change the input"
    );

    let mut a = project(&centered, &straight);
    let mut b = project(&centered, &rotated);
    canonicalise(&mut a);
    canonicalise(&mut b);
    for j in 0..2 {
        // Compared to a tolerance, not bit for bit: the projector is rotation-invariant
        // *exactly* in real arithmetic, and here it is built from rotated floats, so the two
        // `P`s agree to rounding rather than to the bit. A rule that read the columns would
        // disagree by O(1) and be caught by orders of magnitude, not by this epsilon.
        for i in 0..4 {
            assert!(
                (a.column(j)[i] - b.column(j)[i]).abs() < 1e-12,
                "column {j}, node {i}: {} vs {} — the canonical basis cannot depend on the \
                 rotation",
                a.column(j)[i],
                b.column(j)[i]
            );
        }
    }
}

/// The old negative control, kept as the plain purity statement it always was, with the
/// fixture `LF-26` names as the one that cannot see the defect: a `6x10` grid's ties are in
/// the Gram's null space, and a null direction projects to zero however it is rotated — which
/// is exactly why `run(&t) == run(&t)` held before and after LF-11, and why it is not the
/// pin that proves the rule.
#[test]
fn is_deterministic_run_twice() {
    let t = topology(60, &grid_pairs(6, 10));
    assert_eq!(run(&t), run(&t), "same input bits, same output bits");
}

#[test]
fn common_shapes_solve_with_the_expected_pivot_count() {
    for (n, pairs) in [
        (30usize, path_pairs(30)),
        (30, cycle_pairs(30)),
        (30, star_pairs(30)),
        (20, complete_pairs(20)),
    ] {
        let t = topology(n, &pairs);
        let (_, reports) = run(&t).expect("solves");
        assert_eq!(reports.len(), 1, "n={n}");
        assert!(reports[0].solved, "n={n}");
        assert_eq!(
            reports[0].pivots, n as u32,
            "n={n}: pivots = min(100, n) = n"
        );
    }
    // Past MAX_PIVOTS, the pivot count caps at 100 rather than following n.
    let big = topology(150, &cycle_pairs(150));
    let (_, reports) = run(&big).expect("solves");
    assert_eq!(reports[0].pivots, MAX_PIVOTS as u32);
}

#[test]
fn disconnected_graph_reports_only_attempted_components() {
    let mut nodes: Vec<_> = (0..5).map(|i| node(&format!("p{i}"), "")).collect();
    nodes.push(node("iso", ""));
    nodes.extend((0..4).map(|i| node(&format!("c{i}"), "")));
    let mut edges: Vec<_> = (0..4)
        .map(|i| edge(&format!("pe{i}"), &format!("p{i}"), &format!("p{}", i + 1)))
        .collect();
    edges.extend((0..4).map(|i| {
        edge(
            &format!("ce{i}"),
            &format!("c{i}"),
            &format!("c{}", (i + 1) % 4),
        )
    }));
    let t = index_model(&nodes, &edges).expect("fits");

    let (geometry, reports) = run(&t).expect("both size >= 2 components solve");
    assert_eq!(
        reports.len(),
        2,
        "the isolated node gets no report: nothing was attempted for it"
    );
    assert!(reports.iter().all(|r| r.solved));
    assert_eq!(
        reports.iter().map(|r| r.size).collect::<Vec<_>>(),
        vec![5, 4]
    );

    let (x, y) = points(&geometry);
    assert_eq!(x.len(), 10);
    assert!(x.iter().chain(y).all(|v| v.is_finite()));
}

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
