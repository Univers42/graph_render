//! Running the sweep: the timing itself, split from `tiers.rs`'s vocabulary and its report
//! so each file stays inside the house's 300-line cap.
//!
//! Every rule the run obeys is stated where it is obeyed: a **median** over the repeats
//! rather than one timing (`time`), the serial arm's bytes as the reference every other arm
//! is compared against (`size`), and the negative control reaching the non-scalar arms only
//! (`mutate`) — because a reference mutated alongside them would agree with them and the
//! equality column would report nothing.

use super::{Cell, Host, Tier, markdown};
use crate::bench::Plan;
use crate::bench::scale;
use crate::exec_native::Threads;
use graph_core::REFERENCE_DEGREE;
use graph_core::Topology;
use graph_core::exec::Serial;
use graph_core::index_model;
use graph_core::layout::Geometry;
use graph_core::layout::force::{BarnesHut, ForceParams, Split};
use std::time::Instant;

/// How an arm is timed: how many times, and under which negative control.
///
/// One struct rather than two more parameters, and the reason is the house's four-parameter
/// ceiling: `size` needs the topology, the arm list and both of these, and a fifth argument
/// would be the one that made the arm list and the control drift apart in a call site.
#[derive(Debug, Clone, Copy)]
pub struct Under {
    pub repeat: u32,
    pub split: Split,
}

/// The sweep: time every arm at every size, with the serial arm's bytes as the reference.
pub fn run(plan: &Plan, tiers: &[Tier]) -> Result<(Vec<Cell>, Host), String> {
    run_under(plan, tiers, Split::None)
}

/// [`run`] with the negative control: `split` makes every **non-scalar** arm's merge read
/// a neighbouring node's delta too, while the serial arm stays honest. That is the control
/// for the `equal` column — without it, "every arm is byte-equal to scalar" is a check that
/// would report true whatever the arms computed.
pub fn run_under(plan: &Plan, tiers: &[Tier], split: Split) -> Result<(Vec<Cell>, Host), String> {
    let load_start = markdown::loadavg();
    let under = Under {
        repeat: plan.repeat,
        split,
    };
    let mut cells = Vec::new();
    for &n in &plan.sizes {
        cells.extend(size(&topology(plan, n)?, tiers, under)?);
    }
    Ok((cells, markdown::host(load_start, markdown::loadavg())))
}

/// The `n`-node scale model, indexed.
fn topology(plan: &Plan, n: u32) -> Result<Topology, String> {
    let (nodes, edges) = scale::scale_model(plan.seed, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).map_err(|e| format!("n={n}: {e}"))
}

/// Every arm at one size, each compared with the serial arm's bytes.
///
/// The comparison is a **second pass, after every arm has been timed**: an arm's `equal`
/// flag must depend on the arm and the reference, never on the order the caller happened to
/// write the tiers in — `--tiers threads,scalar` times the reference second, and a cell that
/// answered `true` because it had not met the reference yet would be the one lie this
/// column can tell. A sweep that named no scalar arm is not a special case: it runs one
/// honest serial pass, untimed, and compares against that.
fn size(topology: &Topology, tiers: &[Tier], under: Under) -> Result<Vec<Cell>, String> {
    let mut timed: Vec<(Tier, (Vec<f64>, Geometry))> = Vec::with_capacity(tiers.len());
    for &tier in tiers {
        timed.push((tier, time(topology, tier, under)?));
    }
    let reference = match timed.iter().find(|(tier, _)| *tier == Tier::Scalar) {
        Some((_, (_, geometry))) => geometry.clone(),
        None => run_once(topology, Tier::Scalar, Split::None)?,
    };
    Ok(timed
        .into_iter()
        .map(|(tier, (runs_ms, geometry))| Cell {
            n: topology.node_count(),
            tier,
            runs_ms,
            equal: geometry == reference,
        })
        .collect())
}

/// The control an arm runs under: nothing for the scalar arm, which is the honest
/// reference every other arm is compared with.
fn mutate(split: Split, tier: Tier) -> Split {
    if tier == Tier::Scalar {
        Split::None
    } else {
        split
    }
}

/// `repeat` timed runs of the stage under `tier`, and the geometry the last one left.
fn time(topology: &Topology, tier: Tier, under: Under) -> Result<(Vec<f64>, Geometry), String> {
    let mut runs_ms = Vec::with_capacity(under.repeat.max(1) as usize);
    let mut last = None;
    for _ in 0..under.repeat.max(1) {
        let started = Instant::now();
        last = Some(run_once(topology, tier, mutate(under.split, tier))?);
        runs_ms.push(started.elapsed().as_secs_f64() * 1e3);
    }
    Ok((runs_ms, last.expect("at least one run")))
}

/// One stage run under `tier`, with the negative control as an argument and never a
/// second code path: the control a test calls is the control the gate reaches.
fn run_once(topology: &Topology, tier: Tier, split: Split) -> Result<Geometry, String> {
    let params = ForceParams::default();
    let geometry = match tier {
        Tier::Scalar => BarnesHut::run_under(topology, &params, &Serial, 1, split),
        Tier::Threads(workers) => BarnesHut::run_under(topology, &params, &Threads, workers, split),
    };
    geometry.map_err(|e| format!("{tier:?}: {e}"))
}
