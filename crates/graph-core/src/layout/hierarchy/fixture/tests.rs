//! The fixture boundary's own refusals, tested through [`node`] because [`load`] can only
//! reach the four checked-in files. Each case is a `Value` the canonical JSON parser would
//! accept, so nothing here is a value the parser cannot produce.
//!
//! RED for the finding this file answers: a non-finite weight reached
//! `NodeRecord.weight` unchanged and every downstream sum and partition inherited it.

use super::{FIXTURES, load, node};
use graph_contract::canonical_json::Value;

/// One fixture node object: `{"id": <id>, "weight": Number(<weight>)}`.
fn record(id: &str, weight: &str) -> Value {
    Value::Object(vec![
        ("id".into(), Value::String(id.into())),
        ("weight".into(), Value::Number(weight.into())),
    ])
}

/// The overflowing exponent is the reachable one: it is inside the JSON number grammar,
/// so `parse` hands it over and only `f64::from_str` turns it into `+inf`.
#[test]
#[should_panic(expected = "wide: weight \"1e999\" is not finite")]
fn an_overflowing_exponent_is_refused_and_names_the_node() {
    let _ = node(&record("wide", "1e999"));
}

/// The reviewer's own `"NaN"` cannot arrive as a `Value::Number` — the JSON grammar has no
/// such token — but a hand-built `Value` can carry one, and it is refused the same way.
#[test]
#[should_panic(expected = "wide: weight \"NaN\" is not finite")]
fn a_nan_weight_is_refused_and_names_the_node() {
    let _ = node(&record("wide", "NaN"));
}

#[test]
#[should_panic(expected = "wide: weight \"-1e999\" is not finite")]
fn a_negative_infinity_weight_is_refused_and_names_the_node() {
    let _ = node(&record("wide", "-1e999"));
}

/// A finite weight, including the extremes the fixtures actually use, still loads — the
/// refusal above is the non-finite case and nothing else.
#[test]
fn every_finite_weight_still_parses() {
    for weight in ["0", "-1", "0.25", "1", "1.7976931348623157e308"] {
        let id = format!("n{weight}");
        assert_eq!(
            node(&record(&id, weight)).weight,
            weight.parse::<f64>().unwrap()
        );
    }
}

/// Every checked-in fixture still loads, weight check included.
#[test]
fn every_checked_in_fixture_still_loads() {
    for (name, _) in FIXTURES {
        let (nodes, _) = load(name);
        assert!(!nodes.is_empty(), "{name} has no nodes");
    }
}
