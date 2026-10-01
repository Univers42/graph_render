//! Entry points that let another layout drive the Barnes-Hut simulation over its own
//! graph and start positions (the multilevel `yifan_hu` reuses it per level), so there
//! is one tick implementation, not two.
//!
//! [`Tier`] is the seam that made the multilevel layout a *tier*. Before it, `settle`
//! hard-coded `&Serial, 1` and every level of `yifan_hu` ran on one thread whatever the
//! host offered, even though each of its ticks already hands three gathered passes to a
//! runner — the layout was paying for the partition and not being allowed to use it.

use super::Split;
use super::seed::golden_spiral;
use super::sim::{How, Sim};
use crate::layout::force::SimpleGraph;
use crate::layout::force::params::ForceParams;

/// The tier a [`settle`] runs under: who runs the tick's three gathered passes, over how
/// many workers, and which merge the negative control splits.
///
/// The three fields [`sim::How`] already carries per tick, lifted to the level of a whole
/// level's settle — so a caller that owns hundreds of ticks hands the same choice down
/// once instead of rebuilding it per tick, and cannot pair one tick's runner with another
/// tick's control by accident.
///
/// The `Clone`/`Copy` impls are hand-written because the derived ones would put an `R:
/// Clone` bound on the struct, and `R` is the runner itself — a type the caller owns, not
/// one `Tier` stores. `Tier` stores only `&'a R`, so a copy of a `Tier` is a copy of a
/// borrow, which is what lets a `multilevel` run hand the same tier to every level.
pub(crate) struct Tier<'a, R: crate::exec::Runner> {
    /// Who runs the ranges.
    pub(crate) runner: &'a R,
    /// How many workers it may use; below 2 is [`Serial`]'s own serial path.
    pub(crate) workers: u32,
    /// The negative control: which merge, if any, reads a neighbouring node's delta.
    pub(crate) split: Split,
}

impl<R: crate::exec::Runner> Clone for Tier<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: crate::exec::Runner> Copy for Tier<'_, R> {}

/// The engine's golden-angle start positions for `n` nodes.
pub(crate) fn golden_seed(n: u32) -> (Vec<f64>, Vec<f64>) {
    golden_spiral(n)
}

/// Runs `ticks` ticks over `graph` from `start`, with `alpha` as the initial heat, under
/// `tier`.
///
/// The tick loop, the pass order and the `alpha` decay are the same straight-line code in
/// every tier, so `tier` decides only how the three gathered passes are divided. The
/// scratch buffer is allocated here, once for the whole settle rather than per tick.
pub(crate) fn settle<R: crate::exec::Runner>(
    graph: SimpleGraph,
    params: ForceParams,
    start: (Vec<f64>, Vec<f64>),
    (ticks, alpha): (u32, f64),
    tier: Tier<'_, R>,
) -> (Vec<f64>, Vec<f64>) {
    let mut sim = Sim::from_parts(graph, params.into(), 0, start);
    sim.alpha = alpha;
    let mut deltas: Vec<(f64, f64)> = Vec::new();
    for _ in 0..ticks {
        let mut how = How {
            runner: tier.runner,
            workers: tier.workers,
            deltas: &mut deltas,
            split: tier.split,
        };
        sim.tick(&mut how);
    }
    (sim.x, sim.y)
}
