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

/// The three graph-free 3D placements this file routes to `oracle-basic-3d`: one arm file,
/// one record, because they take the same two arguments and read no graph
/// (`harness/oracle-basic-3d.py` compares four together under its `--function` selector).
///
/// **`layout.basic3d.spiral` is deliberately NOT here, and the reason has changed.** The arm
/// DOES compare it since job `sg-basic3d-spiral-oracle` added `--function spiral`; what has
/// not happened is the MOVING of the routing, so naming it in this group would claim a
/// routing that `unproven.rs` does not make. It still reads `scigraphs-conformance`, which is
/// asserted separately below.
pub(crate) const BASIC_3D: [&str; 3] = [
    "layout.basic3d.sphere",
    "layout.basic3d.helix",
    "layout.basic3d.cube",
];

/// The placements whose record is the conformance gate's own byte-for-byte comparison
/// against SciGraphs rather than a coverage differential. Currently one: the spiral, which
/// `oracle-basic-3d` now arms but does not yet route.
pub(crate) const BASIC_3D_UNDIFFERENTIALLED: [&str; 1] = ["layout.basic3d.spiral"];
