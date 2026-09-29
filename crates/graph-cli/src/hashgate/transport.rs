//! The C20 tally: how many seeds the real ABI (the [`TRANSPORT`] stage) reached the same
//! bytes as the retained hash-gate shim (the [`LAYOUT`] stage) inside the wasm32 arm.
//!
//! `harness/wasm-run.mjs` asserts the same thing per seed and exits `1` on the first
//! divergence, so this is the same check counted rather than assumed — and the count is
//! what `hashgate.json` records, so the capabilities ledger reads it instead of taking
//! the transport's word for it. A short tally is a red gate, not a warning: it is the
//! backstop for the harness's own check.

use super::{LAYOUT, TRANSPORT};

/// Seeds on which the arm's `TRANSPORT` and `LAYOUT` digests agree, or why the arm's
/// output cannot be read that way.
pub fn agree_with_shim(seeds: u32, arm: &[String]) -> Result<u32, String> {
    let seeds = seeds as usize;
    if seeds == 0 {
        return Err("0 seeds: a tally over nothing proves nothing".into());
    }
    let transport = digests(arm, TRANSPORT, seeds)?;
    let layout = digests(arm, LAYOUT, seeds)?;
    Ok(transport
        .iter()
        .zip(&layout)
        .filter(|(real, shim)| real == shim)
        .count() as u32)
}

/// `stage`'s digest per seed, in seed order, or the first seed it did not print.
fn digests<'a>(arm: &'a [String], stage: &str, seeds: usize) -> Result<Vec<&'a str>, String> {
    let found: Vec<Option<&str>> = (0..seeds)
        .map(|seed| arm.iter().find_map(|line| digest_of(line, stage, seed)))
        .collect();
    match found.iter().position(Option::is_none) {
        Some(missing) => Err(format!(
            "the wasm arm printed no {stage} line for seed {missing}"
        )),
        None => Ok(found.into_iter().flatten().collect()),
    }
}

/// The digest of `line`, if it is `stage`'s line for `seed`.
fn digest_of<'a>(line: &'a str, stage: &str, seed: usize) -> Option<&'a str> {
    let (name, rest) = line.split_once(' ')?;
    if name != stage {
        return None;
    }
    let (number, digest) = rest.split_once(' ')?;
    (number.parse::<usize>().ok()? == seed).then_some(digest)
}
