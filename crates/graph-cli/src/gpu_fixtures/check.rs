//! `--check`: re-emit each case and byte-compare against what is committed, so the golden
//! pair is what *this* tree emits and not what some earlier one did.
//!
//! Modelled on `ingest_cmd.rs`'s compare, message for message: `up to date`, `STALE` plus the
//! byte of the first difference, exit 1 on stale and 2 on a directory or a file it cannot
//! read. The re-emit is the whole check — nothing is read out of the committed file and
//! compared field by field, because a byte comparison is the only one that cannot pass a
//! file which parses but is wrong.
//!
//! **The cases checked are the files the directory holds**, not the whole size table: at 1M
//! a case is 100 ticks of a 1024-side mesh, and re-deriving that on every gate run to
//! compare a file nobody committed is minutes for nothing. What that could hide is a
//! *deleted* golden, so the 1k pair is required by name and a file the emitter does not
//! produce is a refusal rather than a skip.

use std::path::Path;
use std::process::ExitCode;

use super::emit::{self, Knobs};
use super::settle::{self, SIZES, State};

/// The pair that must be there, whatever else the directory holds: the committed golden
/// `--check` exists for.
const REQUIRED: [u32; 2] = [1_000, 1_000];

/// Compares every fixture `dir` holds against what this tree would write.
pub fn all(dir: &Path) -> ExitCode {
    let mut knobs = Knobs::from_env();
    let names = match present(dir) {
        Ok(names) => names,
        Err(err) => {
            eprintln!("emit-gpu-fixtures --check: {err}");
            return ExitCode::from(2);
        }
    };
    let mut worst: u8 = 0;
    for (name, n, state) in names {
        worst = worst.max(compare_one(dir, &name, n, state, &mut knobs));
    }
    ExitCode::from(worst)
}

/// Every `.gmfx` in `dir`, as `(file name, node count, state)`, ascending. A name this
/// emitter does not produce, and a missing required pair, are both refusals.
fn present(dir: &Path) -> Result<Vec<(String, u32, State)>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("reading {}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".gmfx") {
            continue;
        }
        names.push(name);
    }
    names.sort();
    let cases = names
        .iter()
        .map(|name| named(name))
        .collect::<Result<Vec<_>, String>>()?;
    for n in REQUIRED {
        for state in State::all() {
            let name = settle::file_name(n, state);
            if !cases.iter().any(|(present, _, _)| present == &name) {
                return Err(format!(
                    "{}: the required golden {name} is not there",
                    dir.display()
                ));
            }
        }
    }
    Ok(cases)
}

/// The `(name, node count, state)` one file name carries, or a refusal naming the file: a
/// `.gmfx` this emitter did not write is a stale leftover, not a case to skip.
fn named(name: &str) -> Result<(String, u32, State), String> {
    let parsed = SIZES
        .into_iter()
        .flat_map(|n| State::all().map(|state| (n, state)))
        .find(|(n, state)| settle::file_name(*n, *state) == name);
    match parsed {
        Some((n, state)) => Ok((name.to_string(), n, state)),
        None => Err(format!("{name}: not a case this emitter produces")),
    }
}

/// One case: re-emit, read, compare. `0` up to date, `1` stale, `2` unreadable.
fn compare_one(dir: &Path, name: &str, n: u32, state: State, knobs: &mut Knobs) -> u8 {
    let produced = match produced(n, state, knobs) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("emit-gpu-fixtures --check: {name}: {err}");
            return 2;
        }
    };
    let path = dir.join(name);
    let committed = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!(
                "emit-gpu-fixtures --check: reading {}: {err}",
                path.display()
            );
            return 2;
        }
    };
    report(&path, &committed, &produced)
}

/// The comparison's own verdict and its exit code, so a writer and a checker cannot each
/// have their own idea of what "stale" means.
fn report(path: &Path, committed: &[u8], produced: &[u8]) -> u8 {
    if committed == produced {
        println!(
            "emit-gpu-fixtures --check: up to date  {} ({} bytes)",
            path.display(),
            committed.len()
        );
        return 0;
    }
    println!("emit-gpu-fixtures --check: STALE       {}", path.display());
    let at = committed
        .iter()
        .zip(produced.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(committed.len().min(produced.len()));
    println!("  first difference at byte {at}");
    1
}

/// One case's bytes, as `--check` wants them: the same call `--out` makes, so the two cannot
/// disagree about what a case is.
fn produced(n: u32, state: State, knobs: &mut Knobs) -> Result<Vec<u8>, String> {
    let (session, probe) = settle::case(n, state)?;
    emit::write(&probe, session.xs(), session.ys(), state, knobs)
}
