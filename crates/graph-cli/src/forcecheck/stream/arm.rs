//! How the stream stage's four arms are collected and how the stage reports itself.
//!
//! Split out of [`super`] by the house's 300-line limit: the seed stage's arms live beside
//! the seed stage, and these are the stream stage's, and the two must not grow into one
//! module that reads as one gate.

use super::super::TICKS;
use super::{FIXTURES, batch_count, first_divergence, fixture_path, stream_script};
use crate::hashgate::compare::Arm;
use crate::runner::run_lines;
use std::path::Path;
use std::process::Command;

/// Both stages' arms, and the stream stage's verdict on them.
///
/// One struct rather than two arguments because `forcecheck::report` is at the house's
/// four-parameter limit already, and the stream result belongs beside the arms it was
/// computed from rather than as a fifth argument beside it.
pub struct Collected {
    /// The seed stage: one line per seed, for the rectangular matrix the hash gate's
    /// comparator takes.
    pub seed: Vec<Arm>,
    /// The stream stage: one line per fixture batch, ragged, compared by
    /// [`first_divergence`].
    pub stream: Vec<Arm>,
    /// The first stream batch any arm disagreed on, or `None`. Filled by [`compare`].
    pub diverged: Option<String>,
    /// How many batch lines the arms printed, for the stage's own count.
    pub batches: u32,
}

/// What the stream stage reported, and the refusal that stops the gate being read as a pass.
pub struct StreamReport {
    /// Whether any arm disagreed on any batch.
    pub diverged: bool,
    /// Why the arms could not be compared, if they could not. `Some` here is exit 2, never a
    /// divergence: an arm that could not run is not an arm that agreed.
    pub refusal: Option<String>,
}

/// Runs the stream stage's four arms and compares them.
///
/// Native ×2 and wasm32 ×2, each arm its own process — the same four as the seed stage and
/// for the same reason: run-to-run equality is half of what D7 claims, and one run per target
/// would compare the two targets while saying nothing about reproducibility within one.
pub fn collect(exe: &Path, wasm: &Path) -> Result<Collected, String> {
    let native = || run_lines(Command::new(exe).arg("force-gate-stream-arm"));
    let wasm32 = || run_lines(wasm_arm(wasm).args(arguments()));
    let stream = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    let (diverged, batches) = compare(&stream);
    Ok(Collected {
        seed: Vec::new(),
        stream,
        diverged,
        batches,
    })
}

/// The Node arm's arguments after the wasm path: the tick count, then the fixtures in
/// [`FIXTURES`] order. The tick count goes first so both of this crate's `.mjs` arms
/// take their counts before their inputs.
fn arguments() -> Vec<String> {
    let mut out = vec![TICKS.to_string()];
    out.extend(
        FIXTURES
            .iter()
            .map(|name| fixture_path(name).display().to_string()),
    );
    out
}

/// `node <this crate's stream-arm.mjs> <wasm>`, ready for [`arguments`].
///
/// A *different* script from the seed stage's, deliberately: it drives `gm_graph_extend` and
/// `gm_force_session_grow`, which the seed stage's script never calls, so one script covering
/// both stages would be one script with two modes and one set of failure messages.
fn wasm_arm(wasm: &Path) -> Command {
    let mut command = Command::new("node");
    command.arg(stream_script());
    command.arg(wasm);
    command
}

/// The stage's verdict, and the batch count it was over: `Ok(None)` is equality, which is what
/// makes the report say "stream equal" rather than stay silent.
fn compare(stream: &[Arm]) -> (Option<String>, u32) {
    match first_divergence(stream) {
        Ok(diverged) => (diverged, batch_count(stream)),
        // An incomparable arm is a stage that could not run, not a stage that passed. It is
        // held as a *divergence* here and `report` turns it into exit 2 by name.
        Err(err) => (Some(format!("{INCOMPARABLE}{err}")), 0),
    }
}

/// The marker [`compare`] prefixes a refusal with, so `report` can tell a stage that diverged
/// from a stage that could not be compared. A fixture name can never carry it: the names are
/// the three constants [`FIXTURES`] holds.
const INCOMPARABLE: &str = "arms not comparable: ";

/// Prints the stage's own line and says whether it diverged.
///
/// The wording is what the negative control is read against, so it names the batch and the
/// arm: `force-gate: stream diverged at stream-small batch 2 (wasm32 run 1 differs from
/// native run 1)`.
pub fn report(arms: &Collected) -> StreamReport {
    let Some(text) = &arms.diverged else {
        println!(
            "force-gate: stream equal, {} batches x {} arms",
            arms.batches,
            arms.stream.len()
        );
        return StreamReport {
            diverged: false,
            refusal: None,
        };
    };
    let refusal = text.strip_prefix(INCOMPARABLE).map(str::to_owned);
    if refusal.is_none() {
        println!("force-gate: stream diverged at {text}");
    }
    StreamReport {
        diverged: true,
        refusal,
    }
}
