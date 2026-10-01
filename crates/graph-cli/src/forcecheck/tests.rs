//! What the force gate holds, natively and without Node: that its native arm really is a live
//! session stepped [`TICKS`](super::TICKS) times, that its one control reaches it, that a
//! control which cannot reach it is refused rather than run vacuously, and that the Node arm's
//! script cannot drift from the constants on this side.
//!
//! The cross-target comparison itself is the gate command's own work (`force-gate` runs four
//! arms and compares them); what is checked here is everything that comparison rests on, so a
//! failure names which of the two arms is wrong instead of reporting "diverged".

use super::*;
use crate::hashgate::knob::setting::setting;
use graph_core::layout::force::{ForceSession, LiveParams};
use std::env::VarError;

/// The honest setting: no knob set, so `live_force_params` is the frozen set.
fn honest() -> Setting {
    setting(|_| Err(VarError::NotPresent)).expect("no knobs set is not an error")
}

/// The [`Setting`] `env` alone, for a variable only known at run time.
fn setting_for(name: &'static str, value: &'static str) -> Result<Setting, String> {
    setting(|read| match read == name {
        true => Ok(value.to_owned()),
        false => Err(VarError::NotPresent),
    })
}

/// The gate's native arm is `ForceSession::new` + `step(TICKS)` + the two columns' bytes — not a
/// second simulation that happens to agree. This compares it against a session built and
/// stepped here, bit for bit, through the same `LiveParams`.
#[test]
fn the_native_arm_is_the_session_stepped_exactly_ticks_times() {
    let seed = 3;
    let params = LiveParams::default();
    let mut by_hand =
        ForceSession::new(&native::topology(seed).expect("model"), params).expect("in range");
    by_hand.step(TICKS);

    let mut out = Vec::new();
    for column in [by_hand.xs(), by_hand.ys()] {
        for value in column {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(
        native::positions(seed, &honest()).expect("runs"),
        out,
        "the hashed bytes are x then y, little-endian, after {TICKS} ticks"
    );
}

/// The byte stream is `2 * nodes * 8` long and every node really moved: a gate over a stream
/// that stayed at the seed's positions would hash the same constant on every seed, which
/// [`compare::diverged`] refuses as "one input tested N times" — better to know here.
#[test]
fn every_seed_hashes_bytes_that_reach_its_own_positions() {
    let seen: Vec<String> = (0..4)
        .map(|seed| crate::runner::sha256_hex(&native::positions(seed, &honest()).expect("runs")))
        .collect();
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), seen.len(), "two seeds hashed alike: {seen:?}");
    for seed in 0..4 {
        let bytes = native::positions(seed, &honest()).expect("runs");
        let nodes = graph_core::gate_node_count(seed) as usize;
        assert_eq!(
            bytes.len(),
            16 * nodes,
            "seed {seed}: two f64 columns of {nodes} nodes"
        );
        assert!(
            bytes
                .as_chunks::<8>()
                .0
                .iter()
                .any(|w| *w != 0.0f64.to_le_bytes()),
            "seed {seed}: no coordinate moved off zero"
        );
    }
}

/// The control: a non-zero gravity pulls every node toward the origin, so every seed's bytes
/// must move — this is what makes `GM_MUTATE_FORCE_SESSION_GRAVITY` a control rather than a
/// variable nothing reads.
#[test]
fn the_control_perturbs_every_seed_and_zero_is_the_honest_run() {
    let honest_bytes: Vec<Vec<u8>> = (0..4)
        .map(|seed| native::positions(seed, &honest()).expect("runs"))
        .collect();
    for value in ["0.5", "1", "0.25"] {
        let moved = setting_for(Knob::ForceSessionGravity.env(), value).expect("parses");
        assert_eq!(
            moved.control(),
            Some(Knob::ForceSessionGravity),
            "the control"
        );
        for (seed, honest) in honest_bytes.iter().enumerate() {
            assert_ne!(
                &native::positions(seed as u32, &moved).expect("runs"),
                honest,
                "{value} did not move seed {seed}"
            );
        }
    }
    // Zero is a real gravity — the force is skipped — and it is the honest run's value, so a row
    // that sets it must produce the honest bytes rather than an error (and rather than a
    // vacuous pass with a different meaning).
    let zero = setting_for(Knob::ForceSessionGravity.env(), "0").expect("parses");
    assert_eq!(
        zero.live_force_params().gravity,
        0.0,
        "zero is a real gravity"
    );
    assert_eq!(zero.live_force_params().gravity, 0.0);
    for seed in 0..2 {
        assert_eq!(
            native::positions(seed, &zero).expect("runs"),
            native::positions(seed, &honest()).expect("runs"),
            "gravity 0 is the honest run"
        );
    }
}

