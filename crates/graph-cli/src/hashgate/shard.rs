//! Splitting one arm's seeds across concurrent children, and merging the lines back into
//! the exact list the comparator validates.
//!
//! An arm's seeds are independent — nothing carries from one seed's pipeline run into the
//! next — so `hashgate-arm --seeds 1000` on one core is 1000 units of honest work in a
//! queue, and the queue is what ran the 2700s child budget out. Sharding is the fix that
//! keeps the claim: every seed still runs, on the same core's code, and the merged arm is
//! compared against the other arms line for line.
//!
//! Nothing here decides what is hashed. A shard is a set of seeds and nothing more, so the
//! bytes, the stage list, the seed count and the knobs are the gate's own: `merge` places
//! lines by the position the comparator reads them from (`compare::diverged` reads line `i`
//! as stage `i / seeds`, seed `i % seeds`), so a merge that put one line in the wrong slot
//! is refused as malformed by the comparison rather than quietly agreeing.

use std::fmt;

/// One slice of an arm's seeds: shard `index` of `count`.
///
/// `count == 0` is unrepresentable by construction — [`Shard::parse`] refuses it and
/// [`concurrent`] only ever builds counts of at least one — so [`Shard::seeds`] never
/// divides by zero and every stride below it is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shard {
    pub index: u32,
    pub count: u32,
}

impl Shard {
    /// The whole run in one shard: what a `--shard` left off means, and what every test
    /// compares a merged arm against.
    pub const WHOLE: Shard = Shard { index: 0, count: 1 };

    /// `"i/K"` — shard `i` of `K` — or a refusal naming the text.
    ///
    /// Refused: anything that is not two unsigned numbers around one `/`, `K == 0` (a
    /// stride of zero, and a division by zero in every loop that reads it), and
    /// `i >= K` (a shard of a run that does not exist, which would silently print nothing
    /// and read as an arm that agreed on every seed it never ran).
    pub fn parse(text: &str) -> Result<Shard, String> {
        let Some((index, count)) = text.split_once('/') else {
            return Err(format!("bad shard {text:?}: expected i/K, like 0/4"));
        };
        let bad = |why: &str| format!("bad shard {text:?}: {why}");
        let index = index
            .parse::<u32>()
            .map_err(|_| bad("the i before the / is not a whole number"))?;
        let count = count
            .parse::<u32>()
            .map_err(|_| bad("the K after the / is not a whole number"))?;
        if count == 0 {
            return Err(bad("K is 0, so there is no shard to run"));
        }
        if index >= count {
            return Err(format!(
                "bad shard {text:?}: i is {index}, so it is not one of the {count} shards \
                 0..{count}"
            ));
        }
        Ok(Shard { index, count })
    }

    /// The seeds this shard runs, ascending.
    ///
    /// **Strided, not contiguous**: a seed's cost grows with its node count
    /// (`graph_core::gate_node_count` = `2 + seed % 600`), so the seeds of one run are not
    /// the same weight in order. A contiguous split hands shard 0 the cheap seeds and the
    /// last shard the expensive ones, and the arm is as slow as its slowest shard — the
    /// whole reason the one-core arm did not fit. `index += count` gives every shard the
    /// same mix of cheap and expensive seeds, and `concurrent` collects in shard order
    /// either way, so the stride changes only the wall clock and never the output.
    pub fn seeds(self, seeds: u32) -> impl Iterator<Item = u32> {
        (self.index..seeds).step_by(self.count as usize)
    }
}

impl fmt::Display for Shard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.index, self.count)
    }
}

/// How many shards one arm is split into.
///
/// Caveat: this sizes wall clock and nothing else — it is not a correctness parameter.
/// The merged arm is the same list of lines at any count, `merge` refuses an arm that lost
/// or duplicated a seed whatever the count was, and the bytes a seed hashes do not read the
/// count. What it does decide is the peak: `count` children at once, each with its own
/// share of the lines. Capped at 8 because an arm is not the only work on the box (the four
/// arms are sequential, but the tier arms are not) and past that the queue is what the
/// budget was for.
pub fn per_arm() -> u32 {
    std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 8)
}

