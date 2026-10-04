//! The force gate's **stream stage**: the same four arms, run over a graph that arrives in
//! batches rather than all at once.
//!
//! The seed stage hashes `TICKS` of a session over a fixed model. This one builds line 0 of
//! a fixture, then per batch appends the line through [`graph_wasm::service::extend`],
//! grows the live session onto the new topology with [`ForceSession::grow`], steps `TICKS`
//! and hashes the two position columns again — so what is compared is a session that has
//! been *resized in place* on one target and *carried* through the ABI on the other, batch by
//! batch, rather than two fresh sessions over the same final graph.
//!
//! **Its own comparator.** [`hashgate::compare`](crate::hashgate::compare) needs a rectangular `seeds × stages` matrix,
//! and a stream's output is ragged: three fixtures of different batch counts. So the arms are
//! compared here by [`first_divergence`], which does the one thing the seed stage's
//! comparator cannot — name the first fixture and batch on which any arm disagreed with the
//! first native arm — and refuses an arm that printed a different number of lines rather
//! than comparing what it has.
//!
//! **The negative control.** [`Knob::DropDelta`](crate::hashgate::Knob::DropDelta) makes the native arm skip one batch, so
//! this stage has a control that bites rather than passing vacuously; the wasm arm reads no
//! environment variable and cannot see it, which is what makes the divergence certain.

use super::TICKS;
use crate::hashgate::compare::Arm;
use crate::hashgate::knob::Setting;
use crate::runner::sha256_hex;
use graph_core::layout::force::ForceSession;
use graph_wasm::service::{self, Source};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// This stage's id. Distinct from [`super::STAGE`]: the two hash different things, and a
/// report that named one stage id for both would say a divergence was in a stage it was not
/// run in. Written in `stream-arm.mjs` too, and `tests.rs` holds the pair still together.
pub const STREAM_STAGE: &str = "force.session.stream";

/// The fixtures, in the order both arms walk them. The same list `emit-stream-fixtures`
/// writes, so a fixture added there without a line here is a file nothing reads — and the
/// stage would report equality over three of four fixtures without saying so.
pub const FIXTURES: [&str; 3] = ["stream-small", "stream-hub", "stream-pow2"];

/// The `name` fixture's documents, one per line, in order.
pub fn documents(name: &str) -> Result<Vec<Vec<u8>>, String> {
    let path = fixture_path(name);
    let raw = std::fs::read(&path).map_err(|e| {
        format!(
            "{}: {e}; run `graph-cli emit-stream-fixtures` to write it",
            path.display()
        )
    })?;
    let mut out = Vec::new();
    for line in raw.split(|b| *b == b'\n') {
        if !line.is_empty() {
            out.push(line.to_vec());
        }
    }
    if out.is_empty() {
        return Err(format!("{}: no lines", path.display()));
    }
    Ok(out)
}

/// The `name` fixture's path, resolved against the workspace's `fixtures/` so a build run
/// from anywhere reads the same bytes.
pub fn fixture_path(name: &str) -> PathBuf {
    crate::stream_fixtures::fixture_path(name)
}

