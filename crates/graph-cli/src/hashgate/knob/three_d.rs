//! The five natively 3D layout controls' variable and record names, in
//! [`super::Knob`]'s 3D arm order.
//!
//! The same shape as [`super::igraph`] and the same reason: each is a node count for
//! **one** stage's own model, resolved by variable name through
//! [`crate::hashgate::knobs::THREE_D_LAYOUT_STAGES`], so nothing about a control is spelled
//! here but the two names it is set by and writes.
//!
//! **For `sphere`, `helix` and `cube` this probe is not merely the available one, it is
//! the only one.** Those three read the node count and no edge at all
//! (`layout/basic_3d.rs:22-27`), so the model is their entire input: one more node is
//! exactly what moves them, and no parameter exists that could do it more sharply.
//! `hierarchical3d` reads the graph's components and levels, and `spring3d` is `spring` at
//! `D = 3` over the same `SpringParams` — so both are moved by a larger model too, and
//! `spring3d`'s own *parameter* control is [`super::Knob::SpringIterations`], which reaches
//! both dimensions and therefore names neither alone.
//!
//! The index is the *arm's* place in the 3D group, and
//! `each_knob_names_its_own_variable_and_record` in `hashgate/tests/knob/table.rs` plus
//! `each_three_d_layout_has_its_own_control_that_moves_only_its_stage` in
//! `hashgate/tests/knob/three_d.rs` hold each arm to its row, so a reordering cannot pass
//! by permuting both at once.

/// The variable each 3D control is set by, in 3D arm order. The order matches
/// [`super::Knob`]'s 3D arms and
/// [`crate::hashgate::knobs::THREE_D_LAYOUT_STAGES`]'s.
pub const ENV: [&str; 5] = [
    "GM_MUTATE_BASIC3D_SPHERE_NODES",
    "GM_MUTATE_BASIC3D_HELIX_NODES",
    "GM_MUTATE_BASIC3D_CUBE_NODES",
    "GM_MUTATE_HIERARCHICAL3D_NODES",
    "GM_MUTATE_FORCE_SPRING3D_NODES",
];

/// The record each 3D control's run writes, in the same order as [`ENV`].
pub const RECORD: [&str; 5] = [
    "hashgate-control-basic3d-sphere-nodes",
    "hashgate-control-basic3d-helix-nodes",
    "hashgate-control-basic3d-cube-nodes",
    "hashgate-control-hierarchical3d-nodes",
    "hashgate-control-force-spring3d-nodes",
];
