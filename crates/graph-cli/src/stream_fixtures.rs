//! `emit-stream-fixtures`: the three stream fixtures `force-gate`'s stream stage reads.
//!
//! **One JSON v1 document per line**, line 0 the initial graph and every later line one
//! batch, in the exact bytes [`graph_wasm::ingest_document`] writes — the same writer the
//! ABI's reader path is tested against, so a fixture and a document cannot disagree about
//! what a record looks like. No batch carries a node the graph has not already got, and no
//! two batches share an id: those are `Topology::extend`'s first two refusals, and a fixture
//! that tripped them would be a fixture the stage cannot run at all.
//!
//! **Deterministic by construction**: no clock, no hash order, no allocator address. The
//! only source of variety is the local [`Rng`] below, a fixed-seed splitmix64, so
//! regenerating a fixture on any host gives byte-identical output and the `fixtures-fresh`
//! row is a check rather than a coin toss.
//!
//! Node ids are `n<i>` and edge ids `e<j>`, decimal and unpadded, in creation order: the
//! divergence this gate exists to report names a batch number, and an id that reads as the
//! same number is the difference between a report a reader can follow and one they cannot.

#[cfg(test)]
#[path = "stream_fixtures/tests.rs"]
mod tests;

use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};
use std::path::{Path, PathBuf};

/// The fixtures this module writes, in the order [`run`] writes them. Named by their file
/// stem; `force-gate`'s stream stage reads the same list, which is why it is a constant.
pub const FIXTURES: [&str; 3] = ["stream-small", "stream-hub", "stream-pow2"];

/// One line of a fixture: what this batch adds. Line 0 is the initial graph.
struct Line {
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
}

/// Writes all three fixtures into `out`, creating it if absent.
///
/// Prints one line per file with its byte size, because a silently empty write is a gate
/// that reads three missing files and exits 2 with nothing to point at.
pub fn run(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for name in FIXTURES {
        let written = bytes(name)?;
        let path = out.join(format!("{name}.jsonl"));
        std::fs::write(&path, &written).map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "emit-stream-fixtures: {} ({} lines, {} bytes)",
            path.display(),
            lines(name).len(),
            written.len()
        );
    }
    Ok(())
}

/// Where the fixtures go when `--out` is absent: the workspace's own `fixtures/`, which is
/// where `force-gate`'s stream stage looks and where a checked-in fixture belongs.
pub fn default_out() -> PathBuf {
    crate::runner::workspace_root().join("fixtures")
}

/// The fixture `name` resolves to on disk, for a reader that wants the file rather than the
/// records.
pub fn fixture_path(name: &str) -> PathBuf {
    default_out().join(format!("{name}.jsonl"))
}

