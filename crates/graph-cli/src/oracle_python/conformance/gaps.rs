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
    at: "crates/graph-core/src/layout/random.rs:59",
};
pub(super) const G_GRID_ITER: Gap = Gap {
    parameter: "iterations",
    note: "a closed placement: no iteration to bound",
    at: "crates/graph-core/src/layout/grid.rs:100",
};
pub(super) const G_BASIC3D_SCALE: Gap = Gap {
    parameter: "scale",
    note: "`basic_3d`'s `SCALE` is a const, not a parameter of `sphere`/`helix`/`cube`/`spiral`/`bipartite_3d`",
    at: "crates/graph-core/src/layout/basic_3d.rs:53",
};
pub(super) const G_SNAPSHOT_SCALE: Gap = Gap {
    parameter: "scale",
    note: "`scale` is a **const in each layout**, not a parameter of `run`: `basic_3d.rs:53`, `hierarchical_3d.rs:78` and `circular/hierarchy.rs:45` each publish `SCALE: f64 = 5.0`. Nothing in `layout::snapshot` rescales anything; the centre-and-rescale the coverage doc describes is these three constants plus each layout's own extent, and the reference writes its own units",
    at: "crates/graph-core/src/layout/basic_3d.rs:53",
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
pub(super) const G_FORCEATLAS2_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "the seed **is** passed (`Fa2Params::seed = get_layout_seed()` = 981798123) and Mulberry32 would consume it, but the reference never reads that integer as a start seed: it draws `randint(0, 2**31 - 1)` = 1767573729 first (`forceatlas.py:122`) and starts from `np.random.default_rng(1767573729)`, PCG64, not MT19937 (`simulation.py:1090`). Equal seeds are not equal draws, and neither is the generator",
    at: "crates/graph-core/src/layout/forceatlas2/state.rs:30",
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
    parameter: "reference arm",
    note: "SciGraphs' Graphviz path is `scigraphs_utils.graphviz_layout`, a C++ extension absent from both oracle images, so this arm runs the **engine itself** through `gv_exact.c` (which links libgvc, runs `gvLayout` and prints `ND_coord(n)` with `%a`) and transcribes the five lines the extension would have applied (`yifan_hu.py:318-325`, in `motor/gv_post.rs` and `sc_graphviz.py`). The engine's own **text** is deliberately not the reference: `-Tplain`'s `printdouble` is `agxbprint(&buf, \"%.5g\", v)` (`lib/common/output.c:66-71`), five significant digits and not five decimals, so a coordinate in [1, 10) in lands on a step of `1e-4` in = `7.2e-3` points, and reading that text put this arm's own floor under `GRAPHVIZ_TWOPI`'s `max_gap` at 7.5e-5. What is still missing is the extension's own source of truth: it is handed a node count and an edge list and returns an array, and both what it does to the coordinates between `gvLayout` and that array and what seed it passes down are INFERENCE, not verified — its source is not on disk, only the `scigraphs-utils==0.2.0` pin (`SciGraphs/constraints/linux-x64.txt:21`)",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:279",
};
pub(super) const G_GV_Z: Gap = Gap {
    parameter: "graphviz_dim",
    note: "`sfdp_dim` defaults to `\"2Z\"`, so SciGraphs hands the engine `dimension=\"2Z\"` and then replaces the z with a spectral component (`sfdp_z_method`, `sfdp_z_scale` 0.3, `yifan_hu.py:327-334`). Both arms write `z = 0`: the motor's yifan_hu is planar and the reference is `-Tplain`, which has no third column",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:357",
};
pub(super) const G_GV_DIRECTED: Gap = Gap {
    parameter: "graph kind",
    note: "SciGraphs builds `dot` directed (`yifan_hu.py:302`) and every other engine undirected; the graphviz arm writes an undirected `graph` for all eight",
    at: "SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:302",
};
