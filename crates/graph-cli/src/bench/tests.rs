use super::campaign::arms::{self, ArmReading};
use super::campaign::{SETTLE_TICKS, Sample, largest_fitting, median, settle_ms};
use super::scale;
use super::staging::harness_copy_with_in;
use super::*;
use graph_core::StageError;

fn plan(past_ceiling: bool, dry_run: bool) -> Plan {
    Plan {
        sizes: Vec::new(),
        layouts: Vec::new(),
        seed: 0,
        past_ceiling,
        vs_d3: false,
        dry_run,
        repeat: 1,
        out: None,
        crossover: false,
        budget_ms: 16.67,
        emit_scale_fixture: None,
    }
}

fn path(n: u32) -> Topology {
    let (nodes, edges) = seeded_model(0, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("fits")
}

fn refuses(_: &Topology) -> Result<Geometry, StageError> {
    Err(StageError::Param {
        name: "test",
        rule: "always refused",
    })
}

/// A registered entry whose run always fails, at a chosen ceiling.
fn failing(ceiling: u64) -> Capability {
    let mut entry = *registry::find("layout.grid").expect("registered");
    entry.run = refuses;
    entry.meta.scale_ceiling = ceiling;
    entry
}

#[test]
fn the_default_set_is_the_four_phase6_layouts_and_an_unknown_id_is_refused() {
    let ids: Vec<&str> = resolve(&[])
        .expect("registered")
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, PHASE6);
    let one = resolve(&["layout.grid".into()]).expect("registered");
    assert_eq!(one.len(), 1);
    let err = resolve(&["layout.grid".into(), "layout.none".into()]).map(drop);
    assert_eq!(err, Err("layout.none: not registered".into()));
}

#[test]
fn a_failure_inside_the_ceiling_fails_and_past_it_is_only_reported() {
    let topology = path(12);
    assert_eq!(row(&plan(false, false), &failing(12), &topology), Ok(false));
    assert_eq!(row(&plan(true, false), &failing(11), &topology), Ok(true));
}

#[test]
fn past_the_ceiling_is_refused_and_a_dry_run_runs_nothing() {
    let topology = path(12);
    // Refused and dry runs never reach `run`, so the failing entry passes.
    assert_eq!(row(&plan(false, false), &failing(11), &topology), Ok(true));
    assert_eq!(row(&plan(false, true), &failing(12), &topology), Ok(true));
    assert_eq!(row(&plan(true, true), &failing(11), &topology), Ok(true));
    let grid = registry::find("layout.grid").expect("registered");
    assert_eq!(row(&plan(false, false), grid, &topology), Ok(true));
}

#[test]
fn stress_is_zero_on_an_isometric_path_and_pinned_on_a_folded_one() {
    let (source, target) = ([0, 1], [1, 2]);
    assert_eq!(stress(&[0.0, 2.0, 4.0], &[0.0; 3], &source, &target), 0.0);
    // Folded: node 2 back on node 0. de = ee = 4, dd = 12, so sqrt(8 / 12).
    let folded = stress(&[0.0, 1.0, 0.0], &[0.0; 3], &source, &target);
    assert_eq!(folded, (8.0_f64 / 12.0).sqrt());
}

#[test]
fn a_collapsed_layout_or_an_edgeless_graph_scores_the_worst_stress() {
    assert_eq!(stress(&[5.0; 3], &[5.0; 3], &[0, 1], &[1, 2]), 1.0);
    assert_eq!(stress(&[0.0, 1.0], &[0.0, 0.0], &[], &[]), 1.0);
}

#[test]
fn unreachable_pairs_are_skipped_so_each_component_is_scored_alone() {
    // Two isometric edges far apart: every reachable pair is exact, so 0.
    let (x, y) = ([0.0, 1.0, 100.0, 101.0], [0.0; 4]);
    assert_eq!(stress(&x, &y, &[0, 2], &[1, 3]), 0.0);
}

#[test]
fn the_median_of_an_odd_count_is_the_middle_value_and_of_an_even_one_the_mean() {
    assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    assert_eq!(median(vec![]), 0.0);
    assert_eq!(median(vec![7.0]), 7.0);
}

#[test]
fn settle_is_the_tick_times_the_112_ticks_alpha_decay_needs() {
    assert_eq!(SETTLE_TICKS, 112);
    assert_eq!(settle_ms(0.25), 28.0);
}

#[test]
fn the_crossover_is_the_largest_n_that_fits_the_budget_and_none_fits_when_all_over() {
    let samples = [(220, 1.0), (10_000, 8.0), (100_000, 40.0)];
    assert_eq!(largest_fitting(&samples, 16.67), Some(10_000));
    // Exactly at the budget still fits: a crossover is not a regression test.
    assert_eq!(largest_fitting(&samples, 8.0), Some(10_000));
    assert_eq!(largest_fitting(&samples, 0.5), None);
    assert_eq!(largest_fitting(&[], 16.67), None);
}

