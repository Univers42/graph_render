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
    pub(crate) fn pick(&mut self, len: usize) -> usize {
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
    let mut edges = Vec::with_capacity(count as usize * 2);
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
mod tests {
    use super::*;

    #[test]
    fn count_floors_clamps_and_reads_non_finite_as_two() {
        let cases = [
            (6.0, 6),
            (2.9, 2),
            (f64::NAN, 2),
            (f64::INFINITY, 2),
            (f64::NEG_INFINITY, 2),
            (-4.0, 2),
            (1e9, 100_000),
            (100_000.5, 100_000),
            (99_999.99, 99_999),
        ];
        for (n, want) in cases {
            assert_eq!(synthetic_count(n), want, "{n}");
        }
    }

    /// Why `<` and `<=` are one mutant here (`.cargo/mutants.toml`): the largest model
    /// draws at most 2 per node, 7 per attachment step and 3 per extra edge — under
    /// 1 000 000 — and none of the first 1 000 000 draws is exactly one half.
    #[test]
    fn no_draw_a_synthetic_model_can_reach_is_exactly_one_half() {
        let mut rnd = Mulberry32::new(SEED);
        assert!((0..1_000_000).all(|_| rnd.next_f64() != 0.5));
    }

    /// `buildSyntheticModel(6)` printed by the oracle under node:22-slim.
    #[test]
    fn six_nodes_match_the_oracle() {
        let t = build_synthetic_model(6.0).expect("fits");
        let icons: Vec<_> = (0..6).map(|i| t.node(i).icon).collect();
        let want = [
            "\u{1F33F}",
            "icon:map",
            "\u{1F4C8}",
            "icon:rocket",
            "\u{1F5FA}\u{FE0F}",
            "\u{1F5FA}\u{FE0F}",
        ];
        assert_eq!(icons, want.map(Some));
        let first = t.node(0);
        assert_eq!(
            (first.id, first.kind, first.has_note),
            ("bench:db-0:0", NodeKind::Note, true)
        );
        assert_eq!(
            (first.group, first.weight),
            (Some("Active"), 0.908_497_499_664_568_9)
        );
        let ends: Vec<_> = (0..t.edge_count())
            .map(|e| (t.edge(e).source, t.edge(e).target))
            .collect();
        assert_eq!(ends.len(), 9);
        assert_eq!(ends[8], ("bench:db-5:5", "bench:db-1:1"));
        let e0 = t.edge(0);
        assert_eq!(
            (e0.id, e0.strength, e0.directed),
            ("bench-e-0", 0.534_562_692_884_355_9, true)
        );
    }

    #[test]
    fn stats_match_the_oracle_at_40_nodes_and_at_the_cap() {
        let forty = build_synthetic_model(40.0).expect("fits");
        assert_eq!(forty.stats().edges, 57);
        let note_links = (0..57).filter(|&e| forty.edge(e).kind == EdgeKind::NoteLink);
        assert_eq!(note_links.count(), 2);
        let (nodes, edges) = synthetic_records(MAX_SYNTHETIC_NODES);
        let notes = nodes.iter().filter(|n| n.kind == NodeKind::Note).count();
        assert_eq!((nodes.len(), edges.len(), notes), (100_000, 154_978, 4348));
    }
}
