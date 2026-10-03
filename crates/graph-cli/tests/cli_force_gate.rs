//! The live force session's gate end to end: native ×2 against wasm32 ×2 over the positions
//! after a fixed number of ticks, and its own negative control going red. `force-gate` is a
//! separate command from `hashgate` on purpose — see `src/forcecheck.rs` — so its row here is
//! its own test rather than another assertion in `cli.rs`.
//!
//! Needs `node` and the `wasm32-unknown-unknown` target, as `cli.rs` does. Each run builds the
//! wasm artifact and runs four arms, so the honest run is at four seeds and the control at two
//! — which is enough, because `GM_MUTATE_FORCE_SESSION_GRAVITY` moves every seed's positions
//! from the first tick (the unit test `the_control_perturbs_every_seed_and_zero_is_the_honest_run`
//! is the statement, and it is measured rather than assumed).

mod common;

use common::stdout;
use std::path::Path;
use std::process::Output;

/// A gates directory per test: these four runs write records under the same names, and the test
/// harness runs them in parallel, so one shared directory would have each test reading another's
/// evidence — a race that shows up as a record asserting the wrong run's verdict.
fn gates_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-force-gate-{name}-{}", std::process::id()))
}

fn graph_cli(dir: &std::path::Path, args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(dir, args, mutate)
}

fn record(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(format!("{name}.json"))).expect("recorded")
}

/// Both streams: a refusal is a reason for the caller, and graph-cli writes reasons to standard
/// error (`cli.rs`'s tests read them the same way).
fn said(output: &Output) -> String {
    format!(
        "{}{}",
        stdout(output),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The honest run: every arm agrees on every seed, native against wasm32. Without this the
/// gate's red would be unfalsifiable — a comparison of nothing also "diverges".
#[test]
fn the_live_session_is_four_way_hash_equal_across_targets() {
    let dir = gates_dir("honest");
    let run = graph_cli(&dir, &["force-gate", "--seeds", "4"], None);
    let out = stdout(&run);
    assert_eq!(run.status.code(), Some(0), "{out}");
    assert!(
        out.contains("  force.session.positions: 4-way equal on 4/4 seeds"),
        "{out}"
    );
    assert!(out.contains("PASS"), "{out}");
    // One digest per arm, and all four the same value: "4-way equal on 4/4 seeds" is the claim,
    // these four lines are what backs it.
    let digests: Vec<&str> = out
        .lines()
        .filter_map(|l| l.split("digest ").nth(1))
        .collect();
    assert_eq!(digests.len(), 4, "one digest per arm: {out}");
    assert!(
        digests.windows(2).all(|pair| pair[0] == pair[1]),
        "all four arms agree: {out}"
    );

    let recorded = record(&dir, "force-gate");
    assert!(recorded.contains("\"pass\": true"), "{recorded}");
    assert!(
        recorded.contains("\"force.session.positions\": 4"),
        "{recorded}"
    );
    assert!(recorded.contains("\"mutation\": null"), "{recorded}");
    // The record must say which bytes and how many ticks, or "equal" counts an agreement about
    // nothing in particular.
    assert!(recorded.contains("\"ticks\": 50"), "{recorded}");
    assert!(recorded.contains("little-endian f64"), "{recorded}");
}

/// The negative control: a non-zero `gravity` the wasm arm cannot see, so the two targets must
/// disagree — on **every** seed, and only on this gate's stage (there is only one).
#[test]
fn the_live_sessions_own_control_goes_red() {
    let dir = gates_dir("control");
    let run = graph_cli(
        &dir,
        &["force-gate", "--seeds", "2"],
        Some(("GM_MUTATE_FORCE_SESSION_GRAVITY", "0.5")),
    );
    let out = stdout(&run);
    assert_eq!(run.status.code(), Some(1), "{out}");
    assert!(
        out.contains("  force.session.positions: 4-way equal on 0/2 seeds"),
        "{out}"
    );
    assert!(out.contains("FAIL: 2 of 2 seeds diverge"), "{out}");
    // The two native runs agree with each other under the mutation, which is what makes this a
    // cross-target divergence and not a run that simply fell over.
    assert!(out.contains("DIVERGED force.session.positions"), "{out}");

    let recorded = record(&dir, "forcegate-control-force-session-gravity");
    assert!(recorded.contains("\"pass\": false"), "{recorded}");
    assert!(
        recorded.contains("\"mutation\": \"GM_MUTATE_FORCE_SESSION_GRAVITY\""),
        "{recorded}"
    );
    assert!(
        recorded.contains("\"force.session.positions\": 0"),
        "{recorded}"
    );
}

/// A control that cannot reach a live session is **refused**, not run: every other knob
/// perturbs the frozen pipeline, which this gate does not hash, so running one here would
/// produce the honest bytes, agree everywhere and exit 0 — a vacuous pass wearing a control's
/// name. Exit 2 is "could not run", which is the truth.
#[test]
fn a_control_that_cannot_reach_the_session_refuses_the_run() {
    let dir = gates_dir("refused");
    for knob in [
        "GM_MUTATE_FORCE_THETA",
        "GM_MUTATE_NODE_COUNT",
        "GM_MUTATE_SPLIT_SUM",
    ] {
        let run = graph_cli(&dir, &["force-gate", "--seeds", "2"], Some((knob, "1")));
        let out = said(&run);
        assert_eq!(run.status.code(), Some(2), "{knob}=1 must not run: {out}");
        assert!(
            out.contains(knob),
            "{knob} is not named in the refusal: {out}"
        );
    }
}

/// **RG-42: `0` is the honest run's own value, so it is refused as a control.** It skips the
/// force - gravity off *is* the frozen session - which is exactly why it cannot be written
/// into the control variable: the run would hash the honest bytes, agree everywhere, exit 0,
/// and write `forcegate-control-force-session-gravity` claiming the control had been
/// exercised. Exit 2 is "could not run", which is the truth, and no record is left behind.
///
/// This test used to assert the opposite - that `=0` exits 0 and records a pass - which is
/// precisely the defect RG-42 names. Zero is still a legal *gravity*; it is just not
/// expressible *as a control*, and the knob's own unit tests
/// (`hashgate::knob::setting::tests::no_op_controls_are_refused`) hold that half.
#[test]
fn a_zero_gravity_is_refused_as_a_no_op_control_and_records_nothing() {
    let dir = gates_dir("zero");
    let run = graph_cli(
        &dir,
        &["force-gate", "--seeds", "2"],
        Some(("GM_MUTATE_FORCE_SESSION_GRAVITY", "0")),
    );
    let out = said(&run);
    assert_eq!(run.status.code(), Some(2), "{out}");
    assert!(
        out.contains("GM_MUTATE_FORCE_SESSION_GRAVITY"),
        "the refusal must name the argument: {out}"
    );
    assert!(out.contains("perturbs nothing"), "{out}");
    assert!(
        !Path::new(&dir)
            .join("forcegate-control-force-session-gravity.json")
            .exists(),
        "a refused control must not leave a record standing as the exercised control"
    );
    // A real gravity still runs: `the_control_perturbs_every_seed` above is that half, and
    // its exit 1 is what says the refusal above is about the value and not the knob.
}
