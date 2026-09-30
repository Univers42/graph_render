//! What an arm *prints*: the line order, the threaded arm's claim to be the scalar arm's
//! lines verbatim, and the C20 tally read off the wasm arm.

use super::super::knob::Setting;
use super::super::knob::setting::setting;
use super::super::stages::stage_bytes;
use super::super::stages::stages as stage_ids;
use super::super::transport;
use super::super::{
    FORCE_STAGES, LAYOUT, TRANSPORT, arm_lines, stage_bytes_threaded, threads_lines,
};
use super::env;
use super::honest;
use graph_core::GridParams;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, Split, YifanHu};

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

/// **The negative control for the threaded arm, at the seam the arm is built from.**
///
/// `GM_MUTATE_SPLIT_SUM` reaches the threaded arm only through the recompute in
/// `stage_bytes_threaded`. So with the control on, the arm's bytes must differ from the
/// scalar run's on **both** force stages and on nothing else — and the ids that moved must
/// be exactly the ones the arm recomputes. Without this, "10-way equal" on a force stage
/// whose id was never in the match is a comparison of a run with itself, and the gate is
/// green on a stage it never ran threaded.
#[test]
fn the_split_control_moves_both_force_stages_and_nothing_else() {
    let split = setting(env(vec![("GM_MUTATE_SPLIT_SUM", "1")])).expect("parses");
    assert_eq!(split.split_sum, Split::All);
    let seed = 8_u32;
    let scalar = stage_bytes(seed, &honest()).expect("runs");
    let threaded = stage_bytes_threaded(seed, &split, 4).expect("runs");
    assert_eq!(
        scalar.len(),
        threaded.len(),
        "one line per stage, either way"
    );
    let mut moved: Vec<&str> = Vec::new();
    for ((id, a), (_, b)) in scalar.iter().zip(&threaded) {
        if a != b {
            moved.push(id);
        }
    }
    // Spelled out, **not** read from `FORCE_STAGES`: a test that checked the recompute
    // against the very list that decides the recompute would pass with the list holding
    // one id, which is the vacuous case this test exists to catch.
    assert_eq!(
        moved,
        vec![BarnesHut::ID, YifanHu::ID],
        "the control must move both force stages and no other stage — a stage missing from \
         the match reuses the scalar bytes, so its equality is vacuous"
    );
    assert_eq!(
        FORCE_STAGES.as_slice(),
        [BarnesHut::ID, YifanHu::ID],
        "the arm's recompute list and this test must name the same two stages: a stage in one \
         and not the other is recomputed but unclaimed, or claimed but never compared"
    );
    // And with the control off the recompute is byte-identical, at every gated width: that
    // equality is the claim `--tiers all` rests on, and this is the same recompute.
    for workers in super::tier::WORKER_COUNTS {
        assert_eq!(
            stage_bytes_threaded(seed, &honest(), workers).expect("runs"),
            scalar,
            "workers={workers}: the threaded arm is not the scalar run's bytes"
        );
    }
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
