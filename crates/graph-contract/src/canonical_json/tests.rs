use super::*;
use crate::binary::{SnapshotParts, StringTable};
use crate::notes::Notes;
use crate::snapshot::{Dim, label_for};
use crate::version::CURRENT_VERSION;

mod dim;
mod generated;
mod shape;
mod syntax;
mod text;

/// Every kind here is 2D, so the label is 0.3 and the text carries no `"dim"` — the
/// pinned text below proves it. [`spaced`] builds the 3D counterpart.
fn snapshot(nodes: NodeGeometry, edges: EdgeGeometry) -> Snapshot {
    let ids = |column, items: &[&str]| StringTable::from_strs(column, items.iter().copied());
    Snapshot::new(SnapshotParts {
        version: label_for(Dim::D2),
        node_ids: ids("node.id", &["a", "q\"\\\n\u{1}\u{7f}é\u{1F680}"]).expect("fits"),
        edge_ids: ids("edge.id", &["e"]).expect("fits"),
        source: vec![1],
        target: vec![0],
        nodes,
        z: None,
        edges,
        notes: Notes::default(),
    })
    .expect("valid")
}

/// The same snapshot in three dimensions: a z column and the 0.4 label it needs. Built by
/// hand because this crate cannot run a layout: it depends on no `graph-core`, so one is out
/// of its reach however many the registry holds, and a z column is all it ever reads.
fn spaced(nodes: NodeGeometry, edges: EdgeGeometry, z: Vec<f32>) -> Snapshot {
    let mut p = snapshot(nodes, edges).into_parts();
    p.z = Some(z);
    p.version = label_for(Dim::D3);
    Snapshot::new(p).expect("a 3D snapshot is valid")
}

fn point() -> NodeGeometry {
    NodeGeometry::Point {
        x: vec![1.0, -0.0],
        y: vec![0.1, 1e-45],
    }
}

fn every_kind() -> Vec<Snapshot> {
    let paths = Paths {
        offsets: vec![0, 2],
        pts: vec![0.5, f32::MAX, -3.25, f32::MIN_POSITIVE],
    };
    let nodes = [
        point(),
        NodeGeometry::Circle {
            x: vec![2.0, 3.0],
            y: vec![-1.0, 1e10],
            r: vec![0.0, 0.3],
        },
        NodeGeometry::Box {
            x: vec![0.0; 2],
            y: vec![7.0; 2],
            w: vec![1.5, 0.0],
            h: vec![2.5, 1e-7],
        },
    ];
    let edges = [
        EdgeGeometry::Line,
        EdgeGeometry::Polyline(paths.clone()),
        EdgeGeometry::Curve { degree: 2, paths },
    ];
    let mut all = Vec::new();
    for n in &nodes {
        for e in &edges {
            all.push(snapshot(n.clone(), e.clone()));
        }
    }
    all
}

#[test]
fn the_text_is_pinned_for_a_tiny_snapshot() {
    let text = to_json(&snapshot(point(), EdgeGeometry::Line));
    let expected = concat!(
        r#"{"edges":{"id":["e"],"source":["q\"\\\n\u0001"#,
        "\u{7f}é\u{1F680}",
        r#""],"target":["a"]},"geometry":{"edges":{"kind":"Line"},"nodes":{"kind":"Point","x":[1,-0],"y":[0.1,0.000000000000000000000000000000000000000000001]}},"nodes":{"id":["a","q\"\\\n\u0001"#,
        "\u{7f}é\u{1F680}",
        r#""]},"notes":{"code":[],"index":[]},"version":{"major":0,"minor":3}}"#,
        "\n"
    );
    assert_eq!(text, expected);
}

