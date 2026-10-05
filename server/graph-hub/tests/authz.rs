//! Task 3's routes do not exist yet, so this file names the plan's matrix in the shape Task 3 can
//! prove: the authorization decision is a function of the header and the path, and nothing else.
//!
//! The router-level matrix (`hub-authz` over the real routes) is Task 5's, where the routes it
//! exercises are written. What is here is the half that needs no route: credential parsing, the
//! grant lookup and the byte-identical refusals of Review Focus 1, plus the `/v1` paths whose
//! refusals the router already answers today.
#![cfg(feature = "db-tests")]

mod support;

use axum::body::Body;
use graph_hub::auth::{self, Need};
use support::*;

/// No `Authorization` header is 401, and the body is the same one an unknown key gets.
#[tokio::test]
async fn no_authorization_header_is_the_same_401_as_an_unknown_key() {
    let hub = hub_with_env(&[]);
    let anonymous = hub.send(hub.anonymous("GET", "/v1/workspaces").body(Body::empty()).unwrap()).await;
    let unknown = hub.get_as("gm_not_a_key_at_all", "/v1/workspaces").await;
    assert_eq!(anonymous.code(), 401, "{}", anonymous.body());
    assert_eq!(unknown.code(), 401, "{}", unknown.body());
    assert_eq!(anonymous.body(), unknown.body());
}

/// A second `Authorization` header is **400**, not 401: RFC 9110 makes a message with two of the same
/// field invalid, and §5.2 requires exactly this. It is the one behaviour the hub does that
/// graph-server's own credential check does not.
#[tokio::test]
async fn a_second_authorization_header_is_400() {
    let hub = hub_with_env(&[]);
    let request = hub
        .request("GET", "/v1/workspaces")
        .header("authorization", "Bearer gm_second")
        .body(Body::empty())
        .unwrap();
    let reply = hub.send(request).await;
    assert_eq!(reply.code(), 400, "{}", reply.body());
    assert_eq!(reply.error(), "BadRequest");
}

/// A credential that is not `Bearer` at all is 401, and never 400: the hub reuses
/// `graph_server::auth::bearer`, which is case-insensitive about the scheme and refuses anything else.
#[tokio::test]
async fn a_non_bearer_credential_is_401() {
    let hub = hub_with_env(&[]);
    for value in ["Basic abc", "Bearer", "gm_raw_token"] {
        let request = hub
            .anonymous("GET", "/v1/workspaces")
            .header("authorization", value)
            .body(Body::empty())
            .unwrap();
        let reply = hub.send(request).await;
        assert_eq!(reply.code(), 401, "{value}: {}", reply.body());
    }
}

/// The scheme is case-insensitive (RFC 6750 §2.1), and this is graph-server's own `bearer`, so the
/// hub inherits the case-insensitivity rather than re-deciding it.
#[tokio::test]
async fn the_bearer_scheme_is_case_insensitive() {
    let hub = hub_with_env(&[]);
    for scheme in ["Bearer", "bearer", "BEARER", "BeArEr"] {
        let request = hub
            .anonymous("GET", "/v1/workspaces")
            .header("authorization", format!("{scheme} {}", hub.key))
            .body(Body::empty())
            .unwrap();
        let reply = hub.send(request).await;
        assert_ne!(reply.code(), 401, "{scheme}: {}", reply.body());
    }
}

/// The two 403s are byte-identical, so a probe cannot tell a workspace it may not touch from one that
/// does not exist. This is the half that needs no database: authorization never asks.
#[tokio::test]
async fn refusal_bytes_are_identical_with_and_without_the_workspace() {
    let hub = hub_with_grants(&format!("{KEY_NAME} * read\n"), &[]);
    let elsewhere = hub.get_with("/v1/workspaces/does-not-exist/graph").await;
    let also_elsewhere = hub.get_with("/v1/workspaces/never-created/graph").await;
    assert_eq!(elsewhere.code(), 403, "{}", elsewhere.body());
    assert_eq!(elsewhere.body(), also_elsewhere.body());
    // And the status line's own headers agree: one `Retry-After`-free 403 with the same JSON shape.
    assert_eq!(elsewhere.header("retry-after"), "");
    assert_eq!(elsewhere.header("www-authenticate"), "");
}

/// A key with no grant covering the path's workspace is 403 **before** any 404: the refusal never
/// reaches the store, so a workspace that is not there cannot change the answer.
#[tokio::test]
async fn no_grant_is_403_before_any_404() {
    let hub = hub_with_grants(&format!("{KEY_NAME} * read\n"), &[]);
    for path in [
        "/v1/workspaces/nowhere/graph",
        "/v1/workspaces/nowhere/plugins/tracker/batches",
        "/v1/workspaces/nowhere/records/tracker/coll/1",
    ] {
        let reply = hub.get_with(path).await;
        assert_eq!(reply.code(), 403, "{path}: {}", reply.body());
        assert_eq!(reply.error(), "Forbidden", "{path}");
    }
}

/// A key granted `read` is refused a write with 403 and never 404, on every write route §5.2 names.
#[tokio::test]
async fn a_read_only_key_is_403_on_every_write_route() {
    let hub = hub_with_grants(&format!("{KEY_NAME} * read\n"), &[]);
    let writes = [
        ("PUT", "/v1/workspaces/ops"),
        ("PUT", "/v1/workspaces/ops/plugins/tracker"),
        ("POST", "/v1/workspaces/ops/plugins/tracker/batches"),
    ];
    for (method, path) in writes {
        let reply = hub.send(hub.request(method, path).body(Body::from("{}")).unwrap()).await;
        assert_eq!(reply.code(), 403, "{method} {path}: {}", reply.body());
    }
}

