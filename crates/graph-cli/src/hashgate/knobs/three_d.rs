use super::Stage;

/// The five natively 3D layouts' per-stage node controls, the same shape as the six below
/// and for the same reason — and for the three graph-free ones it is the *only* shape.
///
/// `layout.basic3d.{sphere,helix,cube}` read the node count and no edge at all
/// (`layout/basic_3d.rs:22-27`), so their model is their entire input and one more node is
/// exactly what moves them; `layout.hierarchical3d` reads the graph's components and
/// levels, and `layout.force.spring3d` is `spring` at `D = 3`. None publishes a `Params`,
/// so none has a real parameter to perturb — including `spring3d`, whose iterations budget
/// is read by the one `Solver::settle` it shares with the 2D stage, so
/// `GM_MUTATE_SPRING_ITERATIONS` moves both dimensions and names neither. The per-stage
/// node control is what makes the 3D one attributable.
///
/// **Every id here is a graph-core constant** (`<Layout>::ID` through the `Stage` trait, or
/// the `pub const` a module with no `impl Stage` publishes), never a spelling in this file.
pub const THREE_D_LAYOUT_STAGES: [Stage; 7] = [
    Stage {
        id: graph_core::layout::basic_3d::sphere::ID,
        env: "GM_MUTATE_BASIC3D_SPHERE_NODES",
        record: "hashgate-control-basic3d-sphere-nodes",
    },
    Stage {
        id: graph_core::layout::basic_3d::helix::ID,
        env: "GM_MUTATE_BASIC3D_HELIX_NODES",
        record: "hashgate-control-basic3d-helix-nodes",
    },
    Stage {
        id: graph_core::layout::basic_3d::cube::ID,
        env: "GM_MUTATE_BASIC3D_CUBE_NODES",
        record: "hashgate-control-basic3d-cube-nodes",
    },
    Stage {
        id: graph_core::layout::hierarchical_3d::ID,
        env: "GM_MUTATE_HIERARCHICAL3D_NODES",
        record: "hashgate-control-hierarchical3d-nodes",
    },
    Stage {
        id: graph_core::layout::force::spring::ID_3D,
        env: "GM_MUTATE_FORCE_SPRING3D_NODES",
        record: "hashgate-control-force-spring3d-nodes",
    },
    // knobs-3d-new, step 1: the two 3D layouts that were registered with no control of their
    // own. Both read the node count and nothing else — `basic_3d.rs`'s module doc says so —
    // so a re-drawn model is the same sharp probe the three above it use, and adding one
    // more node is exactly what moves them.
    Stage {
        id: graph_core::layout::basic_3d::spiral::ID,
        env: "GM_MUTATE_BASIC3D_SPIRAL_NODES",
        record: "hashgate-control-basic3d-spiral-nodes",
    },
    Stage {
        id: graph_core::layout::basic_3d::bipartite_3d::ID,
        env: "GM_MUTATE_BIPARTITE_3D_NODES",
        record: "hashgate-control-bipartite-3d-nodes",
    },
];
