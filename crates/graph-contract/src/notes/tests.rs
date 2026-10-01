use super::*;
use crate::binary::{Snapshot, SnapshotParts, StringTable};
use crate::geometry::{EdgeGeometry, NodeGeometry};
use crate::snapshot::{Dim, SnapshotError as E, label_for};
use crate::version::{CURRENT_VERSION, UNVERSIONED};

mod columns;
mod json;

const V0_2: FormatVersion = FormatVersion { major: 0, minor: 2 };

/// These snapshots are 2D, so the label rule gives them 0.3 — the version whose notes
/// section they are about. `CURRENT_VERSION` is 0.4 and is not a label a 2D snapshot
/// takes (`snapshot::label_for`).
const V0_3: FormatVersion = label_for(Dim::D2);

/// Three nodes, two edges (`ab`, `bc`), Point and Line, at `version` with `notes`.
fn parts(version: FormatVersion, notes: Notes) -> SnapshotParts {
    let ids = |column, items: &[&str]| {
        StringTable::from_strs(column, items.iter().copied()).expect("fits")
    };
    SnapshotParts {
        version,
        node_ids: ids("node.id", &["a", "b", "c"]),
        edge_ids: ids("edge.id", &["ab", "bc"]),
        source: vec![0, 1],
        target: vec![1, 2],
        nodes: NodeGeometry::Point {
            x: vec![0.0; 3],
            y: vec![1.0; 3],
        },
        z: None,
        edges: EdgeGeometry::Line,
        notes,
    }
}

fn columns(code: &[u32], index: &[u32]) -> Notes {
    Notes {
        code: code.to_vec(),
        index: index.to_vec(),
    }
}

fn build(code: &[u32], index: &[u32]) -> Result<Snapshot, SnapshotError> {
    Snapshot::new(parts(V0_3, columns(code, index)))
}

/// One note of every code, canonical: `(1,0) (1,1) (2,1) (3,MAX)`.
fn every_code() -> Snapshot {
    build(&[1, 1, 2, 3], &[0, 1, 1, SNAPSHOT_WIDE]).expect("valid")
}

#[test]
fn note_codes_are_a_closed_set_like_the_geometry_tags() {
    for (code, want) in (1..=5).zip(NoteCode::ALL) {
        assert_eq!(NoteCode::from_code(code), Ok(want));
        assert_eq!(want.code(), code);
    }
    let names = NoteCode::ALL.map(NoteCode::name);
    assert_eq!(
        names,
        [
            "hierarchy.cycle_edge_dropped",
            "hierarchy.extra_parent_dropped",
            "packing.approximate",
            "dag.dummy_budget_exceeded",
            "dag.edge_reversed",
        ]
    );
    for code in RESERVED_NOTE_CODES {
        assert_eq!(
            NoteCode::from_code(code),
            Err(NoteCodeError::Reserved(code))
        );
    }
    assert_eq!(RESERVED_NOTE_CODES, [6]);
    for code in [0, 7, u32::MAX] {
        assert_eq!(NoteCode::from_code(code), Err(NoteCodeError::Unknown(code)));
    }
    let reserved = NoteCodeError::Reserved(6).to_string();
    assert_eq!(reserved, "note code 6 is reserved and not implemented");
    assert_eq!(
        NoteCodeError::Unknown(9).to_string(),
        "note code 9 is not allocated"
    );
}

#[test]
fn notes_are_carried_from_format_0_3_on() {
    assert_eq!(CURRENT_VERSION, FormatVersion { major: 0, minor: 4 });
    for (version, carries) in [
        (UNVERSIONED, false),
        (V0_2, false),
        (V0_3, true),
        (CURRENT_VERSION, true),
        (
            FormatVersion {
                major: 0,
                minor: u32::MAX,
            },
            true,
        ),
    ] {
        assert_eq!(carries_notes(version), carries, "{version}");
    }
}

#[test]
fn notes_are_the_last_section_and_the_order_is_the_one_given() {
    let bytes = every_code().to_bytes();
    #[rustfmt::skip]
    let tail: Vec<u8> = [4, 1, 1, 2, 3, 0, 1, 1, u32::MAX]
        .iter().flat_map(|v: &u32| v.to_le_bytes()).collect();
    assert!(bytes.ends_with(&tail), "k, then code × k, then index × k");
    assert_eq!(Snapshot::from_bytes(&bytes), Ok(every_code()));
    let sorted = [(2, 0), (1, 1), (3, SNAPSHOT_WIDE), (1, 0)].map(|(c, index)| Note {
        code: NoteCode::from_code(c).expect("implemented"),
        index,
    });
    let mut notes = sorted.to_vec();
    notes.sort();
    assert_eq!(
        Notes::of(&notes),
        columns(&[1, 1, 2, 3], &[0, 1, 0, u32::MAX])
    );
    assert_eq!((Notes::of(&notes).len(), Notes::default().len()), (4, 0));
}

