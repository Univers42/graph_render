//! Per-edge frames and the four-term compatibility `Ce = Ca · Cs · Cp · Cv`
//! (`fdeb.py:frames` and `fdeb.py:compatibility`; the three-term form is restated
//! independently in `edge_styles.py:312-340`).
//!
//! Every term is a function of the two edges' *endpoints*, and FDEB pins the endpoints, so
//! a frame is built once and every term is constant for the whole run. That is what makes
//! the pair list a build-once list rather than a per-iteration recomputation.
//!
//! The reference computes in `float32` and so does this: the terms are multiplied in its
//! order — `angle * scale * pos`, then `· Cv` — because a product is not associative and
//! the order is the port.

use crate::stage::StageError;

/// The reference's guard on a length in a denominator: `1e-12` against a zero edge
/// (`fdeb.py:88-93`). An edge of no length is a node joined to itself.
const LEN_EPS: f32 = 1e-12;

/// A 2D point, as the motor carries coordinates: `f32`, never `f64` (D1 is about the
/// transcendentals; the geometry columns themselves are `f32`).
type Point = (f32, f32);

/// What one edge contributes to every compatibility it is asked about: its start, its unit
/// direction, its length and its midpoint. The far endpoint is not carried — `a + u·len`
/// reconstructs it, and the visibility term reads the reconstruction, as the reference does
/// (`fdeb.py:60`).
#[derive(Debug, Clone, PartialEq)]
pub struct Frames {
    a: Vec<Point>,
    b: Vec<Point>,
    u: Vec<Point>,
    len: Vec<f32>,
    mid: Vec<Point>,
}

impl Frames {
    /// One frame per edge, from the node columns and the two endpoint columns.
    pub fn of(x: &[f32], y: &[f32], source: &[u32], target: &[u32]) -> Result<Frames, StageError> {
        if x.len() != y.len() {
            return Err(StageError::Param {
                name: "node.y",
                rule: "one coordinate per node, as node.x has",
            });
        }
        let mut frames = Frames {
            a: Vec::with_capacity(source.len()),
            b: Vec::with_capacity(source.len()),
            u: Vec::with_capacity(source.len()),
            len: Vec::with_capacity(source.len()),
            mid: Vec::with_capacity(source.len()),
        };
        for (&from, &to) in source.iter().zip(target) {
            let (from, to) = (from as usize, to as usize);
            let (ax, ay) = (node(x, from), node(y, from));
            let (bx, by) = (node(x, to), node(y, to));
            let (dx, dy) = (bx - ax, by - ay);
            let len = libm::hypotf(dx, dy);
            frames.a.push((ax, ay));
            frames.b.push((bx, by));
            frames
                .u
                .push((dx / len.max(LEN_EPS), dy / len.max(LEN_EPS)));
            frames.len.push(len);
            frames.mid.push((0.5 * (ax + bx), 0.5 * (ay + by)));
        }
        Ok(frames)
    }

    /// How many edges are framed.
    pub fn len(&self) -> usize {
        self.a.len()
    }

    /// Edge `e`'s two nodes, as the iteration pins them back after every step.
    pub fn ends(&self, e: u32) -> ((f32, f32), (f32, f32)) {
        (self.a[e as usize], self.b[e as usize])
    }

    /// `Ce` for the pair `(i, j)`: the product of the angle, scale and position terms, and
    /// the visibility term when `visibility` is set — the smaller of the two directions, so
    /// one edge lying off to the side of the other scores zero (`fdeb.py:126-128`).
    pub fn compatibility(&self, i: usize, j: usize, visibility: bool) -> f32 {
        let product = self.angle(i, j) * self.scale(i, j) * self.position(i, j);
        if visibility {
            product * self.visibility(i, j).min(self.visibility(j, i))
        } else {
            product
        }
    }

    /// `Ca`: `|cos θ|` between the two unit directions, so an antiparallel pair scores as
    /// well as a parallel one (`fdeb.py:111`).
    pub fn angle(&self, i: usize, j: usize) -> f32 {
        dot(self.u[i], self.u[j]).abs()
    }

    /// `Cs`: how alike the two lengths are, `2 / (l_avg/min + max/l_avg)` — 1 for equal
    /// lengths, smaller the more they differ (`fdeb.py:113-117`).
    pub fn scale(&self, i: usize, j: usize) -> f32 {
        let (li, lj) = (self.len[i], self.len[j]);
        let avg = 0.5 * (li + lj);
        if avg <= 0.0 {
            // Two zero-length edges (self-loops) are alike; the ratio below would be 0/0.
            return 1.0;
        }
        2.0 / (avg / li.min(lj).max(LEN_EPS) + li.max(lj) / avg.max(LEN_EPS))
    }

    /// `Cp`: how close the two midpoints are, `l_avg / (l_avg + gap)` — 1 for coincident
    /// midpoints, halving at one edge length of separation (`fdeb.py:118-120`).
    pub fn position(&self, i: usize, j: usize) -> f32 {
        let avg = 0.5 * (self.len[i] + self.len[j]);
        let (mi, mj) = (self.mid[i], self.mid[j]);
        let denominator = avg + libm::hypotf(mi.0 - mj.0, mi.1 - mj.1);
        if denominator <= 0.0 {
            // Coincident zero-length edges: the ratio is 0/0 and they are as close as can be.
            return 1.0;
        }
        avg / denominator
    }

    /// `Cv`: how much of edge `i` lies in the shadow of edge `j` (`fdeb.py:52-65`). `j`'s
    /// two endpoints are projected onto the line through `i`, giving an interval; `i`'s
    /// midpoint is compared with that interval's centre, and `1` is discounted by twice the
    /// relative distance. A `j` perpendicular to `i` has a zero-width interval, which the
    /// guard sends to 0 — no shadow is no compatibility.
    pub fn visibility(&self, i: usize, j: usize) -> f32 {
        let t0 = dot(sub(self.a[j], self.a[i]), self.u[i]);
        let t1 = dot(
            sub(far(self.a[j], self.u[j], self.len[j]), self.a[i]),
            self.u[i],
        );
        let middle = 0.5 * (t0 + t1);
        let width = (t1 - t0).abs();
        let outside = (0.5 * self.len[i] - middle).abs();
        (1.0 - 2.0 * outside / width.max(LEN_EPS)).max(0.0)
    }

    /// `u_i · u_j`: the sign picks which of `j`'s points is "corresponding" to `i`'s
    /// (`fdeb.py:180-186` — pairing by raw index opens an antiparallel bundle into an X).
    pub fn dot(&self, i: usize, j: usize) -> f32 {
        dot(self.u[i], self.u[j])
    }
}

/// One coordinate of a column, refused rather than invented when an endpoint is out of
/// range: a node index past the geometry is a caller bug, not a zero.
fn node(column: &[f32], index: usize) -> f32 {
    *column.get(index).unwrap_or(&f32::NAN)
}

/// `a + u · len`: the far endpoint as the reference reconstructs it, one `f32` multiply and
/// add away from the endpoint it was built from.
fn far(a: Point, u: Point, len: f32) -> Point {
    (a.0 + u.0 * len, a.1 + u.1 * len)
}

/// `p - q`.
fn sub(p: Point, q: Point) -> Point {
    (p.0 - q.0, p.1 - q.1)
}

/// `p · q`.
fn dot(p: Point, q: Point) -> f32 {
    p.0 * q.0 + p.1 * q.1
}
