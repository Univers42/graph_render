//! The id lists this file's routing test is written against, split out by the house's
//! 300-line limit.
//!
//! Both are here for one reason: **a layout the row builder routes and this test does not
//! would fall through to the `roundtrip`/`Gated` arm and claim a gate no differential of
//! its own can earn.** Spelling the ids here rather than inline at the arm means the two
//! readings cannot drift, and a new id in one of these groups has to be added to the other.

/// The six igraph-family layouts.
pub(crate) const IGRAPH: [&str; 6] = [
    "layout.force.fruchterman_reingold",
    "layout.force.kamada_kawai",
    "layout.force.graphopt",
    "layout.force.davidson_harel",
    "layout.force.lgl",
    "layout.force.drl",
];

/// The three graph-free 3D placements `oracle-basic-3d` actually arms: one arm file, one
/// record, because they take the same two arguments and read no graph
/// (`harness/oracle-basic-3d.py` compares them together under its `--function` selector).
///
/// **`layout.basic3d.spiral` is deliberately NOT here.** It is a fourth such layout, but
/// `ARMS` covers three, so naming it in this group would claim an arm that does not exist.
/// It is routed to `scigraphs-conformance` instead and asserted separately.
pub(crate) const BASIC_3D: [&str; 3] = [
    "layout.basic3d.sphere",
    "layout.basic3d.helix",
    "layout.basic3d.cube",
];

/// The placements read by no coverage differential of their own, held instead to the
/// conformance gate's byte-for-byte comparison against SciGraphs. Currently one: the spiral,
/// until job `sg-basic3d-spiral-oracle` adds `--function spiral` to `oracle-basic-3d`.
pub(crate) const BASIC_3D_UNDIFFERENTIALLED: [&str; 1] = ["layout.basic3d.spiral"];
