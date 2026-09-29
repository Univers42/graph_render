//! The committed POST fixtures and the generator `fixtures/post/hairball.json` is a
//! snapshot of.
//!
//! A fixture is `{"about", "nodes": [{"id", "weight"}], "edges": [{"id", "source",
//! "target", "type"}]}`, the shape `fixtures/hierarchy/*.json` already uses, read through
//! the contract's own JSON parser so no second JSON stack enters the motor's dependency
//! list. Every field the fixture does not carry takes the fixed value the hierarchy
//! fixtures' reader gives it, and an edge's wire `type` goes through the same two
//! classifiers the ingest uses.
//!
//! [`hairball`] builds the same graph at any size, and a unit test asserts that it builds
//! exactly the committed file's records — so the fixture cannot drift from the formula, and
//! `graph-cli ink --nodes N` measures the graph the fixture holds rather than a different
//! one that only looks like it.

use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;
use crate::edgekind::{child_first_from_type, edge_kind_from_type};
use crate::records::{EdgeRecord, NodeRecord};
use graph_contract::canonical_json::{Value, parse};

/// The POST fixtures this module owns, by name and text. A caller that reports over them
/// (`graph-cli ink`) knows the names from here rather than from a list of its own.
pub const FIXTURES: [(&str, &str); 2] = [
    (
        "hairball",
        include_str!("../../../../../fixtures/post/hairball.json"),
    ),
    (
        "long-span",
        include_str!("../../../../../fixtures/post/long-span.json"),
    ),
];

/// Fixture `name`'s records, in file order, or why it could not be read. A graph, not a
/// drawing: the positions are a layout's to make, which is why a fixture fixes only the
/// topology and the bundler is measured over a layout of it.
pub fn load(name: &str) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), String> {
    let (_, text) = FIXTURES
        .iter()
        .find(|(fixture, _)| *fixture == name)
        .ok_or_else(|| format!("no post fixture {name}"))?;
    let root = parse(text).map_err(|e| format!("{name}: {e}"))?;
    let nodes = array(&root, "nodes")?
        .iter()
        .map(node)
        .collect::<Result<Vec<_>, String>>()?;
    let edges = array(&root, "edges")?
        .iter()
        .map(edge)
        .collect::<Result<Vec<_>, String>>()?;
    Ok((nodes, edges))
}

/// The hairball of `nodes` nodes, the graph `fixtures/post/hairball.json` holds.
///
/// Two clusters, `A` then `B` in node order, each a path, a stride cycle and a fan from its
/// first node, joined by cross edges from every `A` node at fixed strides. The pattern is
/// arithmetic, not a random draw, so the file rebuilds itself from this function and a
/// sweep over `--nodes` is the same graph at every size. Node `i` of a cluster of `n` reaches
/// cluster `B` node `(i · s + 3k) mod |B|` for `s` in `(1, 5, 11, 17)` and `k` in `0..4`;
/// cluster `A` adds the stride-7 cycle and cluster `B` the stride-4 one.
pub fn hairball(nodes: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let na = nodes.div_ceil(2).max(1);
    let nb = nodes.saturating_sub(na).max(1);
    let mut out = (Vec::new(), Vec::new());
    for (tag, count) in [("a", na), ("b", nb)] {
        for i in 0..count {
            out.0.push(NodeRecord {
                id: format!("{tag}{i}"),
                kind: NodeKind::Record,
                database_id: None,
                source: "fixture".into(),
                label: format!("{tag}{i}"),
                group: None,
                weight: 1.0 + f64::from(i % 4) * 0.25,
                version: 0.0,
                has_note: false,
                icon: None,
            });
        }
    }
    let mut edges: Vec<EdgeRecord> = Vec::new();
    for i in 0..na {
        for (k, stride) in [1_u32, 5, 11, 17].into_iter().enumerate() {
            let to = (i * stride + 3 * k as u32) % nb;
            add(&format!("a{i}"), &format!("b{to}"), &mut edges);
        }
    }
    for (tag, count, stride) in [("a", na, 7_u32), ("b", nb, 4)] {
        for i in 0..count.saturating_sub(1) {
            add(&format!("{tag}{i}"), &format!("{tag}{}", i + 1), &mut edges);
        }
        for i in 0..count {
            add(
                &format!("{tag}{i}"),
                &format!("{tag}{}", (i + stride) % count),
                &mut edges,
            );
        }
        for i in (3..count).step_by(3) {
            add(&format!("{tag}0"), &format!("{tag}{i}"), &mut edges);
        }
    }
    out.1 = edges;
    out
}

/// Appends one relation edge, numbered by its position so the ids are a function of the
/// order the loops produce.
fn add(from: &str, to: &str, edges: &mut Vec<EdgeRecord>) {
    let id = format!("e{}", edges.len());
    edges.push(EdgeRecord {
        id,
        source: from.into(),
        target: to.into(),
        kind: EdgeKind::Relation,
        child_first: false,
        label: "relates_to".into(),
        strength: 1.0,
        directed: true,
        record_id: None,
    });
}

fn node(value: &Value) -> Result<NodeRecord, String> {
    let id = text(value, "id")?;
    let weight = match member(value, "weight")? {
        Value::Number(text) => text.parse().map_err(|e| format!("weight {text:?}: {e}"))?,
        other => return Err(format!("weight is {other:?}")),
    };
    Ok(NodeRecord {
        id: id.clone(),
        kind: NodeKind::Record,
        database_id: None,
        source: "fixture".into(),
        label: id,
        group: None,
        weight,
        version: 0.0,
        has_note: false,
        icon: None,
    })
}

fn edge(value: &Value) -> Result<EdgeRecord, String> {
    let wire_type = text(value, "type")?;
    Ok(EdgeRecord {
        id: text(value, "id")?,
        source: text(value, "source")?,
        target: text(value, "target")?,
        kind: edge_kind_from_type(Some(&wire_type)),
        child_first: child_first_from_type(Some(&wire_type)),
        label: wire_type,
        strength: 1.0,
        directed: true,
        record_id: None,
    })
}

fn member<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    let Value::Object(members) = value else {
        return Err(format!("not an object: {value:?}"));
    };
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("no {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match member(value, key)? {
        Value::Array(items) => Ok(items),
        other => Err(format!("{key} is {other:?}")),
    }
}

fn text(value: &Value, key: &str) -> Result<String, String> {
    match member(value, key)? {
        Value::String(text) => Ok(text.clone()),
        other => Err(format!("{key} is {other:?}")),
    }
}