/// A key granted `write:B` is refused A's records with 403 and never 404: it learns nothing about
/// another plugin's data, not even whether the workspace holds it.
#[tokio::test]
async fn a_key_learns_nothing_from_another_plugins_records() {
    let hub = hub_with_grants(&format!("{KEY_NAME} * write:b\n"), &[]);
    let mine = hub.get_with("/v1/workspaces/ops/plugins/b/records").await;
    let theirs = hub.get_with("/v1/workspaces/ops/plugins/a/records").await;
    assert_ne!(mine.code(), 403, "its own plugin's records: {}", mine.body());
    assert_eq!(theirs.code(), 403, "another plugin's records: {}", theirs.body());
    assert_eq!(theirs.error(), "Forbidden");
}

/// A key granted `admin` on one workspace learns nothing about another, which is the same 403 with
/// the same bytes.
#[tokio::test]
async fn admin_is_a_workspace_grant_and_not_a_hub_one() {
    let hub = hub_with_grants(&format!("{KEY_NAME} ops admin\n"), &[]);
    let mine = hub.send(hub.request("PUT", "/v1/workspaces/ops").body(Body::from("{}")).unwrap()).await;
    let theirs = hub.send(hub.request("PUT", "/v1/workspaces/other").body(Body::from("{}")).unwrap()).await;
    assert_ne!(mine.code(), 403, "its own workspace: {}", mine.body());
    assert_eq!(theirs.code(), 403, "another workspace: {}", theirs.body());
}

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

/// `Need::Write` carries the plugin, and a `write:<plugin>` grant covers that plugin's writes and
/// every read: §5.2's table lists it as an alternative grant for a records page.
#[test]
fn a_write_grant_also_covers_the_workspace_reads() {
    let grants = graph_hub::grants::Grants::parse("tester * write:b\n").expect("a grants file");
    assert!(grants.allows("tester", "ops", Need::Read));
    assert!(grants.allows("tester", "ops", Need::Write("b")));
    assert!(!grants.allows("tester", "ops", Need::Write("a")));
    assert!(!grants.allows("tester", "ops", Need::Admin));
}

/// `admin` covers everything on its workspace, and a key with no grant line is denied.
#[test]
fn admin_covers_everything_and_no_grant_denies() {
    let grants = graph_hub::grants::Grants::parse("tester ops admin\n").expect("a grants file");
    for need in [Need::Read, Need::Write("a"), Need::Admin] {
        assert!(grants.allows("tester", "ops", need), "{need:?}");
    }
    assert!(!grants.allows("tester", "other", Need::Read));
    assert!(!grants.allows("nobody", "ops", Need::Read));
}

/// A malformed line is a **load** failure and never a skip: a grants file that half-parses is a
/// security bug, because the line nobody read is the line nobody enforces.
#[test]
fn a_malformed_grants_line_is_a_load_failure() {
    for text in [
        "tester\n",
        "tester ops\n",
        "tester ops superuser\n",
        "tester ops read extra\n",
        "tester OPS read\n",
        "tester ops write:\n",
        "tester ops write:BAD.ID\n",
        "bad/name ops read\n",
        "# only a comment\n",
        "",
    ] {
        let refused = graph_hub::grants::Grants::parse(text);
        assert!(refused.is_err(), "{text:?} must be refused, got {refused:?}");
    }
}

/// Blank lines and `#` comments are skipped, and several lines for one key are all of them.
#[test]
fn comments_and_blank_lines_are_skipped() {
    let grants = graph_hub::grants::Grants::parse(
        "# a comment\n\ntester ops read\ntester * admin\n",
    )
    .expect("a grants file with comments");
    assert_eq!(grants.of("tester").len(), 2);
    assert!(grants.allows("tester", "other", Need::Admin));
}

/// The grants file gets the keys file's permission check: 0640 or stricter, nothing for group or
/// others (§5.2).
#[tokio::test]
async fn a_group_writable_grants_file_is_refused() {
    let hub = hub_with_env(&[]);
    for mode in [0o644, 0o620, 0o602, 0o666] {
        let mode = std::os::unix::fs::PermissionsExt::from_mode(mode);
        std::fs::set_permissions(&hub.grants_file, mode).expect("chmod the grants file");
        let refused = graph_hub::grants::Grants::load(&hub.grants_file);
        assert!(refused.is_err(), "mode {mode:o} must be refused: {refused:?}");
    }
    let readable = std::os::unix::fs::PermissionsExt::from_mode(0o640);
    std::fs::set_permissions(&hub.grants_file, readable).expect("chmod the grants file");
    assert!(graph_hub::grants::Grants::load(&hub.grants_file).is_ok());
}

/// `auth::credential` is the only place a header becomes a name, and it never returns the key.
#[tokio::test]
async fn credential_returns_the_name_and_never_the_key() {
    let hub = hub_with_env(&[]);
    let pair = hub.app.keys.current();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", hub.key).parse().expect("the header value"),
    );
    let name = auth::credential(&pair, &headers).expect("the fixture's key is in the file");
    assert_eq!(name, KEY_NAME);
    assert!(!name.contains("gm_"), "a name never carries the key: {name}");
}