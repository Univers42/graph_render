//! The subdivision points themselves: how a cycle's starting polyline is built, how it is
//! resampled when the next cycle wants more points, and the one gather-form iteration (D10)
//! every cycle repeats.
//!
//! Points are row-major and every row is the same length: point `(e, p)` is at
//! `2 · (e · k + p)`. That is what lets a threaded tier partition by edge and a SIMD tier
//! vectorise across points without changing the order of any sum.

use super::compat::{Frames, LEN_EPS};
use super::pairs::PairList;
use crate::index::Topology;
use crate::layout::Geometry;
use graph_contract::geometry::Paths;

/// Every edge's points, as one row-major column.
#[derive(Debug, Clone, PartialEq)]
pub struct Points {
    edges: u32,
    per_edge: u32,
    xy: Vec<f32>,
}

impl Points {
    /// `edges` rows of `per_edge` points, all at the origin.
    pub fn empty(edges: u32, per_edge: u32) -> Points {
        Points {
            edges,
            per_edge,
            xy: vec![0.0; 2 * edges as usize * per_edge as usize],
        }
    }

    /// Every edge's starting polyline, resampled to `per_edge` points: the layout's own
    /// interior points between the two nodes, or the straight segment when the layout drew
    /// a `Line`. This is the composition seam — a `Polyline` layout's bends are bundled
    /// like any other path, and a `Line` layout's edges are bundled from straight. `(x, y)`
    /// are the node centres the caller already took from `geometry`.
    pub fn of(
        topology: &Topology,
        geometry: &Geometry,
        (x, y): (&[f32], &[f32]),
        per_edge: u32,
    ) -> Points {
        let mut points = Points::empty(topology.edge_count(), per_edge);
        for e in 0..topology.edge_count() {
            let endpoints = &topology.edges();
            let (from, to) = (
                endpoints.source[e as usize] as usize,
                endpoints.target[e as usize] as usize,
            );
            let row = polyline(geometry, e, (x[from], y[from]), (x[to], y[to]));
            let mut out = vec![(0.0, 0.0); per_edge as usize];
            resample_row(&row, per_edge, &mut out);
            for (p, point) in out.iter().enumerate() {
                points.write(e, p as u32, *point);
            }
        }
        points
    }

    /// How many edges are here, and how many points each has.
    pub fn shape(&self) -> (u32, u32) {
        (self.edges, self.per_edge)
    }

    /// Point `(e, p)`'s coordinates, by index.
    pub fn at(&self, e: u32, p: u32) -> (f32, f32) {
        let at = 2 * (e * self.per_edge + p) as usize;
        (self.xy[at], self.xy[at + 1])
    }

    /// The bundled paths: one `Polyline` row of interior points per edge, the shape the
    /// snapshot's CSR carries.
    pub fn interior_paths(&self) -> Paths {
        let per_edge = self.per_edge - 2;
        let mut pts = Vec::with_capacity(2 * (self.edges * per_edge) as usize);
        for e in 0..self.edges {
            let row = 2 * (e * self.per_edge) as usize;
            pts.extend_from_slice(&self.xy[row + 2..row + 2 * (self.per_edge as usize - 1)]);
        }
        Paths {
            offsets: (0..=self.edges).map(|e| e * per_edge).collect(),
            pts,
        }
    }

    /// Resamples every row to `per_edge` points at even arc length, the reference's
    /// `fdeb.py:resample`. Even spacing rather than inserted midpoints, because the spring
    /// term is a Laplacian and a straight line is a fixed point of it: a row of inserted
    /// midpoints would drift off the segment it started on.
    pub fn resample(&mut self, per_edge: u32) {
        if per_edge == self.per_edge {
            return;
        }
        let mut read = vec![(0.0, 0.0); self.per_edge as usize];
        let mut out = vec![(0.0, 0.0); per_edge as usize];
        let mut next = Points::empty(self.edges, per_edge);
        for e in 0..self.edges {
            for (p, point) in read.iter_mut().enumerate() {
                *point = self.at(e, p as u32);
            }
            resample_row(&read, per_edge, &mut out);
            for (p, point) in out.iter().enumerate() {
                next.write(e, p as u32, *point);
            }
        }
        *self = next;
    }

