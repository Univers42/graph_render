//! The row generators: one edge in, its interior points out. Split from the parent for
//! the house line limit — the parent's module doc carries the stage-level conventions,
//! and this file carries the two that only the arithmetic can state: what a
//! perpendicular and a bulge are here, and where a zero-length chord goes instead.
//!
//! Everything here is `f64`, cast to `f32` once per coordinate in [`Bend::write`], so
//! native and wasm32 round alike (D1). For a chord `p0 -> p1` with `d = p1 - p0`:
//!
//! - the **perpendicular** is `(d.y, -d.x)` — SciGraphs' `cross(d, +z)` with its
//!   `1/length` scaling already applied. The reference then multiplies by a
//!   length-proportional offset, and the two cancel exactly, so the bulge needs no
//!   division and no square root. The one place a length is taken is
//!   [`perpendicular_shift`], where a fan offset has to be scaled onto a unit direction.
//! - the **bulge** is `curvature` to the side the chord's own diagonal picks, `+1` when
//!   `p0.x + p0.y > p1.x + p1.y` and `-1` otherwise. That is SciGraphs' `AUTO`
//!   direction rule, the tie taking `-1`, and it is what bends two edges between the
//!   same pair to opposite sides.
//!
//! A **zero-length chord** — a self-loop, or two distinct nodes a layout placed on one
//! spot — takes the loop shape whatever the style is. No perpendicular exists there, so
//! its fan displacement slides the loop's centre up `y` instead, the axis the loop is
//! already lifted along.

use crate::index::Topology;
use crate::post::styles::{Orthogonal, Style, StyleParams};
use core::f64::consts::PI;

/// One full turn, the divisor of a self-loop's vertex angles.
const TAU: f64 = 2.0 * PI;

/// A position in `f64`, the only arithmetic type these generators use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pt {
    /// Horizontal.
    pub(super) x: f64,
    /// Vertical.
    pub(super) y: f64,
}

impl Pt {
    /// `a + (b - a) * t`, in that order — the reference's own expression, so a cubic's
    /// quarter and three-quarter control points land where it puts them.
    fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
        Pt {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
        }
    }

    /// Coordinate-wise addition, the only way a fan displacement is ever applied.
    fn shift(self, by: Pt) -> Pt {
        Pt {
            x: self.x + by.x,
            y: self.y + by.y,
        }
    }

    /// Component-wise scaling by `k`.
    fn scale(self, k: f64) -> Pt {
        Pt {
            x: self.x * k,
            y: self.y * k,
        }
    }

    /// The midpoint of two points, `(a + b) / 2` in that order.
    fn mid(a: Pt, b: Pt) -> Pt {
        Pt {
            x: (a.x + b.x) * 0.5,
            y: (a.y + b.y) * 0.5,
        }
    }
}

/// The two node columns and the fan, read together so the per-edge step takes one
/// argument instead of three.
pub(super) struct Sheet<'a> {
    x: &'a [f32],
    y: &'a [f32],
    fan: &'a [f64],
}

impl<'a> Sheet<'a> {
    /// The columns and the per-edge fan offsets, already computed.
    pub(super) fn new(x: &'a [f32], y: &'a [f32], fan: &'a [f64]) -> Self {
        Self { x, y, fan }
    }

    /// Node `v`'s centre, widened to `f64` — the only place a `f32` becomes a `f64`,
    /// and the reason the arithmetic above is `f64` on both targets (D1).
    fn at(&self, v: u32) -> Pt {
        Pt {
            x: f64::from(self.x[v as usize]),
            y: f64::from(self.y[v as usize]),
        }
    }
}

/// One edge, ready to be written: its two endpoints, the displacement that separates it
/// from its parallel siblings, and the style parameters. Holding them together is what
/// keeps every generator at one parameter.
pub(super) struct Bend {
    p0: Pt,
    p1: Pt,
    shift: Pt,
    params: StyleParams,
}

impl Bend {
    /// Appends this edge's row. Gather form (D10): this reads only what the struct
    /// already holds — the two node centres and this edge's own fan entry — and writes
    /// only its own row.
    ///
    /// [`Style::Straight`] writes nothing, and that is not a hole: it emits
    /// [`graph_contract::geometry::EdgeGeometry::Line`], which carries no columns at
    /// all, and [`crate::post::styles::style_edges`] returns it before any row is built
    /// — refusing a non-zero `parallel_offset` for it, because a row it never writes is
    /// the only place a gap could live. So the arm is empty rather than unreachable.
    pub(super) fn push(&self, pts: &mut Vec<f32>) {
        if self.p0 == self.p1 {
            self.loop_row(pts);
            return;
        }
        match self.params.style {
            Style::Straight => {}
            Style::Orthogonal => self.orthogonal_row(pts),
            Style::Quadratic => self.quadratic_row(pts),
            Style::Bezier => self.cubic_row(pts),
        }
    }

    /// One bend for `L` — horizontal out of the source, so the elbow sits at
    /// `(target.x, source.y)` — and two for `Z`, which adds a second bend at
    /// `(elbow.x, target.y)`. Both bends sit on one `elbow_x`, which is what keeps the
    /// middle leg vertical and the row right-angled, and both take the same fan
    /// displacement, so a fanned elbow stays a right angle. With the endpoints level the
    /// two bends collapse onto the chord and the row is a straight line, which is exact
    /// and needs no special case.
    fn orthogonal_row(&self, pts: &mut Vec<f32>) {
        let elbow_x = match self.params.orthogonal {
            Orthogonal::L => self.p1.x,
            Orthogonal::Z => (self.p0.x + self.p1.x) * 0.5,
        };
        self.write(
            Pt {
                x: elbow_x,
                y: self.p0.y,
            }
            .shift(self.shift),
            pts,
        );
        if self.params.orthogonal == Orthogonal::Z {
            self.write(
                Pt {
                    x: elbow_x,
                    y: self.p1.y,
                }
                .shift(self.shift),
                pts,
            );
        }
    }

