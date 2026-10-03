//! Registry structure tests.

use super::fixtures::*;

#[test]
fn the_registry_is_the_two_bundlers_then_routing_then_the_four_styles_then_the_node_mover() {
    assert_eq!(crate::post::registry::count(), 8);
    for (i, id) in IDS.iter().enumerate() {
        assert_eq!(
            crate::post::registry::id_at(u32::try_from(i).expect("small")),
            Some(*id)
        );
    }
    // The count is pinned above and the end index is read from the table below, so adding a
    // row cannot leave one of the two stale: the number written down appears exactly once.
    let past_the_end = crate::post::registry::count();
    assert_eq!(
        crate::post::registry::id_at(past_the_end),
        None,
        "one past the end is refused, not a panic"
    );
    assert_eq!(crate::post::registry::id_at(u32::MAX), None);
}

#[test]
fn running_past_the_end_is_refused_rather_than_answered_with_another_row() {
    let (t, g) = pair();
    let past_the_end = crate::post::registry::count();
    assert!(
        crate::post::registry::run(past_the_end, &t, &g).is_none(),
        "index {past_the_end} is one past the last row"
    );
    assert!(crate::post::registry::run(u32::MAX, &t, &g).is_none());
    assert!(
        crate::post::registry::run(0, &t, &g)
            .expect("row 0 exists")
            .is_ok(),
        "row 0 itself runs"
    );
}
