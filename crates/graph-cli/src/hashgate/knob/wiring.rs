//! [`super::Knob`]'s four delegations: the list every gate reads, the variable that sets
//! each one, and the record its run writes.
//!
//! Split out of `knob.rs` by the house's 300-line limit. The enum and the essay on what a
//! control is *for* stay in the parent, because they are the type; this is the wiring, and
//! all three lists it delegates to live beside it in [`arms`] and [`records`] so a name
//! cannot be spelled twice in one file.

use super::{Knob, arms, records};

impl Knob {
    /// Every knob: those that move a parameter or re-draw one layout's model, then
    /// the twenty-seven per-stage controls — the fifteen of
    /// [`super::knobs::ANALYSIS_POST_STAGES`], the six of [`super::knobs::IGRAPH_LAYOUT_STAGES`], the
    /// five of [`super::knobs::THREE_D_LAYOUT_STAGES`] and the one of
    /// [`super::knobs::OSAGE_LAYOUT_STAGES`] — then the two compute-tier controls, then the live
    /// session's own. The list itself is [`arms::ALL`], spelled out there.
    ///
    /// **A `const`, because `capabilities::verdict::Evidence::load` walks it** to collect
    /// one control record each — a ledger read cannot be a function call per row. So the
    /// twenty-seven per-stage arms are spelled out there and held against those four tables
    /// by `the_analysis_and_post_controls_are_the_knobs_table`, which fails on any arm whose
    /// variable, record or stage a table disagrees with.
    pub const ALL: [Self; 50] = arms::ALL;

    /// The variable that sets it.
    pub const fn env(self) -> &'static str {
        arms::env(self)
    }

    /// The record its run writes.
    pub const fn record(self) -> &'static str {
        records::record(self)
    }
}
