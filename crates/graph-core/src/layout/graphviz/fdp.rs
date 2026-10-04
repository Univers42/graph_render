//! Graphviz 16.1.0's `fdp`: force-directed placement, Fruchterman-Reingold with a grid.
//!
//! Reference: `lib/fdpgen/tlayout.c`, `lib/fdpgen/grid.c`, `lib/fdpgen/xlayout.c` and
//! `lib/fdpgen/layout.c` of the pinned Graphviz 16.1.0 release, read as an algorithm
//! reference and reimplemented (never translated, never linked —
//! `docs/decisions/graphviz-oracle.md`). The steps below are the reference's, in its order:
//!
//! 1. `fdp_init_graph` → `fdp_initParams` (`tlayout.c:153-188`) reads `K`, `maxiter`,
//!    `T0` and `start` off the graph, defaulting to `K = 0.3`, `maxIters = 600`,
//!    `T0 = -1` (unset) and `start = 1`.
//! 2. `deriveGraph` (`layout.c:380-527`) makes one derived node per node and one derived
//!    edge per undirected pair, each with `len = K` and `weight = 1`
//!    (`fdpinit.c:71-72`). **This port has no clusters**, so the reference's recursion has
//!    depth one and there are no boundary ports.
//! 3. `initPositions` (`tlayout.c:413-567`) seeds `srand48(start)` and draws two values per
//!    node into a box of half-extent `1.2 * K * (sqrt(n) + 1) / 2`.
//! 4. `fdp_tLayout` (`tlayout.c:573-601`) runs `unscaled * maxIters / 100 = 300` ticks of
//!    `gAdjust` on the temperature `T0 * (maxIters - t) / maxIters`, where
//!    `T0 = K * sqrt(n) / 5`.
//! 5. `fdp_xLayout` (`xlayout.c:247-293`) runs up to nine `x_layout` tries to push apart
//!    the boxes that touch, then `removeOverlapAs(g, "prism")`.
//! 6. `finalCC` (`layout.c:79-175`) translates the drawing so its lower-left corner is the
//!    origin, and `-Tplain` reports inches scaled by 72.
//!
//! **Units are points, and the translation is kept**, as in every engine in this
//! directory: the port emits exactly the coordinates `-Tplain` prints, so the small closed
//! cases are directly comparable with no offset applied on either side.
//!
//! **The edges are read.** Unlike `osage`, this is a force layout: the attraction is over
//! the edge list and the repulsion is over all node pairs within one grid cell. A graph of
//! `n` isolated nodes and a path of `n` nodes get different drawings.
//!
//! Determinism: **`-Gstart` is NOT inert for this engine**, and the job's instruction to
//! measure that first is what the whole shape of this port follows from. `-Gstart=1`,
//! `7` and `99` give three visibly different layouts (measured —
//! `docs/measurements/p13-gv2-fdp.md`), because `initPositions` seeds the placement from
//! it, so the port reproduces the reference's `srand48`/`drand48` sequence exactly rather
//! than drawing from the house's own generator. The engine is also **iterative and
//! chaotic**: 300 ticks of a cooled force model amplify a one-bit difference in the
//! placement into a different picture. The kernel is in gather form (D10) — every tick
//! zeroes the displacement columns and refills them, and the reductions run in a fixed
//! order (edge index, then grid cell in `(i, j)` order, then the reference's eight
//! neighbours).
//!
//! Ponytail: **`hypot` is replaced by `sqrt(dx*dx + dy*dy)` in every separation.** The
//! reference writes `hypot` (`tlayout.c:219`, `xlayout.c:157`) and this port does not, because
//! `hypot` is a libm function and not an IEEE-754 operation: glibc's and wasm32's are
//! different implementations, and with it in the repulsion kernel the hash gate's
//! native-against-wasm32 identity fails — measured, three of eight seeds diverging at
//! `hashgate --seeds 8` while each arm agreed with itself. `sqrt` is correctly rounded by the
//! standard and the substitution is bit-identical on every target; the hash gate then reads
//! 4-way equal on 8 of 8. Failing input: any graph, on any target pair whose `hypot` differs.
//! Direction: the last ulp against the reference, amplified by 300 ticks into 2.7e-3 points
//! on the two-node case (2.5e-3 became 5.2e-3 in `x`) — a *different* drawing at the same
//! scale, never a wrong one. Escape hatch: a `sqrt`-based `hypot` is a one-line change in
//! [`distance`] if a target pair ever needs the reference's exact function, and it would cost
//! the native/wasm32 identity.
//!
//! Ponytail: **the oracle is not reproducible, so this port cannot be gated.** Running the
//! pinned Graphviz 16.1.0 `fdp -Tplain -Gstart=1` twice over the same graph gives
//! byte-different output, in the fifth significant digit, for some inputs and not others —
//! measured, with the bisection that localises it, in `docs/measurements/p13-gv2-fdp.md`.
//! The graph is deterministic for `-Gmaxiter` up to 99 and is not at 600, and the
//! difference survives `-Goverlap=0`, so the divergence is in the expansion phase rather
//! than in the packing. There is no engine fact to reproduce here, only a distribution.
//! Failing input: any graph past roughly a hundred iterations of the expansion phase, which
//! at the default is every graph of more than a few dozen nodes. Direction: a *different*
//! drawing, not a worse one — both arms are the same algorithm, so the gap is the
//! reference's own spread and not a defect in this port. Escape hatch: the ledger row
//! (`crates/graph-core/src/registry/graphviz_fdp.rs`) records the measured oracle self-gap
//! as the floor on any achievable agreement, and the row stays `Status::Implemented`; a
//! future Graphviz that is reproducible, or an oracle arm that runs the engine twice and
//! compares the *pair*, is what would make a ceiling meaningful.
//!
//! Ponytail: **clusters and their boundary ports.** The reference's recursion into
//! `expandCluster` and its `bport_t` ports on the enclosing ellipse are not ported.
//! Failing input: any DOT graph with a `subgraph cluster_*`. Direction: this port lays the
//! cluster's nodes out as ordinary nodes and gives no cluster box. Escape hatch: none
//! inside the motor — the motor's [`Topology`] is a flat node set
//! with no cluster membership to lay out.
//!
//! Ponytail: **disconnected graphs are laid out as one component.** The reference splits
//! into connected components, runs `fdp_tLayout` on each, and packs them with
//! `putGraphs` (`layout.c:828-883`). This port runs one expansion over the whole node set.
//! Failing input: a graph of two or more components. Direction: the components repel each
//! other, so each one's internal drawing is not the reference's, though the drawing as a
//! whole is still a force layout of the same graph. Escape hatch: the component split is a
//! connected-components pass over the same edge list this module already builds.
//!
//! Ponytail: **the `prism` overlap packing.** `x_layout`'s nine tries are ported;
//! `removeOverlapAs(g, "prism")` (`xlayout.c:341`) is not, so whatever the prism pass
//! would have separated, this port leaves where the tries put it. Failing input: a drawing
//! where the nine tries leave a pair of boxes still overlapping — the tries stop on the
//! first round that reaches zero overlaps, and prism is what runs when they do not.
//! Direction: a drawing with residual overlap, i.e. nodes closer than the four-point
//! margin. Escape hatch: the overlap count `x_layout` already returns is the number to
//! test, and `removeOverlapWith` in the same reference is the shape of the missing call.
//!
//! Ponytail: **the `len`, `weight`, `overlap`, `sep`, `K`, `T0` and `maxiter` attributes.**
//! The reference reads all seven and this port reads none, because the motor publishes no
//! attribute channel to a layout at its default parameters (the same decision
//! `layout::graphviz::osage` records). Every one of them is a documented default here, and
//! every one of them is a different drawing.
//!
//! Ponytail: **node box size.** The overlap test uses Graphviz's default `nodesize` of
//! 0.75 x 0.5 inch, which is exact only while every node's *label* fits inside the minimum
//! — the same limit `layout::graphviz::osage` records, and it bites at the same place. It
//! matters here only through the overlap phase's constants, so a label that overflows
//! changes this port's `X_ov` and the reference's differently. Escape hatch: none; there is
//! no font engine in graph-core.

