//! The refusals that are about the path itself rather than about a grant: a bad id, and a cursor
//! that is not a cursor.
//!
//! They share a file because they share the ordering the router promises: the request is refused
//! before any handler runs, so the class of the refusal never depends on whether the ids name
//! anything.

use crate::support::hub_with_env;

/// A percent-decoded `%00` in a path id is 400, before `check_*_id` runs (Decision 8):
/// `check_record_id` accepts `\0`, and the other ids refuse it only incidentally.
#[tokio::test]
async fn a_percent_encoded_nul_in_a_path_id_is_400() {
    let hub = hub_with_env(&[]);
    for path in [
        "/v1/workspaces/ops%00x/graph",
        "/v1/workspaces/ops/plugins/tracker%00x",
        "/v1/workspaces/ops/records/tracker/coll%00x/1",
        "/v1/workspaces/ops/records/tracker/coll/id%00x",
    ] {
        let reply = hub.get_with(path).await;
        assert_eq!(reply.code(), 400, "{path}: {}", reply.body());
    }
}

/// A malformed path id is 422 with the contract's own class name, because `HubError::status()`
/// already decided that (`crates/graph-contract/src/hub/error.rs:66-73`).
#[tokio::test]
async fn a_malformed_path_id_is_422_with_a_contract_class() {
    let hub = hub_with_env(&[]);
    let reply = hub.get_with("/v1/workspaces/OPS/graph").await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "invalid");
}

/// A bare seq as a cursor is 400, never 422: `HubError::Cursor` is the one variant whose status is
/// not 422, and the hub must not re-derive the class.
#[test]
fn a_bare_seq_as_a_cursor_is_400() {
    let refusal = graph_contract::hub::Cursor::parse("42").unwrap_err();
    assert_eq!(refusal.status(), 400, "a bare seq is not `<epoch>.<seq>`");
}
