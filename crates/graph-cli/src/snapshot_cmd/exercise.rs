//! The contract exercise: one snapshot per seed that no layout would produce — every
//! node and edge kind, the floats a text face most easily gets wrong, ids a JSON writer
//! must escape, and every notes case (a 0.2-labelled snapshot, none, each code) — so
//! `roundtrip` checks the whole contract, not only the grid's half-integers. Every third
//! seed is 3D as well, and stays: `roundtrip` already sweeps every registered layout, the
//! seven 3D ones included (`registry.rs:231-281`), so what this exercise adds on top of
//! them is the same torture as above — every node and edge kind, the awkward floats, the
//! ids to escape — carried into a z column under the 0.4 label.

use graph_contract::binary::{Snapshot, SnapshotParts, StringTable};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::notes::{Note, NoteCode, Notes, SNAPSHOT_WIDE};
use graph_contract::snapshot::{Dim, label_for};
use graph_contract::version::FormatVersion;

mod z;

pub use z::{snapshot_or_perturbed, z_refusal_faults};

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
    // A third of the seeds are 3D, from the same `FLOATS` table, so the sweep covers the z
    // column's wire order, its JSON shape and its f64 narrowing on the same terms as every
    // other column. No 3D layout exists yet, so this is where the round trip is proved for
    // 3D.
    //
    // From its **own** stream, seeded so no z value depends on how many draws came before
    // it: `s` is this snapshot's stream and every column in it has a fixed order that other
    // tests pin (`each_seed_draws_the_notes_its_case_names_and_no_others`,
    // `the_tally_of_the_thousand_seed_sweep_is_exact`). Drawing a fourth column from `s`
    // would shift every value after it and restate every one of those seeds, so the z
    // column gets its own stream rather than a place in that order.
    let three_d = seed % 3 == 2;
    let dim = if three_d { Dim::D3 } else { Dim::D2 };
    let mut parts = SnapshotParts {
        // The label follows the snapshot (condition 1), not the crate: a 2D seed is
        // labelled 0.3 and its bytes are what they were before 3D existed.
        version: label_for(dim),
        node_ids: table("node.id", &node_ids)?,
        edge_ids: table("edge.id", &edge_ids)?,
        source: (0..m).map(|_| s.below(u64::from(n))).collect(),
        target: (0..m).map(|_| s.below(u64::from(n))).collect(),
        nodes: nodes(&mut s, seed % 3, n),
        z: None,
        edges: edges(&mut s, seed / 3 % 3, m),
        notes: Notes::default(),
    };
    (parts.version, parts.notes) = notes(&mut s, seed, m, dim);
    parts.z = three_d.then(|| z::draw(seed, n));
    Snapshot::new(parts).map_err(|e| format!("exercise seed {seed}: {e}"))
}

/// Seed `seed`'s version and notes, by `seed % 5`: a 0.2-labelled snapshot (no notes
/// section on either face), 0.3 with `k = 0`, then notes of code 1, of code 2, and of
/// every code. Edge notes fall on edge 0 and on each later edge with odd probability,
/// so any five consecutive seeds with edges draw every case.
fn notes(s: &mut Stream, seed: u32, m: u32, dim: Dim) -> (FormatVersion, Notes) {
    let codes: &[NoteCode] = match seed % 5 {
        // A 0.2 label cannot name a z column, so a 3D seed keeps 0.3 here: the "no notes
        // section" case is already drawn by the 2D seeds.
        0 if dim.is_3d() => return (label_for(Dim::D3), Notes::default()),
        0 => return (FormatVersion { major: 0, minor: 2 }, Notes::default()),
        1 => &[],
        2 => &[NoteCode::CycleEdgeDropped],
        3 => &[NoteCode::ExtraParentDropped],
        _ => &NoteCode::ALL,
    };
    let mut notes = Vec::new();
    for &code in codes {
        if code == NoteCode::PackingApproximate {
            notes.push(Note {
                code,
                index: SNAPSHOT_WIDE,
            });
            continue;
        }
        let picked = (0..m).filter(|&e| e == 0 || s.below(2) == 0);
        notes.extend(picked.map(|index| Note { code, index }));
    }
    (label_for(dim), Notes::of(&notes))
}

/// Tallies which notes cases `snapshot` draws: `[0.2-labelled, 0.3 with k = 0, a code-1
/// note, a code-2 note, a code-3 note]`.
pub fn count_notes_cases(snapshot: &Snapshot, cases: &mut [u64; 5]) {
    let p = snapshot.parts();
    if p.version.minor < 3 {
        cases[0] += 1;
    } else if p.notes.is_empty() {
        cases[1] += 1;
    }
    for (code, slot) in (1..=3).zip(&mut cases[2..]) {
        *slot += u64::from(p.notes.code.contains(&code));
    }
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
