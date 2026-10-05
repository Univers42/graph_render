//! The write-time cell checks: one cell against the field that declares it.
//!
//! Split out of `batch.rs` by the house's 300-line limit, and because it is a different
//! question: `batch.rs` asks what the *batch wire* accepts, this asks what a *cell* may
//! hold once the manifest says what each field means. The two rules are the same rules the
//! engine applies later, applied here so a bad cell is refused before anything is stored.

use super::super::Limits;
use super::batch::{manifest, one};

/// Every cell whose *shape* the role fixes. `check` is the write-time gate: the ingest
/// reader checks a whole document's declarations, but a hub writes one record at a time,
/// and a record that could not appear in the document it will be served from must be
/// refused when it arrives — with nothing stored and nothing said until a read fails.
///
/// `(what it is, field id, the cell as JSON, the whole refusal)` — the message in full
/// because a path is only useful if it is the *whole* path.
const FORBIDDEN: [(&str, &str, &str, &str); 5] = [
    (
        "a tag containing a colon",
        "labels",
        r#"["a","b:c"]"#,
        // A list element's path is index-addressed, which is what makes the refusal
        // pointable: a client with three tags needs to know *which* one is bad.
        "upserts[0].values.labels[1]: a tag may not contain `:`",
    ),
    (
        "a bare text tags cell",
        "labels",
        r#""b:c""#,
        "upserts[0].values.labels: expected a list of strings",
    ),
    (
        "a non-numeric weight",
        "effort",
        r#""x""#,
        "upserts[0].values.effort: expected a number",
    ),
    (
        "a title that is a number",
        "name",
        "1",
        "upserts[0].values.name: expected a string",
    ),
    (
        "a parent that is a list of two",
        "up",
        r#"["a","b"]"#,
        "upserts[0].values.up: expected a single reference",
    ),
];

#[test]
fn a_cell_whose_shape_the_role_forbids_is_refused() {
    for (what, field, cell, expected) in FORBIDDEN {
        let batch = one(&format!(r#""{field}":{cell}"#));
        assert_eq!(
            batch
                .check("tracker", &manifest(), &Limits::DEFAULT)
                .unwrap_err()
                .to_string(),
            expected,
            "{what}"
        );
    }
}

/// Cardinality is *declared*, so a client cannot send a one-element list for a `one`
/// link and a bare text for a `many` one. A one-element list is accepted for `one` —
/// a client that builds both kinds of cell from the same code should not have to branch —
/// but a two-element list is not.
#[test]
fn a_link_cells_shape_follows_the_declared_cardinality() {
    let many_as_text = one(r#""blocks":"a""#);
    assert_eq!(
        many_as_text
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_err()
            .to_string(),
        "upserts[0].values.blocks: expected a list of references"
    );
    let one_ok = one(r#""blocks":["a"]"#);
    assert!(
        one_ok
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .is_ok()
    );
}
