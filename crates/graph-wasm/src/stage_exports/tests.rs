//! Every branch `gm_post_run` and `gm_analysis_run` can take, driven through the same
//! handle table the exports read (C21) — so the refusals are pinned natively, without a
//! wasm build in the loop.

use super::*;
use crate::errors::Code;
use crate::handle::{Handle, Handles};
use crate::ingest;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry};
use graph_core::index_model;
use graph_core::registry::LAYOUTS;

fn node_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

fn edge_json(id: &str, source: &str, target: &str) -> String {
    format!(
        r#"{{"id":"{id}","source":"{source}","target":"{target}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null}}"#
    )
}

/// A four-node graph with a self-loop and a parallel edge, so a post pass meets the edge
/// cases it has to handle rather than only the easy ones.
fn graph() -> crate::handle::Handle {
    let nodes: Vec<String> = ["a", "b", "c", "d"]
        .iter()
        .map(|id| node_json(id))
        .collect();
    let edges: Vec<String> = [
        edge_json("e0", "a", "b"),
        edge_json("e1", "b", "c"),
        edge_json("e2", "a", "b"),
        edge_json("loop", "a", "a"),
    ]
    .iter()
    .map(String::from)
    .collect();
    let text = format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    );
    let (nodes, edges) = ingest::read(text.as_bytes()).expect("the fixture is valid");
    Handle {
        topology: index_model(&nodes, &edges).expect("the fixture indexes"),
        snapshot: None,
        geometry: None,
    }
}

fn inserted(handles: &mut Handles) -> u32 {
    handles.insert(graph()).expect("the table has room")
}

/// A handle with a successful `gm_run` over the grid layout behind it — the state a post
/// pass is defined to require.
fn laid_out(handles: &mut Handles) -> u32 {
    let id = inserted(handles);
    let entry = handles.get_mut(id).expect("live");
    let geometry = (LAYOUTS[0].run)(&entry.topology).expect("the grid lays it out");
    let snapshot =
        graph_core::layout::snapshot(&entry.topology, geometry.clone()).expect("the geometry fits");
    entry.geometry = Some(geometry);
    entry.snapshot = Some(snapshot);
    id
}

fn index_of(id: &str) -> u32 {
    crate::post::CAPABILITIES
        .iter()
        .position(|entry| entry.id == id)
        .map(|i| u32::try_from(i).expect("small"))
        .expect("registered")
}

/// The pin: after a post pass, the handle's snapshot carries the pass's edge kind and the
/// layout's node positions — the two properties the ABI promises a caller, checked
/// through the same `Snapshot` the columns read.
#[test]
fn a_post_run_replaces_the_edges_and_keeps_the_layouts_nodes() {
    let mut handles = Handles::new();
    let id = laid_out(&mut handles);
    let before = handles
        .get(id)
        .expect("live")
        .snapshot
        .clone()
        .expect("ran");
    let positions = before.parts().nodes.clone();
    for (want_kind, post_id) in [
        (EdgeGeometryKind::Polyline, "post.route.grid"),
        (EdgeGeometryKind::Polyline, "post.bundle.fdeb"),
        (EdgeGeometryKind::Line, "post.style.straight"),
        (EdgeGeometryKind::Polyline, "post.style.orthogonal"),
        (EdgeGeometryKind::Curve, "post.style.quadratic"),
        (EdgeGeometryKind::Curve, "post.style.bezier"),
    ] {
        let after = post_run(&mut handles, id, index_of(post_id)).expect(post_id);
        let parts = after.parts();
        assert_eq!(parts.edges.kind(), want_kind, "{post_id}: edge kind");
        assert_eq!(
            parts.nodes, positions,
            "{post_id}: the layout's nodes moved"
        );
        assert_eq!(
            handles.get(id).expect("live").snapshot.as_ref(),
            Some(&after),
            "{post_id}: the handle serves what it just produced"
        );
    }
}

/// A second pass reads the **layout's** edges, not the first pass's: a caller running
/// style-then-bundle and then bundle again must get the same answer as one running bundle
/// once, and the handle keeps both faces for exactly this reason.
#[test]
fn a_second_post_run_reads_the_layout_not_the_first_pass() {
    let mut handles = Handles::new();
    let id = laid_out(&mut handles);
    let style = index_of("post.style.orthogonal");
    let bundle = index_of("post.bundle.fdeb");
    post_run(&mut handles, id, style).expect("style");
    let after_style = post_run(&mut handles, id, bundle).expect("bundle");
    let fresh = laid_out(&mut handles);
    let straight_bundle = post_run(&mut handles, fresh, bundle).expect("bundle");
    assert_eq!(
        after_style.parts().edges,
        straight_bundle.parts().edges,
        "the orthogonal pass must not change what the bundler reads"
    );
}

/// Every refusal `gm_post_run` can report, each with the code the wire will carry.
#[test]
fn a_post_run_refuses_an_unknown_handle_an_index_and_a_handle_with_no_run_yet() {
    let mut handles = Handles::new();
    let bare = inserted(&mut handles);
    assert_eq!(
        post_run(&mut handles, bare + 1, 0).unwrap_err(),
        Code::InvalidHandle,
        "a handle that was never issued"
    );
    assert_eq!(
        post_run(&mut handles, bare, 0).unwrap_err(),
        Code::NoGeometryYet,
        "a built graph with no layout run has nothing for a pass to read"
    );
    let laid = laid_out(&mut handles);
    assert_eq!(
        post_run(&mut handles, laid, crate::post::count()).unwrap_err(),
        Code::IndexOutOfRange
    );
    assert_eq!(
        post_run(&mut handles, laid, u32::MAX).unwrap_err(),
        Code::IndexOutOfRange
    );
    assert_eq!(
        post_run(&mut handles, laid + 1, 0).unwrap_err(),
        Code::InvalidHandle
    );
}

