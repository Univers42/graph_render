//! The ForceAtlas2 differential (`emit-fa2-fixtures` → `harness/oracle-fa2.py` →
//! `oracle-fa2`) end to end: the budget the fixtures carry, the ceiling `oracle-fa2`
//! judges against, and the negative control that has to go red at the gated budget.
//!
//! Its own file rather than another test in `cli_oracles.rs` (at the house's 300-line
//! cap) because it carries a pinned networkx reference and the metric over it, so the
//! control needs neither networkx nor the `ge-python-oracle` image.

mod common;

use common::stdout;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;

/// The reference arm, pinned: networkx 3.6's own `forceatlas2_layout` over the gate model's
/// first 64 seeds at the gated budget, written by `python3 harness/fa2-chaos.py <dir>
/// --write-reference`. Re-pin it only together with a re-measurement
/// (`docs/measurements/fa2-chaos.md`): a stale pin would make the control below pass or fail
/// for a reason that has nothing to do with the port.
const NX_REFERENCE: &str = "tests/fixtures/fa2-nx-reference.jsonl";

/// The budget both arms run at, the seeds the pin covers, and the ceiling at that budget.
/// Pinned as literals rather than read back from the crate: changing one must fail here and
/// force a re-measurement, which a self-referential assertion cannot do.
const GATED_MAX_ITER: u64 = 2;
const PINNED_SEEDS: u64 = 64;
const CEILING: f64 = 1e-7;

fn gates_dir() -> PathBuf {
    std::env::temp_dir().join(format!("gm-cli-fa2-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(&gates_dir(), args, mutate)
}

fn dir(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("gm-cli-fa2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("fixtures dir");
    path.to_str().expect("utf-8").to_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn jsonl(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).expect("fixture file");
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("a line"))
        .collect();
    assert!(!lines.is_empty(), "{} is empty", path.display());
    lines
}

/// `emit-fa2-fixtures --seeds PINNED_SEEDS` into `out`, with `mutate` set or not.
fn emit(out: &str, mutate: Option<(&str, &str)>) -> Vec<Value> {
    let seeds = PINNED_SEEDS.to_string();
    let run = graph_cli(
        &["emit-fa2-fixtures", "--seeds", &seeds, "--out", out],
        mutate,
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}{}",
        stdout(&run),
        stderr(&run)
    );
    jsonl(&Path::new(out).join("fa2.jsonl"))
}

/// The emit writes the budget the differential gates at, and `--max-iter` overrides it:
/// the escape hatch `docs/measurements/fa2-chaos.md` measures the chaos with, and the
/// full-100-iteration comparison that the same doc reports and does not gate.
#[test]
fn the_emit_writes_the_gated_budget_and_max_iter_overrides_it() {
    let honest = dir("budget");
    for line in emit(&honest, None) {
        assert_eq!(
            line["params"]["max_iter"], GATED_MAX_ITER,
            "the gated budget"
        );
        assert_eq!(
            line["params"]["scaling_ratio"], 2.0,
            "the reference's own parameters"
        );
    }
    for budget in ["5", "100"] {
        let out = dir(&format!("sweep{budget}"));
        let run = graph_cli(
            &[
                "emit-fa2-fixtures",
                "--seeds",
                "2",
                "--max-iter",
                budget,
                "--out",
                &out,
            ],
            None,
        );
        assert_eq!(run.status.code(), Some(0), "{}", stderr(&run));
        for line in jsonl(&Path::new(&out).join("fa2.jsonl")) {
            assert_eq!(
                line["params"]["max_iter"],
                budget.parse::<u64>().expect("a budget")
            );
        }
    }
    for bad in ["0", "101", "abc", ""] {
        let out = dir("bad");
        let run = graph_cli(
            &[
                "emit-fa2-fixtures",
                "--seeds",
                "1",
                "--max-iter",
                bad,
                "--out",
                &out,
            ],
            None,
        );
        assert_eq!(
            run.status.code(),
            Some(2),
            "--max-iter {bad:?} must not run"
        );
    }
}

/// The ceiling is the metric, pinned from both sides: the honest port agrees with the
/// pinned networkx arm to well under it, and `oracle-fa2` passes a measured worst equal
/// to the ceiling and fails one a single ulp above. A ceiling nobody can land on, or one
/// a trivially wrong port also passes, would both be worthless.
#[test]
fn the_ceiling_is_pinned_and_oracle_fa2_judges_against_it() {
    let out = dir("ceiling");
    let worst = worst_gap(&emit(&out, None));
    assert!(
        worst <= CEILING,
        "the honest port measured {worst:.3e}, over {CEILING:.0e}"
    );
    let on = oracle_fa2(&out, worst, PINNED_SEEDS);
    assert_eq!(
        on.status.code(),
        Some(0),
        "a worst at the ceiling passes: {}",
        stdout(&on)
    );
    let above = oracle_fa2(&out, f64::from_bits(CEILING.to_bits() + 1), PINNED_SEEDS);
    assert_eq!(
        above.status.code(),
        Some(1),
        "one ulp over the ceiling must fail"
    );
    // The two keys must be able to disagree: 2D one ulp over its ceiling while 3D sits at
    // the measured worst is a state where `fa2` FAILs and `fa2_3d` is ok, so neither
    // verdict rides on the other's number.
    let split = oracle_fa2_split(&out, f64::from_bits(CEILING.to_bits() + 1), worst);
    assert_eq!(split.status.code(), Some(1), "{}", stdout(&split));
    assert!(
        stdout(&split).contains("layout.forceatlas2: "),
        "{}",
        stdout(&split)
    );
    assert!(stdout(&above).contains("FAIL"), "{}", stdout(&above));
    let empty = oracle_fa2(&out, 0.0, 0);
    assert_eq!(
        empty.status.code(),
        Some(1),
        "a result with no compared case fails"
    );
}

/// The negative control, at the gated budget: `GM_MUTATE_FA2_SCALING_RATIO` perturbs
/// `Fa2State::repulsion`'s own variable, and the perturbed port must miss the very same pinned
/// reference by six orders of magnitude over the ceiling. The budget is 2, where a short run
/// could make the gate vacuous, so this asserts the margin and not just the redness.
#[test]
fn a_perturbed_port_goes_red_at_the_gated_budget() {
    let honest_out = dir("control-honest");
    let honest = worst_gap(&emit(&honest_out, None));
    let out = dir("control-mutated");
    let mutated = worst_gap(&emit(&out, Some(("GM_MUTATE_FA2_SCALING_RATIO", "3"))));
    assert!(
        honest <= CEILING,
        "the honest port is already red at {honest:.3e}"
    );
    assert!(
        mutated > CEILING * 1e5,
        "the control is vacuous: {mutated:.3e} is not six decades over {CEILING:.0e}"
    );
    let run = oracle_fa2(&out, mutated, PINNED_SEEDS);
    assert_eq!(run.status.code(), Some(1), "{}", stdout(&run));
    assert!(
        stdout(&run).contains(&format!("{mutated:.3e}")),
        "the failure must name the measured gap: {}",
        stdout(&run)
    );
}

/// `oracle-fa2` on `out` carrying `worst` over `cases` seeds; `cases = 0` is the
/// "over nothing proves nothing" arm.
fn oracle_fa2(out: &str, worst: f64, cases: u64) -> Output {
    write_result(out, worst, worst, cases)
}

/// As [`oracle_fa2`], with a different gap per key — what shows they are judged separately.
fn oracle_fa2_split(out: &str, worst_2d: f64, worst_3d: f64) -> Output {
    write_result(out, worst_2d, worst_3d, PINNED_SEEDS)
}

/// The `<out>/fa2-result.json` both verdicts read, with the fixture set's own fingerprint
/// and digest so only the comparison is under test.
fn write_result(out: &str, worst_2d: f64, worst_3d: f64, cases: u64) -> Output {
    let path = Path::new(out).join("fa2-manifest.json");
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("manifest"))
        .expect("manifest json");
    let result = serde_json::json!({
        "fingerprint": manifest["fingerprint"],
        "sha256": manifest["sha256"]["fa2.jsonl"],
        "oracle": "networkx 3.6 forceatlas2_layout, pinned in the test",
        "layouts": {
            "fa2": { "cases": cases, "worst": worst_2d },
            "fa2_3d": { "cases": cases, "worst": worst_3d },
        },
    });
    let result = serde_json::to_string_pretty(&result).expect("result json") + "\n";
    std::fs::write(Path::new(out).join("fa2-result.json"), result).expect("write result");
    graph_cli(&["oracle-fa2", "--dir", out], None)
}

