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

/// `stage`'s digest per seed, in seed order, or why the arm cannot be read that way.
///
/// **A seed the arm printed twice is refused, not resolved.** Taking the first of two
/// digests for one `(stage, seed)` pair would let a later, differing digest pass as
/// agreement — the very divergence this tally counts. Two lines for one pair mean the arm
/// is not saying which is the count, so the count is not read at all; the first seed with
/// no line is refused on the same terms, checked after so a duplicate is named as one.
fn digests<'a>(arm: &'a [String], stage: &str, seeds: usize) -> Result<Vec<&'a str>, String> {
    let mut found = Vec::with_capacity(seeds);
    for seed in 0..seeds {
        let mut printed: Vec<&'a str> = arm
            .iter()
            .filter_map(|line| digest_of(line, stage, seed))
            .collect();
        if printed.len() > 1 {
            return Err(format!(
                "the wasm arm printed {stage} for seed {seed} twice: \
                 one (stage, seed) pair carries one digest"
            ));
        }
        match printed.pop() {
            Some(digest) => found.push(digest),
            None => {
                return Err(format!(
                    "the wasm arm printed no {stage} line for seed {seed}"
                ));
            }
        }
    }
    Ok(found)
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

#[cfg(test)]
#[path = "transport/tests.rs"]
mod tests;