/// Body of the hidden `force-gate-stream-arm` subcommand: one native pass over every
/// fixture, one line per batch.
pub fn arm() -> ExitCode {
    let setting = match super::env_setting() {
        Ok(setting) => setting,
        Err(err) => {
            eprintln!("force-gate-stream-arm: {err}");
            return ExitCode::from(2);
        }
    };
    match arm_lines(&setting) {
        Ok(lines) => {
            print!("{lines}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("force-gate-stream-arm: {err}");
            ExitCode::from(2)
        }
    }
}

/// Every fixture's lines, in [`FIXTURES`] order, at `setting`'s live parameters.
pub fn arm_lines(setting: &Setting) -> Result<String, String> {
    let mut out = String::new();
    for name in FIXTURES {
        out.push_str(&fixture_lines(setting, name)?);
    }
    Ok(out)
}

/// One fixture's lines: batch 0 from the initial graph, then one per batch after it.
///
/// The control decides only whether a batch is *applied*. A dropped batch still ticks and
/// still prints, which is what keeps the arms' line counts equal: a stage that printed fewer
/// lines under a control would be refused as an incomparable arm rather than reported as a
/// divergence.
///
/// **A dropped batch takes the ones after it with it.** `Topology::extend` refuses an edge
/// whose endpoint names no node, and batch `b + 1` names the nodes batch `b` brought, so a
/// native arm that skipped only batch `k` would exit 2 on batch `k + 1` — a refusal, not a
/// divergence, and the gate's answer to the control would be "could not run". So once the
/// arm is behind it stays behind: every later batch is skipped too, the session keeps
/// stepping and the gate reports the batch where the arms first differed, which is the one
/// the control named. The wasm arm, which reads no environment variable, keeps every batch.
fn fixture_lines(setting: &Setting, name: &str) -> Result<String, String> {
    let docs = documents(name)?;
    let mut topology = service::build(&docs[0], Source::Ingest)
        .map_err(|code| format!("{name} line 0: {}", code.name()))?;
    let mut session = ForceSession::new(&topology, setting.live_force_params())
        .map_err(|e| format!("{name} line 0: the force session refused the graph: {e}"))?;
    session.step(TICKS);
    let mut out = line(name, 0, &super::native::columns(&session));
    let mut behind = false;
    for (batch, doc) in docs.iter().enumerate().skip(1) {
        behind = behind || setting.drop_delta() == Some(batch as u32);
        if !behind {
            service::extend(&mut topology, doc)
                .map_err(|code| format!("{name} batch {batch}: {}", code.name()))?;
            session
                .grow(&topology)
                .map_err(|e| format!("{name} batch {batch}: grow refused: {e}"))?;
        }
        session.step(TICKS);
        out.push_str(&line(name, batch as u32, &super::native::columns(&session)));
    }
    Ok(out)
}

/// One arm's line for `name`'s `batch`: the stage, the fixture, the batch and the digest.
/// The fixture and batch are *in* the line rather than only in the arm's position, so a
/// divergence report names a file and a line a reader can open.
fn line(name: &str, batch: u32, bytes: &[u8]) -> String {
    format!("{STREAM_STAGE} {name} {batch} {}\n", sha256_hex(bytes))
}

/// The first line any arm disagreed with [`arms`](first_divergence)'s first arm on, as
/// `<fixture> batch <b> (<arm> differs from <first arm>)`, or `None` when they all agree.
///
/// `Err` when the arms printed different numbers of lines: a short arm is not a comparison,
/// it is an arm that could not run, and quietly comparing the lines it did print would report
/// a stage as equal while it had never been hashed on that arm at all.
pub fn first_divergence(arms: &[Arm]) -> Result<Option<String>, String> {
    let Some((first_name, first)) = arms.first() else {
        return Err("no arms to compare".to_owned());
    };
    if arms.len() < hashgate_min_arms() {
        return Err(format!(
            "{} arms, need at least {}",
            arms.len(),
            hashgate_min_arms()
        ));
    }
    for (name, lines) in &arms[1..] {
        if lines.len() != first.len() {
            return Err(format!(
                "{name} printed {} lines, need {}",
                lines.len(),
                first.len()
            ));
        }
    }
    for (index, expected) in first.iter().enumerate() {
        for (name, lines) in &arms[1..] {
            if &lines[index] != expected {
                let (fixture, batch) = fixture_and_batch(expected)?;
                return Ok(Some(format!(
                    "{fixture} batch {batch} ({name} differs from {first_name})"
                )));
            }
        }
    }
    Ok(None)
}

/// How many batch lines the arms printed, which is the stream stage's own denominator: three
/// fixtures of different lengths, so this is a total rather than a per-fixture count.
pub fn batch_count(arms: &[Arm]) -> u32 {
    arms.first().map_or(0, |(_, lines)| lines.len() as u32)
}

/// The fewest arms the seed stage's comparator accepts, which is the floor this one inherits:
/// a stage with one arm has nothing to disagree with.
fn hashgate_min_arms() -> usize {
    crate::hashgate::compare::MIN_ARMS
}

/// `<fixture>` and `<batch>` out of one line of this stage's own output.
fn fixture_and_batch(line: &str) -> Result<(String, String), String> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let [stage, fixture, batch, ..] = fields.as_slice() else {
        return Err(format!("malformed stream line: {line:?}"));
    };
    if *stage != STREAM_STAGE {
        return Err(format!("not a {STREAM_STAGE} line: {line:?}"));
    }
    Ok(((*fixture).to_owned(), (*batch).to_owned()))
}

/// The Node arm's script, resolved from this crate's own source directory so a build that
/// runs from anywhere still finds it — the same rule as [`super::arm_script`].
pub fn stream_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("forcecheck")
        .join("stream-arm.mjs")
}

pub mod arm;

#[cfg(test)]
#[path = "stream/tests.rs"]
mod tests;
