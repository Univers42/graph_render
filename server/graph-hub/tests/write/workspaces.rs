//! `PUT /v1/workspaces/{ws}`: the 201 on insert, the 200 when the row was already there, and the
//! absence of any seq, since creating a workspace moves no stream.

use crate::support::fixtures::hub_db;

/// `PUT /v1/workspaces/{ws}` is 201 on the insert and 200 when the row was already there, and the
/// answer carries no seq: creating a workspace moves no stream.
#[tokio::test]
async fn put_workspaces_is_201_then_200_and_takes_no_seq() {
    let hub = hub_db(&[]).await;
    let first = hub.put("/v1/workspaces/created", "").await;
    assert_eq!(first.code(), 201, "{}", first.body());
    assert_eq!(
        first.header("graph-seq"),
        "",
        "no seq on a workspace create"
    );
    let again = hub.put("/v1/workspaces/created", "").await;
    assert_eq!(again.code(), 200, "{}", again.body());
}