fn bytes(name: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (index, line) in lines(name).into_iter().enumerate() {
        let text = graph_wasm::ingest_document(&line.nodes, &line.edges)
            .ok_or_else(|| format!("{name} line {index}: a non-finite value JSON cannot spell"))?;
        out.extend_from_slice(text.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

fn lines(name: &str) -> Vec<Line> {
    match name {
        "stream-small" => small(),
        "stream-hub" => hub(),
        "stream-pow2" => pow2(),
        other => unreachable!("{other} is not one of FIXTURES"),
    }
}

/// `stream-small`: 50 nodes and a ring, then eight batches of ten. Every new node gets one
/// edge into the past and one into its own batch, except each batch's first, which can only
/// reach backwards — so both kinds of endpoint are exercised on every batch.
fn small() -> Vec<Line> {
    let mut out = vec![Line {
        nodes: (0..50).map(node).collect(),
        edges: (0..50).map(|i| edge(i, i, (i + 1) % 50)).collect(),
    }];
    let mut rng = Rng::new(0x5EED);
    let mut ids = 50;
    for batch in 1..=8u32 {
        let old = 50 + (batch - 1) * 10;
        let mut line = Line {
            nodes: (old..old + 10).map(node).collect(),
            edges: Vec::new(),
        };
        for k in 0..10u32 {
            let here = old + k;
            line.edges.push(edge(ids, here, rng.pick(old)));
            ids += 1;
            let within = match k {
                0 => rng.pick(old),
                k => old + rng.pick(k),
            };
            line.edges.push(edge(ids, here, within));
            ids += 1;
        }
        out.push(line);
    }
    out
}

/// `stream-hub`: one hub that never moves and never grows, and 200 edges arriving onto it
/// over five batches — no new nodes at all, so every batch is an edge-only delta.
///
/// Targets cycle through the twenty spokes and so are heavily parallel (every spoke takes
/// ten edges under ten ids); one edge, in batch 3, is a self-loop. Both features are here
/// because `Topology::extend` must *accept* them: an id that is new with endpoints that are
/// old is the whole contract, and the self-loop is the one endpoint pair that names the same
/// node twice.
fn hub() -> Vec<Line> {
    let mut out = vec![Line {
        nodes: (0..21).map(node).collect(),
        edges: (1..21).map(|i| edge(i - 1, 0, i)).collect(),
    }];
    for batch in 0..5u32 {
        let mut edges = Vec::with_capacity(40);
        for k in 0..40u32 {
            let at = batch * 40 + k;
            let to = match (batch, k) {
                (3, 7) => 0,
                _ => (at % 20) + 1,
            };
            edges.push(edge(20 + at, 0, to));
        }
        out.push(Line {
            nodes: Vec::new(),
            edges,
        });
    }
    out
}

/// `stream-pow2`: the growth path's capacity crossings, and one empty batch between them.
///
/// The node count after each line is 60, 70, 70, 128, 129, 149 — it crosses 64 and 128 and
/// stops just past each, and line 3 adds nothing at all. An empty document is a legal batch
/// and the one a reader is most likely to refuse by accident, so it is written here rather
/// than assumed.
fn pow2() -> Vec<Line> {
    let mut out = vec![Line {
        nodes: (0..60).map(node).collect(),
        edges: (1..60).map(|i| edge(i - 1, i - 1, i)).collect(),
    }];
    let mut ids = 59;
    let mut first = 60;
    for count in [10u32, 0, 58, 1, 20] {
        let line = Line {
            nodes: (0..count).map(|k| node(first + k)).collect(),
            edges: (0..count)
                .map(|k| edge(ids + k, first + k - 1, first + k))
                .collect(),
        };
        ids += count;
        first += count;
        out.push(line);
    }
    out
}

fn node(i: u32) -> NodeRecord {
    NodeRecord {
        id: format!("n{i}"),
        kind: match i.is_multiple_of(23) {
            true => NodeKind::Note,
            false => NodeKind::Record,
        },
        database_id: Some(format!("db-{}", i % 4)),
        source: "bench".into(),
        label: format!("Node {i}"),
        group: Some(format!("g{}", i % 3)),
        weight: 0.5,
        version: 0.0,
        has_note: i.is_multiple_of(17),
        icon: None,
    }
}

/// Edge `j` from `from` to `to`. The strength is a fixed function of the id rather than a
/// draw, so a batch's edges are distinguishable by inspection and still reproducible.
fn edge(j: u32, from: u32, to: u32) -> EdgeRecord {
    EdgeRecord {
        id: format!("e{j}"),
        source: format!("n{from}"),
        target: format!("n{to}"),
        kind: EdgeKind::Relation,
        label: String::new(),
        strength: 0.4 + 0.4 * f64::from(j % 7) / 7.0,
        directed: true,
        record_id: None,
        child_first: false,
    }
}

/// A fixed-seed splitmix64, and the only source of variety in this module.
///
/// **Caveat:** this is a heuristic, and it is a heuristic about *coverage*, not about
/// randomness. `pick(n)` chooses "some" index below `n`, so two batches may pick the same
/// old node twice — which is the parallel-edge case on purpose in `stream-hub` and merely
/// untidy in `stream-small`. Failing input: none in range, `n == 0` is a division by zero
/// and is the caller's bug, not a silent clamp. Direction: a smaller seed gives a
/// different, equally valid fixture set. Escape hatch: change [`Rng::new`]'s argument and
/// re-run `emit-stream-fixtures`; the gate re-reads whatever the files say.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn pick(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n)) as u32
    }
}
