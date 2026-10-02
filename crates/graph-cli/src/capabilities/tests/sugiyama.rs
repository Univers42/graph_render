//! The layered drawing's ledger row: backed by its own negative control and its own
//! roundtrip function entry, never by another layout's.

use super::super::*;
use super::find_row_by_id;
use super::honest;
use serde_json::json;

#[test]
fn the_sugiyama_row_stands_only_on_its_own_control() {
    let dag = || vec![find_row_by_id("layout.dag.sugiyama")];
    let mut evidence = honest();
    evidence.controls.truncate(2);
    let blind = problems(&dag(), &evidence);
    assert_eq!(blind.len(), 1, "{blind:?}");
    assert!(
        blind[0].contains(
            "hashgate-control-grid-spacing did not go red on the layout.dag.sugiyama stage"
        ),
        "{blind:?}"
    );
    let mut evidence = honest();
    evidence.by_name.get_mut("roundtrip").expect("set")["functions"]["layout.dag.sugiyama"]["cases"] =
        json!(0);
    let empty = problems(&dag(), &evidence);
    assert!(
        empty[0].contains("ran no layout.dag.sugiyama case"),
        "{empty:?}"
    );
}
