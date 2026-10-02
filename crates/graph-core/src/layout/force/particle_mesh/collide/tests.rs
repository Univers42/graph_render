use super::*;

const CONTACT: Contact = Contact {
    d2: 32.0 * 32.0,
    reach: 32.0,
    seed: 0,
    tick: 3,
};

/// Clumps of overlapping nodes, two exact duplicates, a far outlier and a NaN.
fn positions() -> (Vec<f64>, Vec<f64>) {
    let mut x: Vec<f64> = (0..300)
        .map(|i| libm::sin(i as f64 * 1.7) * 400.0)
        .collect();
    let mut y: Vec<f64> = (0..300)
        .map(|i| libm::cos(i as f64 * 0.9) * 250.0)
        .collect();
    (x[7], y[7]) = (x[3], y[3]);
    (x[8], y[8]) = (x[3] + 5.0, y[3]);
    (x[9], y[9]) = (1e9, -1e9);
    (x[10], y[10]) = (f64::NAN, 0.0);
    (x, y)
}

/// Every pair, no grid: the deltas the grid's gather must reproduce.
fn every_pair(x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    let ids = 0..x.len() as u32;
    ids.clone()
        .map(|i| {
            let mut out = (0.0, 0.0);
            for j in ids.clone().filter(|&j| j != i) {
                let (i, j) = (i as usize, j as usize);
                let offset = (x[i] - x[j], y[i] - y[j]);
                resolve(CONTACT, || (i as u32, j as u32), offset, &mut out);
            }
            out
        })
        .collect()
}

fn gathered(grid: &Grid, workers: u32) -> Vec<(f64, f64)> {
    let mut sorted = Vec::new();
    let gather = Gather {
        grid,
        contact: CONTACT,
    };
    crate::exec::Serial.run(&gather, workers, &mut sorted);
    let mut by_node = vec![(f64::NAN, f64::NAN); sorted.len()];
    for (k, &d) in sorted.iter().enumerate() {
        by_node[grid.order[k] as usize] = d;
    }
    by_node
}

/// A crowded cell: 150 nodes inside one cell, around a sparse ring.
fn crowd() -> (Vec<f64>, Vec<f64>) {
    let (mut x, mut y) = positions();
    x.truncate(40);
    y.truncate(40);
    for i in 0..150 {
        x.push(1000.0 + (i % 13) as f64 * 0.7);
        y.push(1000.0 + (i / 13) as f64 * 0.9);
    }
    (x, y)
}

#[test]
fn the_grid_finds_every_overlap_the_pairwise_scan_finds() {
    for (x, y) in [positions(), crowd()] {
        matches_every_pair(&x, &y);
    }
}

fn matches_every_pair(x: &[f64], y: &[f64]) {
    let mut grid = Grid::new(x.len() as u32);
    grid.build((x, y), CONTACT.reach, (&crate::exec::Serial, 1));
    let want = every_pair(x, y);
    assert!(
        want.iter().filter(|d| d.0 != 0.0).count() > 20,
        "too few overlaps to test"
    );
    for (i, (got, want)) in gathered(&grid, 1).iter().zip(&want).enumerate() {
        let gap = f64::max((got.0 - want.0).abs(), (got.1 - want.1).abs());
        assert!(
            gap <= 1e-9 * (1.0 + want.0.abs() + want.1.abs()),
            "node {i}: {got:?} vs {want:?}"
        );
    }
}

#[test]
fn the_sort_is_a_stable_permutation_and_the_ranges_change_no_byte() {
    let (x, y) = positions();
    let mut grid = Grid::new(x.len() as u32);
    grid.build((&x, &y), CONTACT.reach, (&crate::exec::Serial, 1));
    let mut seen = grid.order.clone();
    seen.sort_unstable();
    assert!(seen.iter().copied().eq(0..x.len() as u32));
    for b in 0..grid.start.len() - 1 {
        let slots = &grid.order[grid.start[b] as usize..grid.start[b + 1] as usize];
        assert!(
            slots.windows(2).all(|w| w[0] < w[1]),
            "bucket {b} is not stable"
        );
    }
    let serial = gathered(&grid, 1);
    for workers in [2, 3, 7, 64] {
        assert!(gathered(&grid, workers) == serial, "workers={workers}");
    }
}

