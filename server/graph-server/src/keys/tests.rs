use super::{KeySet, KeyStore, base64url, keygen};
use std::os::unix::fs::PermissionsExt;

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn base64url_matches_rfc4648() {
    let vectors = [
        ("", ""),
        ("f", "Zg"),
        ("fo", "Zm8"),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg"),
    ];
    for (plain, coded) in vectors
        .into_iter()
        .chain([("fooba", "Zm9vYmE"), ("foobar", "Zm9vYmFy")])
    {
        assert_eq!(base64url(plain.as_bytes()), coded, "{plain:?}");
    }
    assert_eq!(
        base64url(&[0xfb, 0xff]),
        "-_8",
        "the URL alphabet, not + and /"
    );
}

#[test]
fn a_minted_key_matches_its_own_line_and_nothing_else() {
    let minted = keygen("ci-runner").expect("keygen");
    assert!(
        minted.key.starts_with("gm_") && minted.key.len() == 46,
        "{} chars",
        minted.key.len()
    );
    let set = KeySet::parse(&minted.line).expect("its own line parses");
    assert_eq!(set.name_of(&minted.key), Some("ci-runner"));
    assert_eq!(set.name_of(&minted.key[..45]), None, "a truncated key");
    assert_eq!(set.name_of(""), None);
    assert!(
        !minted.line.contains(&minted.key),
        "the line holds the hash, not the key"
    );
    assert!(keygen("bad name").is_err());
}

#[test]
fn comments_blank_lines_and_crlf_are_accepted() {
    let text = format!(
        "# keys\n\nalpha {HASH_A}\r\n  beta\t{}  \n",
        HASH_B.to_uppercase()
    );
    assert_eq!(KeySet::parse(&text).expect("parses").len(), 2);
}

#[test]
fn a_refused_line_names_its_number_and_not_its_text() {
    let cases = [
        (format!("ok {HASH_A}\nnot-a-line"), 2, "expected"),
        (format!("dup {HASH_A}\ndup {HASH_B}"), 2, "duplicate name"),
        (format!("a {HASH_A}\nb {HASH_A}"), 2, "duplicate hash"),
        (format!("a\u{1}b {HASH_A}"), 1, "control character"),
        (format!("a {}", &HASH_A[..63]), 1, "64 hex"),
        (format!("a {}g", &HASH_A[..63]), 1, "64 hex"),
        (format!("a {HASH_A} extra"), 1, "expected"),
        ("# only a comment\n".to_owned(), 0, "no key"),
    ];
    for (text, line, reason) in cases {
        let err = KeySet::parse(&text).unwrap_err();
        assert_eq!(err.line, line, "{text:?}");
        assert!(err.reason.contains(reason), "{text:?}: {}", err.reason);
        assert!(
            !err.to_string().contains(HASH_A),
            "the message quotes the line"
        );
    }
}

#[test]
fn the_file_mode_is_checked() {
    let dir = std::env::temp_dir().join(format!("graph-server-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("keys");
    std::fs::write(&path, format!("a {HASH_A}\n")).expect("write");
    for (mode, accepted) in [
        (0o600, true),
        (0o640, true),
        (0o644, true),
        (0o660, false),
        (0o602, false),
    ] {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("chmod");
        assert_eq!(KeySet::load(&path).is_ok(), accepted, "mode {mode:o}");
    }
    assert_eq!(KeySet::load(&dir).unwrap_err().line, 0, "a directory");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn the_store_swaps_the_whole_set() {
    let store = KeyStore::new(KeySet::parse(&format!("old {HASH_A}")).unwrap());
    let before = store.current();
    store.replace(KeySet::parse(&format!("new {HASH_B}")).unwrap());
    assert_eq!(before.len(), 1, "a reader keeps the set it took");
    assert_ne!(*before, *store.current());
}
