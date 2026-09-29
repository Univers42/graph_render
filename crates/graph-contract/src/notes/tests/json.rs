//! The JSON face of the notes: the same refusals as the binary face, through
//! `Snapshot::new`, plus the shape rules only a text face has.

use super::*;
use crate::canonical_json::{JsonError, from_json, to_json};

const NOTES: &str = r#""notes":{"code":[1,1,2,3],"index":[0,1,1,4294967295]}"#;

/// `every_code`'s canonical text with `from` replaced by `to`, read.
fn read_edited(from: &str, to: &str) -> Result<Snapshot, JsonError> {
    let text = to_json(&every_code());
    assert!(text.contains(from), "{from} in {text}");
    from_json(&text.replacen(from, to, 1))
}

fn refused(err: SnapshotError) -> Result<Snapshot, JsonError> {
    Err(JsonError::Snapshot(err))
}

#[test]
fn the_json_face_writes_notes_sorted_by_key_and_reads_them_back() {
    let text = to_json(&every_code());
    let expected = format!(r#"{NOTES},"version":{{"major":0,"minor":3}}}}"#);
    assert!(text.ends_with(&format!("{expected}\n")), "{text}");
    assert_eq!(from_json(&text), Ok(every_code()));
    let bare = to_json(&build(&[], &[]).expect("valid"));
    assert!(bare.contains(r#""notes":{"code":[],"index":[]},"version""#));
}

#[test]
fn the_json_face_refuses_what_construction_refuses() {
    let code = |error| refused(E::NoteCode { index: 0, error });
    let reserved = read_edited(r#""code":[1,"#, r#""code":[4,"#);
    assert_eq!(reserved, code(NoteCodeError::Reserved(4)));
    let unknown = read_edited(r#""code":[1,"#, r#""code":[8,"#);
    assert_eq!(unknown, code(NoteCodeError::Unknown(8)));
    let repeat = read_edited(r#""index":[0,1,"#, r#""index":[0,0,"#);
    assert_eq!(repeat, refused(E::NoteOrder { index: 1 }));
    let stray = read_edited(r#""index":[0,"#, r#""index":[2,"#);
    assert_eq!(stray, refused(E::NoteTarget { index: 0 }));
    let not_wide = read_edited("4294967295", "7");
    assert_eq!(not_wide, refused(E::NoteTarget { index: 3 }));
    let short = read_edited(",4294967295]", "]");
    let length = E::Length {
        column: "note.index",
        expected: 4,
        found: 3,
    };
    assert_eq!(short, refused(length));
}

#[test]
fn notes_are_required_from_0_3_and_optional_below() {
    let missing = read_edited(&format!("{NOTES},"), "");
    let shape = JsonError::Shape {
        path: "notes".into(),
        what: "is missing",
    };
    assert_eq!(missing, Err(shape));
    let old = r#""version":{"major":0,"minor":2}"#;
    let noted_0_2 = read_edited(r#""version":{"major":0,"minor":3}"#, old);
    assert_eq!(noted_0_2, refused(E::NotesUnsupported { version: V0_2 }));
    let text = to_json(&build(&[], &[]).expect("valid"));
    let bare_0_2 = text
        .replace(r#","notes":{"code":[],"index":[]}"#, "")
        .replace(r#""minor":3"#, r#""minor":2"#);
    let read = from_json(&bare_0_2).expect("a 0.2 document with no notes reads");
    assert_eq!(read.parts().version, V0_2);
    assert!(read.parts().notes.is_empty());
    assert_eq!(to_json(&read), bare_0_2, "and is written back without them");
    let unversioned = read_edited(r#","version":{"major":0,"minor":3}"#, "");
    let too_old = E::NotesUnsupported {
        version: UNVERSIONED,
    };
    assert_eq!(unversioned, refused(too_old));
}

#[test]
fn the_notes_object_has_exactly_its_shape() {
    let extra = read_edited(r#""notes":{"#, r#""notes":{"why":[],"#);
    let path = |path: &str, what| {
        Err(JsonError::Shape {
            path: path.into(),
            what,
        })
    };
    assert_eq!(
        extra,
        path("notes.why", "is not part of the snapshot shape")
    );
    let signed = read_edited(r#""code":[1,"#, r#""code":[-1,"#);
    let integer = "must be an integer from 0 to 4294967295";
    assert_eq!(signed, path("notes.code[0]", integer));
    let wide = read_edited("4294967295", "4294967296");
    assert_eq!(wide, path("notes.index[3]", integer));
    let flat = read_edited(NOTES, r#""notes":[]"#);
    assert_eq!(flat, path("notes", "must be an object"));
}
