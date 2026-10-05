//! §6's limits, one test each: the four gates, the two subscriber counters, the body reader and the
//! connection limits.
//!
//! Every case goes through the real object (`Gate::admit`, `body::read`, the accept loop) and never
//! through a copy of the rule, so a test that passes is a statement about the shipped code.
#![cfg(feature = "db-tests")]

#[path = "limits/gates.rs"]
mod gates;
#[path = "limits/reader.rs"]
mod reader;
#[path = "limits/refusals.rs"]
mod refusals;
mod support;

use support::*;

/// Every §6 default is the value `config::Settings::from_env` reads, so a test that sets one of these
/// names changes one number and nothing else.
#[test]
fn every_limit_default_is_the_section_6_value() {
    let hub = hub_with_env(&[]);
    let settings = &hub.app.settings;
    assert_eq!(settings.gates.writers, 2, "GRAPH_HUB_WRITERS");
    assert_eq!(settings.gates.readers, 2, "GRAPH_HUB_READS");
    assert_eq!(settings.gates.layouts, 1, "GRAPH_HUB_LAYOUTS");
    assert_eq!(
        settings.gates.writers_per_key, 1,
        "GRAPH_HUB_WRITERS_PER_KEY"
    );
    assert_eq!(settings.limits.max_body, 4 << 20, "GRAPH_HUB_MAX_BODY");
    assert_eq!(settings.limits.max_batch, 10_000, "GRAPH_HUB_MAX_BATCH");
    assert_eq!(
        settings.limits.max_record_bytes,
        1 << 20,
        "GRAPH_HUB_MAX_RECORD_BYTES"
    );
    assert_eq!(settings.subscribers.max, 64, "GRAPH_HUB_MAX_SUBSCRIBERS");
    assert_eq!(
        settings.subscribers.per_key, 8,
        "GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY"
    );
    assert_eq!(
        settings.connections.max_connections, 256,
        "GRAPH_HUB_MAX_CONNECTIONS"
    );
    assert_eq!(
        settings.connections.max_header_bytes, 16_384,
        "GRAPH_HUB_MAX_HEADER_BYTES"
    );
    assert_eq!(
        settings.connections.header_timeout.as_millis(),
        5_000,
        "GRAPH_HUB_HEADER_TIMEOUT_MS"
    );
    assert_eq!(
        settings.limits.body_timeout.as_millis(),
        10_000,
        "GRAPH_HUB_BODY_TIMEOUT_MS"
    );
    assert_eq!(
        settings.limits.timeout.as_millis(),
        30_000,
        "GRAPH_HUB_TIMEOUT_MS"
    );
}

/// `App::from_settings` builds one live gate object per §6 row, so a permit exists before any
/// request does.
#[test]
fn the_gates_are_built_from_the_settings_at_start() {
    let hub = hub_with_env(&[("GRAPH_HUB_WRITERS", "3"), ("GRAPH_HUB_READS", "4")]);
    assert_eq!(hub.app.gates.writers.free(), 3);
    assert_eq!(hub.app.gates.readers.free(), 4);
    assert_eq!(hub.app.gates.layouts.free(), 1);
}
