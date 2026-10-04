//! Verdict condition 9, at start, on a real process: every refused key file and every refused
//! setting is exit 2, and the message names the line number or the variable, never the line, the
//! path or the value.

mod common;

use common::{Setup, setup, write_private};
use std::os::unix::fs::PermissionsExt;

/// Runs `setup` to its refusal and checks exit 2, `wanted` in stderr and none of `hidden`.
fn refuses(setup: &Setup, wanted: &str, hidden: &[&str]) {
    let (status, stderr) = setup.refused();
    assert_eq!(
        status.code(),
        Some(2),
        "exit status {status}, stderr {stderr:?}"
    );
    assert!(
        stderr.contains(wanted),
        "stderr {stderr:?} lacks {wanted:?}"
    );
    let path = setup.keys_file.display().to_string();
    for secret in hidden.iter().copied().chain([path.as_str()]) {
        assert!(
            !stderr.contains(secret),
            "stderr {stderr:?} holds {secret:?}"
        );
    }
}

/// A setup whose key file is the minted line, then `extra` as line 2.
fn second_line(extra: &str) -> Setup {
    let setup = setup(&[]);
    let first = std::fs::read_to_string(&setup.keys_file).expect("the key file");
    write_private(&setup.keys_file, &format!("{first}{extra}\n"));
    setup
}

fn chmod(setup: &Setup, mode: u32) {
    let mode = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(&setup.keys_file, mode).expect("chmod");
}

#[test]
fn a_missing_key_file_is_exit_2() {
    let setup = setup(&[]);
    std::fs::remove_file(&setup.keys_file).expect("rm");
    refuses(&setup, "key file: cannot be opened", &[]);
}

#[test]
fn a_group_writable_key_file_is_exit_2() {
    let setup = setup(&[]);
    chmod(&setup, 0o620);
    refuses(&setup, "key file: is group- or world-writable", &[]);
}

#[test]
fn a_world_writable_key_file_is_exit_2() {
    let setup = setup(&[]);
    chmod(&setup, 0o602);
    refuses(&setup, "key file: is group- or world-writable", &[]);
}

#[test]
fn a_malformed_line_is_exit_2_naming_its_number_only() {
    let line = "gm_pasted_a_key_by_mistake";
    refuses(&second_line(line), "key file line 2: expected", &[line]);
}

#[test]
fn a_duplicate_name_is_exit_2() {
    let line = format!("tester {}", "ab".repeat(32));
    refuses(
        &second_line(&line),
        "key file line 2: duplicate name",
        &[&line],
    );
}

#[test]
fn a_control_character_in_a_name_is_exit_2() {
    let line = format!("bad\u{7}name {}", "ab".repeat(32));
    refuses(
        &second_line(&line),
        "key file line 2: control character",
        &["bad"],
    );
}

#[test]
fn a_non_hex_hash_is_exit_2() {
    let line = format!("other {}", "zz".repeat(32));
    refuses(
        &second_line(&line),
        "key file line 2: the hash is not 64 hex",
        &["zzzz"],
    );
}

#[test]
fn auth_off_on_a_public_bind_is_exit_2() {
    let setup = setup(&[("GRAPH_AUTH", "off"), ("GRAPH_BIND", "0.0.0.0")]);
    refuses(
        &setup,
        "GRAPH_AUTH: `off` refuses a non-loopback GRAPH_BIND",
        &["0.0.0.0"],
    );
}

#[test]
fn zero_workers_is_exit_2() {
    let setup = setup(&[("GRAPH_WORKERS", "0")]);
    refuses(&setup, "GRAPH_WORKERS: is out of range", &[]);
}

#[test]
fn the_start_line_names_every_variable_and_no_value() {
    let child = setup(&[("GRAPH_MAX_BODY", "1048576")]).spawn();
    let start = &child.seen[0];
    assert_eq!(start["event"], "start");
    let env = start["env"].as_object().expect("the env map");
    assert_eq!(env.len(), graph_server::config::NAMES.len());
    assert_eq!(env["GRAPH_API_KEYS_FILE"], "set");
    assert_eq!(env["GRAPH_MAX_BODY"], "set");
    assert_eq!(env["GRAPH_CORS_ORIGINS"], "unset");
    let text = start.to_string();
    let path = child.keys_file.display().to_string();
    assert!(!text.contains(&path) && !text.contains("1048576"), "{text}");
}
