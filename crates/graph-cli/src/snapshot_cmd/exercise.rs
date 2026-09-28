//! The contract exercise: one snapshot per seed that no layout would produce — every
//! node and edge kind, the floats a text face most easily gets wrong, and ids a JSON
//! writer must escape — so `roundtrip` checks the whole contract, not only the grid's
//! half-integers.

use graph_contract::binary::{Snapshot, SnapshotParts, StringTable};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::version::CURRENT_VERSION;

/// Floats a shortest-round-trip writer or a double-rounding reader is likeliest to get
/// wrong: signed zeros, the subnormal ends, the finite ends, and values whose shortest
/// decimal is long or sits near a tie.
const FLOATS: [f32; 12] = [
    0.0,
    -0.0,
    f32::from_bits(1),
    f32::from_bits(0x007f_ffff),
    f32::MIN_POSITIVE,
    f32::MAX,
    f32::MIN,
    0.1,
    1.0 / 3.0,
    1.000_000_1,
    16_777_216.0,
    -8_388_607.5,
];

/// Id prefixes a JSON writer must escape or pass through untouched. None holds a digit,
/// so a prefix plus a decimal index is unique per index.
const IDS: [&str; 12] = [
    "\"",
    "\\",
    "\u{0}",
    "\n\t",
    "\u{1f}",
    "\u{7f}",
    "é",
    "\u{1F680}",
    "\u{2028}",
    "</x>",
    "a b",
    "\u{fffd}",
];

/// splitmix64: a seed spread over a stream, independent of graph-core's generators.
struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u32 {
        (self.next() % n) as u32
    }

    /// Half the time a value from [`FLOATS`], else any finite `f32`.
    fn float(&mut self) -> f32 {
        let bits = self.next();
        if bits & 1 == 0 {
            return FLOATS[(bits >> 1) as usize % FLOATS.len()];
        }
        let value = f32::from_bits((bits >> 32) as u32);
        if value.is_finite() {
            value
        } else {
            f32::from_bits(value.to_bits() & 0xff7f_ffff)
        }
    }

    fn floats(&mut self, count: u32) -> Vec<f32> {
        (0..count).map(|_| self.float()).collect()
    }

    /// Floats for a size column: never below zero.
    fn sizes(&mut self, count: u32) -> Vec<f32> {
        (0..count)
            .map(|_| f32::from_bits(self.float().to_bits() & 0x7fff_ffff))
            .collect()
    }
}

/// Seed `seed`'s exercise: node kind `seed % 3`, edge kind `seed / 3 % 3`, 1–9 nodes,
/// 0–6 edges, so any nine consecutive seeds cover every pair of kinds.
pub fn snapshot(seed: u32) -> Result<Snapshot, String> {
    let mut s = Stream(u64::from(seed));
    let (n, m) = (1 + seed % 9, seed % 7);
    let ids = |s: &mut Stream, lead: &str, count: u32| -> Vec<String> {
        let pick = |s: &mut Stream| IDS[s.below(IDS.len() as u64) as usize];
        (0..count)
            .map(|i| format!("{lead}{}{i}", pick(s)))
            .collect()
    };
    let mut node_ids = ids(&mut s, "", n);
    if seed % 2 == 1 {
        node_ids[0] = String::new();
    }
    let edge_ids = ids(&mut s, "e", m);
    let table = |column, items: &[String]| {
        StringTable::from_strs(column, items.iter().map(String::as_str)).map_err(|e| e.to_string())
    };
    let parts = SnapshotParts {
        version: CURRENT_VERSION,
        node_ids: table("node.id", &node_ids)?,
        edge_ids: table("edge.id", &edge_ids)?,
        source: (0..m).map(|_| s.below(u64::from(n))).collect(),
        target: (0..m).map(|_| s.below(u64::from(n))).collect(),
        nodes: nodes(&mut s, seed % 3, n),
        edges: edges(&mut s, seed / 3 % 3, m),
    };
    Snapshot::new(parts).map_err(|e| format!("exercise seed {seed}: {e}"))
}

fn nodes(s: &mut Stream, kind: u32, n: u32) -> NodeGeometry {
    let (x, y) = (s.floats(n), s.floats(n));
    match kind {
        0 => NodeGeometry::Point { x, y },
        1 => NodeGeometry::Circle {
            x,
            y,
            r: s.sizes(n),
        },
        _ => NodeGeometry::Box {
            x,
            y,
            w: s.sizes(n),
            h: s.sizes(n),
        },
    }
}

fn edges(s: &mut Stream, kind: u32, m: u32) -> EdgeGeometry {
    let mut offsets = vec![0];
    for _ in 0..m {
        let last = offsets[offsets.len() - 1];
        offsets.push(last + s.below(3));
    }
    let points = offsets[offsets.len() - 1];
    let paths = Paths {
        offsets,
        pts: s.floats(2 * points),
    };
    match kind {
        0 => EdgeGeometry::Line,
        1 => EdgeGeometry::Polyline(paths),
        _ => EdgeGeometry::Curve {
            degree: 1 + s.below(4),
            paths,
        },
    }
}

#[cfg(test)]
mod tests;
