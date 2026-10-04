//! The validating walk, on the inputs the frozen-reader differential corpus does not
//! produce: whitespace between tokens, escapes in a member *name*, and an array where a
//! scalar member belongs.
//!
//! The corpus in [`super::differential`] is the judge that the walk agrees with
//! `graph_contract::canonical_json::parse` byte for byte. These are the shapes that corpus
//! cannot reach — it mutates documents a studio or a fixture wrote, and none of those put a
//! newline between two tokens or spelled `database_id` with a `\u` escape — so the walk's
//! own handling of them is pinned here instead.

use super::*;
use crate::ingest::element::Shape;
use crate::ingest::scan::{Document, Scan};

/// The document's `nodes` and `edges` element counts, as the first walk counted them.
fn counted(text: &str) -> (usize, usize) {
    let document = Document::new(text).expect("valid JSON");
    let count = |key: &str| {
        document
            .member(key)
            .and_then(|m| m.elements)
            .unwrap_or_default()
    };
    (count("nodes"), count("edges"))
}

/// The same document with newlines and spaces between every token reads the same, because
/// RFC 8259 allows whitespace anywhere a value, a name or a separator may sit.
#[test]
fn whitespace_between_every_token_reads_the_same_document() {
    let tight = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b")],
    );
    let loose = tight
        .replace('{', "{\n  ")
        .replace(',', " ,\n  ")
        .replace(':', " : ")
        .replace('[', "[\n  ")
        .replace(']', "\n]");
    assert_ne!(
        tight, loose,
        "the loose form was not actually different bytes"
    );
    assert_eq!(read(tight.as_bytes()), read(loose.as_bytes()));
    assert_eq!(counted(&loose), counted(&tight));
}

