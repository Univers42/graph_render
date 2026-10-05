//! The grants file as a model and as a file: which line allows what, and what `Grants::load`
//! refuses to read at all.
//!
//! Nothing here goes through a route, because these rows are decided before a route is reached: a
//! line that half-parses is the line nobody enforces.

use graph_hub::auth::Need;

use crate::support::hub_with_env;

/// `Need::Write` carries the plugin, and a `write:<plugin>` grant covers that plugin's writes and
/// every read: §5.2's table lists it as an alternative grant for a records page.
#[test]
fn a_write_grant_also_covers_the_workspace_reads() {
    let grants = graph_hub::grants::Grants::parse("tester * write:b\n").expect("a grants file");
    assert!(grants.allows("tester", "ops", &Need::Read));
    assert!(grants.allows("tester", "ops", &Need::Write(String::from("b"))));
    assert!(!grants.allows("tester", "ops", &Need::Write(String::from("a"))));
    assert!(!grants.allows("tester", "ops", &Need::Admin));
}

/// `admin` covers everything on its workspace, and a key with no grant line is denied.
#[test]
fn admin_covers_everything_and_no_grant_denies() {
    let grants = graph_hub::grants::Grants::parse("tester ops admin\n").expect("a grants file");
    for need in [Need::Read, Need::Write(String::from("a")), Need::Admin] {
        assert!(grants.allows("tester", "ops", &need), "{need:?}");
    }
    assert!(!grants.allows("tester", "other", &Need::Read));
    assert!(!grants.allows("nobody", "ops", &Need::Read));
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
        assert!(
            refused.is_err(),
            "{text:?} must be refused, got {refused:?}"
        );
    }
}

/// Blank lines and `#` comments are skipped, and several lines for one key are all of them.
#[test]
fn comments_and_blank_lines_are_skipped() {
    let grants =
        graph_hub::grants::Grants::parse("# a comment\n\ntester ops read\ntester * admin\n")
            .expect("a grants file with comments");
    assert_eq!(grants.of("tester").len(), 2);
    assert!(grants.allows("tester", "other", &Need::Admin));
}

/// The grants file gets the keys file's permission check: 0640 or stricter, nothing for group or
/// others (§5.2).
#[tokio::test]
async fn a_group_writable_grants_file_is_refused() {
    let hub = hub_with_env(&[]);
    use std::os::unix::fs::PermissionsExt;
    for mode in [0o644u32, 0o620, 0o602, 0o666] {
        std::fs::set_permissions(&hub.grants_file, PermissionsExt::from_mode(mode))
            .expect("chmod the grants file");
        let refused = graph_hub::grants::Grants::load(&hub.grants_file);
        assert!(
            refused.is_err(),
            "mode {mode:o} must be refused: {refused:?}"
        );
    }
    let readable = PermissionsExt::from_mode(0o640);
    std::fs::set_permissions(&hub.grants_file, readable).expect("chmod the grants file");
    assert!(graph_hub::grants::Grants::load(&hub.grants_file).is_ok());
}
