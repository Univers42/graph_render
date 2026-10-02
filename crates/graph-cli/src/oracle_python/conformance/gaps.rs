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
    at: "crates/graph-core/src/layout/grid.rs:101",
};
pub(super) const G_SPRING_SEED: Gap = Gap {
    parameter: "layout seed",
    note: "`SpringParams` has no seed field; the reference passes `seed=get_layout_seed()` to networkx, so the start position is drawn from two different generators",
    at: "crates/graph-core/src/layout/force/spring.rs:120",
};
pub(super) const G_BASIC3D_SCALE: Gap = Gap {
    parameter: "scale",
    note: "`basic_3d`'s `SCALE` is a const, not a parameter of `sphere`/`helix`/`cube`",
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
    // **The reference is seedable; reproducing its stream is what is forbidden.** An earlier
    // version of this note claimed the opposite — that `_reset_layout_rng` seeds numpy and the
    // stdlib `random` while igraph reads "the C library's generator", which neither call reaches.
    // That is false for python-igraph: it installs the stdlib `random` module *as* igraph's RNG
    // at import (`src/_igraph/random.c:295-325`, `igraphmodule_init_rng` →
    // `igraph_rng_Python_set_generator(random_module)`; the rngtype is declared at `:54-58` with
    // `is_seeded = 1`, and `igraph_rng_Python_get` at `:167-` draws from
    // `random.getrandbits`/`random.random`), so `common.py:60`'s `random.seed(...)` does reseed
    // igraph. Two full `--reference` runs over the same fixtures gave byte-identical files on all
    // 64 rows, the five here included, so the reference is reproducible. What remains is the
    // licence: `docs/decisions/layouts-igraph.md` rule 4 says igraph's own RNG is never
    // reproduced, so the motor keeps Mulberry32 and the two streams differ from the first draw on.
    note: "the seed **is** passed on both sides (`common.py:60` seeds the stdlib `random`, which python-igraph installs as igraph's RNG at `src/_igraph/random.c:295-325`), and the reference is reproducible; the two streams are still different generators: graph-core draws from Mulberry32 and igraph from Mersenne Twister, and `docs/decisions/layouts-igraph.md` rule 4 forbids reproducing the latter",
    at: "crates/graph-core/src/rng.rs:14",
};
pub(super) const G_IGRAPH_FIT: Gap = Gap {
    parameter: "scale",
    note: "the reference writes its own units until `_igraph_fit_positions` (`igraph_layouts.py:24-42`) centres every axis on its mean and scales the whole drawing so the largest magnitude over **all three** axes is `scale`; that step is SciGraphs' convention, not igraph's, so it lives in the motor arm (`conformance/motor/fit.rs`) and never inside a motor layout — four ids reach it, named in `FITTED`",
    at: "crates/graph-cli/src/oracle_python/conformance/motor/fit.rs:47",
};
pub(super) const G_KK_NON_FINITE: Gap = Gap {
    parameter: "iterations",
    note: "**the reference raises, this port does not.** python-igraph 0.11.9 returns three infinite coordinates out of nine from `layout_kamada_kawai(dim=3)` on the 3-vertex path `gate-01` — in all six vertex orderings, and at `dim=2` the same graph is finite — so the row compares 957 of 1020 coordinates. Not component count, not an isolated node and not degree: the graph is connected with degrees 2, 1, 1. It is the 3x3 Newton block being near-singular at three vertices, so one step overflows `f64`; `_igraph_fit_positions` then turns those three infinities into all nine, because `extent` is `inf`, the factor is `0` and `inf * 0` is NaN. `kamada_kawai_3d` guards the block and is finite there",
    at: "crates/graph-core/src/layout/force/kamada_kawai_3d/tests.rs:109",
};
pub(super) const G_DRL_NO_3D: Gap = Gap {
    parameter: "dimension",
    note: "**SciGraphs calls DrL at `dim=3`** (`igraph_layouts.py:342`) and this row's motor layout is planar, so the row compares a 2D drawing against a 3D reference and its third column is pure difference. Unlike FR and KK there is **no 3D spec to implement**: `docs/layouts/layout.force.drl.md:3` says the 3D variant \"is out of scope here except where noted\" and defines no 3-D step, no 3-D density grid and no 3-D tent kernel — the whole spec is 2-D (`1000 x 1000` grid, `21 x 21` block, a 2-axis tent). Rule 1 of `docs/decisions/layouts-igraph.md` makes the spec the implementer's only source, so a `drl_3d` needs a spec author to write that section first; the implementer must not fill the hole from `drl/*.cpp`. Left as a recorded gap rather than an improvised third axis",
    at: "crates/graph-core/src/layout/force/drl.rs:93",
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
