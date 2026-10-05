//! The velocity merges, the centering shift, the collide projection and the integrate as
//! range passes, so the workers share them. Each is `step::merge`'s, `Sim::center`'s or
//! `Sim::integrate`'s expression for one node and one axis, evaluated in the same order, so
//! the bytes are theirs. The centering mean stays one thread's fold (D3).
//!
//! `step::merge` scatters slot `k`'s delta onto node `order[k]`; [`Velocity`] gathers it
//! from node `i`'s side, slot `slot[i]`, so each node writes only its own output. A pass
//! writes into `px`/`py`, which hold nothing live outside the collide sort, and the result
//! is swapped in.
//!
//! Caveat: the swap moves `x`, `y`, `vx` and `vy` between two allocations every tick. No
//! caller keeps an address across ticks today; a session that exported one (graph-wasm's
//! C7) would need a copy back instead of the swap.

use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::sim::Sim;
use std::mem;
use std::ops::Range;

/// One axis of a delta.
type Axis = fn((f64, f64)) -> f64;

const AXES: [Axis; 2] = [|d| d.0, |d| d.1];

/// A gathered pass's deltas in slot order, and each node's slot: `None` when the deltas
/// are in node order, as link's are.
#[derive(Clone, Copy)]
pub(super) struct Gathered<'a> {
    pub(super) deltas: &'a [(f64, f64)],
    pub(super) slot: Option<&'a [u32]>,
    /// `step::merge`'s negative control: add the next slot's delta too.
    pub(super) split: bool,
}

/// One axis's velocities after a merge, then the integrate's decay when `decay` is set.
struct Velocity<'a> {
    v: &'a [f64],
    merged: Option<Gathered<'a>>,
    /// The second of a fused pair of merges, added after the first.
    also: Option<Gathered<'a>>,
    axis: Axis,
    /// The decay and the axis's pins: a pinned node's velocity is zeroed.
    decay: Option<(f64, &'a [Option<f64>])>,
}

impl StepRange for Velocity<'_> {
    type Out = f64;

    fn len(&self) -> u32 {
        self.v.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f64]) {
        let axis = self.axis;
        for (out, i) in out.iter_mut().zip(range.start as usize..) {
            let mut v = self.v[i];
            if let Some(g) = self.merged {
                v += added(g, i, axis);
            }
            if let Some(g) = self.also {
                v += added(g, i, axis);
            }
            if let Some((decay, pins)) = self.decay {
                v = if pins[i].is_some() { 0.0 } else { v * decay };
            }
            *out = v;
        }
    }
}

/// One gathered pass's contribution to node `i`: `delta + stolen` as a whole, the grouping
/// `step::merge` adds it in. Each pass keeps its own, and a fused pair adds them in the
/// order the tick did them, so the two sums are the same `f64`.
fn added(g: Gathered<'_>, i: usize, axis: Axis) -> f64 {
    let k = g.slot.map_or(i, |slot| slot[i] as usize);
    let stolen = if g.split {
        g.deltas.get(k + 1).copied().map_or(0.0, axis)
    } else {
        0.0
    };
    axis(g.deltas[k]) + stolen
}

/// One axis's positions moved by the velocities, a pinned node placed at its pin.
struct Position<'a> {
    x: &'a [f64],
    v: &'a [f64],
    pins: Option<&'a [Option<f64>]>,
}

impl StepRange for Position<'_> {
    type Out = f64;

    fn len(&self) -> u32 {
        self.x.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f64]) {
        for (out, i) in out.iter_mut().zip(range.start as usize..) {
            *out = match self.pins.and_then(|pins| pins[i]) {
                Some(pin) => pin,
                None => self.x[i] + self.v[i],
            };
        }
    }
}

/// One axis moved by a constant: `Sim::center`'s `x[i] -= dx`.
struct Shift<'a> {
    x: &'a [f64],
    by: f64,
}

impl StepRange for Shift<'_> {
    type Out = f64;

    fn len(&self) -> u32 {
        self.x.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f64]) {
        for (out, i) in out.iter_mut().zip(range.start as usize..) {
            *out = self.x[i] - self.by;
        }
    }
}

