//! §6's twenty-six defaults, spelled out as literals so a spec change that renames or moves one
//! fails here rather than silently changing a behaviour.
//!
//! `config::read_limits` and `config::read_store` spell the same numbers a second time, for the
//! hub's own copy and the store's copy. This file is what says the two copies are one number.

use std::collections::BTreeMap;
use std::os::unix::ffi::OsStringExt;

use graph_hub::config::{NAMES, Settings};

use crate::settings as read;

/// §6's defaults over `env`, with a refusal a failure: this file tests the reader, not the checks.
fn settings(env: &[(&str, &str)]) -> Settings {
    read(env).expect("the settings under test")
}

/// §6's table, in its own order, every default a literal.
#[test]
fn config_defaults_match_section_6() {
    let settings = settings(&[]);
    let limits = settings.limits;
    assert_eq!(limits.max_body, 4 << 20, "GRAPH_HUB_MAX_BODY");
    assert_eq!(limits.max_batch, 10_000, "GRAPH_HUB_MAX_BATCH");
    assert_eq!(
        limits.max_record_bytes,
        1 << 20,
        "GRAPH_HUB_MAX_RECORD_BYTES"
    );
    assert_eq!(
        limits.max_plugin_bytes,
        16 << 20,
        "GRAPH_HUB_MAX_PLUGIN_BYTES"
    );
    assert_eq!(limits.max_doc_bytes, 64 << 20, "GRAPH_HUB_MAX_DOC_BYTES");
    assert_eq!(limits.retain, 100_000, "GRAPH_HUB_RETAIN");
    assert_eq!(limits.retain_bytes, 512 << 20, "GRAPH_HUB_RETAIN_BYTES");
    assert_eq!(limits.changes_bytes, 8 << 20, "GRAPH_HUB_CHANGES_BYTES");
    assert_eq!(settings.gates.writers, 2, "GRAPH_HUB_WRITERS");
    assert_eq!(settings.gates.readers, 2, "GRAPH_HUB_READS");
    assert_eq!(settings.gates.layouts, 1, "GRAPH_HUB_LAYOUTS");
    assert_eq!(
        settings.gates.writers_per_key, 1,
        "GRAPH_HUB_WRITERS_PER_KEY"
    );
    assert_eq!(
        limits.body_timeout.as_millis(),
        10_000,
        "GRAPH_HUB_BODY_TIMEOUT_MS"
    );
    assert_eq!(
        settings.connections.max_connections, 256,
        "GRAPH_HUB_MAX_CONNECTIONS"
    );
    assert_eq!(
        settings.connections.header_timeout.as_millis(),
        5_000,
        "GRAPH_HUB_HEADER_TIMEOUT_MS"
    );
    assert_eq!(
        settings.connections.max_header_bytes, 16_384,
        "GRAPH_HUB_MAX_HEADER_BYTES"
    );
    assert_eq!(limits.fetch_rows, 4096, "GRAPH_HUB_FETCH_ROWS");
    assert_eq!(settings.subscribers.max, 64, "GRAPH_HUB_MAX_SUBSCRIBERS");
    assert_eq!(
        settings.subscribers.per_key, 8,
        "GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY"
    );
    assert_eq!(limits.sse_page, 256, "GRAPH_HUB_SSE_PAGE");
    assert_eq!(limits.last_seen, 65_536, "GRAPH_HUB_LAST_SEEN");
    assert_eq!(settings.store.pool, 8, "GRAPH_HUB_DB_POOL");
    assert_eq!(
        limits.stream_deadline.as_millis(),
        120_000,
        "GRAPH_HUB_STREAM_DEADLINE_MS"
    );
    assert_eq!(
        limits.motor_timeout.as_millis(),
        45_000,
        "GRAPH_HUB_MOTOR_TIMEOUT_MS"
    );
    assert_eq!(limits.timeout.as_millis(), 30_000, "GRAPH_HUB_TIMEOUT_MS");
    assert_eq!(settings.sse_page, limits.sse_page, "lifted out of Limits");
    assert_eq!(
        settings.motor_timeout, limits.motor_timeout,
        "lifted out of Limits"
    );
    assert_eq!(settings.timeout, limits.timeout, "lifted out of Limits");
}

/// The five names §6's table does not bound, with every default spelled out: the URL is empty
/// (Decision 5) and the two credential paths are unset (Decision 7).
#[test]
fn the_five_unbounded_names_match_decision_5_and_7() {
    // The five names are read as empty here, which is what "unset" means to `Env::text`: the shared
    // fixture sets the credential files, so they are named empty to read their defaults.
    let settings = read(&[
        ("GRAPH_HUB_DB_URL", ""),
        ("GRAPH_HUB_KEYS_FILE", ""),
        ("GRAPH_HUB_GRANTS_FILE", ""),
        ("GRAPH_HUB_MOTOR_KEY_FILE", ""),
        ("GRAPH_HUB_MOTOR_URL", ""),
    ])
    .expect("the defaults");
    assert_eq!(
        settings.db_url, "",
        "GRAPH_HUB_DB_URL is empty in defaults()"
    );
    assert_eq!(
        settings.keys_file.as_os_str(),
        "",
        "GRAPH_HUB_KEYS_FILE unset"
    );
    assert_eq!(
        settings.grants_file.as_os_str(),
        "",
        "GRAPH_HUB_GRANTS_FILE unset"
    );
    assert_eq!(settings.motor_url, "http://127.0.0.1:8080");
    assert_eq!(settings.motor_key_file.as_os_str(), "");
    assert_eq!(settings.bind.to_string(), "127.0.0.1", "GRAPH_HUB_BIND");
    assert_eq!(settings.port, 8080, "GRAPH_HUB_PORT");
    // The store's own `url` is empty in `StoreConfig::defaults()` and the hub fills it from
    // `GRAPH_HUB_DB_URL` (Decision 5), so there is one URL and not two.
    assert_eq!(settings.store.url, "");
}

