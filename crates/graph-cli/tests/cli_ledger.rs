//! The capabilities ledger, run as the real `graph-cli` binary: what `--check` refuses
//! with no recorded run behind it, and what the rows say about themselves.
//!
//! Split out of `cli.rs` for the same reason `cli_oracles.rs` is: the house's 300-line
//! limit. The process helpers are `tests/common`'s.

mod common;

use common::stdout;
use std::process::Output;

/// Gate records land here, never `target/gates`: a test run must not overwrite (or
/// stand in for) the evidence of a real gate run.
fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-ledger-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str]) -> Output {
    common::graph_cli(&gates_dir(), args, None)
}

/// Every `prompt.md` §8 field a row must carry, none of which may be null in the JSON.
const REQUIRED: [&str; 6] = [
    "oracle",
    "complexity",
    "degradation",
    "ponytail",
    "scale_ceiling",
    "geometry",
];

/// The same list for a row that emits no geometry: an `analysis.*` labelling, a `scale.*`
/// hint, an `ingest.*` reader. `geometry` is null there **by design** — none of those
/// stages produces a node or edge geometry kind — so demanding a value would be demanding
/// a lie. Everything §8 calls required is still required, and `geometry` is asserted
/// separately, as null.
const REQUIRED_LABEL: [&str; 5] = [
    "oracle",
    "complexity",
    "degradation",
    "ponytail",
    "scale_ceiling",
];

/// The POST rows Phase 8 registered, by id. Found by id throughout: a registry entry
/// inserted before a row moves every index after it, so an assertion on a position tests
/// the order rather than the row.
const POST_IDS: [&str; 7] = [
    "post.route.grid",
    "post.bundle.fdeb",
    "post.bundle.mingle",
    "post.style.straight",
    "post.style.orthogonal",
    "post.style.quadratic",
    "post.style.bezier",
];

#[test]
fn capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs() {
    assert_eq!(graph_cli(&["capabilities"]).status.code(), Some(2));
    let check = graph_cli(&["capabilities", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", stdout(&check));
    // 18 rows before Phase 7, its 8 analysis.* rows (`Implemented`, no problems),
    // Phase 4's transport (gated, refused twice) and sdk.js rows, Phase 8's seven
    // `post.*` rows, Phase 9's three `scale.*` rows, Phase 10's four
    // `ingest.*`/`adapter.*` rows and `analysis.depth` (all `implemented`, no problem).
    // Every problem is a `gated` row with no record behind it; the `implemented` rows
    // never produce one.
    assert!(
        stdout(&check).contains("capabilities --check: 48 rows, 34 problems"),
        "{}",
        stdout(&check)
    );
}

/// Every POST row is published, `implemented`, and carries every required field. A POST
/// capability with no row is a capability the ledger says nothing about — what it costs
/// and what it owes — which is what §8 exists to prevent.
#[test]
fn every_post_row_is_published_implemented_and_fully_filled() {
    let json = graph_cli(&["capabilities", "--json"]);
    assert_eq!(json.status.code(), Some(0));
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    let listed = rows.as_array().expect("an array");
    assert_eq!(
        listed.len(),
        48,
        "42 before analysis.depth, and 36 before Phase 8's six bundling and style rows"
    );
    for id in POST_IDS {
        let row = listed
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} is registered"));
        assert_eq!(
            row["status"], "implemented",
            "{id}: implemented, never gated without a recorded differential"
        );
        for field in REQUIRED {
            assert!(!row[field].is_null(), "{id}: {field} is filled");
        }
    }
}

/// The style rows differ in exactly one field, `edges`, and each names the kind its own
/// generator emits: a straight chord is stored as `Line`, not folded into `polyline`.
#[test]
fn each_style_row_names_its_own_geometry_kind() {
    let json = graph_cli(&["capabilities", "--json"]);
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    let kind = |id: &str| {
        rows.as_array()
            .expect("an array")
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} is registered"))["geometry"]
            .clone()
    };
    assert_eq!(kind("post.style.straight"), "Line");
    assert_eq!(kind("post.style.orthogonal"), "Polyline");
    assert_eq!(kind("post.style.quadratic"), "Curve");
    assert_eq!(kind("post.style.bezier"), "Curve");
    assert_eq!(kind("post.bundle.fdeb"), "Polyline");
    assert_eq!(kind("post.bundle.mingle"), "Polyline");
}

/// The ledger reads what a gate recorded: a short honest run is found, and refused for
/// its seed count rather than reported missing.
#[test]
fn the_ledger_reads_a_recorded_run_and_names_what_it_lacks() {
    let dir = std::env::temp_dir().join(format!("gm-cli-ledger-{}", std::process::id()));
    let run = |args: &[&str]| common::graph_cli(&dir, args, None);
    assert_eq!(run(&["hashgate", "--seeds", "2"]).status.code(), Some(0));
    let json = run(&["capabilities", "--json"]);
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    let first = rows
        .as_array()
        .expect("an array")
        .iter()
        .find(|r| r["id"] == "topology.index")
        .unwrap_or_else(|| panic!("topology.index is a row"));
    assert_eq!(
        first["hash_4way"],
        "not backed: hashgate ran 2 seeds, need 1000"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// `analysis.depth`, the ninth `analysis.*` row, as the real binary publishes it: the
/// row the process prints, found by id, carrying every required field. A row that
/// exists in the registry but not in `capabilities --json` is a row no consumer can see,
/// and Phase 7's row is the one that was missing for exactly that length of time.
///
/// The count is asserted on the process's own output line, and the problem count is
/// pinned beside it: an `implemented` row contributes no problem, so a new row may move
/// the first number and never the second.
#[test]
fn the_depth_row_is_published_by_the_binary_and_adds_no_problem() {
    let check = graph_cli(&["capabilities", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", stdout(&check));
    assert!(
        stdout(&check).contains("capabilities --check: 48 rows, 34 problems"),
        "the new row is implemented, so it adds a row and not a problem: {}",
        stdout(&check)
    );
    let json = graph_cli(&["capabilities", "--json"]);
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    let row = rows
        .as_array()
        .expect("an array")
        .iter()
        .find(|r| r["id"] == "analysis.depth")
        .unwrap_or_else(|| panic!("analysis.depth is published"));
    assert_eq!(row["stage"], "analysis");
    assert_eq!(row["status"], "implemented");
    assert!(
        row["geometry"].is_null(),
        "a labelling, not a geometry kind"
    );
    for field in REQUIRED_LABEL {
        assert!(!row[field].is_null(), "analysis.depth: {field} is filled");
    }
}
