//! `GM_MUTATE_DROP_DELTA`'s parse and its one reader: which batch of the force gate's stream
//! stage the native arm skips. Split out of the parent by the house's 300-line limit.

use super::Setting;
use crate::hashgate::Knob;

impl Setting {
    /// The batch the force gate's native stream arm skips ([`Knob::DropDelta`]), or `None`
    /// when it skips none.
    ///
    /// One reader, beside the parse that fills the field, so the number the arm acts on and
    /// the number the run was given cannot disagree — the whole control is that one number.
    pub(crate) fn drop_delta(&self) -> Option<u32> {
        self.drop_delta
    }
}

/// The 1-based batch index `knob` names. A batch index rather than a flag because a control
/// that cannot say *which* batch can only ever be tested against the first one.
///
/// `0` is refused here rather than reaching `refuse_a_no_op`: batch 0 is the initial graph,
/// not a delta, so skipping it drops nothing — a control that perturbs nothing by a
/// different route. `u32`, so a negative index is a parse error, not a silent wrap.
pub(super) fn batch(text: &str, knob: Knob) -> Result<u32, String> {
    let batch: u32 = text
        .parse()
        .map_err(|e| format!("{}={text:?}: {e}", knob.env()))?;
    if batch == 0 {
        return Err(format!(
            "{}={text:?}: batch 0 is the initial graph, not a delta, so dropping it \
             perturbs nothing",
            knob.env()
        ));
    }
    Ok(batch)
}
