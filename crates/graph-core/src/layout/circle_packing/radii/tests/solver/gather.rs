//! The D10 claim: `angle_sums` is a gather over each vertex's own corners, in the corner
//! array's order, and that order is the one `np.bincount` accumulates in.

use super::{Flat, bits, scatter_angle_sums, seeded_flower, seeded_radii};
use crate::layout::circle_packing::radii::{angle_sums, solver_angle};

#[test]
fn the_angle_sum_gather_is_bit_identical_to_the_scatter_over_seeded_flowers() {
    for seed in 0..128u32 {
        // `+ 1` so even the first case has corners: `seeded_flower` draws
        // `3 * seed_index` of them, and a flower with none proves nothing.
        let flat = seeded_flower(0xB1C0_1DE5 ^ seed.wrapping_mul(0x9E37_79B9), seed + 1);
        let radii = seeded_radii(&flat, seed);
        assert!(
            !flat.corners.is_empty(),
            "seed {seed}: an empty flower proves nothing"
        );
        let gather = angle_sums(&flat.at, &radii);
        let scatter = scatter_angle_sums(&flat, &radii);
        assert_eq!(
            bits(&gather),
            bits(&scatter),
            "seed {seed}: the gather must reproduce the scatter exactly"
        );
    }
}

/// The same fold with each vertex's own corners taken in reverse — what a gather that
/// reversed within a vertex, or walked the flat array backwards, would produce.
fn reversed_angle_sums(at: &[Vec<(u32, u32)>], radii: &[f64]) -> Vec<f64> {
    (0..at.len())
        .map(|i| {
            let mut sum = 0.0;
            for &(l, r) in at[i].iter().rev() {
                sum += solver_angle(radii[i], radii[l as usize], radii[r as usize]);
            }
            sum
        })
        .collect()
}

#[test]
fn the_corner_order_is_load_bearing_and_the_gather_takes_the_array_order() {
    // A reversed fold is a different `f64` on some flowers, so this is a real constraint
    // on the kernel rather than a formality — and on the rest the two agree, which is why
    // it has to be *searched* for rather than assumed. Only the cases that separate are
    // interesting, and the gather must match the array order on every one of them.
    let mut separating = 0;
    for seed in 0..128u32 {
        let flat = seeded_flower(0x5A17_E5ED ^ seed.wrapping_mul(0x9E37_79B9), seed + 1);
        let radii = seeded_radii(&flat, seed);
        if !flat.at.iter().any(|f| f.len() > 1) {
            continue;
        }
        let gather = bits(&angle_sums(&flat.at, &radii));
        let reversed = bits(&reversed_angle_sums(&flat.at, &radii));
        if gather == reversed {
            continue;
        }
        separating += 1;
        // The array order is the reference's: `np.bincount` accumulates the corner array
        // front to back, so a vertex's terms land in flower order. The scatter is that
        // order, spelled out; the gather has to match it on every separating case.
        assert_eq!(
            gather,
            bits(&scatter_angle_sums(&flat, &radii)),
            "seed {seed}: the array order, not the reverse"
        );
    }
    assert!(
        separating >= 16,
        "only {separating} of 128 flowers separate the two fold orders"
    );
}

#[test]
fn a_gathered_angle_sum_is_that_vertices_own_corners_in_corner_array_order() {
    // Two vertices with two corners each, and the interleaving is what matters: the
    // gather may not regroup the array by centre, nor re-sort, nor fold in a different
    // direction. The two corners of vertex 1 have *different opposite sides*, so they
    // are not the same number (only swapping the two neighbours `j`/`k` of one corner
    // leaves it alone).
    let flat = Flat::from_flowers(vec![
        vec![(1, 2), (1, 2)],
        vec![(2, 0), (0, 3)],
        vec![],
        vec![],
    ]);
    let radii = [0.3, 1.7, 0.9, 2.4];
    let sums = angle_sums(&flat.at, &radii);
    // Vertex 0's two corners, added in its own flower order.
    let a0 = solver_angle(radii[0], radii[1], radii[2]);
    assert_eq!(sums[0].to_bits(), (a0 + a0).to_bits());
    // Vertex 1's are `acos` at different vertices, so not the same numbers.
    let a1a = solver_angle(radii[1], radii[2], radii[0]);
    let a1b = solver_angle(radii[1], radii[0], radii[3]);
    assert_ne!(a1a.to_bits(), a1b.to_bits(), "the two corners differ");
    assert_eq!(sums[1].to_bits(), (a1a + a1b).to_bits());
    // Vertices 2 and 3 carry none.
    assert_eq!(sums[2].to_bits(), 0.0_f64.to_bits());
    assert_eq!(sums[3].to_bits(), 0.0_f64.to_bits());
    assert_ne!(sums[0].to_bits(), sums[1].to_bits(), "the two fans differ");
}

#[test]
fn a_gather_leaves_a_cornerless_vertex_at_exactly_zero() {
    // D10 says element `i` writes only `out[i]`; a vertex with no corners has no terms,
    // so its slot is the `+0.0` it started as — not `-0.0`, a negation, a sign flip or an
    // uninitialised read.
    let flat = Flat::from_flowers(vec![vec![(1, 2)], vec![(2, 0)], vec![]]);
    let radii = [0.5, 0.25, 2.0];
    let sums = angle_sums(&flat.at, &radii);
    assert_eq!(sums[2].to_bits(), 0.0_f64.to_bits());
    assert_eq!(sums[0].to_bits(), solver_angle(0.5, 0.25, 2.0).to_bits());
    assert_eq!(sums[1].to_bits(), solver_angle(0.25, 2.0, 0.5).to_bits());
    // A flower of nothing at all: every slot exactly `+0.0`, and specifically not the
    // `-0.0` an accumulator seeded negative would leave.
    let empty = Flat::from_flowers(vec![vec![]; 4]);
    let sums = angle_sums(&empty.at, &[1.0; 4]);
    assert!(
        sums.iter().all(|&s| s.to_bits() == 0.0_f64.to_bits()),
        "{sums:?}"
    );
}