mod force;
mod grid;
mod model;
mod overlap;
mod rng;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;

use grid::Grid;
use model::Model;
use rng::GlibcRand;

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.force.fdp";

/// Node count `layout.force.fdp` was run at.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
/// layout.force.fdp --n 220,440,880,1000 --past-ceiling --repeat 3`: 727.58 ms at 220 nodes,
/// 2 621.23 ms at 440, 13 438.35 ms at 880 and 14 371.82 ms at 1 000 — a per-node cost rising
/// from 3.31 ms to 14.37 ms over a 4.5x size range, which is the square. The table is in
/// `docs/measurements/p13-gv2-fdp.md`.
///
/// This is a measured **lower** bound, not a size at which the layout was found to stop
/// working: nothing breaks at 1 000, the cost is simply smooth through it. The expansion is
/// `O(pass1 * (m + grid work))` and the overlap phase `O(tries * n^2)` with `tries = 9`, so
/// the second term sets the ceiling and keeps setting it past here.
pub const FDP_CEILING: u64 = 1_000;

/// `POINTS_PER_INCH` (`lib/common/const.h`): the reference computes in inches and reports
/// points.
const POINTS_PER_INCH: f64 = 72.0;

/// `DFLT_K` (`tlayout.c:98`): the spring constant and the ideal edge length.
const K: f64 = 0.3;