/// The store's copy of the numbers both crates hold is the same number, not a second default.
#[test]
fn the_store_config_carries_the_same_numbers() {
    let settings = settings(&[]);
    let store = settings.store;
    assert_eq!(store.retain, 100_000);
    assert_eq!(store.retain_bytes, 512 << 20);
    assert_eq!(store.changes_bytes, 8 << 20);
    assert_eq!(store.fetch_rows, 4096);
    assert_eq!(store.last_seen, 65_536);
    assert_eq!(store.max_body, settings.limits.max_body);
    assert_eq!(store.max_batch, settings.limits.max_batch);
    assert_eq!(store.max_record_bytes, settings.limits.max_record_bytes);
    assert_eq!(store.max_plugin_bytes, settings.limits.max_plugin_bytes);
    assert_eq!(store.max_doc_bytes, settings.limits.max_doc_bytes);
    assert_eq!(store.timeout_ms, 30_000);
    assert_eq!(store.stream_deadline_ms, 120_000);
    // The two the store defaults on its own: a sweeper interval and an idempotency TTL, neither of
    // which §6's table bounds and neither of which the hub reads.
    assert_eq!(store.sweeper_interval_ms, 600_000);
    assert_eq!(store.idem_ttl_ms, 86_400_000);
}

/// `NAMES` holds §6's twenty-six, the five unbounded names and `GRAPH_HUB_TIMEOUT_MS`, and every
/// numeric one is refused by name on a nonsense value.
#[test]
fn every_hub_env_name_is_read() {
    assert_eq!(
        NAMES.len(),
        32,
        "five unbounded names, §6's twenty-six and GRAPH_HUB_TIMEOUT_MS"
    );
    let text = [
        "GRAPH_HUB_DB_URL",
        "GRAPH_HUB_KEYS_FILE",
        "GRAPH_HUB_GRANTS_FILE",
        "GRAPH_HUB_MOTOR_KEY_FILE",
        "GRAPH_HUB_MOTOR_URL",
    ];
    for name in NAMES {
        let refused = read(&[(name, "not-a-number")]);
        if text.contains(&name) {
            assert!(
                refused.is_ok(),
                "{name} is text, so a nonsense value is a legal path: it was refused"
            );
            continue;
        }
        let refusal = refused.expect_err("a nonsense value must be refused");
        assert!(
            ["is malformed", "is out of range", "is not UTF-8"].contains(&refusal.reason),
            "{name} was refused with {:?}, which is none of graph-server's three messages",
            refusal.reason
        );
    }
}

/// The three messages are graph-server's own, byte for byte
/// (`server/graph-server/src/config.rs:260,266,283`).
#[test]
fn the_three_refusal_messages_are_graph_servers_own() {
    assert_eq!(
        read(&[("GRAPH_HUB_MAX_BODY", "not-a-number")])
            .unwrap_err()
            .reason,
        "is malformed"
    );
    assert_eq!(
        read(&[("GRAPH_HUB_MAX_BODY", "0")]).unwrap_err().reason,
        "is out of range"
    );
    assert_eq!(
        read(&[("GRAPH_HUB_BIND", "not-an-address")])
            .unwrap_err()
            .reason,
        "is malformed"
    );
    // A non-UTF-8 value: `OsString::from_vec` is the only way to build one, and it is unix-only,
    // which is the only platform this workspace builds for.
    let bytes = [0x66, 0x80];
    let bad = std::ffi::OsString::from_vec(bytes.to_vec());
    let lookup = |name: &str| (name == "GRAPH_HUB_MAX_BODY").then(|| bad.clone());
    let refusal = Settings::from_env(&lookup).unwrap_err();
    assert_eq!(refusal.reason, "is not UTF-8");
    assert_eq!(refusal.name, "GRAPH_HUB_MAX_BODY");
}

/// The start line names every variable and no value, which is what makes it safe to log whole.
#[test]
fn the_start_line_names_every_variable_and_no_value() {
    let vars: BTreeMap<String, String> = BTreeMap::new();
    let lookup = |name: &str| vars.get(name).map(Into::into);
    let line = Settings::start_line(&lookup);
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("a JSON start line");
    assert_eq!(parsed["event"], "start");
    let env = parsed["env"].as_object().expect("the env map");
    assert_eq!(env.len(), NAMES.len());
    for name in NAMES {
        assert_eq!(env[name], "unset", "{name} is unset in this fixture");
    }
    assert!(!line.contains("postgres://"), "{line}");
}
