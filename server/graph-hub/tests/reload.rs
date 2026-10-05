//! `SIGHUP`: both credential files swap together, or neither does.
#![cfg(feature = "db-tests")]

mod support;

use graph_hub::keys::Keyring;
use support::*;

/// A `SIGHUP` with a malformed grants line keeps the old pair: the old key still works and the old
/// grants still apply. The swap is all-or-nothing because a hub with a new key set and old grants
/// answers 403 to everything the new key was given.
#[tokio::test]
async fn a_sighup_with_a_bad_grants_file_keeps_the_old_pair() {
    let hub = hub_with_env(&[]);
    let second = add_key(&hub, "second");
    write_grants(&hub, "tester * admin\nsecond ops write:a\n");
    assert!(
        hub.app.keys.reload().expect("a good pair"),
        "the good pair swaps"
    );
    write_grants(&hub, "tester * admin\nsecond ops superuser\n");
    let refused = hub.app.keys.reload();
    assert!(refused.is_err(), "a malformed grants file is a refusal");
    // The old pair is still in force: the second key still writes, and a key nobody granted does not.
    let allowed = hub
        .get_as(&second, "/v1/workspaces/ops/plugins/a/records")
        .await;
    assert_ne!(
        allowed.code(),
        403,
        "the old grant survives: {}",
        allowed.body()
    );
    let unknown = hub
        .get_as("gm_never_minted", "/v1/workspaces/ops/graph")
        .await;
    assert_eq!(unknown.code(), 401, "{}", unknown.body());
}

/// The same for the keys file: a malformed key line keeps the old pair, and the old key still works.
#[tokio::test]
async fn a_sighup_with_a_bad_keys_file_keeps_the_old_pair() {
    let hub = hub_with_env(&[]);
    let original = std::fs::read_to_string(&hub.keys_file).expect("the fixture's key file");
    write_keys(&hub, "tester 0000\nnot-a-key-line\n");
    let refused = hub.app.keys.reload();
    assert!(refused.is_err(), "a malformed key file is a refusal");
    write_keys(&hub, &original);
    let reply = hub.get_with("/v1/workspaces/ops/graph").await;
    assert_ne!(
        reply.code(),
        401,
        "the old key file is still in force: {}",
        reply.body()
    );
}

/// A good pair swaps both in one `SIGHUP`: the new key is accepted and the old key is refused.
#[tokio::test]
async fn a_good_pair_swaps_both() {
    let hub = hub_with_env(&[]);
    let old_key = hub.key.clone();
    let second = add_key(&hub, "second");
    // The old key's line goes as well as a new one arriving: a rotation is not an addition.
    keep_only(&hub, "second");
    write_grants(&hub, "second ops read\n");
    assert!(
        hub.app.keys.reload().expect("a good pair"),
        "a swap happened"
    );
    let fresh = hub.get_as(&second, "/v1/workspaces/ops/graph").await;
    assert_ne!(fresh.code(), 401, "the new key works: {}", fresh.body());
    let stale = hub.get_as(&old_key, "/v1/workspaces/ops/graph").await;
    assert_eq!(stale.code(), 401, "the old key is gone: {}", stale.body());
    // And the grants swapped too: the new key is refused a workspace the new file does not name.
    let elsewhere = hub.get_as(&second, "/v1/workspaces/other/graph").await;
    assert_eq!(
        elsewhere.code(),
        403,
        "the new grants are in force: {}",
        elsewhere.body()
    );
}

/// An unchanged pair is not a swap: the same `Arc` stays, so every request in flight keeps the pair
/// it started with.
#[tokio::test]
async fn an_unchanged_pair_is_not_a_swap() {
    let hub = hub_with_env(&[]);
    let before = hub.app.keys.current();
    assert!(
        !hub.app
            .keys
            .reload()
            .expect("an unchanged pair is not a refusal"),
        "an identical reload reports no swap"
    );
    let after = hub.app.keys.current();
    assert!(
        std::sync::Arc::ptr_eq(&before, &after),
        "the same Arc is still in force, so nothing re-reads a grant under a request"
    );
}