#[test]
fn construction_refuses_every_note_outside_the_closed_canonical_set() {
    let code = |index, error| Err(E::NoteCode { index, error });
    assert_eq!(build(&[6], &[0]), code(0, NoteCodeError::Reserved(6)));
    assert_eq!(build(&[1, 6], &[0, 0]), code(1, NoteCodeError::Reserved(6)));
    assert_eq!(build(&[0], &[0]), code(0, NoteCodeError::Unknown(0)));
    assert_eq!(build(&[7], &[0]), code(0, NoteCodeError::Unknown(7)));
    let order = Err(E::NoteOrder { index: 1 });
    assert_eq!(build(&[1, 1], &[0, 0]), order, "a repeat");
    assert_eq!(build(&[2, 1], &[0, 0]), order, "codes out of order");
    assert_eq!(build(&[1, 1], &[1, 0]), order, "indices out of order");
    let target = |index| Err(E::NoteTarget { index });
    assert_eq!(build(&[1], &[2]), target(0), "edge 2 of 2");
    assert_eq!(build(&[1, 2], &[1, u32::MAX]), target(1));
    assert_eq!(build(&[3], &[0]), target(0), "code 3 is snapshot-wide");
    let length = E::Length {
        column: "note.index",
        expected: 1,
        found: 0,
    };
    assert_eq!(build(&[1], &[]), Err(length));
    let old = |version| Snapshot::new(parts(version, columns(&[3], &[SNAPSHOT_WIDE])));
    assert_eq!(old(V0_2), Err(E::NotesUnsupported { version: V0_2 }));
    assert_eq!(
        old(UNVERSIONED),
        Err(E::NotesUnsupported {
            version: UNVERSIONED
        })
    );
    assert!(Snapshot::new(parts(V0_2, Notes::default())).is_ok());
}

#[test]
fn every_note_refusal_says_where_and_what() {
    let cases = [
        (
            E::NoteCode {
                index: 2,
                error: NoteCodeError::Reserved(6),
            },
            "note.code[2]: note code 6 is reserved and not implemented",
        ),
        (
            E::NoteOrder { index: 1 },
            "note[1]: notes must be strictly ascending by (code, index)",
        ),
        (
            E::NoteTarget { index: 0 },
            "note.index[0]: not an index its code allows",
        ),
        (
            E::NotesUnsupported { version: V0_2 },
            "notes: format 0.2 carries none; notes need 0.3 or later",
        ),
    ];
    for (err, message) in cases {
        assert_eq!(err.to_string(), message);
    }
}

/// `every_code`'s bytes with `patch` applied at the notes section, read back. `at` is
/// the first byte of the section: `k`, then `code × 4`, then `index × 4`.
fn decode_patched(patch: impl FnOnce(&mut Vec<u8>, usize)) -> Result<Snapshot, SnapshotError> {
    let mut bytes = every_code().to_bytes();
    let at = bytes.len() - 36;
    patch(&mut bytes, at);
    Snapshot::from_bytes(&bytes)
}

#[test]
fn the_decoder_refuses_what_construction_refuses() {
    let (code, index) = (|i: usize| 4 + 4 * i, |i: usize| 20 + 4 * i);
    let reserved = decode_patched(|b, at| word(b, at + code(0), 6));
    let reserved_err = NoteCodeError::Reserved(6);
    assert_eq!(
        reserved,
        Err(E::NoteCode {
            index: 0,
            error: reserved_err
        })
    );
    let unknown = decode_patched(|b, at| word(b, at + code(3), 9));
    let unknown_err = NoteCodeError::Unknown(9);
    assert_eq!(
        unknown,
        Err(E::NoteCode {
            index: 3,
            error: unknown_err
        })
    );
    let repeat = decode_patched(|b, at| word(b, at + index(1), 0));
    assert_eq!(repeat, Err(E::NoteOrder { index: 1 }));
    let stray = decode_patched(|b, at| word(b, at + index(0), 2));
    assert_eq!(stray, Err(E::NoteTarget { index: 0 }));
    let not_wide = decode_patched(|b, at| word(b, at + index(3), 5));
    assert_eq!(not_wide, Err(E::NoteTarget { index: 3 }));
}

#[test]
fn the_decoder_refuses_a_section_of_the_wrong_length() {
    let more = decode_patched(|b, at| word(b, at, 5));
    let column = "note.index";
    assert_eq!(more, Err(E::Truncated { column }));
    let short = decode_patched(|b, at| b.truncate(at + 2));
    let column = "note.count";
    assert_eq!(short, Err(E::Truncated { column }));
    let trailing = decode_patched(|b, _| b.extend([0; 4]));
    assert_eq!(trailing, Err(E::TrailingBytes { count: 4 }));
}

/// Overwrites the little-endian word at `at`.
fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Phase 5 deviation (`docs/decisions/sugiyama-heuristics.md`): codes 4 and 5 were
/// reserved for exactly this and are activated here. Both faces round-trip a note of
/// each, and each still refuses the one code that stays reserved.
#[test]
fn dag_note_codes_activate_and_round_trip_both_faces() {
    assert_eq!(NoteCode::DummyBudgetExceeded.code(), 4);
    assert_eq!(NoteCode::EdgeReversed.code(), 5);
    let s = build(&[4, 5], &[0, 1]).expect("codes 4 and 5 are implemented");
    assert_eq!(Snapshot::from_bytes(&s.to_bytes()), Ok(s.clone()), "binary");
    let text = crate::canonical_json::to_json(&s);
    assert_eq!(crate::canonical_json::from_json(&text), Ok(s), "json");
    assert!(
        text.contains(r#""notes":{"code":[4,5],"index":[0,1]}"#),
        "{text}"
    );
    assert_eq!(
        build(&[6], &[0]),
        Err(E::NoteCode {
            index: 0,
            error: NoteCodeError::Reserved(6)
        }),
        "6 stays reserved"
    );
}

#[test]
fn snapshots_differing_only_in_notes_differ_in_bytes() {
    let bare = build(&[], &[]).expect("valid").to_bytes();
    let noted = build(&[3], &[SNAPSHOT_WIDE]).expect("valid").to_bytes();
    let other = build(&[2], &[0]).expect("valid").to_bytes();
    let head = bare.len() - 4;
    assert_eq!(
        bare[..head],
        noted[..head],
        "the same up to the notes section"
    );
    assert_eq!((bare.len() + 8, noted.len()), (noted.len(), other.len()));
    assert_ne!(noted, other);
}