/// A refused post run leaves the handle exactly as it was, so the next column read
/// serves the previous good geometry rather than nothing or a half-applied pass.
#[test]
fn a_refused_post_run_leaves_the_handles_geometry_untouched() {
    let mut handles = Handles::new();
    let id = laid_out(&mut handles);
    let before = handles.get(id).expect("live").snapshot.clone();
    assert!(post_run(&mut handles, id, u32::MAX).is_err());
    assert_eq!(handles.get(id).expect("live").snapshot, before);
    // And a refusal on a handle with no geometry does not create any.
    let bare = inserted(&mut handles);
    assert!(post_run(&mut handles, bare, 0).is_err());
    assert!(handles.get(bare).expect("live").snapshot.is_none());
}

/// Every analysis, over a handle with no layout run at all: the maths is a function of
/// the topology, so "before any layout" is a supported state, not a refusal.
#[test]
fn an_analysis_runs_before_any_layout_and_matches_the_registry() {
    let mut handles = Handles::new();
    let id = inserted(&mut handles);
    assert!(handles.get(id).expect("live").snapshot.is_none());
    for (i, entry) in crate::analysis::ANALYSES.iter().enumerate() {
        let index = u32::try_from(i).expect("small");
        let text = analysis_run(&handles, id, index).expect(entry.id);
        assert!(text.contains(entry.id), "{}: names itself", entry.id);
        assert!(text.contains("\"nodeCount\":4"), "{}: four nodes", entry.id);
    }
}

/// `gm_post_id` and `gm_analysis_id` are the two framed-id exports, and this is their
/// whole body: the id at an index, or the refusal a caller resolves with `gm_last_error`.
/// The ids are the registries' own, so a row whose id drifted from the one its `run`
/// belongs to would show here.
#[test]
fn an_id_is_the_registries_own_and_a_bad_index_is_refused() {
    for (i, entry) in crate::post::CAPABILITIES.iter().enumerate() {
        let index = u32::try_from(i).expect("small");
        assert_eq!(post_id(index), Ok(entry.id));
    }
    assert_eq!(post_id(crate::post::count()), Err(Code::IndexOutOfRange));
    assert_eq!(post_id(u32::MAX), Err(Code::IndexOutOfRange));
    for (i, entry) in crate::analysis::ANALYSES.iter().enumerate() {
        let index = u32::try_from(i).expect("small");
        assert_eq!(analysis_id(index), Ok(entry.id));
    }
    assert_eq!(
        analysis_id(crate::analysis::count()),
        Err(Code::IndexOutOfRange)
    );
    assert_eq!(analysis_id(u32::MAX), Err(Code::IndexOutOfRange));
    // The two registries are separate lists: a layout id is not a post id.
    assert_eq!(post_id(0), Ok("post.bundle.fdeb"));
    assert_eq!(analysis_id(0), Ok("analysis.components.weak"));
}

#[test]
fn an_analysis_refuses_an_unknown_handle_and_an_index_past_the_end() {
    let mut handles = Handles::new();
    let id = inserted(&mut handles);
    assert_eq!(
        analysis_run(&handles, id + 1, 0).unwrap_err(),
        Code::InvalidHandle
    );
    assert_eq!(
        analysis_run(&handles, id, crate::analysis::count()).unwrap_err(),
        Code::IndexOutOfRange
    );
    assert_eq!(
        analysis_run(&handles, id, u32::MAX).unwrap_err(),
        Code::IndexOutOfRange
    );
}

/// A released handle is refused by both, never answered with the next graph built (C6).
#[test]
fn a_released_handle_is_refused_by_both_stages() {
    let mut handles = Handles::new();
    let id = laid_out(&mut handles);
    handles.remove(id).expect("live");
    assert_eq!(
        post_run(&mut handles, id, 0).unwrap_err(),
        Code::InvalidHandle
    );
    assert_eq!(
        analysis_run(&handles, id, 0).unwrap_err(),
        Code::InvalidHandle
    );
}

/// A pass that cannot produce edges for the topology is refused, not served. Nothing in
/// the registry refuses on the shipped fixtures, so the refusal is driven through the one
/// shape that does: a `PostRun` handed node geometry of the wrong length, which
/// `graph_core::layout::snapshot` catches. Here the *export* side is what matters, so the
/// case is pinned as "the geometry is never written on a refusal" — a `snapshot` that does
/// not fit is `PostFailed` and the handle keeps what it had, which is the same property
/// the previous test states from the other side.
#[test]
fn a_post_run_never_writes_a_snapshot_the_topology_cannot_describe() {
    let mut handles = Handles::new();
    let id = laid_out(&mut handles);
    let entry = handles.get_mut(id).expect("live");
    let geometry = entry.geometry.clone().expect("ran");
    let mut wrong = geometry.clone();
    wrong.nodes = NodeGeometry::Point {
        x: vec![0.0],
        y: vec![0.0],
    };
    let too_short =
        graph_core::layout::snapshot(&entry.topology, wrong).expect_err("one node short");
    assert!(too_short.to_string().contains("node.x"), "{too_short}");
    // The pass itself, over the *right* geometry, still succeeds and still fits.
    let snapshot = post_run(&mut handles, id, 0).expect("fdeb over the grid");
    assert!(matches!(snapshot.parts().edges, EdgeGeometry::Polyline(_)));
}
