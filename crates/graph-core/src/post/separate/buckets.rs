//! The uniform grid the sweep walks: nodes bucketed into cells, and a node's 3 × 3
//! neighbourhood enumerated in dense cell order.
//!
//! **Cell side is `2 · (largest radius + margin)`, and discs are filed by their CENTRE.**
//! Together those two facts are what make the 3 × 3 neighbourhood complete by construction
//! rather than by search. A pair that can touch is at distance at most
//! `r_i + r_j + 2 · margin ≤ 2 · (largest + margin)` — one cell side — so the two centres are
//! within one cell side, and two values within one cell side differ by at most one cell
//! index. Hence same cell or one of the eight around it, and no pair can hide outside.
//!
//! Filing by **centre** is load-bearing, and filing by a disc's *lower corner* instead is a
//! real bug rather than a slower version: two lower corners can be up to
//! `d + r_i + r_j` apart for the same `d`, which is up to two cell sides, so the pair lands
//! two cells apart and a 3 × 3 walk silently misses it. That was measured — the invariant test
//! failed with discs stacked and reported a worst overlap of 2.0 at any iteration cap, because
//! the sweep could not see the pairs it needed to push apart. It would need a 5 × 5 walk to be
//! correct, which is nine more cell reads per node for nothing.
//!
//! Any *smaller* side would silently miss pairs too, so this is the one size where "smaller is
//! cheaper" is a bug.
//!
//! Sized on the **largest** radius, not the median — the uniform-grid ponytail in
//! `super`'s doc. One huge node among a million small ones sets the cell side for all of
//! them, so its neighbourhood is near-global. Cost, never correctness.
//!
//! The same bound is what makes the residual count exact over the grid and not over all pairs:
//! a pair overlapping by more than the tolerance is nearer than one cell side, so it is in the
//! 3 × 3.
//!
//! Determinism: buckets are a counting sort, so the node indices in every bucket are in
//! ascending dense order (D2, D4). The kernel's reduction therefore sums over a fixed order,
//! and nothing here iterates a hash map.

use crate::stage::StageError;

/// A grid's shape: cells along x and along y, x fastest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// Cells along x.
    pub nx: u32,
    /// Cells along y.
    pub ny: u32,
}

impl Shape {
    /// The grid's total cell count.
    pub fn cells(&self) -> u32 {
        self.nx * self.ny
    }
}

/// One node's cell neighbourhood, as the flat cell indices to walk.
pub struct Neighbourhood {
    /// The cells, up to nine.
    cells: [u32; 9],
    /// How many of `cells` are real — fewer than nine at an edge of the grid.
    len: usize,
}

impl Neighbourhood {
    /// The cells around `cell`, clipped to the grid and in ascending flat index order, which
    /// is the row-major order of the 3 × 3 stencil walked x-innermost.
    pub fn of(cell: u32, shape: Shape) -> Self {
        let (nx, ny) = (shape.nx, shape.ny);
        let (cx, cy) = (cell % nx, cell / nx);
        let mut cells = [0u32; 9];
        let mut len = 0usize;
        for dy in -1i64..=1 {
            let y = i64::from(cy) + dy;
            if !(0..i64::from(ny)).contains(&y) {
                continue;
            }
            for dx in -1i64..=1 {
                let x = i64::from(cx) + dx;
                if !(0..i64::from(nx)).contains(&x) {
                    continue;
                }
                cells[len] = y as u32 * nx + x as u32;
                len += 1;
            }
        }
        Self { cells, len }
    }

    /// The real cells, ascending.
    pub fn cells(&self) -> &[u32] {
        &self.cells[..self.len]
    }
}

/// The node-to-cell buckets, CSR-shaped: cell `c` owns `start[c]..start[c + 1]` of `nodes`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Buckets {
    /// One `cells + 1` offset per cell, dense cell order.
    start: Vec<u32>,
    /// One cell per node, in node order.
    cell_of: Vec<u32>,
    /// The node indices, grouped by cell and ascending within each.
    nodes: Vec<u32>,
}

impl Buckets {
    /// Buckets `cell_of` into CSR by a counting sort. Dense in, dense out: the node indices
    /// within a cell are ascending because the counting pass walks nodes in order.
    pub fn build(cell_of: &[u32], cells: u32) -> Self {
        let mut start = vec![0u32; cells as usize + 1];
        for c in cell_of {
            start[(*c + 1) as usize] += 1;
        }
        for c in 1..=cells as usize {
            start[c] += start[c - 1];
        }
        let mut nodes = vec![0u32; cell_of.len()];
        let mut cursor = start.clone();
        for (node, c) in cell_of.iter().enumerate() {
            nodes[cursor[*c as usize] as usize] = node as u32;
            cursor[*c as usize] += 1;
        }
        Self {
            start,
            cell_of: cell_of.to_vec(),
            nodes,
        }
    }

