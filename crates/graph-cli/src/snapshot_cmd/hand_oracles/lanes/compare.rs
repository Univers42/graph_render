//! `lanes`'s snapshot, held to its own convention bit for bit and to the three structural
//! invariants the spec lists.
//!
//! **Which columns are compared, and at what node counts.** All four: node `x` and node `y`
//! individually, the polyline's `offsets` and every `pts` pair, and the notes as a whole
//! `(code, index)` list. Every value is compared on `to_bits()`, so a `-0.0` where `0.0` is
//! expected is a difference rather than an equality. The node counts are the gate's own: one
//! snapshot per seed of the `roundtrip` sweep, at `gate_node_count(seed)` nodes — 2 through
//! 601 over the 100-seed sweep. There is no other node count here: this oracle is reached
//! only through `roundtrip`, and the unit tests in `graph-core` cover the shapes the gate
//! model cannot produce.
//!
//! The three invariants are checked on the snapshot, because they are the properties a
//! consumer of the bytes can see: rows in topological order of the directed arcs, no edge's
//! interior run sharing a lane with a vertex strictly between its endpoints, and note 5 only
//! on an edge that really is reversed.
//!
//! `rows_are_a_topological_order` and `nothing_sits_on_an_edge` read the node columns from
//! the oracle's own model rather than from the snapshot. That is not a shortcut past the
//! comparison: `node_columns` has already established that the two agree bit for bit, so
//! checking one is checking the other, and it keeps each function inside the four-parameter
//! house limit.

use super::Drawing;
use graph_contract::binary::Snapshot;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::notes::{Note, NoteCode, Notes};
use graph_core::Topology;
use std::collections::BTreeMap;

/// Every check, in the order a reader would want them: the kinds first, then the two node
/// columns, then the structural invariants, then the edges and the notes.
pub(super) fn against(snapshot: &Snapshot, t: &Topology, d: &Drawing) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Point { x, y }, EdgeGeometry::Polyline(paths)) = (&p.nodes, &p.edges) else {
        return Err("not Point nodes with Polyline edges".into());
    };
    if x.len() != d.row.len() || y.len() != d.row.len() {
        return Err(format!(
            "node columns hold {} x and {} y, the model has {} vertices",
            x.len(),
            y.len(),
            d.row.len()
        ));
    }
    node_columns(x, y, d)?;
    rows_are_a_topological_order(t, d)?;
    interior_points(paths, t, d)?;
    nothing_sits_on_an_edge(t, d)?;
    notes(&p.notes, t, d)
}

/// `x[v] = lane(v)` and `y[v] = r(v)`, at unit spacing.
fn node_columns(x: &[f32], y: &[f32], d: &Drawing) -> Result<(), String> {
    for v in 0..x.len() {
        let (want_x, want_y) = (d.lane[v] as f32, d.row[v] as f32);
        if (x[v].to_bits(), y[v].to_bits()) != (want_x.to_bits(), want_y.to_bits()) {
            return Err(format!(
                "node {v} at ({}, {}), the convention puts it at ({want_x}, {want_y})",
                x[v], y[v]
            ));
        }
    }
    Ok(())
}

/// No two vertices share a row, and no directed arc runs from a later row to an earlier one
/// except a self-loop — which is a cycle break, and carries its note.
fn rows_are_a_topological_order(t: &Topology, d: &Drawing) -> Result<(), String> {
    let mut seen: BTreeMap<u32, u32> = BTreeMap::new();
    for v in 0..d.row.len() {
        if let Some(other) = seen.insert(d.row[v], v as u32) {
            return Err(format!("nodes {other} and {v} share row {}", d.row[v]));
        }
    }
    let cols = t.edges();
    for e in 0..cols.source.len() {
        let (s, g) = (cols.source[e] as usize, cols.target[e] as usize);
        if cols.directed[e] && s != g && d.row[s] > d.row[g] && !carries_note(t, d, e) {
            return Err(format!(
                "edge {e} is arc {s} -> {g} running backwards with no note on it"
            ));
        }
    }
    Ok(())
}