/// Where a pass runs: the runner and its worker count.
type On<'a, R> = (&'a R, u32);

/// Adds a gathered pass onto the velocities: `step::merge`.
pub(super) fn merge<R: Runner>(sim: &mut Sim, gathered: Gathered<'_>, on: On<'_, R>) {
    let merges = Merges {
        first: Some(gathered),
        second: None,
    };
    velocities(sim, merges, None, on);
}

/// The tick's link merge and charge merge in one run per axis: two gathers, each with its
/// own slot, order and `split`, added to `vx`/`vy` as they were added before.
///
/// The mesh field is a function of the positions and the params alone — `Mesh::solve` takes
/// `&Sim` and reads no velocity — so the link merge can wait until the charge deltas are in
/// hand. That is the whole saving: one pass over `vx`/`vy` instead of two.
pub(super) fn merge_pair<R: Runner>(
    sim: &mut Sim,
    first: Gathered<'_>,
    second: Gathered<'_>,
    on: On<'_, R>,
) {
    let merges = Merges {
        first: Some(first),
        second: Some(second),
    };
    velocities(sim, merges, None, on);
}

/// `Sim::center`: the mean is one thread's fold, the shift is a pass.
pub(super) fn center<R: Runner>(sim: &mut Sim, (runner, workers): On<'_, R>) {
    let Some((dx, dy)) = sim.center_shift() else {
        return;
    };
    runner.run(&Shift { x: &sim.x, by: dx }, workers, &mut sim.px);
    mem::swap(&mut sim.x, &mut sim.px);
    runner.run(&Shift { x: &sim.y, by: dy }, workers, &mut sim.py);
    mem::swap(&mut sim.y, &mut sim.py);
}

/// Collide's projection, `p = x + v`, into `px`/`py`.
pub(super) fn project<R: Runner>(sim: &mut Sim, (runner, workers): On<'_, R>) {
    let x = Position {
        x: &sim.x,
        v: &sim.vx,
        pins: None,
    };
    runner.run(&x, workers, &mut sim.px);
    let y = Position {
        x: &sim.y,
        v: &sim.vy,
        pins: None,
    };
    runner.run(&y, workers, &mut sim.py);
}

/// Collide's merge, when collide ran, then `Sim::integrate`.
pub(super) fn integrate<R: Runner>(sim: &mut Sim, collided: Option<Gathered<'_>>, on: On<'_, R>) {
    let (runner, workers) = on;
    let merges = Merges {
        first: collided,
        second: None,
    };
    velocities(sim, merges, Some(sim.params.velocity_decay), on);
    let x = Position {
        x: &sim.x,
        v: &sim.vx,
        pins: Some(&sim.fx),
    };
    runner.run(&x, workers, &mut sim.px);
    mem::swap(&mut sim.x, &mut sim.px);
    let y = Position {
        x: &sim.y,
        v: &sim.vy,
        pins: Some(&sim.fy),
    };
    runner.run(&y, workers, &mut sim.py);
    mem::swap(&mut sim.y, &mut sim.py);
}

/// The gathers one velocity run adds, in order: `step::merge`'s own, and the one fused with
/// it. Bundled so the run keeps its four parameters.
#[derive(Clone, Copy, Default)]
struct Merges<'a> {
    first: Option<Gathered<'a>>,
    second: Option<Gathered<'a>>,
}

fn velocities<R: Runner>(
    sim: &mut Sim,
    merges: Merges<'_>,
    decay: Option<f64>,
    (runner, workers): On<'_, R>,
) {
    let x = Velocity {
        v: &sim.vx,
        merged: merges.first,
        also: merges.second,
        axis: AXES[0],
        decay: decay.map(|d| (d, &sim.fx[..])),
    };
    runner.run(&x, workers, &mut sim.px);
    mem::swap(&mut sim.vx, &mut sim.px);
    let y = Velocity {
        v: &sim.vy,
        merged: merges.first,
        also: merges.second,
        axis: AXES[1],
        decay: decay.map(|d| (d, &sim.fy[..])),
    };
    runner.run(&y, workers, &mut sim.py);
    mem::swap(&mut sim.vy, &mut sim.py);
}

#[cfg(test)]
mod tests;
