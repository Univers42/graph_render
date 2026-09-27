use super::*;

/// `text` with `from` replaced by `to`, read.
fn read_edited(from: &str, to: &str) -> Result<Snapshot, JsonError> {
    let text = to_json(&snapshot(point(), EdgeGeometry::Line));
    assert!(text.contains(from), "{from}");
    from_json(&text.replacen(from, to, 1))
}

fn shape_at(path: &str, what: &'static str) -> Result<Snapshot, JsonError> {
    Err(JsonError::Shape {
        path: path.into(),
        what,
    })
}

#[test]
fn a_document_off_the_shape_names_the_path_and_the_fault() {
    let missing = shape_at("geometry.nodes.y", "is missing");
    assert_eq!(
        read_edited(
            r#","y":[0.1,0.000000000000000000000000000000000000000000001]"#,
            ""
        ),
        missing
    );
    let extra = shape_at("nodes.label", "is not part of the snapshot shape");
    assert_eq!(
        read_edited(r#""nodes":{"id""#, r#""nodes":{"label":[],"id""#),
        extra
    );
    let stray = shape_at("edges.target[0]", "is not a node id");
    assert_eq!(read_edited(r#""target":["a"]"#, r#""target":["z"]"#), stray);
    let kind = shape_at("geometry.edges.kind", "is not a kind this version knows");
    assert_eq!(read_edited(r#""kind":"Line""#, r#""kind":"Ribbon""#), kind);
}

#[test]
fn a_value_of_the_wrong_type_names_its_path_and_the_type_it_needs() {
    let integer = shape_at("version.minor", "must be an integer from 0 to 4294967295");
    assert_eq!(
        read_edited(r#""minor":2"#, r#""minor":2.0"#),
        integer.clone()
    );
    assert_eq!(
        read_edited(r#""minor":2"#, r#""minor":4294967296"#),
        integer.clone()
    );
    assert_eq!(read_edited(r#""minor":2"#, r#""minor":-1"#), integer);
    let number = shape_at("geometry.nodes.x[1]", "must be a number");
    assert_eq!(read_edited("[1,-0]", r#"[1,"-0"]"#), number);
    let string = shape_at("nodes.id[0]", "must be a string");
    assert_eq!(read_edited(r#""id":["a""#, r#""id":[null"#), string);
    let array = shape_at("edges.id", "must be an array");
    assert_eq!(read_edited(r#""id":["e"]"#, r#""id":"e""#), array);
    assert_eq!(
        from_json(r#"{"nodes":[]}"#),
        shape_at("nodes", "must be an object")
    );
    assert_eq!(from_json("[]"), shape_at("", "must be an object"));
    assert_eq!(
        JsonError::Shape {
            path: String::new(),
            what: "must be an object"
        }
        .to_string(),
        "the document: must be an object"
    );
}

#[test]
fn a_valid_shape_that_breaks_a_rule_is_refused_by_the_snapshot() {
    let inf = read_edited("[1,-0]", "[1,1e39]");
    let nan = JsonError::Snapshot(SnapshotError::NonFinite {
        column: "node.x",
        index: 1,
    });
    assert_eq!(inf, Err(nan));
    let repeat = read_edited(r#""id":["a","#, r#""id":["a","a","#);
    assert!(matches!(repeat, Err(JsonError::Snapshot(_))), "{repeat:?}");
}
