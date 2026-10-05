//! §5.2's thirteen rows, one case each, plus the relayed body that keeps graph-server's own
//! `error` string.

use super::*;

/// The 400 row: the caller's own `layout`/`post` was refused, so the status and the `error` string
/// are graph-server's.
#[tokio::test]
async fn motor_400_is_relayed_as_400() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            400,
            "BadRequest",
            "`layout=<id>` is required",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 400, "{}", reply.body());
    assert_eq!(reply.error(), "BadRequest");
    assert_eq!(reply.message(), "`layout=<id>` is required");
}

/// The 401 row: the hub's **own** motor key was refused, which is a hub defect, so it is a 502 with
/// §5.2's own name and a line in the log.
#[tokio::test]
async fn motor_401_is_502_motor_auth_and_logged() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(401, "Unauthorized", "no such key")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorAuth");
    assert!(
        logged(&hub, "MotorAuth"),
        "the refusal is in the log: {:?}",
        hub.lines()
    );
}

/// The 406 row: the caller's own `Accept` was refused, so the answer relays.
#[tokio::test]
async fn motor_406_is_relayed_as_406() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(406, "NotAcceptable", "no such face")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 406, "{}", reply.body());
    assert_eq!(reply.error(), "NotAcceptable");
    assert_eq!(reply.message(), "no such face");
}

/// The 408 row: the upload missed graph-server's own body timeout, so it is a 502 and carries **no**
/// `Retry-After`, because the same request would miss it again.
#[tokio::test]
async fn motor_408_is_502_motor_body_timeout_with_no_retry_after() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(408, "Timeout", "the body did not arrive")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorBodyTimeout");
    assert_eq!(reply.header("retry-after"), "", "no Retry-After on a 408");
}

/// The 413 row: the motor's own message is kept, so the caller learns which cap it passed.
#[tokio::test]
async fn motor_413_is_413_graph_too_large_with_the_motors_message() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            413,
            "IngestTooLarge",
            "the document is past the motor's limits",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 413, "{}", reply.body());
    assert_eq!(reply.error(), "GraphTooLarge");
    assert_eq!(
        reply.message(),
        "the document is past the motor's limits",
        "the motor's own message is kept"
    );
}

/// The first half of N3's 422 split: the document did not read at the motor, which `hub-materialize`
/// proves cannot happen, so it is a hub defect and a 502.
#[tokio::test]
async fn motor_422_ingest_invalid_is_502_materialize_invalid() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            422,
            "IngestInvalid",
            "the document is not valid",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MaterializeInvalid");
}

/// The second half of the same split, and the other 502 of N3.
#[tokio::test]
async fn motor_422_contract_invalid_is_502_materialize_invalid() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            422,
            "ContractInvalid",
            "the document is not a contract",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MaterializeInvalid");
}

/// The relayed half of N3's split: the caller's layout failed, which is a 422 relayed whole.
///
/// This and `motor_422_post_failed_is_relayed_as_422` are the two cases row
/// `negctl-layoutfailed-as-502` turns red: its break maps this arm to a 502 instead.
#[tokio::test]
async fn motor_422_layout_failed_is_relayed_as_422() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            422,
            "LayoutFailed",
            "the layout refused this graph",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "LayoutFailed");
    assert_eq!(reply.message(), "the layout refused this graph");
}

/// The other relayed half of the 422 split: a POST pass refused, which is the caller's geometry.
#[tokio::test]
async fn motor_422_post_failed_is_relayed_as_422() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            422,
            "PostFailed",
            "the POST pass refused this geometry",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "PostFailed");
    assert_eq!(reply.message(), "the POST pass refused this geometry");
}

/// The 429 row: the motor's queue was full, so the caller waits in the hub's queue and gets a 503
/// **with** a `Retry-After`, the opposite of the 503 row below.
#[tokio::test]
async fn motor_429_is_503_with_retry_after() {
    let (_motor, hub) =
        hub_with_script(vec![StubReply::new(429, "Busy", "the queue is full")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.header("retry-after"), "1");
}

/// The 503 row: relayed whole, with graph-server's own `error` string and **no** `Retry-After`.
///
/// Caveat: §5.3 says the hub cannot tell an admission wait from an overrun here, so it relays the 503
/// with no `Retry-After` rather than guessing; a distinct overrun code is graph-render-4f's call.
#[tokio::test]
async fn motor_503_is_relayed_as_503_with_no_retry_after() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            503,
            "Timeout",
            "the layout ran past its deadline",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.error(), "Timeout", "the motor's own error string");
    assert_eq!(reply.message(), "the layout ran past its deadline");
    assert_eq!(
        reply.header("retry-after"),
        "",
        "a relayed 503 must not invite a retry"
    );
}

/// The 500 row: a hub defect, so a 502 with §5.2's name and a line in the log.
#[tokio::test]
async fn motor_500_is_502_motor_error() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(500, "Internal", "the motor failed")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorError");
}

/// §5.2's default row: a status the table has no row for is a 502 `MotorError`, never a relayed
/// answer, and it is logged because it is a hub defect.
#[tokio::test]
async fn motor_404_is_502_motor_error_and_logged() {
    let (_motor, hub) =
        hub_with_script(vec![StubReply::new(404, "NotFound", "no such route")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorError");
    assert!(
        logged(&hub, "MotorError"),
        "the default row is logged: {:?}",
        hub.lines()
    );
}

/// §5.2's relayed rows keep graph-server's `error` string byte for byte, so a client parses the same
/// name whichever service answered it.
///
/// The body is compared as text and not field by field, because the claim is about the bytes: a hub
/// that re-derived the class would produce the same `error` here and a different one on a message it
/// chose to rewrite.
#[tokio::test]
async fn a_relayed_body_keeps_the_motors_error_string() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            400,
            "IndexOutOfRange",
            "no POST pass has this id",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(
        reply.body(),
        r#"{"error":"IndexOutOfRange","message":"no POST pass has this id"}"#,
        "the relayed body is graph-server's own"
    );
}

/// Is a `motor-fault` line naming `fault` in the log?
///
/// Caveat: it matches on the `event` and the `fault` member and not on the whole line, because the
/// line also carries the workspace and the status, which §5.2 says nothing about here.
fn logged(hub: &Hub, fault: &str) -> bool {
    hub.lines()
        .iter()
        .any(|line| line["event"] == "motor-fault" && line["fault"] == fault)
}