/// A member name spelled with an escape is the name it unescapes to: `\u0061` is an `a`,
/// so the walk must see `database_id` and read the member, not refuse the document for a
/// member the shape does not name.
#[test]
fn a_member_name_spelled_with_an_escape_is_the_name_it_unescapes_to() {
    let plain = node_json("a");
    let escaped = plain.replace(r#""database_id""#, r#""datab\u0061se_id""#);
    assert_ne!(plain, escaped);
    let (a, _) = read(doc(&[plain], &[]).as_bytes()).expect("plain is valid");
    let (b, _) = read(doc(&[escaped], &[]).as_bytes()).expect("escaped is valid");
    assert_eq!(a, b);
}

/// A member *value* spelled with an escape is unescaped into the record, escapes and all.
#[test]
fn a_member_value_spelled_with_an_escape_is_unescaped_into_the_record() {
    let plain = node_json("a");
    let escaped = plain.replace(r#""label":"L""#, r#""label":"L\nA""#);
    assert_ne!(plain, escaped);
    let (nodes, _) = read(doc(&[escaped], &[]).as_bytes()).expect("valid");
    assert_eq!(nodes[0].label, "L\nA");
}

/// An array where a scalar member belongs is refused as that member's type, not read as a
/// record and not walked past: the walk's array branch counts elements for the *root*, and
/// a record's own array member must not be mistaken for one.
#[test]
fn an_array_where_a_scalar_member_belongs_is_refused_as_that_members_type() {
    let text = doc(
        &[node_json("a").replace(r#""icon":null"#, r#""icon":[1,2]"#)],
        &[],
    );
    assert_eq!(
        read(text.as_bytes()),
        Err(IngestError::Shape(
            "nodes[0].icon: expected a string or null".to_owned()
        ))
    );
}

/// A key repeated in one record is a syntax fault, reported at the second key's opening
/// quote — the offset the reader this replaces reports it at, which is a property of the
/// walk's ordering and not of the shape pass.
#[test]
fn a_key_repeated_in_one_record_is_a_syntax_fault_at_the_second_key() {
    let doubled = node_json("a").replace(r#""label":"L""#, r#""label":"L","label":"M""#);
    let at = doubled.find(r#""label":"M""#).expect("the member is there");
    assert_eq!(
        read(doubled.as_bytes()),
        Err(IngestError::Json(
            graph_contract::canonical_json::JsonError::Syntax {
                at: u32::try_from(at).expect("a small offset"),
                what: "a key repeated in one object",
            }
        ))
    );
}

/// The count the first walk reports is the number of elements the second walk reads: the
/// `Vec` of records is sized from it and must never be short.
#[test]
fn the_counted_length_is_the_length_the_records_come_out_at() {
    let nodes: Vec<String> = (0..7).map(|i| node_json(&format!("n{i}"))).collect();
    let edges: Vec<String> = (0..5)
        .map(|i| edge_json(&format!("e{i}"), "n0", "n1"))
        .collect();
    let text = doc(&nodes, &edges);
    assert_eq!(counted(&text), (7, 5));
    let (read_nodes, read_edges) = read_records(text.as_bytes()).expect("valid");
    assert_eq!((read_nodes.len(), read_edges.len()), (7, 5));
}

/// A root that is not an object is refused by the shape pass, after the walk has had its
/// say on the syntax — the order the reader this replaces refused in.
#[test]
fn a_root_that_is_not_an_object_is_refused_after_the_syntax_has_been_read() {
    assert_eq!(
        read(b"[1,2,3]"),
        Err(IngestError::Shape(": expected an object".to_owned()))
    );
    // A syntax fault anywhere in it still comes first, even though the shape is wrong too.
    assert!(matches!(read(b"[1,2,"), Err(IngestError::Json(_))));
}

/// The root's members are located by the walk that validates the text, so the shape pass
/// reads them without a second pass over the document. What it must find is unchanged: the
/// members in document order, each value span naming its own array, and an *empty* object
/// root still a root object — the flag "expected an object" turns on.
#[test]
fn the_root_is_located_by_the_validating_walk_itself() {
    let text = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b")],
    );
    let document = Document::new(&text).expect("valid");
    assert!(document.is_object());
    let keys: Vec<&str> = document.members().iter().map(|m| m.key.as_str()).collect();
    assert_eq!(keys, ["version", "nodes", "edges"]);
    for (key, elements) in [("nodes", 2usize), ("edges", 1)] {
        let member = document.member(key).expect("the member is there");
        assert_eq!(
            member.elements,
            Some(elements),
            "{key} is not counted as an array"
        );
        let span = member.value;
        let value = &text[span.start as usize..span.end as usize];
        assert!(
            value.starts_with('[') && value.ends_with(']'),
            "{key} is {value}"
        );
    }
    // An empty object is an object, and only the first byte ever decided it.
    assert!(Document::new("{}").expect("valid").is_object());
    for not_object in ["[]", "1", r#""x""#, "null", "true"] {
        assert!(
            !Document::new(not_object).expect("valid").is_object(),
            "{not_object} is not an object"
        );
    }
}

/// Each element is read on its own and into a table of its own, so a record reader never
/// sees its neighbours' bytes — which is what lets the table hold spans into the document
/// and strings borrowed from it. The records that come out are the document's, in order.
#[test]
fn each_element_is_read_into_a_table_of_its_own() {
    let text = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b"), edge_json("f", "b", "a")],
    );
    let document = Document::new(&text).expect("valid");
    let mut ids: Vec<String> = Vec::new();
    let mut scan = Scan::new(&text);
    let mut table = element::Element::new(&text, <NodeRecord as Shape>::FIELDS, At::list("nodes"));
    let nodes = document.member("nodes").expect("the member is there").value;
    scan.records(nodes, &mut table, &mut |table| {
        let at = table.at();
        ids.push(<NodeRecord as Shape>::read(table, at)?.id);
        Ok(())
    })
    .expect("the array reads");
    let mut scan = Scan::new(&text);
    let mut table = element::Element::new(&text, <EdgeRecord as Shape>::FIELDS, At::list("edges"));
    let edges = document.member("edges").expect("the member is there").value;
    scan.records(edges, &mut table, &mut |table| {
        let at = table.at();
        ids.push(<EdgeRecord as Shape>::read(table, at)?.id);
        Ok(())
    })
    .expect("the array reads");
    assert_eq!(ids, ["a", "b", "e", "f"]);
}
