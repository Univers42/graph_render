//! The layout-name list `snapshot` offers, pinned whole against the registry.
//!
//! Split out of `tests.rs` by the house's 300-line limit.

use super::*;

/// The whole list, exactly: one pair per registered layout, in registry order, with no
/// name offered twice — a new layout has to appear here or this goes red.
#[test]
fn layout_names_offers_every_registered_layout_once_by_both_of_its_names() {
    let names = layout_names();
    assert_eq!(
        names,
        [
            "layout.grid",
            "grid",
            "layout.tree.tidy",
            "tree.tidy",
            "layout.treemap.squarified",
            "treemap.squarified",
            "layout.circular.radial",
            "circular.radial",
            "layout.packing.circle",
            "packing.circle",
            "layout.spectral",
            "spectral",
            "layout.mds.pivot",
            "mds.pivot",
            "layout.force.barnes_hut",
            "force.barnes_hut",
            "layout.forceatlas2",
            "forceatlas2",
            "layout.dag.sugiyama",
            "dag.sugiyama",
            "layout.random",
            "random",
            "layout.circular.ring",
            "circular.ring",
            "layout.spiral",
            "spiral",
            "layout.bipartite",
            "bipartite",
            "layout.force.yifan_hu",
            "force.yifan_hu",
            "layout.force.fruchterman_reingold",
            "force.fruchterman_reingold",
            "layout.force.kamada_kawai",
            "force.kamada_kawai",
            "layout.force.graphopt",
            "force.graphopt",
            "layout.force.davidson_harel",
            "force.davidson_harel",
            "layout.force.lgl",
            "force.lgl",
            "layout.force.drl",
            "force.drl",
            "layout.twopi",
            "twopi",
            "layout.packing.osage",
            "packing.osage",
            "layout.force.spring",
            "force.spring",
            "layout.circular.hierarchy",
            "circular.hierarchy",
            "layout.circular.circo",
            "circular.circo",
            "layout.treemap.patchwork",
            "treemap.patchwork",
            "layout.force.neato",
            "force.neato",
            "layout.force.fdp",
            "force.fdp",
            "layout.basic3d.sphere",
            "basic3d.sphere",
            "layout.basic3d.helix",
            "basic3d.helix",
            "layout.basic3d.cube",
            "basic3d.cube",
            "layout.hierarchical3d",
            "hierarchical3d",
            "layout.force.spring3d",
            "force.spring3d",
            "layout.force.sfdp",
            "force.sfdp",
            "layout.forceatlas2.barnes_hut",
            "forceatlas2.barnes_hut",
            "layout.bipartite_3d",
            "bipartite_3d",
            "layout.basic3d.spiral",
            "basic3d.spiral",
            "layout.force.particle_mesh",
            "force.particle_mesh",
            "layout.forceatlas2.forcesim",
            "forceatlas2.forcesim",
            "layout.spectral3d",
            "spectral3d",
            "layout.mds.pivot3d",
            "mds.pivot3d",
            "layout.force.yifan_hu.2z",
            "force.yifan_hu.2z",
            "layout.force.fruchterman_reingold.3d",
            "force.fruchterman_reingold.3d",
            "layout.force.kamada_kawai.3d",
            "force.kamada_kawai.3d",
            "layout.force.drl.3d",
            "force.drl.3d",
            "layout.forceatlas2.3d",
            "forceatlas2.3d",
            "layout.dag.dot",
            "dag.dot",
        ]
    );
    let mut once = names.clone();
    once.sort_unstable();
    once.dedup();
    assert_eq!(once.len(), names.len(), "offered twice: {names:?}");
    for id in registry::LAYOUTS.iter().map(|l| l.id) {
        assert!(names.contains(&id), "{names:?} missing {id}");
        assert!(
            names.contains(&short_name(id)),
            "{names:?} missing {id}'s short name"
        );
    }
    assert_eq!(short_name("layout.grid"), "grid");
    assert_eq!(
        short_name("grid"),
        "grid",
        "an id already short stays as it is"
    );
}
