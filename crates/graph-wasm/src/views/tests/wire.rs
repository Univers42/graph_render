//! `column_wire`: what `gm_column_ptr`/`gm_column_len` hand JS, and the contract table
//! (`docs/contract/wasm-abi.md` "Columns") that tells JS how to read it.

use super::{point, snapshot, with_z};
use crate::errors::Code;
use crate::views::{Column, column, column_wire, id};
use graph_contract::binary::Snapshot;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

fn empty_circle() -> Snapshot {
    let nodes = NodeGeometry::Circle {
        x: vec![],
        y: vec![],
        r: vec![],
    };
    snapshot(nodes, EdgeGeometry::Line, &[], &[])
}

/// C3: absent and present-but-empty both read `(0, 0)`. An empty `Vec`'s `as_ptr()` is
/// a dangling non-null address, and a JS view built over it is an out-of-bounds view.
#[test]
fn a_present_but_empty_column_reads_zero_zero_like_an_absent_one() {
    let snap = empty_circle();
    assert!(matches!(column(&snap, id::NODE_R), Column::F32(v) if v.is_empty()));
    assert_eq!(column_wire(&snap, id::NODE_R, true), Ok(0), "len");
    assert_eq!(column_wire(&snap, id::NODE_R, false), Ok(0), "ptr");
    let absent = snapshot(point(vec![], vec![]), EdgeGeometry::Line, &[], &[]);
    assert_eq!(column_wire(&absent, id::NODE_R, false), Ok(0));
}

/// A present column's address never comes back as a `0` that reads like "absent": on
/// this host it does not fit `u32`, and that is a refusal with its code.
#[test]
fn an_address_past_u32_is_refused_not_read_as_absent() {
    let snap = snapshot(point(vec![1.0], vec![2.0]), EdgeGeometry::Line, &["a"], &[]);
    assert_eq!(column_wire(&snap, id::NODE_X, true), Ok(1));
    match column_wire(&snap, id::NODE_X, false) {
        Ok(address) => assert_ne!(address, 0, "a present column at address 0"),
        Err(code) => assert_eq!(code, Code::IndexOutOfRange),
    }
}

/// The doc table's element type for `id`, read from the committed contract.
fn documented_type(id: u32) -> &'static str {
    let doc = include_str!("../../../../../docs/contract/wasm-abi.md");
    let prefix = format!("| {id} | `");
    let row = doc.lines().find(|line| line.starts_with(&prefix));
    let row = row.unwrap_or_else(|| panic!("no Columns row for id {id}"));
    let cell = row.split('|').nth(3).expect("an element-type cell");
    if cell.contains("`f32") { "f32" } else { "u32" }
}

fn every_kind() -> Vec<Snapshot> {
    let paths = Paths {
        offsets: vec![0, 2],
        pts: vec![0.1, 0.2, 0.3, 0.4],
    };
    let circle = NodeGeometry::Circle {
        x: vec![0.0, 1.0],
        y: vec![0.0, 1.0],
        r: vec![1.0, 1.0],
    };
    let boxes = NodeGeometry::Box {
        x: vec![0.0, 1.0],
        y: vec![0.0, 1.0],
        w: vec![1.0, 1.0],
        h: vec![1.0, 1.0],
    };
    let ids = ["a", "b"];
    vec![
        snapshot(circle, EdgeGeometry::Polyline(paths.clone()), &ids, &["e"]),
        with_z(
            boxes,
            Some(vec![0.0, 0.5]),
            EdgeGeometry::Curve { degree: 3, paths },
            &ids,
            &["e"],
        ),
    ]
}

/// Only `(ptr, len)` crosses the wire, so a column's element type is a function of its
/// id alone, fixed by the contract table. This pins that table to the code: every present
/// column, over every node and edge kind, has the element type its row states.
#[test]
fn every_present_column_has_the_element_type_its_contract_row_states() {
    let mut seen = 0;
    for snap in every_kind() {
        for column_id in 0..=id::NODE_Z {
            let actual = match column(&snap, column_id) {
                Column::Absent => continue,
                Column::F32(_) => "f32",
                Column::U32(_) => "u32",
            };
            assert_eq!(actual, documented_type(column_id), "column {column_id}");
            seen += 1;
        }
    }
    assert_eq!(seen, 17, "every applicable column was checked");
}
