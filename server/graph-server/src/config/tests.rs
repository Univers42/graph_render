use super::{BASE_BYTES, ConfigError, PER_SLOT_BYTES, Settings, default_workers, start_line};
use std::collections::BTreeMap;
use std::ffi::OsString;

fn settings(vars: &[(&str, &str)]) -> Result<Settings, ConfigError> {
    let map: BTreeMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(*v)))
        .collect();
    Settings::from_env(&|name| map.get(name).cloned())
}

const KEYS: (&str, &str) = ("GRAPH_API_KEYS_FILE", "/run/graph/keys");

#[test]
fn defaults_hold_with_only_a_key_file() {
    let s = settings(&[KEYS]).expect("defaults");
    assert_eq!((s.port, s.auth), (8080, true));
    assert!(s.bind.is_loopback());
    assert_eq!(s.limits.max_body, 64 << 20);
    assert_eq!(s.limits.queue, 2 * s.limits.workers);
    assert_eq!(s.connections.max_header_bytes, 16_384);
    assert!(s.cors_origins.is_empty() && s.embed_dir.is_none());
}

#[test]
fn auth_on_needs_a_key_file_and_off_needs_loopback() {
    assert_eq!(settings(&[]).unwrap_err().name, "GRAPH_API_KEYS_FILE");
    assert!(settings(&[("GRAPH_AUTH", "off")]).is_ok());
    let open = settings(&[("GRAPH_AUTH", "off"), ("GRAPH_BIND", "0.0.0.0")]);
    assert_eq!(open.unwrap_err().name, "GRAPH_AUTH");
    assert_eq!(
        settings(&[KEYS, ("GRAPH_AUTH", "maybe")]).unwrap_err().name,
        "GRAPH_AUTH"
    );
}

#[test]
fn a_malformed_value_names_the_variable_and_not_the_value() {
    for (name, value) in [
        ("GRAPH_PORT", "70000"),
        ("GRAPH_WORKERS", "0"),
        ("GRAPH_MAX_HEADER_BYTES", "100"),
        ("GRAPH_TIMEOUT_MS", "soon"),
        ("GRAPH_BIND", "localhost"),
        ("GRAPH_CORS_ORIGINS", "*"),
        ("GRAPH_CORS_ORIGINS", "https://a.example/path"),
    ] {
        let err = settings(&[KEYS, (name, value)]).unwrap_err();
        assert_eq!(err.name, name);
        assert!(!err.to_string().contains(value), "{err} echoes {value}");
    }
}

#[test]
fn an_empty_variable_counts_as_unset() {
    let s = settings(&[KEYS, ("GRAPH_PORT", ""), ("GRAPH_CORS_ORIGINS", "")]).expect("empty");
    assert_eq!(s.port, 8080);
    assert!(s.cors_origins.is_empty());
}

#[test]
fn origins_are_a_trimmed_comma_list() {
    let s = settings(&[
        KEYS,
        ("GRAPH_CORS_ORIGINS", "https://a.example, http://b:8080"),
    ])
    .unwrap();
    assert_eq!(s.cors_origins, ["https://a.example", "http://b:8080"]);
}

#[test]
fn default_workers_is_the_smaller_of_cores_and_memory_slots() {
    assert_eq!(default_workers(8, None), 8);
    assert_eq!(
        default_workers(8, Some(BASE_BYTES + 2 * PER_SLOT_BYTES + 1)),
        2
    );
    assert_eq!(default_workers(2, Some(64 * PER_SLOT_BYTES)), 2);
    assert_eq!(default_workers(8, Some(PER_SLOT_BYTES - 1)), 0);
    assert_eq!(
        default_workers(8, Some(4 << 30)),
        0,
        "4 GiB holds no slot: the published docker run --memory 4g refuses to start"
    );
    assert_eq!(default_workers(8, Some(8 << 30)), 1, "8 GiB holds one slot");
    assert_eq!(
        default_workers(64, Some(64 << 30)),
        14,
        "the doc's 64 GiB row"
    );
}

#[test]
fn memory_under_one_slot_refuses_the_default_and_not_an_explicit_count() {
    let unset = |_: &str| None;
    let refused = super::read_limits(&super::Env(&unset), Some(PER_SLOT_BYTES - 1));
    assert_eq!(refused.map_err(|e| e.name), Err("GRAPH_WORKERS"));
    let one = |name: &str| (name == "GRAPH_WORKERS").then(|| OsString::from("1"));
    let limits = super::read_limits(&super::Env(&one), Some(PER_SLOT_BYTES - 1));
    assert_eq!(limits.map(|l| l.workers), Ok(1));
}

#[test]
fn the_start_line_says_set_or_unset_and_no_value() {
    let secret = "/run/secret/path-that-must-not-print";
    let line = start_line(&|name| (name == "GRAPH_API_KEYS_FILE").then(|| OsString::from(secret)));
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("json");
    assert_eq!(parsed["env"]["GRAPH_API_KEYS_FILE"], "set");
    assert_eq!(parsed["env"]["GRAPH_PORT"], "unset");
    assert_eq!(
        parsed["env"].as_object().map(|env| env.len()),
        Some(super::NAMES.len())
    );
    assert!(!line.contains(secret));
}
