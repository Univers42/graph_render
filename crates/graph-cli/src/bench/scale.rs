//! `fixtures/scale/*` as **generators**, not blobs (`prompts/phase-09-scale-bench.md` §1):
//! a seeded, deterministic model any size, written out as the provisional ingest document
//! `graph-wasm`'s reader accepts, so one graph can be laid out by all three arms.
//!
//! **The generator is the committed artefact.** `n1m.json` as a literal file is ~160 MB
//! of JSON that reproduces itself exactly from the command in its own header, so the
//! command is what this repo keeps and `fixtures/scale/n220.json` is the one literal
//! sample whose size makes committing it honest. The constants are not in prose but in
//! code: the model is `graph_core::seeded_model`, the same graph every other arm lays
//! out, so a benchmark whose input cannot be reproduced is not a risk this generator runs.
//!
//! **Past the model's own 100 000-node cap a fixture is whole components of that same
//! model, concatenated, each node id prefixed `c<component>/`** — not a truncated larger
//! model, which would silently change the degree distribution at exactly the size whose
//! distribution is the point. The cost is stated rather than hidden: independent
//! components have no edges between them, so a 10⁶ fixture is *easier* than one
//! preferential-attachment graph of 10⁶ nodes, and any ceiling it measures is a lower
//! bound on difficulty, never an upper one.
//!
//! Ponytail: the prefix makes the string arena's content differ from the capped model's,
//! so arena bytes for a concatenated fixture are slightly pessimistic (a few bytes per
//! node). Direction: the memory number grows, never shrinks. Escape hatch: read the
//! arena column with that in mind, or use sizes at or below the cap where there is no
//! prefix at all.

use graph_core::{EdgeRecord, NodeRecord, REFERENCE_DEGREE, seeded_model};

/// The model's own cap (`graph_core::synthetic`'s `MAX_SYNTHETIC_NODES`): a fixture past
/// it is whole components of this size, concatenated.
pub const COMPONENT_NODES: u32 = 100_000;

/// The largest `n` the campaign will build, and the bound `bench --n` is parsed against:
/// 10 components of [`COMPONENT_NODES`].
///
/// **The figure lives in the motor, not here** (`graph_core::registry::MAX_BENCH_NODES`),
/// because the registry's own ceilings answer with this same number and the dependency
/// runs cli -> core. Two literals that agree today are not a contract.
pub const MAX_SCALE_NODES: u32 = graph_core::registry::MAX_BENCH_NODES;

/// The members the provisional ingest reader requires of a node, sorted: the shape this
/// module writes, pinned by reading an emitted document back.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the emitted shape; pinned by reading a fixture back"
    )
)]
pub const NODE_FIELDS: [&str; 10] = [
    "database_id",
    "group",
    "has_note",
    "icon",
    "id",
    "kind",
    "label",
    "source",
    "version",
    "weight",
];

/// The members the provisional ingest reader requires of an edge, sorted.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the emitted shape; pinned by reading a fixture back"
    )
)]
pub const EDGE_FIELDS: [&str; 9] = [
    "child_first",
    "directed",
    "id",
    "kind",
    "label",
    "record_id",
    "source",
    "strength",
    "target",
];

/// The `n`-node scale fixture's records: the model itself at or below the cap, whole
/// prefixed components of it above.
pub fn scale_model(seed: u32, n: u32, reference_degree: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    if n <= COMPONENT_NODES {
        return seeded_model(seed, n, reference_degree);
    }
    let (mut nodes, mut edges) = (Vec::new(), Vec::new());
    let mut component = 0;
    let mut left = n;
    while left > 0 {
        let take = left.min(COMPONENT_NODES);
        let (cn, ce) = seeded_model(seed, take, reference_degree);
        let prefix = format!("c{component}/");
        nodes.extend(cn.into_iter().map(|mut node| {
            node.id = format!("{prefix}{}", node.id);
            node
        }));
        edges.extend(ce.into_iter().map(|mut edge| {
            edge.id = format!("{prefix}{}", edge.id);
            edge.source = format!("{prefix}{}", edge.source);
            edge.target = format!("{prefix}{}", edge.target);
            edge
        }));
        left -= take;
        component += 1;
    }
    (nodes, edges)
}

/// The fixture as the provisional ingest document: version 1, `nodes` then `edges`,
/// every member named, numbers as written.
pub fn fixture_json(seed: u32, n: u32, reference_degree: u32) -> String {
    let (nodes, edges) = scale_model(seed, n, reference_degree);
    let mut out = String::from(r#"{"version":1,"nodes":["#);
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        node_json(&mut out, node);
    }
    out.push_str(r#"],"edges":["#);
    for (i, edge) in edges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        edge_json(&mut out, edge);
    }
    out.push_str("]}");
    out
}

