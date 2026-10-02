//! POST edge styles (`post.style.{straight,orthogonal,bezier,quadratic}`): straight,
//! orthogonal (L and Z elbows), quadratic and cubic bezier control points, and the
//! deterministic fan that keeps parallel edges apart. Read alongside SciGraphs'
//! `core/scigraphs_core/mesh/edge_styles.py` (`generate_straight_edge`,
//! `generate_orthogonal_edge`, `generate_curved_edge`, `identify_parallel_edges`,
//! `offset_parallel_edge`, `generate_self_loop`), which is a **reference and not an
//! oracle**: it lays style points into a 3-D mesh, so there is no byte-for-byte
//! comparison to run against a 2-D CSR. Every convention is stated here instead, and
//! pinned by the unit tests in `styles/tests/` and at every CSR boundary by
//! `tests/edge_geometry_invariants.rs`.
//!
//! # What a row holds
//!
//! [`Paths`] stores a row's **interior** points: a row's first and last point are the
//! source's and target's node centres, which come from node geometry and are not
//! stored. Every rule below is written for that shape, and it is why the fan moves
//! *interior* points rather than endpoints — an endpoint cannot be moved, so a fan that
//! moved endpoints would be invisible here.
//!
//! # The generators
//!
//! `shapes` is the arithmetic; in one line each, for a chord `p0 -> p1` with
//! perpendicular `(d.y, -d.x)` and bulge `curvature` to the side the chord's own
//! diagonal picks (`shapes::Bend::bulge`):
//!
//! - `orthogonal`: `L` is one bend at `(p1.x, p0.y)` — horizontal first, the reference's
//!   `HORIZONTAL_FIRST`. `Z` is two, at `(mid.x, p0.y)` then `(mid.x, p1.y)` — a
//!   vertical middle leg at the chord's midpoint, its `CENTERED`.
//! - `quadratic`: one control point, `mid + perp * bulge / 2`.
//! - `bezier`: two, at a quarter and three quarters of the chord, each
//!   `+ perp * bulge / 4`.
//!
//! `straight` has no generator, and that is the one honest asymmetry here: it emits
//! [`EdgeGeometry::Line`], whose endpoints
//! are the node centres and which stores nothing
//! else, so a straight edge costs zero bytes. A parallel *straight* edge therefore has
//! nowhere to keep its gap, and [`style_edges`] refuses a non-zero `parallel_offset` for
//! that style instead of drawing two edges on top of each other. The three styles above
//! store interior points, and they are the ones that fan.
//!
//! # The self-loop
//!
//! A self-loop is not a zero-length row: it is a regular `self_loop_segments`-gon of
//! circumradius `self_loop_radius` centred half a radius above the node, the shape
//! `shapes::Bend::loop_row` draws and states. The same shape is what a **zero-length
//! chord** gets, and it is the *positions* that decide, not the endpoint identities — two
//! distinct nodes a layout placed on the same spot get the same loop, and the fan slides
//! the two centres apart along `y` because a zero-length chord has no perpendicular to
//! turn.
//!
//! One limit of that slide, stated rather than left to be found: it moves the centres,
//! not the circles, so two self-loops on one node whose `parallel_offset` is smaller than
//! the loop's diameter still overlap, however far apart their rows are. Widening
//! `parallel_offset` past `2 * self_loop_radius` separates them; there is no
//! perpendicular to spread them sideways.
//!
//! # The fan
//!
//! `fan` groups edges by **unordered** node pair and gives ascending edge index `i` of
//! a group of `k` the offset `parallel_offset * (i - (k-1)/2)`, centred on the chord. Two
//! chained stable counting sorts, never a map (D4) and never an unstable sort (D5), so
//! the order is fixed by arithmetic and the fan is `O(n + m)`. The displacement is
//! taken along the group's **canonical** direction — lower dense index to higher — which
//! is the one place the grouping and the geometry have to agree; see
//! `shapes`'s note on it.
//!
//! # Determinism
//!
//! `f64` throughout, cast to `f32` only on the final write, so native and wasm32 round
//! alike (D1); `libm::cos`/`libm::sin` for the loop's vertices, never `std`; no `mul_add`
//! (D2); every point's terms summed in the fixed order written above, in gather form —
//! edge `e` reads only the two node columns and its own fan entry, and writes only its
//! own row (D10) — so this module can later run on the SIMD, threaded and GPU tiers
//! unchanged. No clock, no randomness (D8), no `usize` on the wire (D6).
//!
//! No Ponytail marker is owed on the generators, which is what Phase 8 asks for and what
//! the arithmetic supports: no threshold, no sampling, no fallback, no estimate. The one
//! ceiling, `params::MAX_LOOP_SEGMENTS`, is a parameter rule and carries its marker there.
//! Three choices are conventions rather than computations and are stated rather than
//! hidden — the self-loop's half-radius lift, the `L`/`Z` corner rule, and an exact zero as
//! the loop test (`shapes`) — the same treatment `layout/circular.rs` gives its ring
//! spacing.

