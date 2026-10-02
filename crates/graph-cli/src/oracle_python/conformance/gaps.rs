//! Every parameter of `apply_graph_layout` the motor layouts have no slot for, as one constant
//! each, so a row's gap list below reads as a table and not as prose.
//!
//! **A gap is recorded rather than normalised away, because normalising it is the failure this
//! whole module exists to prevent.** `scale = 5.0` against graph-core's `SCALE` const is the
//! same number today; the day it is not, the row's cause changes and nobody would be told. Every
//! `at` is the line the fixed value is written on, so a repair job opens the file at the number
//! and not at a search string.

use super::Gap;

pub(super) const G_RANDOM_ITER: Gap = Gap {
    parameter: "iterations",
    note: "no iteration loop at all: a draw per node, so a budget is not applicable",
    at: "crates/graph-core/src/layout/random.rs:33",
};
pub(super) const G_RANDOM_SCALE: Gap = Gap {
    parameter: "scale",
    note: "no rescale step; the draw is taken in the unit box as written",
    at: "crates/graph-core/src/layout/random.rs:33",
};
pub(super) const G_RANDOM_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "`SEED` is the const `0x5EED`, where the reference draws from `np.random.RandomState(get_layout_seed())`",
    at: "crates/graph-core/src/layout/random.rs:30",
};
pub(super) const G_GRID_ITER: Gap = Gap {
    parameter: "iterations",
    note: "a closed placement: no iteration to bound",
    at: "crates/graph-core/src/layout/grid.rs:92",
};
pub(super) const G_GRID_SCALE: Gap = Gap {
    parameter: "scale",
    note: "the only length is `GridParams::spacing`, default 1.0; nothing rescales to 5.0",
    at: "crates/graph-core/src/layout/grid.rs:51",
};
pub(super) const G_SPRING_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "`SpringParams` has no seed field; the reference passes `seed=get_layout_seed()` to networkx, so the start position is drawn from two different generators",
    at: "crates/graph-core/src/layout/force/spring.rs:120",
};
pub(super) const G_BASIC3D_SCALE: Gap = Gap {
    parameter: "scale",
    note: "`basic_3d`'s `SCALE` is a const, not a parameter of `sphere`/`helix`/`cube`/`spiral`",
    at: "crates/graph-core/src/layout/basic_3d.rs:43",
};
pub(super) const G_SNAPSHOT_SCALE: Gap = Gap {
    parameter: "scale",
    note: "`scale` is a **const in each layout**, not a parameter of `run`: `basic_3d.rs:43`, `hierarchical_3d.rs:78` and `circular/hierarchy.rs:45` each publish `SCALE: f64 = 5.0`. Nothing in `layout::snapshot` rescales anything; the centre-and-rescale the coverage doc describes is these three constants plus each layout's own extent, and the reference writes its own units",
    at: "crates/graph-core/src/layout/basic_3d.rs:43",
};
pub(super) const G_NO_ITERATIONS: Gap = Gap {
    parameter: "iterations",
    note: "a closed form or a library call: the whole computation is `run(&Topology) -> Geometry` with no count in its signature, so a budget has nothing to bound",
    at: "crates/graph-core/src/layout/spiral.rs:63",
};
pub(super) const G_IGRAPH_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "unseedable on the reference side: `_reset_layout_rng` seeds numpy and the stdlib `random` (`common.py:53-62`), and igraph reads the C library's generator, which neither call reaches",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/common.py:60",
};
pub(super) const G_SCALE_FIXED_LAYER: Gap = Gap {
    parameter: "scale",
    note: "`layer_spacing` is the only length and `LAYER_SPACING` is its default; `run` takes no scale",
    at: "crates/graph-core/src/layout/sugiyama/mod.rs:50",
};
pub(super) const G_FORCEATLAS2_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "the seed **is** passed (`Fa2Params::seed = get_layout_seed()`), but the two streams differ: the reference draws its start from `np.random.RandomState(get_layout_seed())` and graph-core from its own Mulberry32 at the same integer, so equal seeds are not equal draws",
    at: "crates/graph-core/src/layout/forceatlas2/state.rs:30",
};
pub(super) const G_CUBE_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "the eight corners are a closed form, but the interior is drawn from the kernel's own generator where the reference draws from `np.random.RandomState(get_layout_seed())`; no seed is a parameter of `cube`",
    at: "crates/graph-core/src/layout/basic_3d/cube.rs:78",
};

pub(super) const G_NEATO_START: Gap = Gap {
    parameter: "layout seed",
    note: "neato seeds a `drand48` initial placement from `-Gstart`, which `run_with` cannot pass: it takes only `epsilon`, so the start is whatever the engine's own default is",
    at: "crates/graph-core/src/layout/graphviz/neato.rs:118",
};
pub(super) const G_FDP_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "FDP's `rand48` stream is seeded inside graph-core, not from a parameter: no slot exists to pass `get_layout_seed()`",
    at: "crates/graph-core/src/layout/graphviz/fdp.rs:230",
};
pub(super) const G_TWOPI_ROOT: Gap = Gap {
    parameter: "root",
    note: "`run` takes no root, so twopi picks its own; SciGraphs passes `graphviz_twopi_root` (empty by default) into the engine",
    at: "crates/graph-core/src/layout/radial/twopi.rs:104",
};
pub(super) const G_OSAGE_BOX: Gap = Gap {
    parameter: "scale",
    note: "the box sizes are `NodeBox::DEFAULT` (54x36pt), the Graphviz node defaults: no size parameter is plumbed from `run`",
    at: "crates/graph-core/src/layout/graphviz/osage/sizes.rs:53",
};
pub(super) const G_GV_UTILS: Gap = Gap {
    parameter: "scale",
    note: "SciGraphs' Graphviz path is `scigraphs_utils.graphviz_layout`, absent from the oracle image: the reference here is the engine's raw `-Tplain` points in points, with SciGraphs' `scale = 5.0` multiply and its z column missing",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:278",
};
pub(super) const G_GV_DIRECTED: Gap = Gap {
    parameter: "graph kind",
    note: "SciGraphs builds `dot` directed (`yifan_hu.py:302`) and every other engine undirected; the graphviz arm writes an undirected `graph` for all eight",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:302",
};
