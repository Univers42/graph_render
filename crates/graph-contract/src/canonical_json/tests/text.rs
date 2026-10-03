//! Three claims about the canonical text that the writer settles and no test said so: a
//! negative zero keeps its sign bit across the JSON face, an endpoint id is never one no
//! node declares, and object keys are ordered by UTF-8 bytes and not by UTF-16 code units.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because these are the text's
//! edge cases rather than its shape (`tests/shape.rs`) or its 3D half (`tests/dim.rs`).

use super::*;

/// A `-0.0` written `-0` reads back as `-0.0`, so the JSON face keeps a sign bit that a
/// `JSON.stringify`-shaped writer would drop. What decides is the binary face's bytes:
/// `-0.0` and `0.0` are the same number and different snapshots.
#[test]
fn a_negative_zero_survives_the_json_face_with_its_sign_bit() {
    let s = snapshot(point(), EdgeGeometry::Line); // x = [1.0, -0.0]
    let text = to_json(&s);
    assert!(text.contains(r#""x":[1,-0]"#), "{text}");
    let back = from_json(&text).expect("reads back");
    let NodeGeometry::Point { x, .. } = &back.parts().nodes else {
        panic!("a Point is a Point")
    };
    assert_eq!(
        x[1].to_bits(),
        (-0.0f32).to_bits(),
        "the sign bit is still set"
    );
    assert_ne!(
        x[1].to_bits(),
        0.0f32.to_bits(),
        "and it is not a plain zero"
    );
    assert_eq!(back.to_bytes(), s.to_bytes(), "so the bytes compare equal");
}

/// What it gets wrong: `to_json` writes an endpoint as a node id and has no error channel,
/// so an out-of-range endpoint position would come out as `""` — an id no snapshot defines.
/// The escape hatch is [`Snapshot::new`], which refuses such a snapshot outright, and the
/// only two ways to hold a [`Snapshot`] — `Snapshot::new` and `Snapshot::from_bytes`, which
/// decodes through it — both go through that refusal; the second half below shows the reader
/// refusing before it can build one. `Ponytail:` kept as-is because making it an error would
/// change `to_json`'s public signature; the invariant is a constructor's, not a writer's.
#[test]
fn an_endpoint_that_is_not_a_node_is_refused_before_any_writer_can_see_it() {
    let mut parts = snapshot(point(), EdgeGeometry::Line).into_parts();
    parts.source = vec![2]; // two nodes, so position 2 names none of them
    assert_eq!(
        Snapshot::new(parts),
        Err(SnapshotError::Endpoint {
            column: "edge.source",
            index: 0, // the edge whose endpoint is out of range, not the endpoint value
        })
    );
    let text = to_json(&snapshot(point(), EdgeGeometry::Line));
    let orphan = text.replace(r#""target":["a"]"#, r#""target":["ghost"]"#);
    assert_ne!(orphan, text, "the edit landed");
    assert_eq!(
        from_json(&orphan),
        Err(JsonError::Shape {
            path: "edges.target[0]".into(),
            what: "is not a node id",
        }),
        "and the reader refuses an endpoint id no node declares before it builds a snapshot"
    );
}

/// Keys are ordered by UTF-8 **bytes** (`binary-layout.md` §Canonical), which is not
/// JavaScript's UTF-16 code-unit order: U+10000 is a surrogate pair (`D800 DC00`) and so
/// sorts before U+E000 there, while its UTF-8 bytes (`F0 90 80 80`) sort last.
#[test]
fn node_ids_are_written_in_utf8_byte_order_where_utf16_would_disagree() {
    let ids = ["\u{E000}", "\u{FFFD}", "\u{10000}"];
    let mut parts = snapshot(point(), EdgeGeometry::Line).into_parts();
    parts.node_ids = StringTable::from_strs("node.id", ids).expect("fits");
    parts.nodes = NodeGeometry::Point {
        x: vec![0.0; 3],
        y: vec![0.0; 3],
    };
    parts.source = vec![2];
    parts.target = vec![0];
    let text = to_json(&Snapshot::new(parts).expect("three nodes, three columns"));
    assert!(
        text.contains("\"nodes\":{\"id\":[\"\u{E000}\",\"\u{FFFD}\",\"\u{10000}\"]}"),
        "{text}"
    );
    assert_sorted(&parse::parse(&text).expect("json"));
}
