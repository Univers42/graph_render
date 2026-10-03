//! `GM_MUTATE_LAYOUT_PARAM_DEFAULT`'s parse: which published default the control
//! perturbs, and which stage it perturbs it on
//! (`docs/decisions/layout-params.md`). Split out of the parent by the house's
//! 300-line limit.

use crate::hashgate::Knob;
use graph_core::Stage;
use graph_core::layout::force::FruchtermanReingold;

/// The stage [`Knob::LayoutParamDefault`] perturbs: the first layout in the registry that
/// publishes parameters. Named here rather than left to a search so the control names the
/// stage it moved, which is the property every knob in this table is held to.
pub(crate) const PARAM_DEFAULT_STAGE: &str = FruchtermanReingold::ID;

/// Which published default `knob` perturbs, refused rather than clamped: an index past the
/// end of the list, or one whose default has no in-range neighbour to move to, is an error
/// rather than a control that perturbs nothing.
pub(crate) fn param_index(text: &str, knob: Knob) -> Result<usize, String> {
    let index: usize = text
        .parse()
        .map_err(|e| format!("{}={text:?}: {e}", knob.env()))?;
    let specs = graph_core::registry::find(PARAM_DEFAULT_STAGE)
        .expect("the stage this control perturbs is registered")
        .params
        .specs;
    let spec = specs.get(index).ok_or_else(|| {
        format!(
            "{}={index}: {} publishes {} parameters",
            knob.env(),
            PARAM_DEFAULT_STAGE,
            specs.len()
        )
    })?;
    if (spec.default + 1.0) < spec.min || (spec.default + 1.0) > spec.max {
        return Err(format!(
            "{}={index}: {}'s default {} has no in-range neighbour, so nothing would move",
            knob.env(),
            spec.name,
            spec.default
        ));
    }
    Ok(index)
}
