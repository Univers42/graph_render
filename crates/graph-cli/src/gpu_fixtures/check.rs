//! `--check`: re-emit each case and byte-compare against what is committed, so the golden
//! pair is what *this* tree emits and not what some earlier one did.
//!
//! Modelled on `ingest_cmd.rs`'s compare, message for message: `up to date`, `STALE` plus
//! the byte of the first difference, exit 1 on stale and 2 on a directory it cannot read.
//! The re-emit is the whole check — nothing is read out of the committed file and compared
//! field by field, because a byte comparison is the only one that cannot pass a file which
//! parses.

use std::path::Path;
use std::process::ExitCode;

use super::emit::{self, Knobs};
use super::settle::{self, SIZES, State};

/// Compares every case this tree knows about against `dir`. `None` when `dir` cannot be
/// listed: a missing directory is exit 2, never a pass.
pub fn all(dir: &Path) -> ExitCode {
    let mut knobs = Knobs::from_env();
    let mut worst: u8 = 0;
    for (name, state) in cases() {
        let code = compare_one(dir, &name, state, &mut knobs);
        worst = worst.max(code);
    }
    ExitCode::from(worst)
}

/// Every `(file name, state)` this emitter produces, in wire order: the sizes ascending, and
/// within a size the start state before the settled one.
fn cases() -> Vec<(String, State)> {
    let mut out = Vec::new();
    for n in SIZES {
        for state in State::all() {
            out.push((settle::file_name(n, state), state));
        }
    }
    out
}

/// One case: re-emit, read, compare. `0` up to date, `1` stale, `2` unreadable.
fn compare_one(dir: &Path, name: &str, state: State, knobs: &mut Knobs) -> u8 {
    let n = node_count(&name);
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
            eprintln!("emit-gpu-fixtures --check: reading {}: {err}", path.display());
            return 2;
        }
    };
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

/// The node count a file name carries: `mesh-<tag>-<state>.gmfx`, and the tag is either a
/// count in thousands or a plain count.
fn node_count(name: &str) -> u32 {
    let tag = name
        .trim_start_matches("mesh-")
        .split('-')
        .next()
        .unwrap_or_default();
    let digits: String = tag.chars().take_while(char::is_ascii_digit).collect();
    let value: u32 = digits.parse().unwrap_or(1_000);
    match tag.strip_suffix('k') {
        Some("m") => value * 1_000_000,
        Some(_) => value * 1_000,
        None => value,
    }
}

/// One case's bytes, as `--check` wants them: the same call `--out` makes, so the two
/// cannot disagree about what a case is.
fn produced(n: u32, state: State, knobs: &mut Knobs) -> Result<Vec<u8>, String> {
    let (session, probe) = settle::case(n, state)?;
    emit::write(&probe, session.xs(), session.ys(), state, knobs)
}