    /// One control point: the chord's midpoint pushed off it by half the bulge.
    fn quadratic_row(&self, pts: &mut Vec<f32>) {
        let control = Pt::mid(self.p0, self.p1)
            .shift(self.perp().scale(self.bulge() * 0.5))
            .shift(self.shift);
        self.write(control, pts);
    }

    /// Two control points, at a quarter and three quarters of the chord, each pushed off
    /// by a quarter of the bulge — the reference's cubic.
    fn cubic_row(&self, pts: &mut Vec<f32>) {
        let push = self.perp().scale(self.bulge() * 0.25);
        for t in [0.25, 0.75] {
            self.write(
                Pt::lerp(self.p0, self.p1, t).shift(push).shift(self.shift),
                pts,
            );
        }
    }

    /// The self-loop's vertices: a regular `self_loop_segments`-gon of circumradius
    /// `self_loop_radius`, centred half a radius **above** the node. SciGraphs'
    /// `generate_self_loop` pushes its loop off the node for the same reason, and a 2-D
    /// graph has no third axis, so the in-plane half of its push is what is kept. A
    /// vertex reaches the node centre only at radius zero, which the parent refuses.
    ///
    /// The row is a closed chain of vertices, not one curve segment: that is what
    /// SciGraphs emits too, marking every style point `is_intersection=0` and letting
    /// the vertex list carry as many as the shape needs. The angle is `i * TAU / n`, in
    /// that order and never folded, so vertex `i` is the same vertex on every target.
    ///
    /// The chord is zero-length by construction — [`Self::push`] checks it before getting
    /// here — so the node is `p0` and `p1` alike; the assertion below says so rather than
    /// leaving it to be re-derived.
    fn loop_row(&self, pts: &mut Vec<f32>) {
        debug_assert_eq!(
            self.p0, self.p1,
            "a loop is only drawn on a zero-length chord"
        );
        let r = f64::from(self.params.self_loop_radius);
        let n = self.params.self_loop_segments;
        let centre = self.p0.shift(Pt {
            x: 0.0,
            y: r * 0.5 + self.shift.y,
        });
        for i in 0..n {
            let angle = f64::from(i) * TAU / f64::from(n);
            let vertex = centre.shift(Pt {
                x: r * libm::cos(angle),
                y: r * libm::sin(angle),
            });
            self.write(vertex, pts);
        }
    }

    /// The chord turned a quarter turn clockwise, `(d.y, -d.x)`.
    fn perp(&self) -> Pt {
        Pt {
            x: self.p1.y - self.p0.y,
            y: self.p0.x - self.p1.x,
        }
    }

    /// The bulge: `curvature` to the side the chord's own diagonal picks, so two edges
    /// between the same pair bend to opposite sides. The tie takes `-1`, as the
    /// reference's `AUTO` does.
    fn bulge(&self) -> f64 {
        let side = if self.p0.x + self.p0.y > self.p1.x + self.p1.y {
            1.0
        } else {
            -1.0
        };
        f64::from(self.params.curvature) * side
    }

    /// The one cast to `f32` in this module, and the only place a point reaches the wire.
    fn write(&self, p: Pt, pts: &mut Vec<f32>) {
        pts.push(p.x as f32);
        pts.push(p.y as f32);
    }
}

/// Edge `e` of `t` as one bend. A zero-length chord has no perpendicular, so its fan
/// displacement slides the shape up `y` — the axis the loop is already lifted along —
/// rather than turning a direction that does not exist.
pub(super) fn bend(t: &Topology, sheet: &Sheet<'_>, params: &StyleParams, e: u32) -> Bend {
    let (s, g) = (t.edges().source[e as usize], t.edges().target[e as usize]);
    let (p0, p1) = (sheet.at(s), sheet.at(g));
    let amount = sheet.fan[e as usize];
    let shift = match p0 == p1 {
        true => Pt { x: 0.0, y: amount },
        false => canonical(sheet, s, g, amount),
    };
    Bend {
        p0,
        p1,
        shift,
        params: *params,
    }
}

/// The fan displacement along the chord's **canonical** direction — lower dense index to
/// higher, never the edge's own source-to-target.
///
/// This is the one place the fan's grouping and its direction have to agree. A reversed
/// pair (`a -> b` beside `b -> a`) is a single group, so the two edges are handed
/// opposite fan amounts; taking each one's perpendicular from its own direction would
/// flip the vector as well and hand both the *same* line — the overlay the fan exists to
/// prevent, arrived at by a route that looks right edge by edge. Keying the direction to
/// the group is what makes the amounts separate anything.
fn canonical(sheet: &Sheet<'_>, s: u32, g: u32, amount: f64) -> Pt {
    let (lo, hi) = if s <= g { (s, g) } else { (g, s) };
    perpendicular_shift(sheet.at(lo), sheet.at(hi), amount)
}

/// The fan displacement of `amount` along the chord's clockwise perpendicular:
/// `(d.y, -d.x) * amount / |d|`. Zero for a zero-length chord, where no perpendicular
/// exists — the only square root in this module, and the only division.
fn perpendicular_shift(p0: Pt, p1: Pt, amount: f64) -> Pt {
    let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
    let length = libm::sqrt(dx * dx + dy * dy);
    if length == 0.0 {
        return Pt { x: 0.0, y: 0.0 };
    }
    let k = amount / length;
    Pt {
        x: dy * k,
        y: -dx * k,
    }
}