/// The worst `max |ours - theirs| / extent` over the fixture lines, against the pinned
/// networkx arm: the harness's own metric (`harness/oracle-fa2.py`), so the control is
/// measured the way the gate measures and not by a second definition of "close".
fn worst_gap(lines: &[Value]) -> f64 {
    let reference = jsonl(&Path::new(env!("CARGO_MANIFEST_DIR")).join(NX_REFERENCE));
    assert_eq!(
        reference.len() as u64,
        PINNED_SEEDS,
        "the pin's seed count moved"
    );
    let mut worst: f64 = 0.0;
    for line in lines {
        let seed = line["seed"].as_u64().expect("a seed");
        let theirs = &reference[seed as usize];
        assert_eq!(
            theirs["n"], line["n"],
            "pin and fixture disagree at seed {seed}"
        );
        assert_eq!(
            theirs["max_iter"], line["params"]["max_iter"],
            "pinned at another budget"
        );
        worst = worst.max(gap(&line["fa2"], theirs));
    }
    assert!(
        worst > 0.0,
        "a gap of exactly zero would make the ceiling vacuous"
    );
    worst
}

/// `max |ours - theirs|` over both coordinates, over the larger side of the reference's own
/// bounding box: the harness's normalization. Over `["x", "y"]` because the pinned reference
/// is the **2D** arm's; the 3D arm's worst is measured against its own fixture set.
fn gap(ours: &Value, theirs: &Value) -> f64 {
    let mut worst: f64 = 0.0;
    let mut extent: f64 = 0.0;
    for axis in ["x", "y"] {
        let ours = column(ours, axis);
        let theirs = column(theirs, axis);
        assert_eq!(
            ours.len(),
            theirs.len(),
            "a different node count is not a gap"
        );
        let lo = theirs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = theirs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        extent = extent.max(hi - lo);
        for (a, b) in ours.iter().zip(&theirs) {
            worst = worst.max((a - b).abs());
        }
    }
    worst / extent.max(1e-300)
}

fn column(layout: &Value, axis: &str) -> Vec<f64> {
    layout[axis]
        .as_array()
        .unwrap_or_else(|| panic!("{axis} column"))
        .iter()
        .map(|v| v.as_f64().expect("a coordinate"))
        .collect()
}
