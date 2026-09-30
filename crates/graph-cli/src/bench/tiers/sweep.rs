//! Running the sweep: the timing itself, split from `tiers.rs`'s vocabulary and its report
//! so each file stays inside the house's 300-line cap.
//!
//! Every rule the run obeys is stated where it is obeyed: a **median** over the repeats
//! rather than one timing (`time`), the serial arm's bytes as the reference every other arm
//! is compared against (`size`), and the negative controls reaching the non-scalar arms
//! only (`mutate`) — because a reference mutated alongside them would agree with them and
//! the equality column would report nothing.
//!
//! **The sweep times a named layout, and the reference is that layout's own serial arm**.
//! The grid, the ring and the spiral are each their own stage with their own per-node
//! gather, so a "speedup" between two of them would be a subtraction, not a ratio.

use super::route::{self, Control};
use super::{Cell, Host, Tier, markdown};
use crate::bench::Plan;
use crate::bench::scale;
use graph_core::REFERENCE_DEGREE;
use graph_core::Topology;
use graph_core::index_model;
use graph_core::layout::Geometry;
use std::time::Instant;

/// How an arm is timed: how many times, and under which negative controls.
#[derive(Debug, Clone, Copy)]
struct Under {
    repeat: u32,
    control: Control,
}

/// The sweep: time every arm of every named layout at every size, with each layout's serial
/// arm's bytes as the reference.
pub fn run(
    plan: &Plan,
    layouts: &[&'static str],
    tiers: &[Tier],
) -> Result<(Vec<Cell>, Host), String> {
    run_under(plan, layouts, tiers, Control::HONEST)
}

/// [`run`] with the negative controls: `control.split` makes every **non-scalar**
/// Barnes-Hut arm's merge read a neighbouring node's delta too, and `control.split_rescale`
/// does the same to the closed-form layouts' shared `rescale` merge, while every serial arm
/// stays honest. That is the control for the `equal` column — without it, "every arm is
/// byte-equal to scalar" is a check that would report true whatever the arms computed.
pub fn run_under(
    plan: &Plan,
    layouts: &[&'static str],
    tiers: &[Tier],
    control: Control,
) -> Result<(Vec<Cell>, Host), String> {
    let load_start = markdown::loadavg();
    let under = Under {
        repeat: plan.repeat,
        control,
    };
    let mut cells = Vec::new();
    for &n in &plan.sizes {
        let topology = topology(plan, n)?;
        for &layout in layouts {
            cells.extend(size(&topology, layout, tiers, under)?);
        }
    }
    Ok((cells, markdown::host(load_start, markdown::loadavg())))
}

/// The `n`-node scale model, indexed.
fn topology(plan: &Plan, n: u32) -> Result<Topology, String> {
    let (nodes, edges) = scale::scale_model(plan.seed, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).map_err(|e| format!("n={n}: {e}"))
}

/// Every arm of one layout at one size, each compared with that layout's serial arm's bytes.
///
/// The comparison is a **second pass, after every arm has been timed**: an arm's `equal`
/// flag must depend on the arm and the reference, never on the order the caller happened
/// to write the tiers in — `--tiers threads,scalar` times the reference second, and a cell
/// that answered `true` because it had not met the reference yet would be the one lie this
/// column can tell. A sweep that named no scalar arm is not a special case: it runs one
/// honest serial pass, untimed, and compares against that.
fn size(
    topology: &Topology,
    layout: &'static str,
    tiers: &[Tier],
    under: Under,
) -> Result<Vec<Cell>, String> {
    let mut timed: Vec<(Tier, (Vec<f64>, Geometry))> = Vec::with_capacity(tiers.len());
    for &tier in tiers {
        timed.push((tier, time(topology, layout, tier, under)?));
    }
    let reference = match timed.iter().find(|(tier, _)| *tier == Tier::Scalar) {
        Some((_, (_, geometry))) => geometry.clone(),
        None => route::run_once(topology, layout, Tier::Scalar, Control::HONEST)?,
    };
    Ok(timed
        .into_iter()
        .map(|(tier, (runs_ms, geometry))| Cell {
            layout,
            n: topology.node_count(),
            tier,
            runs_ms,
            equal: geometry == reference,
        })
        .collect())
}

/// The control an arm runs under: nothing for the scalar arm, which is the honest reference
/// every other arm is compared with.
fn mutate(control: Control, tier: Tier) -> Control {
    if tier == Tier::Scalar {
        Control::HONEST
    } else {
        control
    }
}

/// `repeat` timed runs of the stage under `tier`, and the geometry the last one left.
fn time(
    topology: &Topology,
    layout: &'static str,
    tier: Tier,
    under: Under,
) -> Result<(Vec<f64>, Geometry), String> {
    let control = mutate(under.control, tier);
    let mut runs_ms = Vec::with_capacity(under.repeat.max(1) as usize);
    let mut last = None;
    for _ in 0..under.repeat.max(1) {
        let started = Instant::now();
        last = Some(route::run_once(topology, layout, tier, control)?);
        runs_ms.push(started.elapsed().as_secs_f64() * 1e3);
    }
    Ok((runs_ms, last.expect("at least one run")))
}
