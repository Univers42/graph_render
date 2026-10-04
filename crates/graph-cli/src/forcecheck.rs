//! `force-gate`: the live force session's own cross-target hash gate.
//!
//! The 4-way hash gate ([`crate::hashgate`]) hashes the *frozen* pipeline's snapshots, which is
//! 112 ticks of a default session run to a finished picture. This gate hashes the other thing
//! graph-core owns: **a live session driven tick by tick over the wire** —
//! `gm_seed_ingest → gm_alloc → gm_build → gm_force_session_create → gm_force_session_tick` —
//! and reads the positions back through `gm_force_session_column_ptr`/`_len`.
//!
//! **Same shape, one stage, reused comparator.** Native ×2 against wasm32 ×2, each arm its own
//! process (so no allocator address or hash seed leaks between them), compared line for line by
//! [`hashgate::compare`] — the same refusal set, including "every seed hashed alike is one input
//! tested N times", which is what keeps a control that cannot bite from passing vacuously.
//! Deliberately *not* folded into the hash gate's stage list: that gate's arms are built for a
//! per-stage report over the whole registry, and this one's wasm arm is a different script, so
//! adding a stage there would have meant threading a second runner through the first one.
//!
//! ## The bytes
//!
//! One stage, [`STAGE`], whose hash is over the `x` column then the `y` column's `f64` bits
//! after [`TICKS`] ticks, in row order. Not a snapshot's binary face: a live session has no
//! snapshot, and these are the only two `f64` columns in the ABI — narrowing them to `f32` to
//! match the transport would test a conversion instead of the tick.
//!
//! ## The negative control
//!
//! [`Knob::ForceSessionGravity`], which perturbs the native arm's live `gravity` and the wasm
//! arm cannot see it — the same shape as every other control, and the only one that can reach a
//! session (nothing else in the list touches `LiveParams`; see that arm's doc). A knob that
//! does not reach it is **refused rather than ignored**, because a control that perturbs
//! nothing exits 0 and reads as evidence.

mod native;
pub mod stream;

#[cfg(test)]
mod tests;

use crate::evidence;
use crate::hashgate::knob::Setting;
use crate::hashgate::{self, Knob, compare, report};
use crate::runner::{build_wasm, file_sha256, run_lines, sha256_hex};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The one stage this gate hashes: the live session's two position columns after [`TICKS`].
///
/// Named `force.session.*` for the same reason the hash gate's transport stage is
/// `transport.wasm.columnar` — a dotted id that says what ran, and which arm ran it, rather
/// than a bare "positions".
pub const STAGE: &str = "force.session.positions";

/// How many ticks the session runs before its positions are hashed.
///
/// **A fixed 50, on both arms, passed to the wasm arm as an argument** so the number is written
/// once: a hard-coded 50 in the Node arm next to one in Rust is two numbers free to disagree,
/// and a disagreement there would read as a determinism failure. 50 is enough that every seed's
/// layout has visibly moved (the golden spiral is gone by a handful of ticks) and few enough
/// that the arm is cheap at 600 nodes.
pub const TICKS: u32 = 50;

/// The stage list both arms print in: one stage, so the gate's report reads like the hash
/// gate's rather than like a special case.
pub const STAGES: usize = 1;

/// The one stage, as a list, for [`compare::diverged`] — which takes a slice so its own tests
/// can work at a small fixed size independent of the real stage count.
pub fn stages() -> [&'static str; STAGES] {
    [STAGE]
}