    /// The nodes in `cell`, as a slice of ascending node indices.
    pub fn of(&self, cell: u32) -> &[u32] {
        let (from, to) = (
            self.start[cell as usize] as usize,
            self.start[cell as usize + 1] as usize,
        );
        &self.nodes[from..to]
    }

    /// The cell node `node` is in.
    pub fn cell_of(&self, node: usize) -> u32 {
        self.cell_of[node]
    }
}

/// A grid over a fixed set of positions: its shape, its origin and its cell side, plus the
/// buckets. Rebuilt once per sweep, because the positions move.
pub struct Grid {
    /// Cells along x and along y.
    pub shape: Shape,
    /// The buckets.
    pub buckets: Buckets,
}

impl Grid {
    /// The grid for these positions and radii, at the cell side `cell` implies.
    ///
    /// The origin is the minimum **centre** over all discs, minus one cell of slack, so no
    /// disc sits left of or below the first cell and the division never sees a negative
    /// quotient. The shape is chosen so the drawing fits at that one cell side — cells are
    /// cubic, so the longer axis sets the count.
    ///
    /// Refused past [`MAX_CELLS`]: a grid that large cannot be indexed in `u32` offsets, and
    /// returning an error is the only honest answer to a caller that asked for it.
    pub fn over(x: &[f32], y: &[f32], cell: f32) -> Result<Self, StageError> {
        let n = x.len();
        if n == 0 {
            return Ok(Self {
                shape: Shape { nx: 1, ny: 1 },
                buckets: Buckets::build(&[], 1),
            });
        }
        let (mut lx, mut ly, mut hx, mut hy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for i in 0..n {
            // **Centres, not lower corners** — see this module's doc for why that is
            // load-bearing rather than a choice.
            lx = lx.min(x[i]);
            ly = ly.min(y[i]);
            hx = hx.max(x[i]);
            hy = hy.max(y[i]);
        }
        let (origin_x, origin_y) = (lx - cell, ly - cell);
        let nx = span(hx - lx, cell);
        let ny = span(hy - ly, cell);
        if u64::from(nx) * u64::from(ny) > MAX_CELLS {
            return Err(StageError::Param {
                name: "node extent",
                rule: "a grid past u32::MAX / 16 cells",
            });
        }
        let shape = Shape { nx, ny };
        let cell_of: Vec<u32> = (0..n)
            .map(|i| Self::cell_at(x[i], y[i], &shape, origin_x, origin_y, cell))
            .collect();
        Ok(Self {
            shape,
            buckets: Buckets::build(&cell_of, shape.cells()),
        })
    }

    /// The cell a disc's **centre** is in, under the half-open boundary rule
    /// [`crate::post::grid_index`] uses: an exact boundary belongs to the cell it opens.
    fn cell_at(x: f32, y: f32, shape: &Shape, ox: f32, oy: f32, cell: f32) -> u32 {
        let ix = axis_cell(x, ox, cell, shape.nx);
        let iy = axis_cell(y, oy, cell, shape.ny);
        iy * shape.nx + ix
    }

    /// The neighbourhood of the cell `node` is in.
    pub fn around(&self, node: usize) -> Neighbourhood {
        Neighbourhood::of(self.buckets.cell_of(node), self.shape)
    }
}

/// The most cells a grid may hold, `u32::MAX / 16`: the CSR holds one `u32` offset per cell
/// plus one, and the offsets must not wrap.
///
/// Ponytail: a width limit, not a memory one. Failing input: a drawing whose discs span more
/// than ~268 million cells at the cell side the largest radius implies. Direction: the pass
/// refuses rather than allocating, so an over-wide drawing is an error a caller sees instead
/// of an abort. Escape hatch: pass `margin` and `point_radius` at 0 and scale the drawing.
const MAX_CELLS: u64 = u32::MAX as u64 / 16;

/// Cells across a span of `extent` at a cell side of `cell`, at least 2: one for the drawing
/// and one for the slack the origin left, so no cell is empty of the whole grid.
fn span(extent: f32, cell: f32) -> u32 {
    let q = libm::ceilf(extent / cell);
    (q as i64 + 2).clamp(2, i64::from(u32::MAX)) as u32
}

/// The cell index holding `value` on one axis, by the same half-open boundary rule
/// [`crate::post::grid_index::axis_cell`] uses, including its tolerance snap, so a
/// coordinate on a boundary belongs to the cell it opens rather than to whichever side the
/// division rounded to. Clamped into `0..n`.
fn axis_cell(value: f32, origin: f32, cell: f32, n: u32) -> u32 {
    let q = (value - origin) / cell;
    let nearest = libm::roundf(q);
    let snapped = if libm::fabsf(q - nearest) <= 1e-9 {
        nearest
    } else {
        q
    };
    snapped.clamp(0.0, n as f32 - 1.0) as u32
}