/// Every column of two builds, the positions bit for bit.
fn assert_same_build(grid: &Grid, one: &Grid, what: &str) {
    assert_eq!(grid.order, one.order, "{what}");
    assert_eq!(grid.start, one.start, "{what}");
    assert_eq!(grid.slot, one.slot, "{what}");
    let bits = |g: &Grid| {
        g.at.iter()
            .flatten()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    };
    assert!(bits(grid) == bits(one), "{what}");
    assert_eq!(grid.hash.origin, one.hash.origin, "{what}");
}

/// A second build after a move starts from the first build's columns, so it must not depend on
/// them; 100 workers is more than the model has blocks.
#[test]
fn every_division_of_the_build_is_the_one_thread_build() {
    let n = 2 * frame::BLOCK + 300;
    let x: Vec<f64> = (0..n).map(|i| libm::sin(i as f64 * 0.37) * 900.0).collect();
    let mut y: Vec<f64> = (0..n).map(|i| libm::cos(i as f64 * 0.11) * 400.0).collect();
    (y[5], y[frame::BLOCK as usize + 1]) = (f64::NAN, f64::NEG_INFINITY);
    let moved: Vec<f64> = (0..n as usize)
        .map(|i| x[i] + libm::sin(i as f64) * 70.0)
        .collect();
    let built = |xy: (&[f64], &[f64]), workers| {
        let mut grid = Grid::new(n);
        grid.build(xy, CONTACT.reach, (&crate::exec::Serial, workers));
        grid
    };
    let (one, one_moved) = (built((&x, &y), 1), built((&moved, &y), 1));
    for (k, &i) in one.order.iter().enumerate() {
        assert_eq!(one.slot[i as usize], k as u32, "slot is order's inverse");
    }
    assert_ne!(
        one.order, one_moved.order,
        "the move must reorder, or the rebuild is vacuous"
    );
    for workers in [2, 3, 7, 8, 64, 100] {
        let mut grid = built((&x, &y), workers);
        assert_same_build(&grid, &one, &format!("workers={workers}"));
        grid.build((&moved, &y), CONTACT.reach, (&crate::exec::Serial, workers));
        assert_same_build(&grid, &one_moved, &format!("rebuilt, workers={workers}"));
    }
}

/// The gather before the overlap filter: one branch per candidate.
fn branched(grid: &Grid, k: usize) -> (f64, f64) {
    let reads = grid.reads(grid.hash.cell_of((grid.at[k][0], grid.at[k][1])));
    let [px, py] = grid.at[k];
    let mut out = (0.0, 0.0);
    for &(lo, hi) in &reads.runs[..reads.len] {
        let lo = lo as usize;
        for (q, &[qx, qy]) in (lo..).zip(&grid.at[lo..hi as usize]) {
            if q != k {
                let ids = || (grid.order[k], grid.order[q]);
                resolve(CONTACT, ids, (px - qx, py - qy), &mut out);
            }
        }
    }
    out
}

#[test]
fn the_filtered_gather_is_the_branched_one_bit_for_bit() {
    let mut longest = 0;
    for (x, y) in [positions(), crowd()] {
        let mut grid = Grid::new(x.len() as u32);
        grid.build((&x, &y), CONTACT.reach, (&crate::exec::Serial, 1));
        let mut sorted = Vec::new();
        let gather = Gather {
            grid: &grid,
            contact: CONTACT,
        };
        crate::exec::Serial.run(&gather, 1, &mut sorted);
        let runs = (1..grid.start.len()).map(|b| grid.start[b] - grid.start[b - 1]);
        longest = longest.max(runs.max().unwrap_or(0));
        for (k, got) in sorted.iter().enumerate() {
            let want = branched(&grid, k);
            assert_eq!(
                (got.0.to_bits(), got.1.to_bits()),
                (want.0.to_bits(), want.1.to_bits()),
                "slot {k}"
            );
        }
    }
    assert!(longest > HITS as u32, "no bucket spans two batches");
}
