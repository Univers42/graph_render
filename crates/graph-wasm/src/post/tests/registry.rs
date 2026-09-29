use super::*;

#[test]
fn the_registry_is_the_two_bundlers_then_routing_then_the_four_styles() {
    assert_eq!(count(), 7);
    for (i, id) in IDS.iter().enumerate() {
        assert_eq!(id_at(u32::try_from(i).expect("small")), Some(*id));
    }
    assert_eq!(id_at(7), None, "one past the end is refused, not a panic");
    assert_eq!(id_at(u32::MAX), None);
}

#[test]
fn running_past_the_end_is_refused_rather_than_answered_with_another_row() {
    let (t, g) = pair();
    assert!(run(7, &t, &g).is_none(), "index 7 is one past the last row");
    assert!(run(u32::MAX, &t, &g).is_none());
    assert!(
        run(0, &t, &g).expect("row 0 exists").is_ok(),
        "row 0 itself runs"
    );
}

/// The style ids come from `Style::ALL` in graph-core, in that order: a fourth style
/// added there must appear here, not be silently unroutable.
#[test]
fn the_four_style_rows_are_the_four_style_variants_at_their_own_defaults() {
    let styles: Vec<Style> = Style::ALL.into_iter().collect();
    assert_eq!(styles.len(), 4);
    for (row, style) in styles.iter().enumerate() {
        let i = index_of(style.id());
        assert_eq!(i, 3 + row, "row {row} is {}", style.id());
    }
}