/// A typo is refused rather than read as the default — a control whose value failed to parse
/// would perturb nothing and pass.
#[test]
fn the_control_is_parsed_rather_than_treated_as_a_flag() {
    for value in ["maybe", "", "0.5 0.5"] {
        let err = setting_for(Knob::ForceSessionGravity.env(), value)
            .expect_err("a typo is not a gravity");
        assert!(
            err.starts_with(Knob::ForceSessionGravity.env()),
            "{value:?}: {err}"
        );
    }
}

/// **A control that cannot bite refuses the run; it does not pass it.** Every other knob
/// perturbs the frozen pipeline, which this gate does not hash, so running one here would
/// produce the honest bytes and exit 0 — a vacuous pass.
#[test]
fn a_control_that_cannot_reach_the_session_is_refused() {
    assert_eq!(refuse_a_control_that_cannot_bite(None), Ok(()));
    let own = setting_for(Knob::ForceSessionGravity.env(), "0.5").expect("parses");
    assert_eq!(
        refuse_a_control_that_cannot_bite(own.control()),
        Ok(()),
        "its own control runs"
    );
    for name in [
        "GM_MUTATE_FORCE_THETA",
        "GM_MUTATE_NODE_COUNT",
        "GM_MUTATE_SPLIT_SUM",
        "GM_MUTATE_GRID_SPACING",
    ] {
        let knob = crate::hashgate::Knob::ALL
            .iter()
            .find(|knob| knob.env() == name)
            .copied();
        assert_eq!(
            refuse_a_control_that_cannot_bite(knob),
            Err(format!(
                "{name} does not reach the force session: it perturbs the frozen pipeline, whose \
                 stages this gate does not hash. Run the hash gate for that control, or use \
                 GM_MUTATE_FORCE_SESSION_GRAVITY here."
            )),
            "{name} must be refused, not run vacuously"
        );
    }
}

/// The Node arm is a separate script in another language, and the two agree on three literals:
/// the stage id, and that the gate passes the seed and tick counts to it rather than baking
/// them in. The tick count is the one that matters — two copies of "50" in two languages would
/// diverge as a determinism failure with nothing to point at.
#[test]
fn the_wasm_arm_script_agrees_with_this_crate() {
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("forcecheck")
            .join("arm.mjs"),
    )
    .expect("the arm script is committed beside the module that runs it");
    assert!(
        script.contains(&format!("const STAGE = \"{STAGE}\";")),
        "the stage id must be written identically in both languages"
    );
    assert!(
        script.contains("process.argv.slice(2)")
            && script.contains("usage: arm.mjs <wasm> <seeds> <ticks>"),
        "the arm takes its seed and tick counts as arguments"
    );
    assert!(
        !script.contains(&format!("{TICKS}")),
        "the tick count must be passed in, never written into the script"
    );
    for name in [
        "gm_force_session_create",
        "gm_force_session_tick",
        "gm_force_session_column_ptr",
        "gm_force_session_release",
    ] {
        assert!(script.contains(name), "{name} is missing from the arm");
    }
}

/// The gate is one stage, and its name says what ran — the hash gate's transport stage sets the
/// naming (`transport.wasm.columnar`), and a stage id is what a divergence is reported against.
#[test]
fn the_gate_hashes_one_named_stage() {
    assert_eq!(stages(), [STAGE]);
    assert_eq!(STAGE, "force.session.positions");
    // A const assertion, because that is what it is: the tick count is a compile-time decision
    // and the test says so at compile time too.
    const {
        assert!(
            TICKS >= 4,
            "a handful of ticks must be enough to leave the seed spiral"
        )
    };
}
