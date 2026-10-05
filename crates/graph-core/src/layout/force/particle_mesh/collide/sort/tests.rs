//! The coherent sort against a fresh build, tick after tick.
//!
//! [`Grid::build`](super::Grid::build) reads the previous tick's `order` and `slot` to
//! walk the counting sort coherently, so a rebuild is no longer independent of the state
//! it started from. The contract it must keep is that it is not: after every rebuild,
//! `order`, `start`, `slot` and `at` are the ones a fresh `Grid::new` over the same
//! positions produces. This walks twenty rebuilds of one grid over a moving layout and
//! compares each against a fresh grid, at three worker counts.

use super::*;
use crate::layout::force::particle_mesh::frame;
use crate::rng::Mulberry32;

/// The collide diameter these tests build over: the same 32 the sibling tests use, so the
/// cells are one collide diameter wide as they are in a tick.
const REACH: f64 = 32.0;

/// A walk over `STEPS` ticks of `n` nodes. Every node moves about half a cell a tick, so
/// most buckets keep their nodes and a few change; the first [`JUMPERS`] move far enough
/// to cross the frame, [`CROWD`] of them sit inside one cell so a bucket goes over
/// [`sort::INSERTION`](super::sort::INSERTION) entries, and one position is a NaN and one
/// an infinity. Halfway through, the walk grows to `n + EXTRA` nodes, which is what a live
/// session does.
fn walk(n: u32, seed: u32) -> Walk {
    let mut rng = Mulberry32::new(seed);
    let xy = scatter(n, &mut rng);
    let grid = Grid::new(n);
    Walk {
        xy,
        grid,
        rng,
        step: 0,
    }
}

/// The nodes a walk starts from: a cloud over the frame, with the crowd, the jumpers, the
/// NaN and the infinity folded in.
fn scatter(n: u32, rng: &mut Mulberry32) -> (Vec<f64>, Vec<f64>) {
    let mut x: Vec<f64> = (0..n).map(|_| rng.next_f64() * 4000.0 - 2000.0).collect();
    let mut y: Vec<f64> = (0..n).map(|_| rng.next_f64() * 4000.0 - 2000.0).collect();
    let at = |i: u32| i as usize % n as usize;
    for i in 0..JUMPERS {
        let i = at(i);
        (x[i], y[i]) = (9.0e8, -9.0e8);
    }
    for i in JUMPERS..JUMPERS + CROWD {
        let i = at(i);
        (x[i], y[i]) = (700.0 + (i % 5) as f64 * 0.3, -400.0 + (i % 3) as f64 * 0.4);
    }
    let (nan, inf) = (at(CROWD + JUMPERS), at(CROWD + JUMPERS + 1));
    (x[nan], y[nan]) = (f64::NAN, 1.0);
    (x[inf], y[inf]) = (1.0, f64::INFINITY);
    (x, y)
}

/// Nodes that jump across the frame on the first tick.
const JUMPERS: u32 = 5;
/// Nodes inside one cell, so one bucket holds more than the insertion bound.
const CROWD: u32 = 100;
/// Ticks walked before the layout grows.
const STEPS: u32 = 20;
/// The nodes the walk grows by, halfway.
const EXTRA: u32 = 300;
/// The seed the walk's generator starts from.
const SEED: u32 = 20_251_005;

/// One grid over a moving layout, one rebuild at a time.
struct Walk {
    xy: (Vec<f64>, Vec<f64>),
    grid: Grid,
    rng: Mulberry32,
    step: u32,
}

impl Walk {
    /// Moves every node about half a cell, jumps the first [`JUMPERS`] across the frame
    /// and grows the layout on the tenth tick, then rebuilds at `workers`.
    fn rebuild(&mut self, workers: u32) {
        if self.step == STEPS / 2 {
            let grown = self.xy.0.len() as u32 + EXTRA;
            self.grid.grow(grown);
            self.xy = scatter(grown, &mut self.rng);
        }
        let (x, y) = &mut self.xy;
        let n = x.len();
        let half = REACH * 0.5;
        for i in 0..n {
            x[i] += (self.rng.next_f64() - 0.5) * half;
            y[i] += (self.rng.next_f64() - 0.5) * half;
        }
        for i in 0..JUMPERS as usize {
            x[i] = 8.0e8 - x[i];
            y[i] = -8.0e8 - y[i];
        }
        self.step += 1;
        self.grid.build(
            (&self.xy.0, &self.xy.1),
            REACH,
            (&crate::exec::Serial, workers),
        );
    }
}

