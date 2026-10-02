//! The id lists this file's routing test is written against, split out by the house's
//! 300-line limit.
//!
//! Both are here for one reason: **a layout the row builder routes and this test does not
//! would fall through to the `roundtrip`/`Gated` arm and claim a gate no differential of
//! its own can earn.** Spelling the ids here rather than inline at the arm means the two
//! readings cannot drift, and a new id in one of these groups has to be added to the other.

/// The eight igraph-family layouts: the six 2D ones plus FR's and KK's two `_3d` siblings.
///
/// The `_3d` pair are routed to the **same** `oracle-igraph` record as their 2D siblings
/// (`unproven::IGRAPH_LAYOUTS`), because they are the same two algorithms at the dimension
/// SciGraphs actually calls, over one kernel at `D = 3`. Both lists spell them out, so the row
/// builder and this routing test cannot drift apart.
pub(super) const IGRAPH: [&str; 8] = [
    "layout.force.fruchterman_reingold",
    "layout.force.kamada_kawai",
    "layout.force.fruchterman_reingold_3d",
    "layout.force.kamada_kawai_3d",
    "layout.force.graphopt",
    "layout.force.davidson_harel",
    "layout.force.lgl",
    "layout.force.drl",
];

/// The three p12-t3 graph-free 3D placements: one arm file, one record, because they take
/// the same two arguments and read no graph (`harness/oracle-basic-3d.py` compares them
/// together under its `--function` selector).
pub(super) const BASIC_3D: [&str; 3] = [
    "layout.basic3d.sphere",
    "layout.basic3d.helix",
    "layout.basic3d.cube",
];