use crate::arena::CapacityError;
use crate::index::Topology;
use crate::post::styles::fan::fan;
use crate::post::styles::shapes::{Sheet, bend};
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, Paths};

mod fan;
mod ledger;
mod params;
mod shapes;

#[cfg(test)]
mod tests;

pub use ledger::{POST_STYLE_CEILING, STYLES, StyleCapability, find};
pub use params::StyleParams;

/// The elbow shape an orthogonal style takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orthogonal {
    /// One bend: a horizontal leg out of the source, then a vertical one into the
    /// target, so the elbow sits at `(target.x, source.y)`.
    L,
    /// Two bends around a vertical middle leg at the chord's midpoint — the reference's
    /// `CENTERED`. With the two endpoints level the bends collapse onto the chord and the
    /// row is a straight line, which is exact and needs no special case.
    #[default]
    Z,
}

/// Which style a snapshot's edges are drawn in. Each variant is one registered
/// capability, and the geometry kind it emits is fixed — a style never emits two kinds,
/// so a ledger row can name one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Style {
    /// A straight chord: the node centres, and nothing else. The one style with no row
    /// generator, because [`EdgeGeometry::Line`]
    /// stores no interior point — see the
    /// module doc for what that costs a parallel pair and how it is refused rather than
    /// drawn on top of itself.
    Straight,
    /// Right angles, as a polyline; a fanned `L` is the exception, its one bend moved
    /// off both legs' levels (`shapes.rs`, `orthogonal_row`).
    Orthogonal,
    /// A quadratic bezier: one control point per edge.
    Quadratic,
    /// A cubic bezier: two control points per edge.
    Bezier,
}

impl Style {
    /// Every style, in the order the ledger lists them.
    pub const ALL: [Self; 4] = [
        Self::Straight,
        Self::Orthogonal,
        Self::Quadratic,
        Self::Bezier,
    ];

    /// The style registered under `id`, if it is one of this module's.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.id() == id)
    }

    /// The capability id it is registered and hashed under.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Straight => "post.style.straight",
            Self::Orthogonal => "post.style.orthogonal",
            Self::Quadratic => "post.style.quadratic",
            Self::Bezier => "post.style.bezier",
        }
    }

    /// The edge geometry kind this style emits, constant per style.
    pub const fn kind(self) -> EdgeGeometryKind {
        match self {
            Self::Straight => EdgeGeometryKind::Line,
            Self::Orthogonal => EdgeGeometryKind::Polyline,
            Self::Quadratic | Self::Bezier => EdgeGeometryKind::Curve,
        }
    }

    /// The curve degree it stamps on its [`EdgeGeometry::Curve`], or 0 for the two
    /// styles that emit no curve.
    pub const fn degree(self) -> u32 {
        match self {
            Self::Quadratic => 2,
            Self::Bezier => 3,
            Self::Straight | Self::Orthogonal => 0,
        }
    }
}