/// Every column's bits, so a `-0.0` and a `+0.0` do not compare equal.
fn columns(grid: &Grid) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u64>) {
    let at = grid
        .at
        .iter()
        .flatten()
        .map(|v| v.to_bits())
        .collect::<Vec<_>>();
    (
        grid.order.clone(),
        grid.start.clone(),
        grid.slot.clone(),
        at,
    )
}

/// The coherent rebuild of one grid is the fresh build of the same positions, at every
/// worker count, on every tick of a moving layout that grows halfway through.
#[test]
fn the_coherent_build_is_the_fresh_build_tick_after_tick() {
    for workers in [1, 3, 8] {
        let mut walk = walk(2 * frame::BLOCK + 700, SEED);
        for tick in 0..STEPS {
            walk.rebuild(workers);
            let mut fresh = Grid::new(walk.xy.0.len() as u32);
            fresh.build(
                (&walk.xy.0, &walk.xy.1),
                REACH,
                (&crate::exec::Serial, workers),
            );
            assert_eq!(
                columns(&walk.grid),
                columns(&fresh),
                "tick {tick}, workers={workers}"
            );
        }
    }
}

/// The comparison above is not vacuous. The first build starts from the identity, which is
/// today's algorithm; then the order is a real permutation, it changes from tick to tick as
/// the moves cross cells, and each of those rebuilds really did start from the previous
/// order. Without that, the coherent sort's whole input would be the identity.
#[test]
fn the_walk_reorders_so_the_comparison_is_not_vacuous() {
    let mut walk = walk(2 * frame::BLOCK + 700, SEED);
    let identity: Vec<u32> = (0..walk.xy.0.len() as u32).collect();
    let mut previous: Option<Vec<u32>> = None;
    let mut changed = 0;
    for tick in 0..STEPS {
        walk.rebuild(1);
        let order = walk.grid.order.clone();
        assert_ne!(order, identity, "tick {tick}: the order is the identity");
        if let Some(seen) = &previous {
            changed += u32::from(&order != seen);
        }
        previous = Some(order);
    }
    assert!(changed > 0, "the moves never changed the order");
}

/// The coherent sort's own columns: `order` stays a permutation, each bucket's run stays
/// ascending by node index, and `slot` stays `order`'s inverse, after a rebuild that
/// started from the previous tick's order rather than the identity.
#[test]
fn a_rebuild_from_the_last_order_keeps_the_contract() {
    let mut walk = walk(2 * frame::BLOCK + 700, SEED);
    for tick in 0..4 {
        walk.rebuild(4);
        let grid = &walk.grid;
        let mut seen = grid.order.clone();
        seen.sort_unstable();
        assert!(
            seen.iter().copied().eq(0..grid.order.len() as u32),
            "tick {tick}"
        );
        for (k, &i) in grid.order.iter().enumerate() {
            assert_eq!(
                grid.slot[i as usize], k as u32,
                "tick {tick}: slot is the inverse"
            );
        }
        for b in 0..grid.start.len() - 1 {
            let run = &grid.order[grid.start[b] as usize..grid.start[b + 1] as usize];
            assert!(
                run.windows(2).all(|w| w[0] < w[1]),
                "tick {tick}: bucket {b}"
            );
        }
    }
}

/// One worker and many must agree, on the coherent walk: a range-kernel contract that the
/// sort's own passes keep.
#[test]
fn every_division_of_the_coherent_build_is_the_one_thread_build() {
    let mut one = walk(2 * frame::BLOCK + 700, SEED);
    let mut many = walk(2 * frame::BLOCK + 700, SEED);
    for tick in 0..6 {
        one.rebuild(1);
        many.rebuild(7);
        assert_eq!(columns(&one.grid), columns(&many.grid), "tick {tick}");
    }
}

/// The sort's insertion bound is where the design says it is: a run longer than it goes to
/// `sort_unstable`, a shorter one to insertion, and both must leave the run ascending. A
/// reversed run is the failing direction for the insertion path's quadratic worst case.
#[test]
fn the_insertion_bound_sorts_a_reversed_run_ascending() {
    let keys: Vec<u64> = (0..70u64).map(|i| (69 - i) << 32 | i).collect();
    for len in [0usize, 1, 2, 31, 32, 33, 64] {
        let mut run = keys[..len].to_vec();
        sort_run(&mut run);
        assert!(run.windows(2).all(|w| w[0] < w[1]), "len={len}: {run:?}");
    }
}
