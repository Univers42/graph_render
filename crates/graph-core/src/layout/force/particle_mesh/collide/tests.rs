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

/// A crowd of 150 nodes inside one cell, more candidates than a window holds, around a
/// sparse ring.
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
    grid.build((x, y), CONTACT.reach);
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
    grid.build((&x, &y), CONTACT.reach);
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
