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
use crate::hashgate::Setting;
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
    Ok(graph_core::gate_node_count(seed) + setting.extra_nodes + own)
}

/// The refusal, naming the control that fed the count and the sum that would not fit.
///
/// The variable rather than the field: a run is told which `GM_MUTATE_*` it is under, and
/// "that variable" is what a reader can go and edit, where `extra_nodes` is a name only
/// this crate uses. The arithmetic is spelled out too, because a refusal that says only
/// "too many nodes" leaves the reader to work out which of the two knobs was to blame.
fn too_many_nodes(seed: u32, setting: &Setting, own: u32) -> String {
    let gate = graph_core::gate_node_count(seed);
    let knobs = setting.control.map_or_else(
        || "the seed's own node count".to_owned(),
        |knob| knob.env().to_owned(),
    );
    let total = u64::from(gate) + u64::from(setting.extra_nodes) + u64::from(own);
    format!(
        "{total} nodes is past the {} a node record can be indexed by ({gate} for seed {seed} \
         plus {} plus {own}, from {knobs})",
        u32::MAX,
        setting.extra_nodes
    )
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
    for layout in layouts {
        if !seen.insert(layout.id) {
            return Err(format!("{} appears twice in the registry", layout.id));
        }
    }
    Ok(())
}