/// A request that started before the swap keeps the pair it started with: the `Arc` is cloned inside
/// the request, never held by the layer, so a `SIGHUP` cannot change what a request in flight sees.
#[tokio::test]
async fn a_relay_still_holds_a_key_set_across_a_sighup() {
    let hub = hub_with_env(&[]);
    let held = hub.app.keys.current();
    add_key(&hub, "second");
    write_grants(&hub, "second ops admin\n");
    hub.app.keys.reload().expect("a good pair");
    // The held pair is the one the request would have read: the old key set and the old grants.
    assert_eq!(held.0.len(), 1, "the held pair holds one key");
    assert!(
        held.1
            .allows("tester", "anywhere", &graph_hub::grants::Need::Admin)
    );
    assert!(
        !held
            .1
            .allows("second", "ops", &graph_hub::grants::Need::Admin),
        "the held pair cannot see the grant the new file gave the new key"
    );
    // And the pair in force now is the new one.
    let now = hub.app.keys.current();
    assert_eq!(now.0.len(), 2, "the pair in force holds two keys");
    assert!(
        now.1
            .allows("second", "ops", &graph_hub::grants::Need::Admin)
    );
    assert!(
        !now.1
            .allows("tester", "anywhere", &graph_hub::grants::Need::Admin)
    );
}

/// A keyring cannot be built from a keys file without a grants file: Decision 7 refuses the pair,
/// because a hub with keys and no grants answers 403 to everything.
#[test]
fn a_keyring_needs_both_files() {
    let dir = scratch();
    let keys = write_private(&dir.join("keys"), &one_key_line());
    let refused = Keyring::load(&keys, &dir.join("missing"));
    assert!(refused.is_err(), "a missing grants file is a refusal");
    let grants = write_private(&dir.join("grants"), "tester * admin\n");
    assert!(
        Keyring::load(&keys, &grants).is_ok(),
        "both files present is a loaded keyring"
    );
}

/// `SIGHUP` never turns a live hub's keys into an empty pair: `KeySet::load` refuses an empty file,
/// so a truncated write is a refusal and not a hub that 401s everything.
#[tokio::test]
async fn an_empty_keys_file_is_refused_and_keeps_the_old_pair() {
    let hub = hub_with_env(&[]);
    let original = std::fs::read_to_string(&hub.keys_file).expect("the fixture's key file");
    write_keys(&hub, "");
    assert!(
        hub.app.keys.reload().is_err(),
        "an empty key file is a refusal"
    );
    write_keys(&hub, &original);
    let reply = hub.get_with("/v1/workspaces/ops/graph").await;
    assert_ne!(
        reply.code(),
        401,
        "the old pair is still in force: {}",
        reply.body()
    );
}

/// The keys file's own refusals reach the start path as messages that name a line and a reason, and
/// never the line itself.
#[test]
fn a_bad_keys_file_names_a_line_and_not_a_secret() {
    let dir = scratch();
    let keys = write_private(
        &dir.join("keys"),
        &format!("{}pasted_a_key_by_mistake\n", one_key_line()),
    );
    let grants = write_private(&dir.join("grants"), "tester * admin\n");
    let refused = Keyring::load(&keys, &grants).expect_err("a malformed key line");
    assert!(refused.contains("line 2"), "{refused}");
    assert!(!refused.contains("pasted_a_key_by_mistake"), "{refused}");
}

/// The grants file's refusals say what is wrong and never the file's path or a key.
#[test]
fn a_bad_grants_file_says_what_is_wrong() {
    let dir = scratch();
    let keys = write_private(&dir.join("keys"), &one_key_line());
    let grants = write_private(&dir.join("grants"), "tester ops superuser\n");
    let refused = Keyring::load(&keys, &grants).expect_err("a malformed grants line");
    assert!(refused.starts_with("grants file: line 1:"), "{refused}");
    assert!(!refused.contains("superuser"), "{refused}");
    let path = grants.display().to_string();
    assert!(!refused.contains(&path), "{refused}");
}

/// One key file line, minted now: a fixture never writes a hash by hand, because the loader hashes
/// the *key* and a hand-written digest would not match any key a test could present.
fn one_key_line() -> String {
    graph_server::keys::keygen("tester")
        .expect("a key from /dev/urandom")
        .line
        + "\n"
}

/// Keep only `name`'s line in the key file, so a rotation removes the old key as well as adding one.
fn keep_only(hub: &Hub, name: &str) {
    let text = std::fs::read_to_string(&hub.keys_file).expect("the fixture's key file");
    let kept: String = text
        .lines()
        .filter(|line| line.starts_with(&format!("{name} ")))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(kept.lines().count(), 1, "the fixture minted {name}");
    write_keys(hub, &kept);
}
