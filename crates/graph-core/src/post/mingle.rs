//! MINGLE — multilevel edge bundling by ink minimisation (Gansner et al. 2011), ported
//! from `SciGraphs/engine/scigraphs_engine/bundling/mingle.py`.
//!
//! A bundle is a chain `a → p → q → b` whose members share `p` and `q`, and its **ink** is
//! `sum |s − p| + |p − q| + sum |t − q|` over its members' two sides. Round 0 is every edge
//! alone. Each round is up to [`PASSES_PER_ROUND`] passes; a pass scores candidate pairs,
//! keeps a **matching** of them (each bundle at most once, so a score measured against a
//! bundle is never spent twice), and fuses the accepted pairs. What the round produced
//! becomes the next round's edges: one trunk per bundle, weighted by its load, which is what
//! makes it *multilevel* rather than one flat pass.
//!
//! **Single-threaded on purpose** (phase 8, step 5). A pass is a sequential matching — pair
//! `k`'s score is measured against bundles `k − 1` has already moved — so unlike FDEB there
//! is no gather form to lift to the SIMD, threaded or GPU tiers (D10). It stays on the scalar
//! tier until the hierarchy is rebuilt as one.
//!
//! **Determinism.** Fixed-order reductions everywhere (D3): the Weiszfeld solve is Jacobi
//! (each step reads start-of-iteration `p` and `q` and writes a new pair), the candidate row
//! sorts by `(distance, index)`, and the matching sorts by `(−gain, candidate position)`.
//! Every one of those is a *total* order (D5), so equal ink gains break by candidate
//! position, which is itself `(u, v)` ascending. Weights never enter the ink — the trunk is
//! one drawn line whatever it carries — they only pull the meeting points, exactly as in the
//! reference.
//!
//! **Ponytail (greedy):** the hierarchy is a non-optimal one. Two bundles that would pay only
//! when merged *together* are scored pairwise and never merged. Failing input: a bundle whose
//! members each need a third, unrelated edge to be worth bundling. Direction: cosmetic — the
//! drawing stays valid, only less tidy. Escape hatch: [`Params::rounds`], and a caller that
//! wants the optimum has FDEB.
//!
//! **Ponytail (two inks):** the score and the report do not measure the same thing. The
//! score sums each member's own arcs with the trunk counted once — the reference's `ink`, and
//! the right quantity for deciding a merge. [`Bundles::ink_after`] is what a viewer sees:
//! drawn length with a shared trunk counted once however many members carry it. They agree
//! unless members are *coincident*, where the score counts one line as several and so
//! overstates the gain. Failing input: three copies of one edge plus a fourth leaving the
//! same node, where the score sees 33 of ink fold to 20 and the drawing grows from 13 to
//! 20.4 (pinned in the tests). Direction: cosmetic — the paths are valid, the drawing is
//! longer. Escape hatch: [`Params::min_gain`], and [`Bundles::ink_after`] against
//! [`Bundles::ink_before`] is the check a host that cares should make on the result itself.
//!
//! **Ponytail (solve, eps):** the meeting points are a 12-step weighted Jacobi fixed point,
//! not a minimum, and a Weiszfeld distance is floored at [`SOLVE_EPS_FRAC`] of the drawing's
//! diagonal because the optimum sits on a source when a bundle's members share a node.
//! Against 64 steps the solve falls 2.9% short of the ink minimum; direction: cosmetic, a
//! sub-pixel move at any sane scale. Escape hatch: [`SOLVE_ITERS`] and [`SOLVE_EPS_FRAC`].
//!
//! Arithmetic is `f64` throughout, where the reference carries `f32`: a gain is decided with
//! the same relative precision the layout stages use, and the contract's `f32` wire columns
//! round once, on output. `+ − × ÷` and `sqrt` are IEEE 754, so this is bit-identical native
//! and wasm32 (D1, D2: no `mul_add` anywhere).

mod draw;
mod level;
mod pass;
mod run;
#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::post::{Bundled, Metadata, centres};
use crate::stage::StageError;
use draw::{drawn_ink, paths, polygons};
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind};
use level::diagonal;
use run::Run;

/// A point, in the layout's own coordinates.
pub type P = [f64; 2];

/// The capability id this stage registers and hashes under.
pub const ID: &str = "post.bundle.mingle";

/// Candidate bundles scored per bundle per pass, and the hard cap on it. The reference's
/// `MAX_K`, kept for its reason: the pass is a matching over what it scores.
pub const MAX_NEIGHBORS: u32 = 16;

