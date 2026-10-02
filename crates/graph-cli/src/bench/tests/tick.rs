//! `graph-cli tick`: one row per run, with every column the header names.

use crate::bench::tick::{HEADER, Plan, measure};

#[test]
fn a_tick_row_has_one_cell_per_header_column() {
    let plan = Plan {
        n: 300,
        ticks: 3,
        warm: 1,
        seed: 0,
        grow: None,
    };
    let row = measure(&plan).expect("a 300-node model builds");
    let columns = |line: &str| line.matches('|').count();
    let header = HEADER.lines().next().expect("a header line");
    assert_eq!(columns(&row), columns(header), "{row}");
    assert!(row.starts_with("| 300 | "), "{row}");
}