    /// One gather-form iteration, written into `out`: point `(e, p)` reads the start-of-step
    /// state of its own two neighbours and of every surviving partner's corresponding
    /// point, and writes only itself. The partner sum is taken in ascending edge index — the
    /// row's own order — so it is one fixed sequence of additions per point (D3, D10).
    ///
    /// The force is the reference's (`fdeb.py:289-305`, and `edge_styles.py:381-401`): the
    /// spring is `SPRING_GAIN · (½(p₋ + p₊) − p)`, the attraction is the
    /// compatibility-and-distance weighted mean of the partner displacements, and the step
    /// scales the sum of the two. The first and last point of every row are pinned to the two
    /// nodes: an edge has to touch them, and no force is the reason.
    ///
    /// Ponytail (infinite weight): a partner on the point itself with the softening
    /// underflowed to 0 (a drawing under ~1e-19 across) weighs `compat / 0 = +inf`, and the
    /// reference's mean is then `NaN` (`fdeb.py:290-298`). A non-finite `wsum` takes no
    /// attraction instead: the exact limit when the infinite weight is a coincident partner,
    /// and within the ~1e-19 the overflow needs otherwise. Direction: a sub-1e-19 drawing
    /// is not pulled, cosmetic. Escape hatch: lay the drawing out at a usable scale.
    #[allow(clippy::too_many_arguments)] // The reference's update has one term per argument.
    pub fn step_into(
        &self,
        out: &mut Points,
        pairs: &PairList,
        frames: &Frames,
        strength: f32,
        step: f32,
        soften: f32,
    ) {
        for e in 0..self.edges {
            let row = pairs.row(e);
            let mirrored = self.per_edge - 1;
            for p in 1..(self.per_edge - 1) {
                let (x, y) = self.at(e, p);
                let (bx, by) = self.at(e, p - 1);
                let (cx, cy) = self.at(e, p + 1);
                let spring = (0.5 * (bx + cx) - x, 0.5 * (by + cy) - y);
                let (mut ax, mut ay, mut wsum) = (0.0, 0.0, 0.0);
                for entry in row {
                    let q = if entry.flipped { mirrored - p } else { p };
                    let (jx, jy) = self.at(entry.partner, q);
                    let (dx, dy) = (jx - x, jy - y);
                    let weight = entry.compat / ((dx * dx + dy * dy) + soften);
                    ax += weight * dx;
                    ay += weight * dy;
                    wsum += weight;
                }
                let attract = if wsum > 0.0 && wsum.is_finite() {
                    (ax / wsum, ay / wsum)
                } else {
                    (0.0, 0.0)
                };
                out.write(e, p, move_point(x, y, step, strength, spring, attract));
            }
            let (start, end) = frames.ends(e);
            out.write(e, 0, start);
            out.write(e, self.per_edge - 1, end);
        }
    }

    /// Writes a whole row of points, edge `e`'s, in order.
    #[cfg(test)]
    pub fn write_row(&mut self, e: u32, row: &[(f32, f32)]) {
        for (p, point) in row.iter().enumerate() {
            self.write(e, p as u32, *point);
        }
    }

    /// Writes one point.
    fn write(&mut self, e: u32, p: u32, point: (f32, f32)) {
        let at = 2 * (e * self.per_edge + p) as usize;
        self.xy[at] = point.0;
        self.xy[at + 1] = point.1;
    }
}

/// `p + step · (SPRING_GAIN · spring + strength · attract)`, the reference's update
/// (`fdeb.py:303-305`).
fn move_point(
    x: f32,
    y: f32,
    step: f32,
    strength: f32,
    spring: (f32, f32),
    attract: (f32, f32),
) -> (f32, f32) {
    let fx = super::SPRING_GAIN * spring.0 + strength * attract.0;
    let fy = super::SPRING_GAIN * spring.1 + strength * attract.1;
    (x + step * fx, y + step * fy)
}

/// Edge `e`'s drawn path with its two node endpoints attached: the layout's own interior
/// points between the nodes, or a straight segment when the layout drew a `Line`. A `Line`
/// stores no interior points, so this is the composition seam — no assumption about which
/// layout produced the geometry.
fn polyline(geometry: &Geometry, e: u32, from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32)> {
    let mut row = Vec::with_capacity(2 + crate::post::row(&geometry.edges, e).len() / 2);
    row.push(from);
    row.extend(
        crate::post::row(&geometry.edges, e)
            .as_chunks::<2>()
            .0
            .iter()
            .map(|point| (point[0], point[1])),
    );
    row.push(to);
    row
}

/// One row resampled to `k` points at even arc length: `fdeb.py:resample`, in 2D. The
/// segment a target arc length falls in is found by counting the segments short of it,
/// exactly as the reference counts them, and the first and last points are the row's own.
fn resample_row(row: &[(f32, f32)], k: u32, out: &mut [(f32, f32)]) {
    if k <= 1 || row.len() < 2 {
        for point in out.iter_mut() {
            *point = row.first().copied().unwrap_or((0.0, 0.0));
        }
        return;
    }
    let cumulative = cumulative(row);
    let total = cumulative[cumulative.len() - 1];
    for (j, point) in out.iter_mut().enumerate() {
        *point = at_arc(row, &cumulative, total * (j as f32 / (k - 1) as f32));
    }
    out[0] = row[0];
    out[k as usize - 1] = row[row.len() - 1];
}

/// The arc length reached at each point of `row`, from 0.
fn cumulative(row: &[(f32, f32)]) -> Vec<f32> {
    let mut out = Vec::with_capacity(row.len());
    let mut total = 0.0_f32;
    out.push(total);
    for pair in row.windows(2) {
        total += libm::hypotf(pair[1].0 - pair[0].0, pair[1].1 - pair[0].1);
        out.push(total);
    }
    out
}

/// The point of `row` at arc length `target`: the segment it falls in, found by counting
/// the segments short of it as the reference does, then the fraction along that segment. A
/// degenerate segment (two coincident points) takes its own start, which is the reference's
/// `f = 0` guard. The lerp is `a + (b − a)·f` where `fdeb.py:171` writes `a·(1 − f) + b·f`,
/// and the fraction is `j / (k − 1)` in f32 where `fdeb.py:158` takes `np.linspace`: equal in
/// ℝ, not bit for bit, which [`super::META`]'s oracle row already disclaims.
fn at_arc(row: &[(f32, f32)], cumulative: &[f32], target: f32) -> (f32, f32) {
    let last = cumulative.len() - 2;
    let mut segment = 0_usize;
    while segment < last && cumulative[segment + 1] < target {
        segment += 1;
    }
    let low = cumulative[segment];
    let length = cumulative[segment + 1] - low;
    let f = if length > LEN_EPS {
        ((target - low) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (a, b) = (row[segment], row[segment + 1]);
    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
}