/// `work` once per shard of `count`, concurrently, and the results **in shard order**.
///
/// Collected by index and never by completion: a merge whose input order depended on which
/// thread finished first would be the one determinism hole in this module, and
/// [`merge`] would then place the same lines in the same slots by luck of the scheduler.
/// `count` below one is one shard, so a caller that asks for no parallelism at all still
/// gets a well-formed shard list.
pub fn concurrent<T: Send, F>(count: u32, work: F) -> Vec<Result<T, String>>
where
    F: Fn(Shard) -> T + Sync,
{
    let count = count.max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..count)
            .map(|index| scope.spawn(move || work(Shard { index, count })))
            .collect();
        handles
            .into_iter()
            .enumerate()
            .map(|(index, handle)| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err(format!("shard {index}/{count} panicked")))
            })
            .collect()
    })
}

/// Every shard's result, or the first refusal **in shard order**.
///
/// The first `Err` of the iterator, not the first to be produced: a failure is reported for
/// the same shard on a loaded host as on an idle one, which is what makes the message in
/// the evidence file worth reading.
pub fn gathered<T>(shards: Vec<Result<T, String>>) -> Result<Vec<T>, String> {
    shards.into_iter().collect()
}

/// The one arm the shards were split from: every line back, in stage-major, seed-minor
/// order — the order `compare::diverged` reads positions in.
///
/// Each `"<stage> <seed> <sha256>"` line goes to slot `stage_index * seeds + seed`, so the
/// result is the same list of lines the un-sharded arm printed, byte for byte. Refused,
/// naming the line or the slot: a stage that is not in `stages`, a seed outside
/// `0..seeds`, a slot written twice (two shards claiming one seed), and a slot left empty
/// (a seed no shard ran). The last two are the ones that matter: a missing line would
/// otherwise become a *shifted* arm, whose lines still parse and still compare — against
/// the wrong seed.
pub fn merge(seeds: u32, stages: &[&str], shards: &[Vec<String>]) -> Result<Vec<String>, String> {
    let mut slots: Vec<Option<String>> = vec![None; seeds as usize * stages.len()];
    for (shard, lines) in shards.iter().enumerate() {
        for line in lines {
            let Some((stage_index, seed)) = slot_of(line, seeds, stages)? else {
                return Err(format!(
                    "shard {shard}: {line:?} is not \"<stage> <seed> <sha256>\""
                ));
            };
            let slot = stage_index * seeds as usize + seed as usize;
            let Some(held) = slots[slot].take() else {
                slots[slot] = Some(line.clone());
                continue;
            };
            return Err(format!(
                "slot {slot} ({stages[stage_index]} {seed}) filled twice: {held:?} and {line:?}"
            ));
        }
    }
    let mut out = Vec::with_capacity(slots.len());
    for (slot, held) in slots.into_iter().enumerate() {
        match held {
            Some(line) => out.push(line),
            None => {
                return Err(format!(
                    "slot {slot} ({} {}) is empty: no shard ran it",
                    stages[slot / seeds as usize],
                    slot % seeds as usize
                ))
            }
        }
    }
    Ok(out)
}

/// The line's `(stage_index, seed)`, or `None` when it is not three space-separated parts.
fn slot_of(line: &str, seeds: u32, stages: &[&str]) -> Result<Option<(usize, u32)>, String> {
    let parts: Vec<&str> = line.split(' ').collect();
    let [stage, seed, _digest] = parts.as_slice() else {
        return Ok(None);
    };
    let Some(stage_index) = stages.iter().position(|id| id == stage) else {
        return Err(format!("{line:?} names stage {stage:?}, which is not in the gate's list"));
    };
    let Ok(seed) = seed.parse::<u32>() else {
        return Err(format!("{line:?} names seed {seed:?}, which is not a seed number"));
    };
    if seed >= seeds {
        return Err(format!("{line:?} names seed {seed}, outside the run's {seeds} seeds"));
    }
    Ok(Some((stage_index, seed)))
}

#[cfg(test)]
mod tests;