/// One node record, in the member order the reader requires.
fn node_json(out: &mut String, node: &NodeRecord) {
    out.push('{');
    out.push_str(r#""id":"#);
    escape(out, &node.id);
    out.push_str(r#","kind":"#);
    escape(out, node.kind.as_str());
    out.push_str(r#","database_id":"#);
    opt(out, node.database_id.as_deref());
    out.push_str(r#","source":"#);
    escape(out, &node.source);
    out.push_str(r#","label":"#);
    escape(out, &node.label);
    out.push_str(r#","group":"#);
    opt(out, node.group.as_deref());
    out.push_str(r#","weight":"#);
    number(out, node.weight);
    out.push_str(r#","version":"#);
    number(out, node.version);
    out.push_str(r#","has_note":"#);
    out.push_str(if node.has_note { "true" } else { "false" });
    out.push_str(r#","icon":"#);
    opt(out, node.icon.as_deref());
    out.push('}');
}

/// One edge record, in the member order the reader requires.
fn edge_json(out: &mut String, edge: &EdgeRecord) {
    out.push('{');
    out.push_str(r#""id":"#);
    escape(out, &edge.id);
    out.push_str(r#","source":"#);
    escape(out, &edge.source);
    out.push_str(r#","target":"#);
    escape(out, &edge.target);
    out.push_str(r#","kind":"#);
    escape(out, edge.kind.as_str());
    out.push_str(r#","label":"#);
    escape(out, &edge.label);
    out.push_str(r#","strength":"#);
    number(out, edge.strength);
    out.push_str(r#","directed":"#);
    out.push_str(if edge.directed { "true" } else { "false" });
    out.push_str(r#","record_id":"#);
    opt(out, edge.record_id.as_deref());
    out.push_str(r#","child_first":"#);
    out.push_str(if edge.child_first { "true" } else { "false" });
    out.push('}');
}

/// A `null` or a quoted string; the reader requires the member, never an omitted key.
fn opt(out: &mut String, text: Option<&str>) {
    match text {
        None => out.push_str("null"),
        Some(text) => escape(out, text),
    }
}

/// A quoted string, escaping only what RFC 8259 requires.
fn escape(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A finite number, in full round-trip precision; a non-finite one is refused here
/// rather than written as the invalid JSON token `NaN`.
fn number(out: &mut String, value: f64) {
    assert!(value.is_finite(), "a fixture cannot carry {value}");
    out.push_str(&format!("{value:?}"));
}

/// `bench --emit-scale-fixture <path> --n N --seed S`: the fixture, written.
pub fn emit(path: &std::path::Path, n: u32, seed: u32) -> Result<String, String> {
    let json = fixture_json(seed, n, REFERENCE_DEGREE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, &json).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_contract::canonical_json::{Value, parse};

    /// Every string the fixture writes must survive the reader's own parse, and every
    /// record must be an object whose members are exactly the required ones.
    #[test]
    fn every_record_carries_exactly_the_required_members() {
        let json = fixture_json(0, 40, REFERENCE_DEGREE);
        let value = parse(&json).expect("the fixture is valid JSON");
        let Value::Object(fields) = &value else {
            panic!("not an object");
        };
        let mut top: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        top.sort_unstable();
        assert_eq!(top, ["edges", "nodes", "version"]);
        for (member, required) in [("nodes", &NODE_FIELDS[..]), ("edges", &EDGE_FIELDS[..])] {
            let (_, items) = fields
                .iter()
                .find(|(name, _)| name == member)
                .expect("the array");
            let Value::Array(items) = items else {
                panic!("{member} is not an array")
            };
            assert!(!items.is_empty(), "{member} is empty");
            for item in items {
                let Value::Object(members) = item else {
                    panic!("{member} holds a non-object")
                };
                let mut names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
                names.sort_unstable();
                assert_eq!(names, required);
            }
        }
    }

    #[test]
    fn the_fixture_is_byte_identical_for_the_same_seed_and_size() {
        assert_eq!(
            fixture_json(0, 40, REFERENCE_DEGREE),
            fixture_json(0, 40, REFERENCE_DEGREE)
        );
        assert_ne!(
            fixture_json(0, 40, REFERENCE_DEGREE),
            fixture_json(1, 40, REFERENCE_DEGREE)
        );
    }

    #[test]
    fn a_fixture_past_the_cap_is_whole_components_and_the_last_one_is_the_remainder() {
        let (nodes, edges) = scale_model(0, COMPONENT_NODES + 7, REFERENCE_DEGREE);
        assert_eq!(nodes.len(), COMPONENT_NODES as usize + 7);
        let last = &nodes[COMPONENT_NODES as usize];
        assert!(last.id.starts_with("c1/"), "{}", last.id);
        assert!(
            edges
                .iter()
                .all(|e| !e.id.starts_with("c1/") || e.source.starts_with("c1/"))
        );
    }
}
