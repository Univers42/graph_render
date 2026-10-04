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
    // **The reference is seedable; reproducing its stream is what is forbidden.** An earlier
    // version of this note claimed the opposite — that `_reset_layout_rng` seeds numpy and the
    // stdlib `random` while igraph reads "the C library's generator", which neither call reaches.
    // That is false for python-igraph: it installs the stdlib `random` module *as* igraph's RNG
    // at import (`src/_igraph/random.c:295-325`, `igraphmodule_init_rng` →
    // `igraph_rng_Python_set_generator(random_module)`), so `common.py:60`'s `random.seed(...)`
    // does reseed igraph. Two full `--reference` runs over the same fixtures gave
    // byte-identical files on all 64 rows, the igraph ones included, so the reference is
    // reproducible. What remains is the licence: `docs/decisions/layouts-igraph.md` rule 4 says
    // igraph's own RNG is never reproduced, so the motor keeps Mulberry32 and the two streams
    // differ from the first draw on.
    note: "the seed **is** passed on both sides (`common.py:60` seeds the stdlib `random`, which python-igraph installs as igraph's RNG at `src/_igraph/random.c:295-325`), and the reference is reproducible; the two streams are still different generators: graph-core draws from Mulberry32 and igraph from Mersenne Twister, and `docs/decisions/layouts-igraph.md` rule 4 forbids reproducing the latter",
    at: "crates/graph-core/src/rng.rs:14",
};
pub(super) const G_IGRAPH_FIT: Gap = Gap {
    parameter: "scale",
    note: "the reference writes its own units until `_igraph_fit_positions` (`igraph_layouts.py:24-42`) centres every axis on its mean and scales the whole drawing so the largest magnitude over **all three** axes is `scale`; that step is SciGraphs' convention, not igraph's, so it lives in the motor arm (`conformance/motor/fit.rs`) and never inside a motor layout — five ids reach it, named in `FITTED`",
    at: "crates/graph-cli/src/oracle_python/conformance/motor/fit.rs:35",
};
pub(super) const G_KK_NON_FINITE: Gap = Gap {
    parameter: "iterations",
    note: "**the reference returns infinities, this port does not.** python-igraph 0.11.9 returns three infinite coordinates out of nine from `layout_kamada_kawai(dim=3)` on the fixture `gate-01` (3 nodes, the path `1-0-2`, degrees 2, 1, 1) — in all six vertex orderings, and at `dim=2` the same graph is finite. SciGraphs passes no `seed=` for KK (`igraph_layouts.py:97-99`), so this is igraph's own 3-D start and not a start of ours, which is also why a 3-column start of our own would not reproduce it. It is not component count, not an isolated node and not degree: the graph is connected with those degrees, so the cause is the 3x3 Newton block going non-finite at three vertices. `_igraph_fit_positions` then turns those three infinities into all nine, because `extent` is `inf`, the factor is `0` and `inf * 0` is NaN, and SciGraphs' own `_check_positions` (`common.py:183`) then raises `IGRAPH_KK produced 9 non-finite coordinate(s)`, so the row reaches a reference on 23 of its 24 fixtures and the judge scores **1011** coordinates, the motor's 1020 less that fixture's 9. `layout.force.kamada_kawai.3d` returns a zero step rather than the division (`kamada_kawai/solve.rs:30`) and is finite there. Measured 2026-10-04, `scripts/scigraphs-conformance.sh`",
    at: "crates/graph-core/src/layout/force/kamada_kawai/solve.rs:30",
};
// `G_FORCEATLAS2_SEED` is GONE as of job `sg-fa2-forcesim` (2026-10-03), and the const with
// it: the row now runs `layout.forceatlas2.forcesim`, whose `seed` parameter *is* the integer
// the reference draws (`FORCESIM_SEED` = 1767573729, `forceatlas.py:122`), and whose start is
// produced by graph-core's own PCG64 — the same `np.random.default_rng` the reference uses.
// The seed is now honoured end to end, so the gap would be a false record. What is left is
// `G_SNAPSHOT_SCALE`, which is about `scale` and not about the seed. Measured in
// `docs/measurements/sg-fa2-forcesim.md`.
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
