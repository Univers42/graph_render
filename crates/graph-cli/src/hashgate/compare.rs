//! Comparing the four arms' output: line by line, after refusing every comparison that
//! would agree vacuously. `stages` names the stages in the order both arms printed them
//! (`super::STAGES` at the real call sites) — a parameter, not a global, so this module's
//! own tests can exercise it at a small, fixed size independent of how many layouts the
//! registry carries.

use std::collections::BTreeSet;

/// One arm: its name and its `stage seed sha256` lines, stage by stage, seed by seed.
pub type Arm = (&'static str, Vec<String>);

/// The arm the C20 tally reads: the first wasm32 arm, **named** rather than indexed.
///
/// `collect_arms` lists it fourth of ten under `--tiers all`, and `compare` accepts any
/// list of at least [`MIN_ARMS`] arms — so a positional read (`arms[2]`) of the arm the
/// transport verdict depends on was a panic waiting for a shorter list, outside the
/// 0/1/2 exit contract (RG-36). [`arm`] turns a missing arm into a refusal instead.
pub const C20_ARM: &str = "wasm32 run 1";

/// The lines of the arm called `name`, or why there is none.
pub fn arm<'a>(arms: &'a [Arm], name: &str) -> Result<&'a [String], String> {
    arms.iter()
        .find(|(arm, _)| *arm == name)
        .map(|(_, lines)| lines.as_slice())
        .ok_or_else(|| format!("no arm called {name:?} among {}", arms.len()))
}

/// The fewest arms a gate run may compare. Two is the minimum that can disagree at all;
/// the honest run has four (native ×2, wasm32 ×2) and `--tiers all` has more, so a lower
/// bound rather than an exact count is the rule that survives adding a tier.
pub const MIN_ARMS: usize = 2;

/// Lines on which the arms disagree — or why they cannot be compared at all.
/// Refused: zero seeds, fewer than [`MIN_ARMS`] arms, a short arm, a line that is not
/// `stage seed <64 hex>` for its own position, and a stage whose every seed hashed alike
/// (the seed never reached the output, so `seeds` seeds tested one input).
pub fn diverged(seeds: u32, stages: &[&str], arms: &[Arm]) -> Result<Vec<usize>, String> {
    if seeds == 0 {
        return Err("0 seeds: a gate over nothing proves nothing".into());
    }
    if arms.len() < MIN_ARMS {
        return Err(format!("{} arms, need at least {MIN_ARMS}", arms.len()));
    }
    // A *short* arm is refused below, when its line count is checked; the count here only
    // says the list itself is too short to be a comparison at all.
    let per_arm = seeds as usize * stages.len();
    for (name, lines) in arms {
        if lines.len() != per_arm {
            return Err(format!(
                "{name} printed {} lines, need {per_arm}",
                lines.len()
            ));
        }
        if let Some((i, bad)) = lines
            .iter()
            .enumerate()
            .find(|(i, l)| !well_formed(l, seeds, *i, stages))
        {
            return Err(format!("{name} line {i} is malformed: {bad:?}"));
        }
    }
    let first = &arms[0].1;
    for (stage, block) in stages.iter().zip(first.chunks(seeds as usize)) {
        if seeds > 1 && block.iter().all(|line| digest(line) == digest(&block[0])) {
            return Err(format!(
                "{stage}: every seed hashed to one digest: the seed never reaches the output, so {seeds} seeds test one input"
            ));
        }
    }
    Ok((0..per_arm)
        .filter(|&i| arms.iter().any(|(_, l)| l[i] != first[i]))
        .collect())
}

/// Divergent lines folded back to stages and seeds.
#[derive(Debug, PartialEq, Eq)]
pub struct Tally {
    /// Seeds on which all arms agree, per stage in the caller's `stages` order.
    pub equal: Vec<u32>,
    /// Seeds with a divergence in any stage.
    pub diverged_seeds: u32,
}

/// Folds the line indices from [`diverged`] into per-stage and per-seed counts, over
/// `stage_count` stages (`stages.len()` at the real call sites).
pub fn per_stage(seeds: u32, stage_count: usize, lines: &[usize]) -> Tally {
    let per = seeds as usize;
    let mut equal = vec![seeds; stage_count];
    let mut bad_seeds = BTreeSet::new();
    for &line in lines {
        equal[line / per] -= 1;
        bad_seeds.insert(line % per);
    }
    Tally {
        equal,
        diverged_seeds: bad_seeds.len() as u32,
    }
}

fn digest(line: &str) -> &str {
    line.rsplit(' ').next().unwrap_or("")
}

fn well_formed(line: &str, seeds: u32, index: usize, stages: &[&str]) -> bool {
    let (stage, seed) = (stages[index / seeds as usize], index % seeds as usize);
    let mut parts = line.split(' ');
    let prefix_ok = parts.next() == Some(stage) && parts.next() == Some(seed.to_string().as_str());
    let digest = parts.next().unwrap_or("");
    prefix_ok
        && parts.next().is_none()
        && digest.len() == 64
        && digest.bytes().all(|b| b.is_ascii_hexdigit())
}