/// The Phase 9 oracle arm's own self-check, run under Node: the pure arithmetic of the
/// campaign (median over repeats, the largest N that fits a budget) and the graph it
/// shares with the native arm, pinned in the harness that has to agree with us.
#[test]
fn the_oracle_tick_harness_self_check_passes() {
    let lines = oracle_harness(&["--self-check"]);
    assert!(
        lines.iter().any(|l| l.contains("self-check ok")),
        "expected the self-check to report itself: {lines:?}"
    );
}

/// The negative control for the row above: a harness whose pinned constant has been
/// changed must go red. A self-check that cannot fail is not a check — so this copies
/// the harness, breaks one number in the copy, and runs *that*.
#[test]
fn a_broken_copy_of_the_oracle_harness_fails_its_own_self_check() {
    let mutant = harness_copy_with_in(
        "oracle-tick-bench.mjs",
        "const SETTLE_TICKS = 112;",
        "const SETTLE_TICKS = 113;",
    );
    let ran = run_node(&mutant.0, &["--self-check"]);
    assert!(
        !ran.success(),
        "the mutant's self-check passed: the check cannot fail"
    );
    mutant.cleanup();
}

/// The graph both arms lay out: the TypeScript oracle's own `buildSyntheticModel`, which
/// graph-core's synthetic model is a call-for-call port of. Pinned here in the native arm
/// and in the harness, so a drift in either is a number, not a silent difference in what
/// the two arms were measured on.
#[test]
fn the_model_the_oracle_arm_lays_out_is_the_model_this_arm_lays_out() {
    let (nodes, edges) = seeded_model(0, 6, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    assert_eq!(
        (nodes.len(), edges.len()),
        (6, 9),
        "the pinned six-node model changed on this side of the port"
    );
    assert_eq!(topology.edge_count(), 9);
}

/// Runs `harness/oracle-tick-bench.mjs` under Node with `args`.
fn oracle_harness(args: &[&str]) -> Vec<String> {
    let root = crate::runner::workspace_root();
    let mut node = std::process::Command::new("node");
    node.current_dir(&root)
        .arg(root.join("harness").join("oracle-tick-bench.mjs"));
    node.args(args);
    crate::runner::run_lines(&mut node).expect("the oracle tick harness runs")
}

// The negative controls' staged mutant copies, and the test that pins where a
// transient file may be written, live in `bench/staging.rs`.

fn run_node(script: &std::path::Path, args: &[&str]) -> std::process::ExitStatus {
    let mut node = std::process::Command::new("node");
    node.current_dir(crate::runner::workspace_root())
        .arg(script)
        .args(args);
    crate::runner::run_status(&mut node, std::time::Duration::from_secs(300)).expect("node runs")
}

/// The wasm32 arm's own self-check: the ingest document it hands the motor is the strict
/// provisional shape `graph-wasm`'s `ingest.rs` reads, and its campaign arithmetic is the
/// same pinned pair. Runs without the wasm binary, so it runs anywhere Node runs.
#[test]
fn the_wasm_tick_harness_self_check_passes() {
    let lines = harness_lines("wasm-tick-bench.mjs", &["--self-check"]);
    assert!(
        lines.iter().any(|l| l.contains("self-check ok")),
        "expected the wasm self-check to report itself: {lines:?}"
    );
}

/// The negative control for the row above: a changed constant in a copy must go red.
#[test]
fn a_broken_copy_of_the_wasm_tick_harness_fails_its_own_self_check() {
    let mutant = harness_copy_with_in(
        "wasm-tick-bench.mjs",
        "const SETTLE_TICKS = 112;",
        "const SETTLE_TICKS = 111;",
    );
    let ran = run_node(&mutant.0, &["--self-check"]);
    assert!(
        !ran.success(),
        "the mutant's self-check passed: the check cannot fail"
    );
}

/// Runs `harness/<name>` under Node with `args`.
fn harness_lines(name: &str, args: &[&str]) -> Vec<String> {
    let root = crate::runner::workspace_root();
    let mut node = std::process::Command::new("node");
    node.current_dir(&root).arg(root.join("harness").join(name));
    node.args(args);
    crate::runner::run_lines(&mut node).expect("the harness runs")
}

#[test]
fn a_sample_reports_a_one_shot_run_and_the_tick_it_implies() {
    let sample = Sample {
        n: 220,
        edges: 327,
        run_ms: 1120.0,
        build_ms: 1.0,
        columns: 0,
        arena: 0,
        bin: 0,
        json: 0,
        past_ceiling: false,
    };
    // The ABI has no per-tick entry, so the campaign derives it: the same division the
    // two harness arms make, which is what lets three crossover cells be compared.
    assert_eq!(sample.tick_ms(), 10.0);
    assert_eq!(sample.settle_ms(), 1120.0);
}

#[test]
fn a_measured_arm_fills_its_crossover_cell_and_an_absent_one_says_so() {
    let measured = ArmReading::measured("wasm32", Some(220), &[(220, 0.319), (10_000, 34.97)]);
    let absent = ArmReading::absent(
        "TypeScript oracle",
        "harness/oracle-tick-bench.mjs was not run",
    );
    let text = arms::arms_markdown(&[measured, absent], 16.67);
    assert!(text.contains("| wasm32 | 220 |"), "{text}");
    assert!(text.contains("not measured"), "{text}");
    assert!(
        text.contains("harness/oracle-tick-bench.mjs was not run"),
        "{text}"
    );
}

/// The harness JSON the two JS arms write is read into the crossover table, and a
/// truncated or foreign file is a refusal naming it, never a silent zero.
#[test]
fn an_arm_json_file_is_read_into_its_ladder_and_a_bad_one_is_refused() {
    let dir = std::env::temp_dir().join(format!("gm-arms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let path = dir.join("arm.json");
    let good = r#"{"arm":"wasm32","rows":[{"n":220,"tick_ms":0.319},{"n":10000,"tick_ms":34.97}]}"#;
    std::fs::write(&path, good).expect("written");
    let rows = arms::read_arm_json(&path).expect("the good file reads");
    assert_eq!(rows, vec![(220, 0.319), (10_000, 34.97)]);
    std::fs::write(&path, "{ not json").expect("written");
    let err = arms::read_arm_json(&path).expect_err("a truncated file is refused");
    assert!(err.contains("arm.json"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A scale fixture at or below the model's own cap is the model itself, byte for byte:
/// the generator is not a second implementation of the benchmark graph.
#[test]
fn a_scale_fixture_at_or_below_the_cap_is_the_synthetic_model() {
    let (nodes, edges) = scale::scale_model(0, 220, REFERENCE_DEGREE);
    let (want_nodes, want_edges) = seeded_model(0, 220, REFERENCE_DEGREE);
    assert_eq!(nodes.len(), want_nodes.len());
    assert_eq!(
        nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        want_nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(edges.len(), want_edges.len());
    assert_eq!(edges[0].id, want_edges[0].id);
    assert_eq!(edges[0].strength, want_edges[0].strength);
}

/// Past the cap the fixture is whole components of the same model, concatenated, each
/// node id prefixed by its component so no id repeats: a 250k fixture is 100k + 100k +
/// 50k, not a truncated 250k.
#[test]
fn a_scale_fixture_past_the_cap_is_whole_prefixed_components() {
    let (nodes, edges) = scale::scale_model(0, 250_000, REFERENCE_DEGREE);
    assert_eq!(nodes.len(), 250_000);
    let mut ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "a component reused a node id");
    assert!(nodes[0].id.starts_with("c0/"), "{}", nodes[0].id);
    assert!(
        nodes[100_000].id.starts_with("c1/"),
        "{}",
        nodes[100_000].id
    );
    assert!(
        nodes[200_000].id.starts_with("c2/"),
        "{}",
        nodes[200_000].id
    );
    let known: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for edge in &edges {
        assert!(known.contains(edge.source.as_str()), "{}", edge.source);
        assert!(known.contains(edge.target.as_str()), "{}", edge.target);
    }
}

/// The emitted fixture is the provisional ingest document `graph-wasm`'s reader accepts:
/// version 1, every member named, no member beyond the ten and nine it requires.
#[test]
fn an_emitted_scale_fixture_is_the_provisional_ingest_shape() {
    let json = scale::fixture_json(0, 220, REFERENCE_DEGREE);
    assert!(
        json.starts_with(r#"{"version":1,"nodes":["#),
        "{}",
        &json[..40.min(json.len())]
    );
    let value = graph_contract::canonical_json::parse(&json).expect("valid JSON");
    let nodes = members(&value, "nodes");
    let edges = members(&value, "edges");
    let (_, model_edges) = seeded_model(0, 220, REFERENCE_DEGREE);
    assert_eq!((nodes.len(), edges.len()), (220, model_edges.len()));
    assert_eq!(member_names(&nodes[0]), scale::NODE_FIELDS);
    assert_eq!(member_names(&edges[0]), scale::EDGE_FIELDS);
}

/// The `member` array of a parsed document, or the test's own failure.
fn members(
    value: &graph_contract::canonical_json::Value,
    member: &str,
) -> Vec<graph_contract::canonical_json::Value> {
    let graph_contract::canonical_json::Value::Object(fields) = value else {
        panic!("the document is not an object");
    };
    match fields.iter().find(|(name, _)| name == member) {
        Some((_, graph_contract::canonical_json::Value::Array(items))) => items.clone(),
        _ => panic!("no `{member}` array"),
    }
}

/// An object's member names, sorted, which is what the ingest reader's `require_only`
/// compares against.
fn member_names(value: &graph_contract::canonical_json::Value) -> Vec<&str> {
    let graph_contract::canonical_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    let mut names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names
}
