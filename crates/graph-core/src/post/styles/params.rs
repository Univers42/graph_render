//! One style's parameters, the defaults each capability is registered at, and [`refuse`],
//! which enforces the rules stated on the fields. Split from the parent for the house line
//! limit; the parent's module doc carries the geometry.
//!
//! Every rule is a **refusal**, never a clamp and never a silent fallback: a style handed
//! a value it cannot honour says which field and why, because quietly drawing something
//! else is the one genuinely dangerous defect this module could have.

use super::{Orthogonal, Style};
use crate::stage::StageError;

/// The most vertices a self-loop row may hold.
///
/// Ponytail (loop ceiling): the reference's own — the loop's vertex count is its
/// `edge_segments` (`edge_styles.py:458-459`), whose panel bounds it at 32
/// (`SciGraphs/properties/edge_style_properties.py:78-85`). Unbounded, one self-loop at
/// 1e8 wrote 800 MB before the offset check could refuse. Direction: a caller wanting a
/// smoother loop than a 32-gon is refused, never drawn coarser. Escape hatch: raise this
/// with a re-priced `ledger::COMPLEXITY`.
pub(super) const MAX_LOOP_SEGMENTS: u32 = 32;

/// One style's parameters. [`StyleParams::for_style`] pins the defaults each capability
/// is registered at; the fields are public so a caller can bend further, and
/// [`style_edges`](super::style_edges) refuses any value outside the rules on
/// each field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleParams {
    /// Which style to draw.
    pub style: Style,
    /// The elbow shape, read by [`Style::Orthogonal`] alone.
    pub orthogonal: Orthogonal,
    /// The bend as a fraction of the chord, read by the two curve styles alone, and not
    /// negative: a negative bend would fold onto the side the bulge already picks, so it
    /// is refused rather than silently ignored.
    pub curvature: f32,
    /// The gap between two parallel edges, in the same units as the node centres, and not
    /// negative: the fan orders a group by edge index, and a negative gap would reverse
    /// that order silently. Must be `0.0` for [`Style::Straight`], which emits
    /// [`EdgeGeometry::Line`](graph_contract::geometry::EdgeGeometry::Line) and so
    /// stores no interior point for an offset to live in.
    pub parallel_offset: f32,
    /// A self-loop's circumradius, in the same units as the node centres, and strictly
    /// positive — at zero the loop's vertices land on the node. Two self-loops on one
    /// node separate only once this is below half of `parallel_offset`: the fan slides
    /// their centres, not their circles.
    pub self_loop_radius: f32,
    /// A self-loop's vertex count, from 3 — a one- or two-gon is not a loop — to
    /// `MAX_LOOP_SEGMENTS`.
    pub self_loop_segments: u32,
}

impl StyleParams {
    /// `style`'s pinned defaults: the reference's `GEPHI_DEFAULT` bend of 0.3 and parallel
    /// gap of 0.05 (`edge_styles.py:15`, `:18`), its `CENTERED` orthogonal shape, and the
    /// self-loop radius of 0.2 over 8 vertices that `compute_styled_edge_points` defaults
    /// to (`edge_styles.py:449`, `:453`). [`Style::Straight`] is the one exception: it emits
    /// [`EdgeGeometry::Line`](graph_contract::geometry::EdgeGeometry::Line) and stores no interior
    /// point, so its gap is pinned to `0.0` — the value
    /// [`style_edges`](super::style_edges) accepts for it.
    pub fn for_style(style: Style) -> Self {
        Self {
            style,
            orthogonal: Orthogonal::Z,
            curvature: 0.3,
            parallel_offset: match style {
                Style::Straight => 0.0,
                Style::Orthogonal | Style::Quadratic | Style::Bezier => 0.05,
            },
            self_loop_radius: 0.2,
            self_loop_segments: 8,
        }
    }
}

impl Default for StyleParams {
    fn default() -> Self {
        Self::for_style(Style::Straight)
    }
}

/// The parameters a style refuses, named by the field and the rule it breaks, in the order
/// the fields are declared. A style handed a value it cannot honour says so rather than
/// quietly producing something else — a silent clamp here would be the one genuinely
/// dangerous kind of defect this module could have.
pub(super) fn refuse(params: &StyleParams) -> Result<(), StageError> {
    let bad = |name, rule| Err(StageError::Param { name, rule });
    for (name, value) in [
        ("curvature", params.curvature),
        ("parallel_offset", params.parallel_offset),
        ("self_loop_radius", params.self_loop_radius),
    ] {
        if !value.is_finite() {
            return bad(name, "must be finite");
        }
    }
    if params.curvature < 0.0 {
        return bad("curvature", CURVATURE_SIGN);
    }
    if params.parallel_offset < 0.0 {
        return bad("parallel_offset", OFFSET_SIGN);
    }
    if params.self_loop_radius <= 0.0 {
        return bad("self_loop_radius", RADIUS_SIGN);
    }
    if !(3..=MAX_LOOP_SEGMENTS).contains(&params.self_loop_segments) {
        return bad("self_loop_segments", LOOP_COUNT);
    }
    if params.style == Style::Straight && params.parallel_offset != 0.0 {
        return bad("parallel_offset", STRAIGHT_GAP);
    }
    Ok(())
}

const CURVATURE_SIGN: &str = "must be >= 0: the bend side is chosen from the chord, not signed";
const OFFSET_SIGN: &str = "must be >= 0: the fan orders a group by edge index, not by sign";
const RADIUS_SIGN: &str = "must be > 0: at 0 a loop's vertices sit on the node";
const LOOP_COUNT: &str = "must be 3..=32: a one- or two-gon is not a loop, and 32 is the reference's edge_segments ceiling";
const STRAIGHT_GAP: &str = "must be 0.0 for post.style.straight: Line stores no interior points, so an offset has nowhere to go — use post.style.orthogonal, .quadratic or .bezier";
