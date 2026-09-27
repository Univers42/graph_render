//! The case generator: seed `s` → a handful of cases per oracle function, drawn by a
//! splitmix64 stream seeded with `s`. It only ever *writes* inputs; both arms then load
//! the written file, so there is no second generator on the TypeScript side to drift.
//!
//! The pools are adversarial on purpose: duplicate and dangling ids, self-loops, empty
//! strings, `:` inside coordinates, mixed case, astral code points, `-0`, `NaN`, and
//! more than 256 sources for the H9 arm.

use super::wire::{WireEdge, WireNode, hex};
use graph_core::{EdgeKind, NodeKind};
use serde_json::{Value, json};

/// A generated graph: raw nodes and edges, in wire form.
pub type Graph = (Vec<WireNode>, Vec<WireEdge>);

/// splitmix64 (Vigna): the generator is recorded in the manifest by this name.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(u64::from(seed))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`; 0 when `n` is 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<T: Copy>(&mut self, pool: &[T]) -> T {
        pool[self.below(pool.len())]
    }
}

/// Ids where byte order and `localeCompare` disagree, and ids that parse oddly.
pub const ID_POOL: [&str; 22] = [
    "a",
    "A",
    "b",
    "B",
    "Z",
    "z",
    "note:1",
    "NOTE:1",
    "tag:x",
    "\u{e9}",
    "e",
    "_x",
    "ax",
    "10",
    "9",
    "a-b",
    "ab",
    "",
    "pg:db:1",
    "pg:db:1:2",
    "x",
    "\u{1F680}",
];
const TEXT_POOL: [&str; 9] = [
    "",
    "Label",
    "\u{e9}t\u{e9}",
    "\u{65e5}\u{672c}",
    "a\"b\\c",
    "line\nbreak",
    "tab\there",
    "\u{1F5FA}\u{FE0F}",
    "\u{7f}\u{1}",
];
const TYPE_POOL: [&str; 24] = [
    "parent",
    "PARENT",
    "parent_of",
    "child_of",
    "Child_Of",
    "x_hierarchy_y",
    "HIERARCHY",
    "note_link",
    "my_note_link",
    "links_to",
    "LIN\u{212A}S_TO",
    "note_of",
    "annotates",
    "tagged",
    "TAG",
    "tag",
    "parent_tag",
    "vintage",
    "stage",
    "child-of",
    "note_link_note_of",
    "note_link_hierarchy",
    "",
    "relation",
];
const FLOATS: [f64; 8] = [0.5, 0.0, -0.0, 1.0, 0.2, 1e-300, 1.0 / 3.0, f64::NAN];
const VERSIONS: [f64; 6] = [0.0, 1.0, 2.0, -1.0, 9_007_199_254_740_992.0, 0.5];

fn text(rng: &mut Rng, i: usize) -> String {
    if rng.chance(30) {
        format!("L{i}")
    } else {
        rng.pick(&TEXT_POOL).to_owned()
    }
}

/// `Some(value(rng))` `percent`% of the time; the value is drawn only when kept.
fn maybe(rng: &mut Rng, percent: usize, value: impl FnOnce(&mut Rng) -> String) -> Option<String> {
    if rng.chance(percent) {
        Some(value(rng))
    } else {
        None
    }
}

/// A random node; `pool` is the id range drawn from, so ids repeat.
pub fn node(rng: &mut Rng, pool: usize) -> WireNode {
    let id = if rng.chance(15) {
        rng.pick(&ID_POOL).to_owned()
    } else {
        format!("n{}", rng.below(pool))
    };
    let database = match rng.below(20) {
        0..=4 => None,
        5 => Some(String::new()),
        _ => Some(format!("db{}", rng.below(4))),
    };
    let label = text(rng, pool);
    let group = maybe(rng, 70, |rng| {
        rng.pick(&["Active", "Draft", "", "x"]).into()
    });
    WireNode {
        id,
        kind: rng.pick(&NodeKind::ALL).as_str().into(),
        database_id: database,
        source: rng.pick(&["pg", "mongo", "json", "my:src"]).into(),
        label,
        group,
        weight: hex(rng.pick(&FLOATS)),
        version: hex(rng.pick(&VERSIONS)),
        has_note: rng.chance(50),
        icon: maybe(rng, 60, |rng| {
            rng.pick(&["\u{1F680}", "icon:map", "img:x", ""]).into()
        }),
    }
}