/// Runs every arm over seeds `0..seeds` and compares them line by line, then runs every arm
/// over the stream fixtures and compares those.
///
/// Both stages in one run and one exit code, because a gate that hashed the stream on a
/// different day would be two gates, and the one that ran is the one anybody reads.
pub fn run(seeds: u32) -> ExitCode {
    let started = env_setting().and_then(|setting| {
        refuse_a_control_that_cannot_bite(setting.control())?;
        let stamp = evidence::Stamp::take()?;
        Ok((setting.control(), stamp, collect_arms(seeds)?))
    });
    match started {
        Ok((control, stamp, arms)) => report(&stamp, control, seeds, &arms),
        Err(err) => {
            eprintln!("force-gate: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// Body of the hidden `force-gate-arm` subcommand: one native run, printing
/// `stage seed sha256` lines in the same order `hashgate`'s own arm does — stage-major,
/// seed-minor, which is what [`compare::diverged`] reads.
pub fn arm(seeds: u32) -> ExitCode {
    match env_setting().and_then(|setting| arm_lines(seeds, &setting)) {
        Ok(lines) => {
            print!("{lines}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("force-gate-arm: {err}");
            ExitCode::from(2)
        }
    }
}

fn arm_lines(seeds: u32, setting: &Setting) -> Result<String, String> {
    let mut out = String::new();
    for seed in 0..seeds {
        let bytes =
            native::positions(seed, setting).map_err(|err| format!("seed {seed}: {err}"))?;
        out.push_str(&format!("{STAGE} {seed} {}\n", sha256_hex(&bytes)));
    }
    Ok(out)
}

/// Native ×2 and wasm32 ×2, each arm its own process. Two runs per target, exactly as the hash
/// gate does it: run-to-run equality is half of what D7 claims, and a single run per target
/// would compare the two targets while saying nothing about reproducibility within one.
fn collect_arms(seeds: u32) -> Result<stream::arm::Collected, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm(&[])?;
    let count = seeds.to_string();
    let native = || run_lines(Command::new(&exe).args(["force-gate-arm", "--seeds", &count]));
    let wasm32 = || run_lines(wasm_arm(&wasm).args([&count, &TICKS.to_string()]));
    let seed = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    println!("force-gate: {} arms", seed.len());
    println!(
        "force-gate: wasm artifact {} sha256 {}",
        wasm.display(),
        file_sha256(&wasm)?
    );
    let mut collected = stream::arm::collect(&exe, &wasm)?;
    collected.seed = seed;
    Ok(collected)
}

/// `node <this crate's arm.mjs> <wasm>`, ready for the seed count and the tick count.
///
/// The runner lives inside the crate rather than in `harness/` beside the hash gate's, because
/// this arm is the only consumer of it: the hash gate's `wasm-run.mjs` resolves its stages out
/// of the module's own registries and would have had to learn a fourth kind of stage to grow
/// this one. Two scripts, one shape, no shared registry guessing.
fn wasm_arm(wasm: &Path) -> Command {
    let mut command = Command::new("node");
    command.arg(arm_script());
    command.arg(wasm);
    command
}

/// The Node arm's script, resolved from this crate's own source directory so a build that runs
/// from anywhere still finds it (the hash gate's runner resolves `harness/` the same way).
pub fn arm_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("forcecheck")
        .join("arm.mjs")
}

fn env_setting() -> Result<Setting, String> {
    hashgate::env_setting()
}

/// **A control that cannot bite refuses the run; it does not pass it.**
///
/// Only [`Knob::ForceSessionGravity`] reaches a live session, and every other variable in
/// [`Knob::ALL`] perturbs a stage this gate does not hash — so a run with one of them set would
/// produce the honest bytes, agree on every seed and exit **0**: a vacuous pass, which is the
/// one failure mode a negative control must not have (`hashgate.rs`'s
/// `refuse_a_vacuous_control` is the same rule for the same reason).
fn refuse_a_control_that_cannot_bite(control: Option<Knob>) -> Result<(), String> {
    match control {
        None | Some(Knob::ForceSessionGravity) | Some(Knob::DropDelta) => Ok(()),
        Some(other) => Err(format!(
            "{} does not reach the force session: it perturbs the frozen pipeline, whose stages \
             this gate does not hash. Run the hash gate for that control, or use {} here.",
            other.env(),
            Knob::ForceSessionGravity.env()
        )),
    }
}

/// The hash is over `f64` bits, so the record says which bytes: without it, `equal` counts an
/// agreement about nothing in particular.
const HASHED: &str = "x then y, little-endian f64, in row order";

fn report(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    arms: &stream::arm::Collected,
) -> ExitCode {
    let mutation = control.map_or("none", Knob::env);
    println!(
        "force-gate: stage={STAGE} ticks={TICKS} seeds={seeds} control={mutation} hashed={HASHED}"
    );
    let lines = match compare::diverged(seeds, &stages(), &arms.seed) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("force-gate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    let mut detail = String::new();
    report::arm_report(&mut detail, &arms.seed, &lines);
    print!("{detail}");
    let tally = compare::per_stage(seeds, STAGES, &lines);
    let ways = arms.seed.len();
    println!(
        "  {STAGE}: {ways}-way equal on {}/{} seeds",
        tally.equal[0], seeds
    );
    let stream = stream::arm::report(arms);
    // A stream that cannot be compared is "could not run", like the seed arms above: no record.
    if let Some(text) = &stream.refusal {
        eprintln!("force-gate: stream arms not comparable: {text}");
        return ExitCode::from(2);
    }
    if let Err(err) = record(stamp, control, seeds, &tally, arms) {
        eprintln!("force-gate: not recorded: {err}");
        return ExitCode::from(2);
    }
    verdict(tally.diverged_seeds, seeds, stream.diverged)
}

fn verdict(diverged_seeds: u32, seeds: u32, stream_diverged: bool) -> ExitCode {
    if diverged_seeds != 0 {
        println!("FAIL: {diverged_seeds} of {seeds} seeds diverge");
    }
    if diverged_seeds != 0 || stream_diverged {
        return ExitCode::from(1);
    }
    println!("PASS");
    ExitCode::SUCCESS
}

/// The record for the ledger: the same keys `hashgate` writes, so
/// `capabilities::verdict`'s control reader can read this one too (it looks a control up by
/// its own name and asks whether it went red on a stage — a stage of *this* gate, so it says
/// "did not go red on the layout.grid stage" for the hash gate's rows, which is the truth).
fn record(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &compare::Tally,
    arms: &stream::arm::Collected,
) -> Result<(), String> {
    let name = control.map_or("force-gate", Knob::record);
    let stages: serde_json::Map<_, _> = stages()
        .iter()
        .zip(&tally.equal)
        .map(|(stage, equal)| ((*stage).to_owned(), json!(equal)))
        .collect();
    evidence::record(
        stamp,
        name,
        json!({
            "seeds": seeds,
            "ticks": TICKS,
            "stage": STAGE,
            "hashed": HASHED,
            "arms": arms.seed.len(),
            "arm_names": arms.seed.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "pass": tally.diverged_seeds == 0,
            "equal": stages,
            "mutation": control.map(Knob::env),
            "stream": {
                "stage": stream::STREAM_STAGE,
                "fixtures": stream::FIXTURES,
                "batches": arms.batches,
                "arms": arms.stream.len(),
                "diverged": arms.diverged.is_some(),
                "first_divergence": arms.diverged,
            },
        }),
    )
}
