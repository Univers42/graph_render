//! Which stage the tier sweep times, named by the plan's `--layout`.
//!
//! Its own file because "which layout is this table about" is a question every part of the
//! sweep has to answer the same way — the run, the equality check and the report's title —
//! and a copy of the answer in each of them is three places to drift.

use super::Plan;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, YifanHu};

/// The stage the sweep times.
///
/// **The layout is a parameter of the measurement, not of the code.** Both entries run the
/// same three gathered `StepRange` kernels through the same `Sim::tick`, so a sweep that
/// hard-coded one of them would silently report the other's numbers under this one's id —
/// which is the whole defect `docs/measurements/tiers-audit.md` row 1 is about. Enumerated
/// rather than held as a `&dyn Fn` so the report can *name* what it timed, and so a third
/// force layout is one match arm rather than a dynamic dispatch on the timed path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// The single-level solve (`layout.force.barnes_hut`), the sweep's default.
    BarnesHut,
    /// The multilevel solve (`layout.force.yifan_hu`).
    YifanHu,
}

impl Layout {
    /// The stage this layout times, for the report's title and its cell labels.
    pub const fn label(self) -> &'static str {
        match self {
            Layout::BarnesHut => "Barnes-Hut",
            Layout::YifanHu => "Yifan-Hu",
        }
    }

    /// The layouts the sweep can time, by registered id. The plan's `--layout` is looked
    /// up here, so a layout the registry knows but the sweep cannot time is **refused by
    /// name** rather than quietly run under the default's numbers.
    pub fn parse(id: &str) -> Result<Self, String> {
        match id {
            BarnesHut::ID => Ok(Layout::BarnesHut),
            YifanHu::ID => Ok(Layout::YifanHu),
            other => Err(format!(
                "bench --tiers: {other} is not a layout with a tier \
                 ({} and {} are)",
                BarnesHut::ID,
                YifanHu::ID
            )),
        }
    }
}

/// The stage the plan's `--layout` names, or [`Layout::BarnesHut`] when it named none.
///
/// The default is Barnes-Hut and not an error, so every row in
/// `docs/measurements/phase11-threads.md` still reproduces from the command that wrote it.
///
/// A plan may name several layouts, as the non-tier bench does — but a tier sweep is one
/// table with one title and one crossover bracket, so more than one is refused rather than
/// silently timed and reported under whichever came first.
pub fn layout(plan: &Plan) -> Result<Layout, String> {
    match plan.layouts.as_slice() {
        [] => Ok(Layout::BarnesHut),
        [id] => Layout::parse(id),
        many => Err(format!(
            "bench --tiers times one layout per table; got {}",
            many.join(", ")
        )),
    }
}