/// Live points one side of a bundle may hold before a merge is refused. The reference's
/// `MAX_GROUP`, kept for the same reason — it bounds a fused bundle.
pub const MAX_GROUP: usize = 32;

/// Jacobi steps per scored merge. The reference's `SOLVE_ITERS`.
pub const SOLVE_ITERS: u32 = 12;

/// Floor on a Weiszfeld distance, as a fraction of the drawing's diagonal: a bundle's
/// members share a node, so the optimum sits on a source. The reference's
/// `SOLVE_EPS_FRAC`.
pub const SOLVE_EPS_FRAC: f64 = 1e-4;

/// Matching passes per round. A pass is a matching, so the bundle count at least halves and
/// eight passes reach [`MAX_GROUP`]. The reference's `PASSES_PER_ROUND`.
pub const PASSES_PER_ROUND: u32 = 8;

/// Rounds of the hierarchy, and the hard cap on [`Params::rounds`]. Past this the graph's
/// edges are all trunks and a further round would be handed exactly the level it refused.
pub const MAX_ROUNDS: u32 = 8;

/// Edge count past which MINGLE stops being usable, and why it is this one.
///
/// Measured, not estimated: `graph-cli ink --nodes N` sweeps the hairball generator and
/// reports the wall time of one pass. The cost is quadratic in the edge count, because a pass
/// scores each bundle against its `k` nearest by scanning every bundle, and
/// `PASSES_PER_ROUND × rounds` do not change that shape. The sweep behind this number is in
/// `docs/measurements/phase08-ink.md`; re-run the command to refresh it. A second of
/// bundling per redraw is past what a view absorbs, so the ceiling is the edge count that
/// fits it, rounded down to two figures.
pub const MINGLE_CEILING: u64 = 3_000;

/// MINGLE's ledger row.
pub const META: Metadata = Metadata {
    tier: 1,
    edges: EdgeGeometryKind::Polyline,
    oracle: "hand: Gansner et al. 2011 restated in f64, ported from \
SciGraphs/engine/scigraphs_engine/bundling/mingle.py; the merge order, the ink accounting and the \
tie-break are pinned per input in graph-core's post/mingle tests and re-checked per seed by \
graph-cli roundtrip. No third-party MINGLE exists to differential-test against, and the reference \
carries f32 ink where this carries f64, so a byte comparison would be lenient rather than strict",
    complexity: "O(rounds x passes x (m^2 proximity + m x k x (MAX_GROUP + SOLVE_ITERS)))",
    scale_ceiling: MINGLE_CEILING,
    degradation: "past the ceiling the run is still correct and still deterministic — nothing is \
refused and no approximation is added — it simply stops finishing inside a frame, at O(m^2) on the \
per-pass candidate scan. The escape hatches are Params::neighbors (fewer candidates scored per \
bundle) and Params::rounds (a shallower hierarchy); a spatial index over the bundles would remove the \
scan and is not implemented. Single-threaded throughout, so there is no wider tier to fall back to",
    ponytail: "Ponytail (greedy): the hierarchy is a non-optimal one — two bundles that would pay only \
when merged together are scored pairwise and never merged. Failing input: a bundle whose members each \
need a third, unrelated edge to be worth bundling. Direction: cosmetic, the paths stay valid and the \
drawing is merely less tidy. Escape hatch: Params::rounds. Ponytail (two inks): the score sums each \
member's own arcs with the trunk counted once, while ink_after is the deduplicated drawn length, and \
they disagree when members are coincident. Failing input: three copies of one edge plus a fourth \
leaving the same node, where the score sees 33 of ink fold to 20 while the drawing grows from 13 to \
20.4. Direction: cosmetic, a longer drawing. Escape hatch: Params::min_gain, and comparing \
ink_after against ink_before in the output. Ponytail (solve): the meeting points are a 12-step \
weighted Jacobi fixed point, not a minimum — against 64 steps it falls 2.9% short of the ink \
minimum, a visible 1.2% move. Direction: cosmetic. Escape hatch: SOLVE_ITERS. Ponytail (eps): a \
Weiszfeld distance is floored at SOLVE_EPS_FRAC of the drawing's diagonal, so a bundle of coincident \
edges is held that fraction off the line it lies on. Direction: cosmetic, sub-pixel at any sane \
scale. Escape hatch: SOLVE_EPS_FRAC. Ponytail (scale_ceiling): measured in wall clock on one host, \
which is a claim about this machine and not about wasm32 (docs/measurements/phase08-ink.md)",
};

