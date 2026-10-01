//! The reference's own recursion: the oracle the iterative kernel is graded against.
//!
//! Transcribed from `tree_map.c`'s `squarify` and kept in the tests because it is a check on
//! the port, not part of it — and it is written the reference's way, one recursion per closed
//! row, so "the iterative form draws the same thing" is a claim with a witness
//! (`the_tiling_is_identical_to_the_recursive_form`, in `super`).
//!
//! This is also the stack depth the port removed: the recursive form overflows at n = 9 340 on
//! an 8 MiB stack (`super::how_deep_the_recursive_form_goes_before_the_stack_runs_out`).

use super::super::squarify::{AREA, Field, Tile};

/// The open row's state, recomputed from the areas each time the reference's own
/// arguments carry it: `nadded`, `maxarea`, `minarea`, `totalarea`, `asp`.
pub struct Open {
    added: usize,
    max: f64,
    min: f64,
    total: f64,
    asp: f64,
}

/// `squarify` as `tree_map.c` writes it. `out` is the whole tile array and `offset` is
/// where this level's areas start in it — the reference's `recs + nadded`.
pub(super) fn squarify(
    areas: &[f64],
    out: &mut [Tile],
    offset: usize,
    added: usize,
    open: Open,
    field: Field,
) {
    if areas.is_empty() {
        return;
    }
    let across = field.across();
    if added == 0 {
        let first = areas[0];
        let asp = (first / (across * across)).max(across * across / first);
        let seeded = Open {
            added: 1,
            max: first,
            min: first,
            total: first,
            asp,
        };
        squarify(areas, out, offset, 1, seeded, field);
        return;
    }
    let next = areas.get(added).copied();
    let grown = next.map(|area| {
        let max = open.max.max(area);
        let min = open.min.min(area);
        let total = open.total + area;
        let h = total / across;
        (max, min, total, (h / (min / h)).max((max / h) / h))
    });
    // The reference's `nadded < n && newasp <= asp`, as one guard: growing the row is
    // the branch, and falling through is closing it.
    let improves = grown.filter(|(_, _, _, asp)| *asp <= open.asp);
    if let Some((max, min, total, asp)) = improves {
        let grown = Open {
            added: added + 1,
            max,
            min,
            total,
            asp,
        };
        squarify(areas, out, offset, added + 1, grown, field);
        return;
    }
    let leftover = close(areas, out, offset, &open, field);
    let rest = Open {
        added: 0,
        max: 0.0,
        min: 0.0,
        total: 0.0,
        asp: 1.0,
    };
    squarify(&areas[added..], out, offset + added, 0, rest, leftover);
}

/// Lay the row out along the field's longer side; returns the leftover field.
fn close(areas: &[f64], out: &mut [Tile], offset: usize, open: &Open, mut field: Field) -> Field {
    let across = field.across();
    if field.w <= field.h {
        let h = open.total / across;
        let mut edge = field.x - field.w / 2.0;
        for (i, &area) in areas[..open.added].iter().enumerate() {
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
        for (i, &area) in areas[..open.added].iter().enumerate() {
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

/// `tree_map` over `count` equal areas filling a square field of side `sqrt(AREA*count)`.
pub fn tile(count: u32) -> Vec<Tile> {
    let n = count as usize;
    let areas = vec![AREA; n];
    let mut out = vec![
        Tile {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0
        };
        n
    ];
    let side = (AREA * f64::from(count)).sqrt();
    let open = Open {
        added: 0,
        max: 0.0,
        min: 0.0,
        total: 0.0,
        asp: 1.0,
    };
    squarify(&areas, &mut out, 0, 0, open, Field::square(side));
    out
}
