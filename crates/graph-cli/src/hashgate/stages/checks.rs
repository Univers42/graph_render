//! The two refusals the native arm makes **before** it hashes anything: a node count no
//! `u32` model can be drawn from, and a stage list it cannot print.
//!
//! Split out of `stages.rs` by the house's 300-line limit, and for the reason
//! [`node_count`] gives: both rules here are read from more than one arm — the native arm
//! and the threaded arm both draw a model, and only the threaded arm lives in another file
//! — so a rule kept as an inline `checked_add` at each call site is a rule with as many
//! chances to be forgotten as it has call sites.

#[cfg(test)]
mod tests;

use super::{LAYOUT, TRANSPORT, staged};
use crate::hashgate::{Knob, Setting};
use graph_core::registry as core;
use std::collections::BTreeSet;

/// The node count a model is drawn from: [`graph_core::gate_node_count`] for `seed`, plus
/// the gate's shared [`Setting::extra_nodes`], plus `own` for the one stage whose control
/// re-draws its own model. Refused rather than wrapped.
///
/// **One rule at four call sites, not four `checked_add`s.** The two arms of the gate draw
/// this count twice over — the native arm at [`stage_bytes_for`](super::stage_bytes_for)
/// and twice more for the re-drawn models, the threaded arm at
/// [`stage_bytes_threaded`](super::super::tiered::stage_bytes_threaded) — and all four
/// added it as a bare `u32` sum. `GM_MUTATE_NODE_COUNT=4294967295` therefore wrapped that
/// sum in a release build, every stage was hashed from a *smaller* model than the control
/// asked for, and the gate compared two small graphs and exited 0: a red control that
/// reported green. The wrap is invisible from the output, which is exactly why the refusal
/// has to be here rather than in the diff: the arithmetic is in `u64`, and the only thing
/// that can fail is the narrowing back to the `u32` a `NodeRecord` index is.
///
/// `checked_add` on the `u64`s is kept even though three `u32`s cannot overflow one: it
/// is the statement of the rule rather than a proof of today's arithmetic, and it is what a
/// reader checks first.
pub(crate) fn node_count(seed: u32, setting: &Setting, own: u32) -> Result<u32, String> {
    u64::from(graph_core::gate_node_count(seed))
        .checked_add(u64::from(setting.extra_nodes))
        .and_then(|sum| sum.checked_add(u64::from(own)))
        .and_then(|sum| u32::try_from(sum).ok())
        .ok_or_else(|| too_many_nodes(seed, setting, own))
}

/// The refusal: the sum that did not fit, every term that fed it, and the variable each
/// term came from.
///
/// **The terms are listed rather than the total alone.** Two knobs add nodes and only one
/// may be set at a time, so a refusal that said only "4294967301 nodes" would leave the
/// reader to work out which of the two variables to go and edit — and the answer is the one
/// whose value is non-zero, which is why a zero term is left out of the list instead of
/// printed as `plus 0`. The knob's own `env()` is used rather than the field name: a run
/// is told which variable it is under, and a variable is what a reader can edit.
fn too_many_nodes(seed: u32, setting: &Setting, own: u32) -> String {
    let gate = graph_core::gate_node_count(seed);
    let extra = setting.extra_nodes;
    let mut terms = vec![format!("{gate} for seed {seed}")];
    if extra != 0 {
        terms.push(format!(
            "{extra} from {}",
            knob_name(setting, Knob::NodeCount)
        ));
    }
    if own != 0 {
        terms.push(format!(
            "{own} from {}",
            setting.control.map_or_else(
                || "the per-stage control".to_owned(),
                |knob| knob.env().to_owned()
            )
        ));
    }
    let total = u64::from(gate) + u64::from(extra) + u64::from(own);
    format!(
        "{total} nodes is past the {} a node record can be indexed by: {}",
        u32::MAX,
        terms.join(" plus ")
    )
}

/// `knob`'s variable, naming the per-stage control's own instead when that is the one set.
fn knob_name(setting: &Setting, fallback: Knob) -> String {
    setting.control.unwrap_or(fallback).env().to_owned()
}

/// The gate's whole stage list over `layouts`: the topology, then every layout of the
/// registry slice, then every registered analysis, then every registered POST capability,
/// then the transport.
///
/// **One derivation for the list the gate prints and the list [`check`] audits.** Those two
/// were separate before: `stages()` walked `LAYOUTS` and `check` deduped the slice it was
/// handed, so an id shared between a layout registry and a graph-wasm registry produced two
/// stages printed under one name — `staged::position` then resolved the *first*, so one of
/// the two was hashed and never compared, and the per-stage tally counted one stage twice
/// while claiming both were equal.
pub(super) fn stage_list(layouts: &[core::Capability]) -> Vec<&'static str> {
    let mut stages = vec!["topology"];
    stages.extend(layouts.iter().map(|layout| layout.id));
    stages.extend(staged::analyses());
    stages.extend(staged::posts());
    stages.push(TRANSPORT);
    stages
}

/// A stage list the gate cannot print: no stage id twice across the **whole** list, and no
/// missing [`LAYOUT`].
///
/// Both refusals, in that order, because the cheaper one is the one that says a list was
/// built wrong rather than that it collided. The duplicate scan covers the layout slice and
/// the two graph-wasm registries together for the reason [`stage_list`] gives: a repeated id
/// folds into one key in the record and makes the per-stage counts lie.
pub(super) fn check(layouts: &[core::Capability]) -> Result<(), String> {
    if !layouts.iter().any(|layout| layout.id == LAYOUT) {
        return Err(format!(
            "the {TRANSPORT} stage restates {LAYOUT}, which is not registered"
        ));
    }
    let mut seen = BTreeSet::new();
    for id in stage_list(layouts) {
        if !seen.insert(id) {
            return Err(format!(
                "{id} appears twice in the gate's stage list: one stage id is one record key"
            ));
        }
    }
    Ok(())
}
