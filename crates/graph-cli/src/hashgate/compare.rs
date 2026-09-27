//! Comparing the four arms' output: line by line, after refusing every comparison that
//! would agree vacuously.

use super::STAGES;
use std::collections::BTreeSet;

/// One arm: its name and its `stage seed sha256` lines, stage by stage, seed by seed.
pub type Arm = (&'static str, Vec<String>);

/// Lines on which the four arms disagree — or why they cannot be compared at all.
/// Refused: zero seeds, a missing arm, a short arm, a line that is not
/// `stage seed <64 hex>` for its own position, and a stage whose every seed hashed alike
/// (the seed never reached the output, so `seeds` seeds tested one input).
pub fn diverged(seeds: u32, arms: &[Arm]) -> Result<Vec<usize>, String> {
    if seeds == 0 {
        return Err("0 seeds: a gate over nothing proves nothing".into());
    }
    if arms.len() != 4 {
        return Err(format!("{} arms, need 4", arms.len()));
    }
    let per_arm = seeds as usize * STAGES.len();
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
            .find(|(i, l)| !well_formed(l, seeds, *i))
        {
            return Err(format!("{name} line {i} is malformed: {bad:?}"));
        }
    }
    let first = &arms[0].1;
    for (stage, block) in STAGES.iter().zip(first.chunks(seeds as usize)) {
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
    /// Seeds on which all four arms agree, per stage in [`STAGES`] order.
    pub equal: Vec<u32>,
    /// Seeds with a divergence in any stage.
    pub diverged_seeds: u32,
}

/// Folds the line indices from [`diverged`] into per-stage and per-seed counts.
pub fn per_stage(seeds: u32, lines: &[usize]) -> Tally {
    let per = seeds as usize;
    let mut equal = vec![seeds; STAGES.len()];
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

fn well_formed(line: &str, seeds: u32, index: usize) -> bool {
    let (stage, seed) = (STAGES[index / seeds as usize], index % seeds as usize);
    let mut parts = line.split(' ');
    let prefix_ok = parts.next() == Some(stage) && parts.next() == Some(seed.to_string().as_str());
    let digest = parts.next().unwrap_or("");
    prefix_ok
        && parts.next().is_none()
        && digest.len() == 64
        && digest.bytes().all(|b| b.is_ascii_hexdigit())
}
