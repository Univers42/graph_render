//! What an arm *prints*: the line order, the threaded arm's claim to be the scalar arm's
//! lines verbatim, and the C20 tally read off the wasm arm.

use super::super::knob::Setting;
use super::super::stages::stages as stage_ids;
use super::super::transport;
use super::super::{LAYOUT, TRANSPORT, arm_lines, threads_lines};
use super::honest;
use graph_core::GridParams;

#[test]
fn an_arm_prints_every_seed_of_one_stage_before_the_next() {
    let lines = arm_lines(2, &honest()).expect("runs");
    let prefixes: Vec<_> = lines
        .lines()
        .map(|l| l.rsplit_once(' ').expect("digest").0)
        .collect();
    let expected: Vec<String> = stage_ids()
        .iter()
        .flat_map(|stage| (0..2).map(move |seed| format!("{stage} {seed}")))
        .collect();
    assert_eq!(prefixes, expected);
    let refused = Setting {
        grid: GridParams { spacing: -1.0 },
        ..honest()
    };
    assert!(
        arm_lines(2, &refused)
            .expect_err("refused")
            .starts_with("seed 0: ")
    );
}

/// A wasm arm's lines over the three stages the C20 tally reads, 2 seeds each.
fn transport_arm(fill: char) -> Vec<String> {
    ["topology", LAYOUT, TRANSPORT]
        .iter()
        .flat_map(|stage| {
            (0..2).map(move |seed| format!("{stage} {seed} {}", fill.to_string().repeat(64)))
        })
        .collect()
}

/// The threaded arm and the scalar arm must be **the same list of lines**, in the same
/// order — not merely equal digests.
///
/// This is the test that a bug in `threads_lines` actually caught while this slice was
/// being written: the arm was built seed-major while the comparison reads stage-major, so
/// the gate refused it as malformed at line 1 rather than comparing anything. A test that
/// only compared the *sets* of lines would have passed that arm, and the gate would still
/// have been red — but the failure would have been "not comparable" (exit 2) instead of the
/// "equal" the arm claims.
#[test]
fn the_threaded_arm_prints_its_stages_in_the_same_order_as_the_scalar_one() {
    let setting = honest();
    let scalar: Vec<String> = arm_lines(2, &setting)
        .expect("runs")
        .lines()
        .map(str::to_owned)
        .collect();
    for workers in super::tier::WORKER_COUNTS {
        let threaded = threads_lines(2, &setting, workers).expect("runs");
        assert_eq!(
            threaded.len(),
            scalar.len(),
            "workers={workers}: wrong line count"
        );
        assert_eq!(
            threaded, scalar,
            "workers={workers}: the threaded arm is not the scalar arm's lines"
        );
    }
    // And the order is stage-major: line 0 and line 1 are the same stage, two seeds.
    let prefixes: Vec<&str> = scalar
        .iter()
        .map(|line| line.rsplit_once(' ').expect("digest").0)
        .collect();
    assert_eq!(prefixes[0], format!("{} 0", stage_ids()[0]));
    assert_eq!(prefixes[1], format!("{} 1", stage_ids()[0]));
    assert_eq!(prefixes[2], format!("{} 0", stage_ids()[1]));
}

#[test]
fn the_transport_tally_counts_the_seeds_where_the_real_abi_matches_the_shim() {
    let wasm = |fill: char| transport_arm(fill);
    assert_eq!(transport::agree_with_shim(2, &wasm('a')), Ok(2));
    let mut diverged_at_1 = wasm('a');
    diverged_at_1[5] = format!("{TRANSPORT} 1 {}", "b".repeat(64));
    assert_eq!(transport::agree_with_shim(2, &diverged_at_1), Ok(1));
    assert!(transport::agree_with_shim(2, &wasm('a')[..4]).is_err());
    let mut no_transport = wasm('a');
    no_transport[4] = no_transport[4].replace(TRANSPORT, "topology");
    assert!(transport::agree_with_shim(2, &no_transport).is_err());
    let mut no_layout = wasm('a');
    no_layout[2] = no_layout[2].replace(LAYOUT, "topology");
    assert!(transport::agree_with_shim(2, &no_layout).is_err());
    assert!(transport::agree_with_shim(0, &[]).is_err());
}