/// `DFLT_maxIters` (`tlayout.c:97`).
const MAX_ITERS: u32 = 600;

/// `unscaled` (`globals.c:30`): the percentage of the iterations spent in the expansion
/// phase, so `T_pass1 = unscaled * maxIters / 100`.
const UNSCALED: u32 = 50;

/// `T_pass1`, the expansion phase's tick count.
const PASS1: u32 = UNSCALED * MAX_ITERS / 100;

/// `xpms->numIters = maxIters - pass1` (`tlayout.c:135`), the overlap phase's tick count.
/// Equal to [`PASS1`] at the defaults, and kept as its own name because the two are
/// different quantities that happen to coincide.
const X_ITERS: u32 = MAX_ITERS - PASS1;

/// `EXPFACTOR` (`tlayout.c:96`): the initial placement box is this much larger than the
/// ideal spring length times the node count.
const EXP_FACTOR: f64 = 1.2;

/// `T_Cell = 3 * K` (`tlayout.c:177-180`), the repulsion grid's cell size in inches.
const CELL: f64 = 3.0 * K;

/// `DFLT_seed` (`tlayout.c:100`): the `-Gstart` value `initPositions` seeds from. Not
/// inert — see the module header.
const START_SEED: u64 = 1;

/// Graphviz's default `nodesize` (0.75 x 0.5 inch) in inches, the box the overlap test
/// grows by the margin. See the last Ponytail marker.
const NODE_W: f64 = 0.75;
const NODE_H: f64 = 0.5;

/// `DFLT_MARGIN` (`lib/neatogen/adjust.h:24`) in inches: `sepFactor`'s default is four
/// points, added to each box's half-extent.
const SEP_INCH: f64 = 4.0 / POINTS_PER_INCH;