#[test]
fn every_kind_writes_its_members_in_byte_order() {
    let curve = to_json(&every_kind()[8]);
    assert!(curve.contains(r#"{"edges":{"degree":2,"kind":"Curve","offsets":[0,2],"pts":[0.5,340282350000000000000000000000000000000,-3.25,0.000000000000000000000000000000000000011754944]},"nodes":{"h":[2.5,0.0000001],"kind":"Box","w":[1.5,0],"x":[0,0],"y":[7,7]}}"#), "{curve}");
    let circle = to_json(&every_kind()[4]);
    assert!(
        circle.contains(r#""nodes":{"kind":"Circle","r":[0,0.3],"x":[2,3],"y":[-1,10000000000]}"#),
        "{circle}"
    );
    for s in every_kind() {
        assert_sorted(&parse::parse(&to_json(&s)).expect("json"));
    }
}

fn assert_sorted(value: &Value) {
    match value {
        Value::Object(members) => {
            let keys: Vec<&str> = members.iter().map(|(k, _)| k.as_str()).collect();
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "{keys:?}");
            members.iter().for_each(|(_, v)| assert_sorted(v));
        }
        Value::Array(items) => items.iter().for_each(assert_sorted),
        _ => {}
    }
}

#[test]
fn binary_to_json_to_binary_is_byte_exact_for_every_kind() {
    for s in every_kind() {
        let text = to_json(&s);
        let back = from_json(&text).expect("reads back");
        assert_eq!(back.to_bytes(), s.to_bytes());
        assert_eq!(to_json(&back), text, "and the text is a fixed point");
    }
}

#[test]
fn any_json_spelling_of_the_same_snapshot_reads_the_same() {
    let loose = r#" { "version" : { "minor" : 2 , "major" : 0 } ,
        "nodes" : { "id" : [ "a" , "\u0062" ] } ,
        "geometry" : { "nodes" : { "y" : [ 1E0 , -0.5e+0 ] , "x" : [ 0.10000000149011612 , 2.0 ] , "kind" : "Point" } ,
                       "edges" : { "kind" : "Line" } } ,
        "edges" : { "target" : [ "b" ] , "source" : [ "a" ] , "id" : [ "\/e\ud83d\ude80" ] } } "#;
    let s = from_json(loose).expect("valid JSON of a valid snapshot");
    let canonical = to_json(&s);
    assert!(canonical.starts_with(r#"{"edges":{"id":["/e🚀"],"source":["a"],"target":["b"]}"#));
    assert!(
        canonical.contains(r#""x":[0.1,2],"y":[1,-0.5]"#),
        "{canonical}"
    );
}

#[test]
fn version_refusal_of_a_json_snapshot_one_major_ahead() {
    let text = to_json(&every_kind()[3]);
    let newer = text.replace(r#""major":0"#, r#""major":1"#);
    let refusal = JsonError::Version(NewerMajor {
        found: crate::version::FormatVersion { major: 1, minor: 3 },
        known: CURRENT_VERSION,
    });
    assert_eq!(from_json(&newer), Err(refusal.clone()));
    assert!(
        refusal
            .to_string()
            .contains("1.3 is newer than this reader's 0.4")
    );
    let reshaped = newer.replace(r#""geometry""#, r#""geometries""#);
    assert_eq!(
        from_json(&reshaped),
        Err(refusal),
        "refused for its version before its shape is looked at"
    );
}

#[test]
fn version_refusal_is_not_a_missing_version_which_reads_as_0_0() {
    let text = to_json(&every_kind()[0]);
    let bare = text.replace(
        r#","notes":{"code":[],"index":[]},"version":{"major":0,"minor":3}"#,
        "",
    );
    let s = from_json(&bare).expect("an unversioned document reads");
    assert_eq!(s.parts().version, UNVERSIONED);
    assert!(to_json(&s).ends_with("\"version\":{\"major\":0,\"minor\":0}}\n"));
    let newer_minor = text.replace(r#""minor":3"#, r#""minor":9"#);
    assert_eq!(
        from_json(&newer_minor)
            .expect("reads")
            .parts()
            .version
            .minor,
        9
    );
}

#[test]
fn f32_display_is_the_shortest_decimal_and_both_read_paths_give_the_value_back() {
    // Every 65521st bit pattern (a prime stride, so every exponent and mantissa region is
    // visited), plus the edges of the range. `roundtrip` covers every value it writes.
    let edges = [0, 1, 0x007f_ffff, 0x0080_0000, 0x7f7f_ffff, 0x3f80_0000];
    let sweep = (0..=u32::MAX)
        .step_by(65_521)
        .chain(edges)
        .chain(edges.map(|b| b | 1 << 31));
    let mut checked = 0;
    for bits in sweep {
        let value = f32::from_bits(bits);
        if !value.is_finite() {
            continue;
        }
        let text = value.to_string();
        let exact: f32 = text.parse().expect("f32 reads its own Display");
        let via_f64 = text.parse::<f64>().expect("f64 reads it") as f32;
        assert_eq!(exact.to_bits(), bits, "{text}");
        assert_eq!(
            via_f64.to_bits(),
            bits,
            "{text} through f64 (the JSON.parse path)"
        );
        assert!(!text.contains(['e', 'E']), "{text}");
        checked += 1;
    }
    assert!(checked > 60_000, "{checked}");
}

#[test]
fn the_writer_escapes_only_what_json_requires() {
    let mut out = String::new();
    string(&mut out, "\"\\\u{8}\u{c}\n\r\t\u{0}\u{1f} \u{7f}\u{2028}é/");
    assert_eq!(
        out,
        "\"\\\"\\\\\\b\\f\\n\\r\\t\\u0000\\u001f \u{7f}\u{2028}é/\""
    );
    assert_eq!(
        parse::parse(&out),
        Ok(Value::String(
            "\"\\\u{8}\u{c}\n\r\t\u{0}\u{1f} \u{7f}\u{2028}é/".into()
        ))
    );
}

#[test]
fn every_error_message_names_where_and_what() {
    let cases = [
        (
            JsonError::Syntax { at: 7, what: "x" }.to_string(),
            "not JSON at byte 7: x",
        ),
        (
            JsonError::Shape {
                path: "nodes.id[2]".into(),
                what: "must be a string",
            }
            .to_string(),
            "nodes.id[2]: must be a string",
        ),
        (
            JsonError::Snapshot(SnapshotError::CurveDegree).to_string(),
            "degree 1 or more",
        ),
    ];
    for (message, needle) in cases {
        assert!(message.contains(needle), "{message:?} lacks {needle:?}");
    }
}

#[test]
fn the_kind_tables_name_every_kind_once() {
    assert_eq!(NODE_KINDS.map(|(k, _)| k.tag()), [0, 1, 2]);
    assert_eq!(EDGE_KINDS.map(|(k, _)| k.tag()), [0, 1, 2]);
    assert_eq!(node_kind_name(NodeGeometryKind::Box), "Box");
    assert_eq!(edge_kind_name(EdgeGeometryKind::Polyline), "Polyline");
}

#[cfg(feature = "codegen")]
#[test]
fn the_schema_types_read_every_canonical_text_and_write_what_it_reads_back() {
    for s in every_kind() {
        let text = to_json(&s);
        let typed: schema::Snapshot = serde_json::from_str(&text).expect("the schema's shape");
        let written = serde_json::to_string(&typed).expect("serialises");
        assert_eq!(from_json(&written).expect("reads serde's text"), s);
    }
    let bare = to_json(&every_kind()[0]).replace(r#","version":{"major":0,"minor":3}"#, "");
    let typed: schema::Snapshot = serde_json::from_str(&bare).expect("version is optional");
    assert_eq!(typed.version, UNVERSIONED);
    let extra =
        to_json(&every_kind()[0]).replace(r#""nodes":{"id""#, r#""nodes":{"label":[],"id""#);
    assert!(
        serde_json::from_str::<schema::Snapshot>(&extra).is_err(),
        "no unknown member"
    );
}
