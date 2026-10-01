//! Reading the two arms of a Graphviz differential and measuring the gap between them.
//!
//! Split from `graphviz.rs` by the house's 300-line cap, and deliberately free of any
//! engine name: the metric is the same for every engine, and the engine only decides which
//! files are opened and which key the coordinates are under.
//!
//! **The rescale, and why it is one uniform scale.** Graphviz translates its drawing so the
//! bounding box's lower-left corner is the origin, and lays out in inches reported in points,
//! while our layout centres its field on the origin and computes in points. The two arms
//! therefore differ by a translation and a scale that say nothing about the tiling, so both
//! are mapped onto Graphviz's own node-centre bounding box before anything is compared.
//!
//! Why one uniform scale and not one per axis: a per-axis map would let an aspect-ratio error
//! and a shape error both arrive as an unscaled shape difference, indistinguishable. A single
//! `max` scale keeps the aspect honest, so a drawing that is right but stretched is a
//! *larger* gap.
//!
//! **One record at a time.** Both arms are read through a buffered line iterator rather than
//! slurped, so the 1000-seed sweep holds one record of each file and not the two files twice.

use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// One `.jsonl` arm, read line by line.
pub struct Arm {
    lines: std::io::Lines<BufReader<File>>,
}

impl Arm {
    /// Open `path` for reading. The seed count is not taken from here and not from a
    /// manifest's `seeds`: a manifest's count is the claim and the rows are the fact, so the
    /// two arms are checked against each other in lockstep by [`Arm::next_row`] returning
    /// `None` on one side and `Some` on the other.
    pub fn open(path: &Path) -> Result<Arm, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Arm {
            lines: BufReader::new(file).lines(),
        })
    }

    /// The next record, or `None` at end of file.
    pub fn next_row(&mut self) -> Result<Option<Value>, String> {
        let Some(line) = self.lines.next() else {
            return Ok(None);
        };
        let line = line.map_err(|e| format!("a differential fixture line did not read: {e}"))?;
        if line.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&line)
            .map(Some)
            .map_err(|e| format!("a fixture line is not json: {e}"))
    }
}

/// The point cloud's bounding box: `((min_x, min_y), (width, height))`.
pub fn bbox_of(points: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    let mut low = (f64::INFINITY, f64::INFINITY);
    let mut high = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in points {
        low = (low.0.min(x), low.1.min(y));
        high = (high.0.max(x), high.1.max(y));
    }
    (low, (high.0 - low.0, high.1 - low.1))
}

/// `points` onto `target`, one uniform scale from the larger axis's span.
pub fn rescale(points: &[(f64, f64)], target: ((f64, f64), (f64, f64))) -> Vec<(f64, f64)> {
    let ((_, _), (width, height)) = target;
    let ((low_x, low_y), (span_x, span_y)) = bbox_of(points);
    let span = span_x.max(span_y);
    if span <= 0.0 {
        // A degenerate cloud — one node, or a collinear drawing — has no scale to speak of,
        // so the translation alone is compared rather than a division by zero.
        return points
            .iter()
            .map(|&(x, y)| (x - low_x, y - low_y))
            .collect();
    }
    let scale = width.max(height) / span;
    points
        .iter()
        .map(|&(x, y)| ((x - low_x) * scale, (y - low_y) * scale))
        .collect()
}

/// The largest absolute coordinate difference in points, on one shared box.
pub fn gap(ours: &[(f64, f64)], theirs: &[(f64, f64)]) -> Result<f64, String> {
    if ours.len() != theirs.len() {
        return Err(format!("{} nodes against {}", ours.len(), theirs.len()));
    }
    let target = bbox_of(theirs);
    let mine = rescale(ours, target);
    let other = rescale(theirs, target);
    Ok(mine
        .iter()
        .zip(&other)
        .map(|(a, b)| (a.0 - b.0).abs().max((a.1 - b.1).abs()))
        .fold(0.0, f64::max))
}

/// Our arm's coordinates, in dense-index order, from the `{x, y}` columns the emit wrote
/// under the engine's key.
pub fn read_fixture(record: &Value, key: &str) -> Result<Vec<(f64, f64)>, String> {
    let column = &record[key];
    let xs = column["x"]
        .as_array()
        .ok_or(format!("{key}: no x column"))?;
    let ys = column["y"]
        .as_array()
        .ok_or(format!("{key}: no y column"))?;
    if xs.len() != ys.len() {
        return Err(format!("{key}: {} x against {} y", xs.len(), ys.len()));
    }
    let column = |v: &Value| {
        v.as_f64()
            .ok_or_else(|| format!("{key}: {} is not a number", v))
    };
    let mut out = Vec::with_capacity(xs.len());
    for (x, y) in xs.iter().zip(ys) {
        out.push((column(x)?, column(y)?));
    }
    Ok(out)
}

/// Graphviz's arm, from the `nodes` object the harness wrote: node id to `[x, y]` in points.
///
/// The ids are `n0..n{n-1}` in the harness's own DOT declaration order, so the mapping back
/// to our dense index is by name and not by position — a plain-format reordering would
/// otherwise be graded as a layout difference.
pub fn read_oracle(record: &Value, key: &str) -> Result<Vec<(f64, f64)>, String> {
    let nodes = record[key].as_object().ok_or(format!("no {key} object"))?;
    let count = nodes.len();
    let mut out = vec![(0.0, 0.0); count];
    for (id, point) in nodes {
        let index: usize = id
            .strip_prefix('n')
            .and_then(|rest| rest.parse().ok())
            .ok_or(format!("{id}: not a dense node id"))?;
        if index >= count {
            return Err(format!("{id}: outside the node count {count}"));
        }
        let pair = point.as_array().ok_or(format!("{id}: not a point"))?;
        let read = |at: usize| {
            pair.get(at)
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("{id}: no coordinate {at}"))
        };
        out[index] = (read(0)?, read(1)?);
    }
    Ok(out)
}

/// Read a whole JSON document, naming the path in any error.
pub fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