/// What a host may set. Every field is clamped on the way in, as the reference's `settings`
/// clamps: an out-of-range value becomes a stated default, never a silent one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// Candidate bundles scored per bundle per pass, clamped to `1..=MAX_NEIGHBORS`.
    pub neighbors: u32,
    /// Hierarchy rounds, clamped to `0..=MAX_ROUNDS`.
    pub rounds: u32,
    /// A merge must gain at least this fraction of the pair's ink, not merely a positive
    /// amount. Never negative.
    pub min_gain: f64,
}

impl Default for Params {
    /// The reference's `DEFAULTS`.
    fn default() -> Self {
        Self {
            neighbors: 10,
            rounds: 4,
            min_gain: 0.0,
        }
    }
}

impl Params {
    /// `neighbors` and `rounds` inside their clamps, `min_gain` non-negative. A `min_gain`
    /// that is not finite takes 0 rather than poisoning every comparison with a NaN.
    pub fn clamped(self) -> Self {
        Self {
            neighbors: self.neighbors.clamp(1, MAX_NEIGHBORS),
            rounds: self.rounds.min(MAX_ROUNDS),
            min_gain: if self.min_gain.is_finite() {
                self.min_gain.max(0.0)
            } else {
                0.0
            },
        }
    }
}

/// What one `bundle` run produced. Every field is in the input's own edge order.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundles {
    /// Each edge's control polygon, interior points only: the contract's `Polyline` row, so
    /// the result survives serialization.
    pub paths: graph_contract::geometry::Paths,
    /// Which bundle each edge rides, dense, in the input's edge order. Two edges share a
    /// bundle id exactly when their drawn paths share a trunk.
    pub cluster: Vec<u32>,
    /// Levels each edge was bundled through: its number of meeting points, so its row holds
    /// `2 × depth` interior points. `0` is an edge that never merged.
    pub depth: Vec<u32>,
    /// Merges accepted, over every pass of every round.
    pub merges: u32,
    /// The ink drawn by the input's straight edges, deduplicated the same way.
    pub ink_before: f64,
    /// The ink drawn by `paths`, deduplicated. Below `ink_before` for every input whose
    /// members are not coincident; above it for the coincident case the module doc names,
    /// which is what a host that cares should compare.
    pub ink_after: f64,
}

impl Bundles {
    /// The edges that merged with nothing and are therefore drawn straight, ascending: the
    /// count the ledger's `Bundled::unbundled` reports.
    pub fn unbundled(&self) -> u32 {
        self.depth.iter().filter(|depth| **depth == 0).count() as u32
    }
}

/// MINGLE at its default parameters — the run [`ID`] is registered and hashed under.
pub fn run(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    let out = over(topology, geometry, &Params::default())?;
    let (merges, unbundled) = (out.merges, out.unbundled());
    Ok(Bundled {
        // Handed on by the one rebuild path, so a 3D layout's z column survives bundling.
        geometry: geometry.with_edges(EdgeGeometry::Polyline(out.paths)),
        pairs: merges,
        unbundled,
    })
}

/// MINGLE over a finished layout's geometry. The composition seam: node positions come from
/// the layout, whichever kind it emitted, and the topology's edge endpoints say which edges
/// are bundled. Nothing here knows which layout ran.
pub fn over(
    topology: &Topology,
    geometry: &Geometry,
    params: &Params,
) -> Result<Bundles, StageError> {
    let (x, y) = centres(&geometry.nodes);
    let pos: Vec<P> = x
        .iter()
        .zip(y)
        .map(|(&x, &y)| [f64::from(x), f64::from(y)])
        .collect();
    let endpoints = &topology.edges();
    bundle(&pos, &endpoints.source, &endpoints.target, params)
}

/// Bundles the edges `src → dst` over node positions `pos`, greedily and multilevel.
///
/// Refused only when an edge endpoint is not a dense index into `pos`; the caller owns
/// finiteness (D9), which the snapshot contract refuses at the boundary.
pub fn bundle(pos: &[P], src: &[u32], dst: &[u32], params: &Params) -> Result<Bundles, StageError> {
    let p = params.clamped();
    let ends = level::endpoints(pos, src, dst)?;
    let mut run = Run::new(&ends);
    let eps = (SOLVE_EPS_FRAC * diagonal(pos)).max(1e-12);
    for _ in 0..p.rounds {
        if !run.round(&p, eps) {
            break;
        }
    }
    let polys = polygons(&ends, &run.chains());
    Ok(Bundles {
        paths: paths(&polys),
        cluster: run.cluster(),
        depth: run.depths(),
        merges: run.merges(),
        ink_before: drawn_ink(&level::straight(&ends)),
        ink_after: drawn_ink(&polys),
    })
}