/// Every edge's interior points: the bends the convention puts there, in order. A self-loop
/// has none, the same convention `layout.dag.sugiyama` emits.
fn interior_points(got: &Paths, t: &Topology, d: &Drawing) -> Result<(), String> {
    let cols = t.edges();
    if got.offsets.len() != cols.source.len() + 1 {
        return Err(format!(
            "polyline offsets hold {} entries, the graph has {} edges",
            got.offsets.len().saturating_sub(1),
            cols.source.len()
        ));
    }
    if got.pts.len() != 2 * *got.offsets.last().unwrap_or(&0) as usize {
        return Err("polyline offsets and points disagree on the point count".into());
    }
    for e in 0..cols.source.len() {
        let want = route(cols.source[e], cols.target[e], d.carried[e], d);
        let span = got.offsets[e] as usize..got.offsets[e + 1] as usize;
        let mine: Vec<(f32, f32)> = span.map(|p| (got.pts[2 * p], got.pts[2 * p + 1])).collect();
        if mine.len() != want.len() {
            return Err(format!(
                "edge {e} has {} interior points, the convention puts {} there",
                mine.len(),
                want.len()
            ));
        }
        for (k, (&(mx, my), &(wx, wy))) in mine.iter().zip(&want).enumerate() {
            if (mx.to_bits(), my.to_bits()) != (wx.to_bits(), wy.to_bits()) {
                return Err(format!(
                    "edge {e} interior point {k} at ({mx}, {my}), the convention puts it at \
                     ({wx}, {wy})"
                ));
            }
        }
    }
    Ok(())
}

/// One edge's interior points, from the oracle's own rows and lanes: a bend into the
/// carrying lane half a row after the earlier end, and out of it half a row before the later
/// end, each only where the carrying lane is not that end's own lane, and reversed when the
/// source is the later end.
fn route(s: u32, g: u32, lane: u32, d: &Drawing) -> Vec<(f32, f32)> {
    if s == g {
        return Vec::new();
    }
    let (rs, rt) = (d.row[s as usize], d.row[g as usize]);
    let (early, late) = if rs < rt { (s, g) } else { (g, s) };
    let x = lane as f32;
    let mut points = Vec::new();
    if lane != d.lane[early as usize] {
        points.push((x, rs.min(rt) as f32 + 0.5));
    }
    if lane != d.lane[late as usize] {
        points.push((x, rs.max(rt) as f32 - 0.5));
    }
    if early != s {
        points.reverse();
    }
    points
}

/// No vertex shares a row strictly inside an edge's own span with the lane that edge runs
/// down — the promise the whole layout exists to keep.
fn nothing_sits_on_an_edge(t: &Topology, d: &Drawing) -> Result<(), String> {
    let mut at: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for v in 0..d.row.len() {
        at.insert((d.lane[v], d.row[v]), v as u32);
    }
    let cols = t.edges();
    for e in 0..cols.source.len() {
        let (s, g) = (cols.source[e] as usize, cols.target[e] as usize);
        if s == g {
            continue;
        }
        let lo = d.row[s].min(d.row[g]) + 1;
        let hi = d.row[s].max(d.row[g]);
        for row in lo..hi {
            if let Some(v) = at.get(&(d.carried[e], row)) {
                return Err(format!(
                    "edge {e}: vertex {v} sits on its lane {} at row {row}",
                    d.carried[e]
                ));
            }
        }
    }
    Ok(())
}

/// The notes, as the whole `(code, index)` list: one `EdgeReversed` per directed non-loop
/// edge whose source has the later row, ascending, and nothing else.
fn notes(got: &Notes, t: &Topology, d: &Drawing) -> Result<(), String> {
    let want = want_notes(t, d);
    let mine: Vec<(u32, u32)> = got
        .code
        .iter()
        .copied()
        .zip(got.index.iter().copied())
        .collect();
    if mine.len() != want.len() {
        return Err(format!(
            "the snapshot carries {} notes, the convention gives {want:?}",
            mine.len()
        ));
    }
    for (k, (&(mc, mi), note)) in mine.iter().zip(&want).enumerate() {
        let (wc, wi) = (note.code as u32, note.index);
        if (mc, mi) != (wc, wi) {
            return Err(format!(
                "note {k} is ({mc}, {mi}), the convention gives ({wc}, {wi})"
            ));
        }
    }
    Ok(())
}

fn want_notes(t: &Topology, d: &Drawing) -> Vec<Note> {
    let cols = t.edges();
    (0..cols.source.len())
        .filter(|&e| {
            let (s, g) = (cols.source[e] as usize, cols.target[e] as usize);
            cols.directed[e] && s != g && d.row[s] > d.row[g]
        })
        .map(|e| Note {
            code: NoteCode::EdgeReversed,
            index: e as u32,
        })
        .collect()
}

/// Whether edge `e` is one of the reversed edges, and so carries note 5.
fn carries_note(t: &Topology, d: &Drawing, e: usize) -> bool {
    let cols = t.edges();
    let (s, g) = (cols.source[e] as usize, cols.target[e] as usize);
    cols.directed[e] && s != g && d.row[s] > d.row[g]
}