/// Every edge of `topology` drawn in `params.style`, reading node centres out of
/// `nodes` and writing one CSR row per edge in edge order.
///
/// Refused, before any point is written, when the node geometry does not fit the
/// topology (which is where D9 catches a non-finite centre, before it can reach the
/// wire) or when a parameter breaks the rule its field documents; and after, when the
/// point CSR would pass `2^32 - 1` points (D6) or a point left the `f32` range (D9). Gather form (D10): edge `e` reads only the two node
/// columns and its own fan entry, and writes only its own row.
pub fn style_edges(
    topology: &Topology,
    nodes: &NodeGeometry,
    params: &StyleParams,
) -> Result<EdgeGeometry, StageError> {
    // `None` z: a style reads and writes edge geometry only, and it never sees the
    // snapshot's z column, so it cannot check one. The z column is checked where it enters
    // and leaves the payload, `Snapshot::new` — never here, and never rewritten here.
    nodes
        .check(topology.node_count(), None)
        .map_err(StageError::Snapshot)?;
    params::refuse(params)?;
    if params.style == Style::Straight {
        return Ok(EdgeGeometry::Line);
    }
    let (x, y) = centres(nodes);
    let fan = fan(topology, f64::from(params.parallel_offset)).map_err(StageError::Capacity)?;
    let paths = build(topology, &Sheet::new(x, y, &fan), params)?;
    Ok(match params.style {
        Style::Orthogonal => EdgeGeometry::Polyline(paths),
        Style::Quadratic | Style::Bezier => EdgeGeometry::Curve {
            degree: params.style.degree(),
            paths,
        },
        Style::Straight => EdgeGeometry::Line,
    })
}

/// The `x`/`y` a style reads. `Circle` and `Box` carry the same two columns as `Point`
/// and differ only in a radius or a size, which a style has no use for — that is exactly
/// why a style composes with every layout instead of being written once per layout.
fn centres(nodes: &NodeGeometry) -> (&[f32], &[f32]) {
    match nodes {
        NodeGeometry::Point { x, y } => (x, y),
        NodeGeometry::Circle { x, y, .. } => (x, y),
        NodeGeometry::Box { x, y, .. } => (x, y),
    }
}

/// Every edge's row, in edge order, as a `Polyline`- or `Curve`-shaped CSR: `m + 1`
/// offsets from 0, and `2 x` the last offset coordinates. The tail offset is pushed
/// after the last row, which is the step that only the zero-edge and single-edge
/// boundaries of `tests/edge_geometry_invariants.rs` can see missing.
///
/// Finite centres can still bend past the `f32` range — a chord near `f32::MAX` pushes its
/// control point beyond it, and the one cast to `f32` makes that `±inf` — so the written
/// column is refused whole rather than handed to the wire non-finite (D9).
fn build(t: &Topology, sheet: &Sheet<'_>, params: &StyleParams) -> Result<Paths, StageError> {
    let m = t.edge_count();
    let mut offsets = Vec::with_capacity(m as usize + 1);
    let mut pts: Vec<f32> = Vec::new();
    offsets.push(0u32);
    for e in 0..m {
        bend(t, sheet, params, e).push(&mut pts);
        offsets.push(offset(&pts)?);
    }
    if !pts.iter().all(|v| v.is_finite()) {
        return Err(StageError::NonFinite { column: "edge.pts" });
    }
    Ok(Paths { offsets, pts })
}

/// A CSR offset: `points` scalar `f32`s, so the number of *points* is half that and the
/// wire's offset is a `u32` (D6). Every row this module writes holds an even number of
/// scalars, so the halving is exact; the checked conversion is what refuses a graph too
/// large to describe, never a silent truncation.
fn offset(points: &[f32]) -> Result<u32, StageError> {
    u32::try_from(points.len() / 2).map_err(|_| {
        StageError::Capacity(CapacityError {
            what: "edge style offsets",
        })
    })
}
