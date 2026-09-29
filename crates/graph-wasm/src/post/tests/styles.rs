//! Style row tests.

use super::fixtures::*;
use graph_core::post::styles::Style;

#[test]
fn the_four_style_rows_are_the_four_style_variants_at_their_own_defaults() {
    let styles: Vec<Style> = Style::ALL.into_iter().collect();
    assert_eq!(styles.len(), 4);
    for (row, style) in styles.iter().enumerate() {
        let i = index_of(style.id());
        assert_eq!(i, 3 + row, "row {row} is {}", style.id());
    }
}