//! One style's parameters, and the defaults each capability is registered at. Split from
//! the parent for the house line limit; the parent's module doc carries the geometry, and
//! the parent's `refuse` is what enforces the rules stated on the fields here.
//!
//! Every rule is a **refusal**, never a clamp and never a silent fallback: a style handed
//! a value it cannot honour says which field and why, because quietly drawing something
//! else is the one genuinely dangerous defect this module could have.

use super::{Orthogonal, Style};

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
    /// The gap between two parallel edges, in the same units as the node centres. Must
    /// be `0.0` for [`Style::Straight`], which emits
    /// [`EdgeGeometry::Line`](graph_contract::geometry::EdgeGeometry::Line) and so
    /// stores no interior point for an offset to live in.
    pub parallel_offset: f32,
    /// A self-loop's circumradius, in the same units as the node centres, and strictly
    /// positive — at zero the loop's vertices land on the node. Two self-loops on one
    /// node separate only once this is below half of `parallel_offset`: the fan slides
    /// their centres, not their circles.
    pub self_loop_radius: f32,
    /// A self-loop's vertex count, at least 3: a one- or two-gon is not a loop.
    pub self_loop_segments: u32,
}

impl StyleParams {
    /// `style`'s pinned defaults: the reference's `MINIMAL` bend of 0.3, its `CENTERED`
    /// orthogonal shape, its parallel gap of 0.05 and its self-loop radius of 0.2, over
    /// 8 vertices. [`Style::Straight`] is the one exception: it emits
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