/// `hypot(dx, dy)`, the reference's own expression for a separation
/// (`tlayout.c:219`, `tlayout.c:300`, `xlayout.c:157`, `xlayout.c:172`).
///
/// **This is `sqrt(dx * dx + dy * dy)` and not `f64::hypot`, and that is a deliberate
/// departure from the reference.** `hypot` is a libm function and not an IEEE-754
/// operation: glibc's and wasm32's are different implementations that agree to an ulp or two
/// and not always. The hash gate requires the geometry to be **bit-identical native against
/// wasm32** (D10), and with `hypot` in the repulsion kernel it is not — measured, at
/// `hashgate --seeds 8`, three of eight seeds diverge between the two arms while each arm
/// agrees with itself. `sqrt` is correctly rounded by the standard, so the substitution is
/// bit-identical on every target. The cost is the last ulp against the reference, which is
/// not a number worth holding here: the reference is not reproducible against *itself* at
/// this engine's default (see the module header), so there is no exact value to be within an
/// ulp of. It is a Ponytail marker because it is a substitution, not a derivation.
fn distance(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// `RAD(n) = hypot(WD2(n), HT2(n))` (`xlayout.c:61-65`), the same for every node at the
/// default margin, so it is a function of nothing and not a per-node lookup.
fn radius() -> f64 {
    distance(NODE_W / 2.0 + SEP_INCH, NODE_H / 2.0 + SEP_INCH)
}

/// `xParams.C` (`xlayout.c:39`), the repulsion factor. `xpms->C` is `fdp_parms->C`, whose
/// default is `0.0`, and `xinit_params` only overwrites the static when it is positive
/// (`xlayout.c:77-78`) — so the phase runs on 1.5, not on the engine's own `C`.
const REPULSION_C: f64 = 1.5;

/// The tries in `DFLT_overlap = "9:prism"` (`xlayout.c:33`).
const OVERLAP_TRIES: u32 = 9;

/// `layout.force.fdp` at its defaults: the seeded expansion, then the overlap tries, then
/// the origin translation `-Tplain` reports.
///
/// No `Stage` impl and no `Params`, by the same decision `layout::graphviz::osage`
/// records: the reference exposes seven graph attributes, none of which the ledger row or
/// the oracle sets, so the layout is a pure function of the topology and publishing a
/// `Params` would buy a knob with nothing behind it. There is no `fdp` hash-gate knob,
/// because `hashgate/knob.rs` is shared with the parallel Graphviz engine jobs.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let mut model = Model::new(topology);
    let mut rand = GlibcRand::new();
    expand(&mut model, &mut rand);
    let count = model.count;
    if count >= 2 {
        overlap::x_layout(&mut model, t0(count) / 2.0, &mut rand);
    }
    Ok(to_points(&mut model))
}

/// `T_T0 = Tfact * K * sqrt(n) / 5` (`tlayout.c:121`), with `Tfact = 1.0` (`globals.c:31`).
fn t0(count: u32) -> f64 {
    K * f64::from(count).sqrt() / 5.0
}

/// `fdp_tLayout`'s expansion phase: [`PASS1`] ticks of [`force::tick`] on the cooling
/// schedule, against a grid that is refilled each tick.
fn expand(model: &mut Model, rand: &mut GlibcRand) {
    let mut grid = Grid::new();
    let t0 = t0(model.count);
    for tick in 0..PASS1 {
        let temp = t0 * f64::from(MAX_ITERS - tick) / f64::from(MAX_ITERS);
        force::tick(model, &mut grid, temp, rand);
    }
}

/// `finalCC`'s translation (`layout.c:137-142`) and `-Tplain`'s scale: the drawing's
/// lower-left corner becomes the origin and the inches become points.
///
/// The origin is the lower-left corner of the drawing's **bounding box**, and
/// `compute_bb` (`utils.c`) builds that box from each node's *box*, not its centre — so
/// the translation is `-(min(position) - half_size)`, and a lone node lands at half its
/// box rather than at the origin. That is the whole of the one-node answer, and it is why
/// this fold is over `position - half_size` and not over `position`.
///
/// The reference divides the translation by 72 on the way in and multiplies every
/// coordinate by 72 on the way out, so the two cancel exactly and there is no rounding to
/// reproduce. Each axis is folded on its own — `finalCC` gives `pt.x` and `pt.y` separate
/// expressions — and each fold is a `min` in dense-index order, which is the one reduction
/// whose order cannot move the bytes.
fn to_points(model: &mut Model) -> Geometry {
    let low_x = model
        .x
        .iter()
        .fold(f64::INFINITY, |acc, &v| acc.min(v - NODE_W / 2.0));
    let low_y = model
        .y
        .iter()
        .fold(f64::INFINITY, |acc, &v| acc.min(v - NODE_H / 2.0));
    for (x, y) in model.x.iter_mut().zip(model.y.iter_mut()) {
        *x = (*x - low_x) * POINTS_PER_INCH;
        *y = (*y - low_y) * POINTS_PER_INCH;
    }
    point_geometry(&model.x, &model.y)
}