/// A random edge between `ends` (15% of endpoints dangle), ids drawn from `0..pool`.
pub fn edge(rng: &mut Rng, pool: usize, ends: &[String]) -> WireEdge {
    let end = |rng: &mut Rng| {
        if ends.is_empty() || rng.chance(15) {
            format!("ghost{}", rng.below(3))
        } else {
            ends[rng.below(ends.len())].clone()
        }
    };
    let source = end(rng);
    let target = if rng.chance(8) {
        source.clone()
    } else {
        end(rng)
    };
    let id = if rng.chance(20) {
        rng.pick(&ID_POOL).to_owned()
    } else {
        format!("e{}", rng.below(pool))
    };
    WireEdge {
        id,
        source,
        target,
        kind: rng.pick(&EdgeKind::ALL).as_str().into(),
        label: text(rng, pool),
        strength: hex(rng.pick(&FLOATS)),
        directed: rng.chance(50),
        record_id: maybe(rng, 30, |rng| format!("row{}", rng.below(4))),
    }
}

/// Up to `max_nodes` nodes and about twice as many edges.
pub fn graph(rng: &mut Rng, max_nodes: usize) -> Graph {
    let n = rng.below(max_nodes + 1);
    let nodes: Vec<_> = (0..n).map(|_| node(rng, n * 5 / 4 + 1)).collect();
    let ids: Vec<String> = nodes.iter().map(|x| x.id.clone()).collect();
    let m = rng.below(2 * n + 2);
    let edges = (0..m).map(|_| edge(rng, m + 1, &ids)).collect();
    (nodes, edges)
}

/// `previous` after random drops, field edits, swaps and additions: the `next` of a diff.
pub fn mutate(rng: &mut Rng, previous: &Graph) -> Graph {
    let mut nodes: Vec<WireNode> = previous
        .0
        .iter()
        .filter(|_| !rng.chance(15))
        .cloned()
        .collect();
    for i in 0..nodes.len() {
        if rng.chance(20) {
            nodes[i] = edit_node(rng, &nodes[i]);
        }
        if i > 0 && rng.chance(10) {
            nodes.swap(i - 1, i);
        }
    }
    let pool = nodes.len() + 3;
    (0..rng.below(4)).for_each(|_| nodes.push(node(rng, pool)));
    let ids: Vec<String> = nodes.iter().map(|x| x.id.clone()).collect();
    let mut edges: Vec<WireEdge> = previous
        .1
        .iter()
        .filter(|_| !rng.chance(15))
        .cloned()
        .collect();
    for e in &mut edges {
        if rng.chance(20) {
            *e = edit_edge(rng, e, &ids);
        }
    }
    let pool = edges.len() + 3;
    (0..rng.below(4)).for_each(|_| edges.push(edge(rng, pool, &ids)));
    (nodes, edges)
}

/// `node` with one field redrawn (possibly to the same value).
pub fn edit_node(rng: &mut Rng, node: &WireNode) -> WireNode {
    let mut other = self::node(rng, 3);
    let mut out = node.clone();
    match rng.below(10) {
        0 => out.id = std::mem::take(&mut other.id),
        1 => out.kind = other.kind,
        2 => out.database_id = other.database_id,
        3 => out.source = other.source,
        4 => out.label = other.label,
        5 => out.group = other.group,
        6 => out.weight = other.weight,
        7 => out.version = other.version,
        8 => out.has_note = !out.has_note,
        _ => out.icon = other.icon,
    }
    out
}

/// `edge` with one field redrawn (possibly to the same value).
pub fn edit_edge(rng: &mut Rng, edge: &WireEdge, ends: &[String]) -> WireEdge {
    let other = self::edge(rng, 3, ends);
    let mut out = edge.clone();
    match rng.below(8) {
        0 => out.id = other.id,
        1 => out.source = other.source,
        2 => out.target = other.target,
        3 => out.kind = other.kind,
        4 => out.label = other.label,
        5 => out.strength = other.strength,
        6 => out.directed = !out.directed,
        _ => out.record_id = other.record_id,
    }
    out
}

/// A `type` string: from the pool, re-cased at random half the time; sometimes absent.
pub fn wire_type(rng: &mut Rng) -> Value {
    if rng.chance(5) {
        return Value::Null;
    }
    let base = rng.pick(&TYPE_POOL);
    if rng.chance(50) {
        return json!(base);
    }
    let recased: String = base
        .chars()
        .map(|c| {
            if rng.chance(50) {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    json!(recased)
}

/// A short string over ASCII, `:`, Latin-1, CJK and astral code points.
pub fn any_string(rng: &mut Rng) -> String {
    let alphabet = [
        'a',
        'Z',
        ':',
        '-',
        '_',
        '0',
        '\u{e9}',
        '\u{65e5}',
        '\u{1F680}',
        '\u{10FFFF}',
        ' ',
    ];
    (0..rng.below(12)).map(|_| rng.pick(&alphabet)).collect()
}
