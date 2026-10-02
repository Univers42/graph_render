//! The squarified fill: the row-growing recursion that tiles the field.
//!
//! Reference: `lib/patchwork/tree_map.c` of the pinned Graphviz 16.1.0 release, read as an
//! algorithm reference and reimplemented (never translated, never linked — Graphviz is
//! EPL-1.0 and `docs/decisions/graphviz-oracle.md` records that the port is an independent
//! implementation agreeing on output, not a copy of the source).
//!
//! **The algorithm is Bruls/Huizing/van Wijk's squarified treemap.** A row is kept open
//! while adding the next area *improves* the worst aspect ratio in it, and is closed — laid
//! out along whichever side of the leftover field is longer, then the field shrinks — as soon
//! as adding one more would worsen it. Two details are load-bearing and are easy to lose:
//!
//! 1. **The first area seeds the row's aspect ratio as `max(a/w², w²/a)`, not `1`.** That
//!    ratio is *asymmetric* — for a wide field and a small area it is huge, and it is what
//!    makes the first row long. Seeding it at `1` instead makes every row close immediately
//!    and tiles the field into a chain of one-item rows, which is a different drawing.
//! 2. **A square field takes the row along `x`.** The leftover is compared with `<=`, so
//!    the first row of a square field goes across, and only a field taller than it is wide
//!    turns the next row over to stack along `y`.
//!
//! **Iterative, where the reference recurses.** The reference's `squarify` recurses once per
//! row-growing step *and* once per closed row, so its depth grows with the node count, not
//! with the row count. **Measured on this tree**: the recursive form draws n=9 330 nodes on an
//! 8 MiB stack and aborts at n=9 340 (`scripts/orch/gr -e PROBE_N=<n> cargo test -p
//! graph-core --lib -- how_deep -- --ignored --nocapture`, one size per process because an
//! overflow aborts it).
//! That is below the gate's own largest single-seed model (n=601) by a wide margin and far
//! below the 1 000 000-node ceiling, so the reference's shape could not carry this layout.
//! The pending rows are an explicit stack here, so depth is heap: `bench --layout
//! layout.treemap.patchwork --n 1000000` draws a million nodes in 21 ms at flat stack depth.
//! The row-growing step was a tail call in the reference and is a loop here for the same
//! reason. Nothing else differs: the order of writes, and so the drawing, is identical to the
//! recursive form — `the_tiling_is_identical_to_the_recursive_form` in `tests.rs` pins that
//! against a straight transcription kept in the test.
//!
//! Determinism: the fill walks areas in index order and writes tile `i` at index `i`, so the
//! output order is the dense node order and nothing iterates a hash or compares
//! floating-point keys (`prompt.md` §6 D1-D10). One `sqrt`, IEEE-754's own.

/// Every node's default area, scaled up so that 1 is a reasonable drawing size.
pub const AREA: f64 = 1000.0;

/// A tile: its centre and its size, in points. The centre is what the layout emits; the
/// size is what the area-conservation and unit-area tests read.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// Centre `x`, in points.
    pub(super) x: f64,
    /// Centre `y`, in points.
    pub(super) y: f64,
    /// Width, in points.
    pub(super) w: f64,
    /// Height, in points.
    pub(super) h: f64,
}

impl Tile {
    /// The tile's own area, in square points.
    pub(super) fn area(&self) -> f64 {
        self.w * self.h
    }
}

/// The leftover field a row is laid into: centre and size, in points.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

impl Field {
    /// The square of side `side`, centred on the origin.
    pub(super) fn square(side: f64) -> Self {
        Field {
            x: 0.0,
            y: 0.0,
            w: side,
            h: side,
        }
    }

    /// The shorter of the two sides — the width a row laid across the field is measured by.
    pub(super) fn across(&self) -> f64 {
        self.w.min(self.h)
    }
}

