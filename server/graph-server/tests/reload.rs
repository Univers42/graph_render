//! Verdict condition 9, `SIGHUP`, on a real process: the whole file is parsed, then the key set
//! is swapped at once; a bad or empty file keeps the old set and the refusal names no line text.

mod common;

use common::{Child, bearer, exchange, head, setup, write_private};
use graph_server::keys;

/// `GET /v1/meta` with `key`: its status.
fn meta_with(child: &Child, key: &str) -> u16 {
    exchange(
        child.addr,
        head("GET", "/v1/meta", &[bearer(key)]).as_bytes(),
    )
    .0
}

/// Writes `text` as the key file, sends `SIGHUP` and returns the `keys` log line it causes.
fn reload(child: &mut Child, text: &str) -> serde_json::Value {
    write_private(&child.keys_file.clone(), text);
    child.signal("HUP");
    child.wait_line(|line| line["event"] == "keys")
}

#[test]
fn sighup_swaps_the_whole_key_set() {
    let mut child = setup(&[]).spawn();
    let old = child.key.clone();
    assert_eq!(meta_with(&child, &old), 200);
    let first = keys::keygen("first").expect("a key");
    let second = keys::keygen("second").expect("a key");
    let line = reload(&mut child, &format!("{}\n{}\n", first.line, second.line));
    assert_eq!(line["reloaded"], 2, "{line}");
    assert_eq!(meta_with(&child, &old), 401);
    assert_eq!(meta_with(&child, &first.key), 200);
    assert_eq!(meta_with(&child, &second.key), 200);
}

#[test]
fn a_bad_file_on_sighup_keeps_the_old_set() {
    let mut child = setup(&[]).spawn();
    let pasted = "gm_a_key_pasted_by_mistake";
    let fresh = keys::keygen("fresh").expect("a key");
    let line = reload(&mut child, &format!("{}\n{pasted}\n", fresh.line));
    assert_eq!(
        line["refused"],
        "key file line 2: expected `<name> <sha256-hex>`"
    );
    assert_eq!(line["kept"], 1);
    assert!(!line.to_string().contains(pasted), "{line}");
    assert_eq!(meta_with(&child, &child.key.clone()), 200);
    assert_eq!(
        meta_with(&child, &fresh.key),
        401,
        "half the bad file landed"
    );
}

#[test]
fn an_empty_file_on_sighup_keeps_the_old_set() {
    let mut child = setup(&[]).spawn();
    let line = reload(&mut child, "");
    assert_eq!(line["refused"], "key file: holds no key");
    assert_eq!(meta_with(&child, &child.key.clone()), 200);
}
