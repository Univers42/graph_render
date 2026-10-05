//! The change, notice and answer writers: what a hub *answers* with.
//!
//! Everything so far is a request. These four are the responses, and they share one
//! property that the tests below check together: **no space and no newline**. A change is
//! compared and skipped, not pretty-printed, and one space in it would make two clients
//! that stored the same change disagree about what they stored.

use super::super::change::{
    ChangeHead, answer_json, change_json, check_change, manifest_change_json, max_change,
    notice_json,
};
use super::super::manifest::read_manifest;
use super::super::*;
use crate::ingest::{JsonValue, Record};

/// The stored record the upsert half of a change carries. Its `rev` is not a field: the
/// store's revision lives beside the record in the change, so it is the second half of
/// the `(Record, u64)` pair rather than anything inside [`Record`].
fn record(id: &str) -> Record {
    Record {
        id: id.to_owned(),
        collection: "t.c".to_owned(),
        deleted: false,
        updated_at: 5,
        values: vec![("name".to_owned(), JsonValue::Text("Write".into()))],
    }
}

fn head() -> ChangeHead<'static> {
    ChangeHead {
        seq: 118,
        plugin: "t",
        at: "2026-01-02T03:04:05Z",
    }
}

#[test]
fn a_change_writes_its_members_in_byte_order_with_the_literals_it_is_given() {
    let text = change_json(&head(), &[(record("r1"), 4)], &[("t.c".into(), "r2".into(), 7)]);
    assert_eq!(
        text,
        r#"{"at":"2026-01-02T03:04:05Z","deletes":[{"collection":"t.c","id":"r2","rev":7}],"kind":"batch","plugin":"t","seq":118,"upserts":[{"collection":"t.c","deleted":false,"id":"r1","rev":4,"updatedAt":5,"values":{"name":"Write"}}]}"#,
        "the change's own bytes are pinned: a diff here is a wire change"
    );
}

/// Every writer's output is parseable and compact. One test for all four, because the
/// property is one property: a response a client cannot parse, or that carries a space,
/// fails the same way.
#[test]
fn every_writer_output_is_one_compact_line_that_parses() {
    let manifest = read_manifest(
        r#"{"version":1,"manifestVersion":3,"name":"Tasks","collections":[
            {"id":"c","name":"C","titleField":"t","fields":[
              {"id":"t","name":"T","role":"title","link":null}]}]}"#,
        "t",
    )
    .unwrap();
    let texts = [
        change_json(&head(), &[(record("r1"), 4)], &[("t.c".into(), "r2".into(), 7)]),
        change_json(&head(), &[], &[]),
        manifest_change_json(&head(), &manifest),
        notice_json(&head()),
        answer_json(118, 0),
    ];
    for text in texts {
        assert!(!text.contains(' '), "a space in {text}");
        assert!(!text.contains('\n'), "a newline in {text}");
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(crate::canonical_json::parse(&text).is_ok(), "{text}");
    }
}

#[test]
fn a_manifest_change_names_the_manifest_and_nothing_else() {
    let manifest = read_manifest(
        r#"{"version":1,"manifestVersion":3,"name":"Tasks","collections":[]}"#,
        "t",
    )
    .unwrap();
    let text = manifest_change_json(&head(), &manifest);
    let keys: [&str; 5] = ["at", "kind", "manifest", "plugin", "seq"];
    let mut at = 0;
    for key in keys {
        let needle = format!("\"{key}\":");
        let found = text[at..]
            .find(&needle)
            .unwrap_or_else(|| panic!("{needle} after byte {at} in {text}"));
        at += found;
    }
    assert!(text.contains(r#""kind":"manifest""#), "{text}");
    assert!(!text.contains(r#""upserts""#), "a manifest change carries no records");
    // The manifest is the writer's own text, nested — so it is byte-identical to what a
    // client published, rather than a re-spelling that differs in member order.
    assert!(
        text.contains(&super::super::manifest_json(&manifest)),
        "{text}"
    );
}

#[test]
fn an_answer_says_what_was_applied_and_where_the_stream_is_now() {
    assert_eq!(answer_json(118, 0), r#"{"applied":0,"seq":118}"#);
    assert_eq!(answer_json(119, 3), r#"{"applied":3,"seq":119}"#);
}

/// `max_change` is the largest change body the service will accept, and it is
/// `MAX_BODY + 96 × MAX_BATCH` — the room every operation needs for its own wrapper. The
/// arithmetic is pinned so a change to `Limits` cannot silently move the cap.
#[test]
fn the_change_cap_is_the_body_cap_plus_room_for_every_operation() {
    let limits = Limits::DEFAULT;
    assert_eq!(
        max_change(&limits),
        4_194_304 + 96 * 10_000,
        "4 MiB + 96 bytes per operation"
    );
}

/// **The finding this test exists for.** A five-byte `1e300` in a `scalar` cell writes as a
/// 301-digit integer, so a batch body *under* `max_body` can produce a change *over*
/// `max_change`: the cap on the way in and the cap on the way out are not the same
/// number, and no amount of checking the request prevents it. So the change is refused on
/// the way out, by `check_change`, as a size — and this is reported to hub-report rather
/// than fixed here, because the fix (a cap on the expanded form, or a different cell
/// encoding) is a spec decision, not a code fix.
#[test]
fn a_body_under_max_body_can_still_produce_a_change_over_max_change() {
    let limits = Limits::DEFAULT;
    let plugin = "p".repeat(63);
    let ops: Vec<String> = (0..limits.max_batch)
        .map(|i| {
            // Ids must be distinct or the batch is refused as a repeat — so the
            // expansion has to come from the *cell*, not from many cells per record.
            let id = format!("{:050}", i);
            format!(
                r#"{{"collection":"c","id":"{id}","updatedAt":1,"values":{{"s":1e308}}}}"#
            )
        })
        .collect();
    let body = format!(r#"{{"upserts":[{}],"deletes":[]}}"#, ops.join(","));
    assert!(
        body.len() as u64 <= limits.max_body,
        "the body must be inside its own cap for this to be a finding: {}",
        body.len()
    );
    let batch = super::super::batch::read_batch(&body, &limits).expect("the batch reads");
    let records: Vec<(Record, u64)> = batch
        .upserts
        .iter()
        .map(|u| (u.record(&plugin), 9_007_199_254_740_991))
        .collect();
    let change = change_json(
        &ChangeHead {
            seq: 1,
            plugin: &plugin,
            at: "2026-01-02T03:04:05Z",
        },
        &records,
        &[],
    );
    assert!(
        change.len() as u64 > max_change(&limits),
        "the finding needs an over-cap change: {} vs {}",
        change.len(),
        max_change(&limits)
    );
    assert_eq!(
        check_change(&change, &limits),
        Err(HubError::TooLarge {
            what: "change",
            limit: max_change(&limits)
        }),
        "the over-cap change is refused as a size, on the way out"
    );
}

/// And the other side of the same check: a change inside the cap is accepted, so
/// `check_change` is a cap and not a blanket refusal.
#[test]
fn a_change_inside_the_cap_is_accepted() {
    let limits = Limits::DEFAULT;
    let change = change_json(&head(), &[(record("r1"), 4)], &[]);
    assert_eq!(check_change(&change, &limits), Ok(()));
}