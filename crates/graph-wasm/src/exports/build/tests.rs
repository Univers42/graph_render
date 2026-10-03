//! The two parameter-carrying exports, driven natively.
//!
//! **What a native host cannot do, and why this file still means something.** Every
//! address on the 64-bit test host is past `u32`, so `gm_alloc` refuses to return one and
//! `is_live` can never admit a buffer: the two paths that need a real pointer cannot run
//! here, and the ones that do run are the ones that decide *whether* to reach them. That
//! is the refusal order — handle, layout, buffer length, liveness, range, run — and this
//! file walks it.
//!
//! `gm_layout_params`'s framed body is read back through `crate::wire::last_frame`
//! rather than through the returned address, for the same reason: the body is what a
//! caller decodes, and the address it would come back at is not part of it.

use super::params::gm_layout_params;
use super::*;
use crate::errors::Code;
use crate::wire::last_frame;
use graph_core::REFERENCE_DEGREE;
use graph_core::{EdgeRecord, NodeRecord, gate_node_count, index_model, seeded_model};

#[test]
fn the_schema_export_publishes_exactly_what_graph_core_publishes() {
    for (index, layout) in LAYOUTS.iter().enumerate() {
        gm_layout_params(u32::try_from(index).unwrap());
        assert_eq!(
            last_frame(),
            layout.params().encode(),
            "{}: the export published something other than the registry's own encoding",
            layout.id
        );
    }
}

#[test]
fn a_layout_that_publishes_nothing_publishes_a_bare_count() {
    let quiet = LAYOUTS
        .iter()
        .position(|layout| layout.params().is_empty())
        .expect("most layouts publish nothing");
    gm_layout_params(u32::try_from(quiet).unwrap());
    assert_eq!(last_frame(), 0u32.to_le_bytes().to_vec());
    let _ = errors::get();
}

#[test]
fn an_index_past_the_registry_is_refused_by_both_layout_exports() {
    let past = u32::try_from(LAYOUTS.len()).unwrap();
    assert_eq!(gm_layout_params(past), 0);
    assert_eq!(errors::get(), Code::IndexOutOfRange as u32);
    assert_eq!(gm_run(handle(), past, 0, 0), 0);
    assert_eq!(
        errors::get(),
        Code::UnknownLayoutId as u32,
        "the layout is resolved before the buffer, so an unknown layout is named first"
    );
}

#[test]
fn an_empty_buffer_is_the_layouts_own_defaults_and_a_non_empty_one_is_not() {
    let grid = index_of("layout.grid");
    assert_eq!(gm_run(handle(), grid, 0, 0), 1);
    // One byte is not `specs.len() * 8`, so it is refused — and never read, which is why
    // the pointer here is one the host never allocated.
    assert_eq!(gm_run(handle(), grid, 1, 1), 0);
    assert_eq!(errors::get(), Code::ParamsMalformed as u32);
    // A dead pointer at the right length is refused too, not trapped on.
    assert_eq!(gm_run(handle(), grid, 1, 64), 0);
    assert_eq!(errors::get(), Code::ParamsMalformed as u32);
}

#[test]
fn a_layout_that_publishes_nothing_refuses_a_non_empty_buffer() {
    let quiet = index_of("layout.tree.tidy");
    assert!(LAYOUTS[quiet as usize].params().is_empty());
    assert_eq!(gm_run(handle(), quiet, 0, 0), 1);
    assert_eq!(gm_run(handle(), quiet, 1, 8), 0);
    assert_eq!(errors::get(), Code::ParamsNotAccepted as u32);
}

#[test]
fn a_dead_handle_is_refused_before_anything_else_is_read() {
    assert_eq!(gm_run(u32::MAX, index_of("layout.grid"), 0, 0), 0);
    assert_eq!(errors::get(), Code::InvalidHandle as u32);
}

#[test]
fn a_refused_run_leaves_no_geometry_behind() {
    let handle = handle();
    assert_eq!(gm_run(handle, index_of("layout.grid"), 0, 0), 1);
    assert_ne!(
        gm_geometry_kind(handle),
        u32::MAX,
        "a successful run has geometry to serve"
    );
    assert_eq!(gm_run(handle, index_of("layout.grid"), 1, 8), 0);
    assert_eq!(
        gm_geometry_kind(handle),
        u32::MAX,
        "a refused run cleared the geometry and stored nothing, so no stale snapshot is \
         served (C4)"
    );
    assert_eq!(errors::get(), Code::NoGeometryYet as u32);
}

/// A fresh handle over the hash gate's own seeded model, through the module's own
/// `insert` — `gm_build`'s parameter is a framed pointer, which a 64-bit host cannot
/// produce, so the handle is made the one step below the export.
fn handle() -> u32 {
    let (nodes, edges) = model();
    insert(index_model(&nodes, &edges).expect("indexes"))
}

fn index_of(id: &str) -> u32 {
    let index = LAYOUTS
        .iter()
        .position(|l| l.id == id)
        .unwrap_or_else(|| panic!("{id}"));
    u32::try_from(index).unwrap()
}

fn model() -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    seeded_model(6, gate_node_count(6), REFERENCE_DEGREE)
}
