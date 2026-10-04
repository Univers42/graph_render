//! The oracle's deterministic benchmark graph (`src/core/model/synthetic.ts`), ported
//! call for call: the same mulberry32 stream, consumed in the same order, so the two
//! produce the same graph. It is byte-compared like any other port (`prompt.md` §7.4):
//! if it drifted, every differential built on it would compare different graphs.

use crate::arena::CapacityError;
use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;
use crate::index::{Topology, index_model};
use crate::records::{EdgeRecord, NodeRecord};
use crate::weights::apply_degree_weights;

const DATABASES: u32 = 8;
const EMOJI: [&str; 12] = [
    "\u{1F680}",
    "\u{1F4DA}",
    "\u{1F9E0}",
    "\u{1F33F}",
    "\u{1F52C}",
    "\u{1F3AF}",
    "\u{1F5FA}\u{FE0F}",
    "\u{1F4A1}",
    "\u{1F3A8}",
    "\u{2699}\u{FE0F}",
    "\u{1F4C8}",
    "\u{1F9E9}",
];
const GROUPS: [&str; 5] = ["Active", "Draft", "Review", "Done", "Archived"];
const ICONS: [&str; 4] = ["icon:rocket", "icon:book", "icon:target", "icon:map"];
/// The oracle's fixed seed (`synthetic.ts:99`).
const SEED: u32 = 0x05_1042;
/// Largest model the oracle builds (`synthetic.ts:97`).
pub(crate) const MAX_SYNTHETIC_NODES: u32 = 100_000;

/// mulberry32 (`synthetic.ts:27-35`): integer steps, then `/ 2^32`, so every target
/// produces the same `f64` stream.
pub(crate) struct Mulberry32(u32);

impl Mulberry32 {
    pub(crate) fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub(crate) fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    /// `Math.floor(rnd() * len)` — an index into a table of `len` entries.
    ///
    /// `len == 0` has no index to return: the JS would read `undefined` off the empty
    /// table and carry `undefined` into the record, where it becomes `"undefined"` or a
    /// `NaN` weight downstream. Every call site passes a table's own length
    /// (`EMOJI`, `ICONS`, `SOURCES`, `EdgeKind::ALL`) or `count`, which
    /// [`synthetic_records`] holds at `MAX_SYNTHETIC_NODES`, so the assert cannot fire for
    /// any count this module accepts.
    pub(crate) fn pick(&mut self, len: usize) -> usize {
        debug_assert!(len > 0, "pick from a table of at least one entry");
        libm::floor(self.next_f64() * len as f64) as usize
    }
}

/// `buildSyntheticModel(n)` (`synthetic.ts:85-105`).
pub fn build_synthetic_model(n: f64) -> Result<Topology, CapacityError> {
    let (mut nodes, edges) = synthetic_records(synthetic_count(n));
    apply_degree_weights(&mut nodes, &edges);
    index_model(&nodes, &edges)
}

/// The oracle's node count for a requested `n`: floored, `NaN` and ±∞ read as 2, then
/// clamped to `2..=100_000`.
pub(crate) fn synthetic_count(n: f64) -> u32 {
    let requested = if n.is_finite() { libm::floor(n) } else { 2.0 };
    requested.clamp(2.0, f64::from(MAX_SYNTHETIC_NODES)) as u32
}

/// The raw nodes and edges of the `count`-node model, weights not yet applied.
///
/// `count` is the **caller's**, not the oracle's: `synthetic_count` clamps a requested `n`
/// to `2..=MAX_SYNTHETIC_NODES`, but `seeded_model` (public) hands this whatever the
/// pipeline asks for and the bench campaign asks for up to
/// [`MAX_BENCH_NODES`](crate::registry::MAX_BENCH_NODES) = 10 × that. So the oracle's
/// clamp is *not* re-applied here: it would refuse every bench fixture above 100 000
/// nodes, and those fixtures are hashed by the gate.
///
/// **Caveat:** a `count` of 0 or 1 is accepted and builds the 0- or 1-node model the
/// oracle would build; the oracle never asks for one, because `synthetic_count` floors at
/// 2. Nothing else refuses it.
pub(crate) fn synthetic_records(count: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let mut rnd = Mulberry32::new(SEED);
    let nodes: Vec<_> = (0..count).map(|i| synthetic_node(i, &mut rnd)).collect();
    let edges = synthetic_edges(count, &mut rnd, &nodes);
    (nodes, edges)
}

/// `syntheticNode` (`synthetic.ts:37-56`).
fn synthetic_node(i: u32, rnd: &mut Mulberry32) -> NodeRecord {
    let roll = rnd.next_f64();
    let icon = if roll < 0.6 {
        Some(EMOJI[rnd.pick(EMOJI.len())])
    } else if roll < 0.8 {
        Some(ICONS[rnd.pick(ICONS.len())])
    } else {
        None
    };
    let database = format!("db-{}", i % DATABASES);
    NodeRecord {
        id: format!("bench:{database}:{i}"),
        kind: if i.is_multiple_of(23) {
            NodeKind::Note
        } else {
            NodeKind::Record
        },
        database_id: Some(database),
        source: "bench".into(),
        label: format!("Node {i}"),
        group: Some(GROUPS[i as usize % GROUPS.len()].into()),
        weight: 0.5,
        version: 0.0,
        has_note: i.is_multiple_of(17),
        icon: icon.map(str::to_owned),
    }
}

/// `syntheticEdges` (`synthetic.ts:58-83`): preferential attachment, then 5% `note_link`
/// extras. Each endpoint is drawn before the edge's strength, as JS evaluates a call's
/// arguments before its body; a self-edge draws no strength.
fn synthetic_edges(count: u32, rnd: &mut Mulberry32, nodes: &[NodeRecord]) -> Vec<EdgeRecord> {
    // `count as usize * 2` wraps on wasm32 (a 32-bit `usize`) at `count > 2³¹` and
    // panics in the allocator's `capacity overflow` check; `saturating_mul` on the `u32`
    // hands the same figure a 64-bit host would, so both targets ask for the same buffer.
    let mut edges = Vec::with_capacity(count.saturating_mul(2) as usize);
    let mut push = |rnd: &mut Mulberry32, a: usize, b: usize, kind: EdgeKind| {
        if a == b {
            return;
        }
        edges.push(EdgeRecord {
            id: format!("bench-e-{}", edges.len()),
            source: nodes[a].id.clone(),
            target: nodes[b].id.clone(),
            kind,
            label: String::new(),
            strength: 0.4 + rnd.next_f64() * 0.4,
            directed: kind == EdgeKind::Relation,
            record_id: None,
            child_first: false,
        });
    };
    let earlier = |rnd: &mut Mulberry32, i: u32| {
        let (x, y) = (rnd.next_f64(), rnd.next_f64());
        libm::floor(x * y * f64::from(i)) as usize
    };
    for i in 1..count {
        let target = earlier(rnd, i);
        push(rnd, i as usize, target, EdgeKind::Relation);
        if rnd.next_f64() < 0.5 {
            let target = earlier(rnd, i);
            push(rnd, i as usize, target, EdgeKind::Relation);
        }
    }
    let extras = libm::floor(f64::from(count) * 0.05) as u32;
    for _ in 0..extras {
        let (a, b) = (rnd.pick(count as usize), rnd.pick(count as usize));
        push(rnd, a, b, EdgeKind::NoteLink);
    }
    edges
}

#[cfg(test)]
mod tests;
