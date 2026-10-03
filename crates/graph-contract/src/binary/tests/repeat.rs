//! What `first_repeat` answers, on tables far larger than a unit test's usual two ids:
//! the *index* of the repeat, not the string, and the smallest such index when a table
//! repeats more than once. Pinned before any of it was made linear, so an O(n) membership
//! test cannot quietly answer something else.

use super::*;

/// `ids` as the `&[&str]` [`table`] takes, borrowed for the call.
fn borrowed(ids: &[String]) -> Vec<&str> {
    ids.iter().map(String::as_str).collect()
}

/// A repeat is refused at the position of the repeat, not of the string it repeats: ten
/// thousand distinct ids followed by a second copy of id 4 321 reports 10 000.
#[test]
fn a_repeated_id_is_refused_at_the_index_of_the_repeat() {
    let mut ids: Vec<String> = (0..10_000).map(|i| format!("n{i:05}")).collect();
    ids.push(ids[4_321].clone());
    let node_ids = table("node.id", &borrowed(&ids));
    assert_eq!(
        node_ids.first_repeat(),
        Some(10_000),
        "the repeat's own index, not 4 321"
    );
    let mut p = parts(point(), EdgeGeometry::Line);
    p.node_ids = node_ids;
    assert_eq!(
        Snapshot::new(p),
        Err(SnapshotError::DuplicateId {
            column: "node.id",
            index: 10_000,
        })
    );
}

/// Of two repeats, the earlier one is the answer: this table repeats at 7 and again at
/// 9 000, and it must report 7 — which a first-repeat-in-sorted-order scan, or one that
/// only ever compared neighbours, would get wrong.
#[test]
fn of_two_repeats_the_earliest_position_is_the_answer() {
    let mut ids: Vec<String> = (0..10_000).map(|i| format!("n{i:05}")).collect();
    ids[9_000] = ids[2].clone();
    ids[7] = ids[3].clone();
    let node_ids = table("node.id", &borrowed(&ids));
    assert_eq!(
        node_ids.first_repeat(),
        Some(7),
        "7 repeats 3, 9 000 repeats 2, and 7 comes first in the table's own order"
    );
    let mut p = parts(point(), EdgeGeometry::Line);
    p.node_ids = node_ids;
    assert_eq!(
        Snapshot::new(p),
        Err(SnapshotError::DuplicateId {
            column: "node.id",
            index: 7,
        })
    );
}