/// `count` tiles of the default area filling a square field of side `sqrt(AREA * count)`,
/// in node order: tile `i` is node `i`.
pub fn tile(count: u32) -> Vec<Tile> {
    if count == 0 {
        return Vec::new();
    }
    let side = (AREA * f64::from(count)).sqrt();
    let areas = vec![AREA; count as usize];
    let mut out = vec![
        Tile {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0
        };
        count as usize
    ];
    fill(&areas, &mut out, Field::square(side));
    // The tiles exactly fill the field. `debug_assertions` is off in the release build the
    // bench measures, so this guards the property in the profile the tests run in and costs
    // the measured build nothing; `the_tiles_conserv_the_fields_area_exactly` is the same
    // claim at every size, and `Tile::area` exists so it reads in one place.
    debug_assert!(
        out.iter().map(Tile::area).sum::<f64>() >= side * side * (1.0 - 1e-9),
        "the tiles left a hole in a field of side {side}"
    );
    out
}

/// The open row's running state: how many areas it holds, their extremes, their sum, and the
/// worst aspect ratio among them so far.
#[derive(Clone, Copy)]
struct Open {
    added: usize,
    max: f64,
    min: f64,
    total: f64,
    asp: f64,
}

/// The row's aspect ratio if `next` were added: the worst of `h/narrowest` and `widest/h`
/// over the enlarged row, where `h` is the row's total area divided by the field's shorter
/// side. Returns the enlarged extremes and total alongside, so growing the row is one
/// assignment per field rather than a recomputation.
fn asp_of(open: &Open, next: f64, across: f64) -> (f64, f64, f64, f64) {
    let max = open.max.max(next);
    let min = open.min.min(next);
    let total = open.total + next;
    let h = total / across;
    let asp = (h / (min / h)).max((max / h) / h);
    (max, min, total, asp)
}

/// Tile `areas` into `field`, writing at `out[offset..]`. Pending rows go on an explicit
/// stack rather than the reference's call stack.
fn fill(areas: &[f64], out: &mut [Tile], root: Field) {
    let mut pending = vec![(0_usize, root)];
    while let Some((offset, mut field)) = pending.pop() {
        let Some(open) = open_row(areas, offset, field) else {
            continue;
        };
        let consumed = open.added;
        field = close(areas, out, offset, &open, field);
        pending.push((offset + consumed, field));
    }
}

/// The row the field's next tile starts, grown for as long as the aspect ratio improves.
/// `None` when there is nothing left to lay out.
fn open_row(areas: &[f64], offset: usize, field: Field) -> Option<Open> {
    let first = *areas.get(offset)?;
    let across = field.across();
    let asp = (first / (across * across)).max(across * across / first);
    let mut open = Open {
        added: 1,
        max: first,
        min: first,
        total: first,
        asp,
    };
    while offset + open.added < areas.len() {
        let (max, min, total, asp) = asp_of(&open, areas[offset + open.added], across);
        if asp > open.asp {
            break;
        }
        open.max = max;
        open.min = min;
        open.total = total;
        open.asp = asp;
        open.added += 1;
    }
    Some(open)
}

/// Lay the open row out along the field's longer side and shrink the field by what it took.
/// Returns the leftover field.
fn close(areas: &[f64], out: &mut [Tile], offset: usize, open: &Open, mut field: Field) -> Field {
    let across = field.across();
    if field.w <= field.h {
        let h = open.total / across;
        let mut edge = field.x - field.w / 2.0;
        for (i, &area) in areas[offset..offset + open.added].iter().enumerate() {
            let w = area / h;
            out[offset + i] = Tile {
                x: edge + w / 2.0,
                y: field.y + field.h / 2.0 - h / 2.0,
                w,
                h,
            };
            edge += w;
        }
        field.y -= h / 2.0;
        field.h -= h;
    } else {
        let w = open.total / across;
        let mut edge = field.y + field.h / 2.0;
        for (i, &area) in areas[offset..offset + open.added].iter().enumerate() {
            let h = area / w;
            out[offset + i] = Tile {
                x: field.x - field.w / 2.0 + w / 2.0,
                y: edge - h / 2.0,
                w,
                h,
            };
            edge -= h;
        }
        field.x += w / 2.0;
        field.w -= w;
    }
    field
}
