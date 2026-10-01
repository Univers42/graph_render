//! The six igraph-family layout controls' variable and record names, in
//! [`super::Knob`]'s igraph arm order.
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because these six are one
//! thing the other arms are not: they spell out nothing of their own. Each is the same
//! shape as the ANALYSIS and POST per-stage controls — a node count for **one** stage's own
//! model, resolved by variable name through [`crate::hashgate::knobs`] — so the names live
//! in one table beside the stage ids, and `Knob::env`/`Knob::record` read them by index
//! rather than repeating a pair six times.
//!
//! The index is the *arm's* place in the igraph group, and the test
//! `each_igraph_arm_names_the_table_row_at_its_own_index` in
//! `hashgate/tests/knob/table.rs` holds each arm to its row, so a reordering cannot pass
//! by permuting both at once.

/// The variable each igraph control is set by, in igraph arm order. The order matches
/// [`super::Knob`]'s igraph arms and [`crate::hashgate::knobs::IGRAPH_LAYOUT_STAGES`].
pub const ENV: [&str; 6] = [
    "GM_MUTATE_FORCE_FRUCHTERMAN_REINGOLD_NODES",
    "GM_MUTATE_FORCE_KAMADA_KAWAI_NODES",
    "GM_MUTATE_FORCE_GRAPHOPT_NODES",
    "GM_MUTATE_FORCE_DAVIDSON_HAREL_NODES",
    "GM_MUTATE_FORCE_LGL_NODES",
    "GM_MUTATE_FORCE_DRL_NODES",
];

/// The record each igraph control's run writes, in the same order as [`ENV`].
pub const RECORD: [&str; 6] = [
    "hashgate-control-force-fruchterman-reingold-nodes",
    "hashgate-control-force-kamada-kawai-nodes",
    "hashgate-control-force-graphopt-nodes",
    "hashgate-control-force-davidson-harel-nodes",
    "hashgate-control-force-lgl-nodes",
    "hashgate-control-force-drl-nodes",
];
