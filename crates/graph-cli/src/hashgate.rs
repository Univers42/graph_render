//! The 4-way hash gate (`prompt.md` §7.1): for every stage and seed, native run 1,
//! native run 2, wasm32 run 1 and wasm32 run 2 must produce the same SHA-256 (D7) —
//! cross-target and run-to-run in one check. Each run is its own process, so nothing (an
//! allocator address, a hash seed) can leak from one run into the next.
//!
//! The stages are the pipeline's (`graph_core::run_pipeline`): the topology, then every
//! registered layout, each hashed on its own, so a divergence names the stage it began
//! in, then the transport — the real ABI over the same model, a stage of the gate in its
//! own right (C20). The wasm arm is the real `graph_wasm.wasm` driven by
//! `harness/wasm-run.mjs` under Node, which hashes with its built-in crypto: two
//! independent SHA-256 implementations, so a broken hasher cannot agree with itself and
//! pass.
//!
//! An honest run records its result in `target/gates/hashgate.json`, and a negative
//! control (one [`Knob`] set) in that knob's own record, for the capabilities ledger.

pub(crate) mod compare;
pub(crate) mod knob;
mod knobs;
pub(crate) mod report;
pub(crate) mod shard;
mod staged;
mod stages;
mod tier;
pub(crate) mod tiered;
mod transport;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
pub(crate) use compare::{Arm, Tally, diverged, per_stage};
use graph_core::layout::force::Split;
use graph_core::layout::force::spring::SpringParams;
use graph_core::layout::forceatlas2::Fa2Params;
pub use knob::Knob;
pub(crate) use knob::{Setting, env_setting};
pub(crate) use stages::{LAYOUT, TRANSPORT};
// `stage_bytes_for` is the test seam behind `stage_bytes` (`tests/stages.rs`), not a second
// call site: the gate itself always runs the real registry.
use stages::stage_bytes;
#[cfg(test)]
use stages::stage_bytes_for;
pub(crate) use stages::stages;
use std::process::{Command, ExitCode};
// The threaded arm's own recompute, split into `tiered.rs` for the house's 300-line cap.
// `stage_bytes_threaded` is the arm's only caller-facing item, re-exported so a control's
// own test can drive the arm the gate drives. `THREADED_STAGES` is named from `tiered`
// directly, so the list and the match that reads it cannot drift into two imports.
pub(crate) use tiered::stage_bytes_threaded;

/// The differential's own negative control: `GM_MUTATE_FA2_SCALING_RATIO` applied to the
/// compiled-in ForceAtlas2 parameters, so `emit-fa2-fixtures` measures a perturbed port
/// against the very reference the honest run is measured against. Refused on a typo'd or
/// doubled knob, like every other read here, rather than falling back to the default and
/// passing as green.
pub(crate) fn fa2_perturbation() -> Result<Fa2Params, String> {
    Ok(env_setting()?.fa2)
}

/// The spring differential's own negative control, the same bargain
/// [`fa2_perturbation`] strikes: `GM_MUTATE_SPRING_ITERATIONS` applied to the compiled-in
/// `SpringParams`, so `emit-spring-fixtures` measures a perturbed port against the very
/// reference the honest run is measured against. Refused on a typo'd or doubled knob rather
/// than falling back to the default and passing as green.
///
/// **`None` when the knob is absent, not `Some(default)`.** A caller cannot tell a control
/// that was never set from one set to the value the default already holds, and mistaking
/// the second for the first is how a control becomes a no-op that still reports green. The
/// presence is `Knob::control`'s, which records the arm rather than inferring it from the
/// value, and a caller that gets `None` must lay the graph out at the differential's own
/// parameters.
pub(crate) fn spring_perturbation() -> Result<Option<SpringParams>, String> {
    let setting = env_setting()?;
    Ok(matches!(setting.control, Some(Knob::SpringIterations)).then_some(setting.spring))
}

pub(crate) use tier::Tiers;
/// The gate's own worker counts, re-exported so the benchmark sweep can hold every width
/// it times to one the gate has proved hash-equal: a threshold promoting a width nothing
/// was compared at would be a claim with no equality behind it.
pub(crate) use tier::WORKER_COUNTS;
/// `--tiers` as `main.rs`'s flag parser reads it: the arm list's own [`tier::parse`], so
/// the accepted words and the arms they add are one definition.
pub(crate) fn parse_tiers(text: &str) -> Result<Tiers, String> {
    tier::parse(text)
}

/// Runs every arm over seeds `0..seeds` and compares them line by line.
pub fn run(seeds: u32, tiers: Tiers) -> ExitCode {
    let started = env_setting().and_then(|setting| {
        refuse_a_control_this_gate_cannot_bite(setting.control)?;
        refuse_a_vacuous_control(seeds, setting.split_sum)?;
        let stamp = evidence::Stamp::take()?;
        Ok((setting.control, stamp, collect_arms(seeds, tiers)?))
    });
    match started {
        Ok((control, stamp, arms)) => report::verdict(&stamp, control, seeds, &arms),
        Err(err) => {
            eprintln!("hashgate: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// **A control that cannot bite refuses the run; it does not pass it.**
///
/// `split` is the pass whose merge the control splits, and `seeds` is how many the row runs.
/// Collide's own control moves nothing below five seeds — see
/// [`graph_core::layout::force::Split::min_seeds`], which measures it — so a row at two
/// seeds would corrupt no bytes and exit **0**, a *vacuous pass*: an exit code that reads as
/// evidence for a control that never ran. Refusing is the only honest answer, and the
/// existing `Err` path already carries it as exit 2, "could not run".
///
/// **`GM_MUTATE_SPLIT_RESCALE` has no floor and needs none.** The `coords` merge steals the
/// next node's term, and the gate's smallest model is `2 + seed % 600` nodes — so seed 0 is
/// already two nodes, which is where the control bites. That is *measured*, not assumed:
/// `coords/tests.rs`'s `the_control_cannot_bite_on_a_one_node_cloud_and_does_on_two` is the
/// statement, and it is why no `min_seeds` sibling is declared for the flag. A floor here
/// would have been a second number to keep in agreement with that test for no extra
/// protection.
/// **A control that cannot reach this gate refuses the run; it does not pass it.**
///
/// [`Knob::OverlapRelaxation`] belongs to the node-overlap pass, and the hash gate runs every
/// POST stage over `layout.grid` — whose nodes are `Point`s. A `Point` has no extent, so the
/// pass is a **documented no-op on it** (`SeparateParams::point_radius` defaults to `0`): the
/// stage hashes the same bytes with the control set and without it. Measured, not assumed —
/// `GM_MUTATE_OVERLAP_RELAXATION=0 hashgate --seeds 4` reports `post.separate.grid: 4-way
/// equal on 4/4 seeds` and exits **0**, a vacuous pass reading as evidence.
///
/// So the control is refused here rather than accepted and ignored, the same bargain
/// `force-gate` strikes in the other direction: a control that cannot bite must not report
/// green. Its home is the row it was written for — `graph-cli overlap`, which lays the graph
/// out as discs and where `=0` does turn the invariant row red.
fn refuse_a_control_this_gate_cannot_bite(control: Option<Knob>) -> Result<(), String> {
    match control {
        None => Ok(()),
        Some(Knob::OverlapRelaxation) => Err(format!(
            "{} cannot reach this gate: every POST stage here runs over layout.grid's Point \
nodes, and the overlap pass is a no-op on a point (point_radius defaults to 0), so the \
stage hashes the same bytes with and without the control. Run `graph-cli overlap` for this \
control — that row gives the graph discs, and the invariant row does go red.",
            Knob::OverlapRelaxation.env()
        )),
        Some(_) => Ok(()),
    }
}

fn refuse_a_vacuous_control(seeds: u32, split: Split) -> Result<(), String> {
    let floor = split.min_seeds();
    if seeds < floor {
        return Err(format!(
            "GM_MUTATE_SPLIT_SUM splits a merge that cannot move anything in {seeds} \
             seed(s): the collide pass needs at least {floor} seeds, because below that the \
             gate's model leaves it no overlap to resolve. Run with --seeds {floor} or more, \
             or with GM_MUTATE_SPLIT_SUM naming another pass."
        ));
    }
    Ok(())
}

/// Body of the hidden `hashgate-arm` subcommand: one native run, every stage, over the
/// seeds `shard` names.
///
/// One shard, not the whole run: `collect_arms` splits an arm across
/// `shard::per_arm()` children and merges the lines back, and a child that ran the whole
/// seed range would do the work the split exists to divide.
pub fn arm(seeds: u32, shard: shard::Shard) -> ExitCode {
    match env_setting().and_then(|setting| arm_lines(seeds, shard, &setting)) {
        Ok(lines) => {
            print!("{lines}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("hashgate-arm: {err}");
            ExitCode::from(2)
        }
    }
}

/// `stage seed sha256` lines, stage by stage, seed by seed: the pipeline runs once per
/// seed and each of its stages lands in its own block.
///
/// **Zero seeds is refused here, not only by the flag's range** (RG-05). `command::seed_count`
/// refuses it at the parser, but `arm_lines` is also reached in-process by `tier::scalar_arm`
/// and by this module's own tests, and the shape it has to refuse is the same one
/// `compare::diverged` already refuses: a loop over no seed concatenates zero blocks, prints
/// nothing, and an arm that printed nothing is a comparison of nothing that reads as a pass.
///
/// Only `shard`'s seeds. `shard::merge` puts the lines back at the slots they came from, so
/// the block-per-stage shape and the seed-minor order inside a shard are what make the merge
/// an identity rather than a reshuffle.
fn arm_lines(seeds: u32, shard: shard::Shard, setting: &Setting) -> Result<String, String> {
    if seeds == 0 {
        return Err("0 seeds: an arm over nothing prints nothing and proves nothing".into());
    }
    let mut blocks = vec![String::new(); stages().len()];
    for seed in shard.seeds(seeds) {
        let stages = stage_bytes(seed, setting).map_err(|err| format!("seed {seed}: {err}"))?;
        for (block, (stage, bytes)) in blocks.iter_mut().zip(stages) {
            block.push_str(&format!("{stage} {seed} {}\n", sha256_hex(&bytes)));
        }
    }
    Ok(blocks.concat())
}

/// `stage seed sha256` lines for every **threaded** stage run over `workers`
/// `std::thread`s, and every other stage byte-identical to the serial arm's.
///
/// The stages are the gate's own registry list, in its own order, so the comparison is
/// line-for-line against the scalar arm. The threaded stages are computed by their own
/// `run_under(..., &Threads, workers)` — the same call the scalar arm reaches with one
/// worker and [`graph_core::exec::Serial`], so an arm checks a *schedule* rather than a
/// second implementation. Which ones is [`tiered::THREADED_STAGES`], and a stage absent
/// from it is hashed from the scalar arm's own bytes: its "equal" is then the arm compared
/// with itself, which is why that list is the list of what `--tiers all` proves.
/// **Stage-major, seed-minor**, exactly as [`arm_lines`] prints them: every seed of one
/// stage, then every seed of the next. That ordering is not cosmetic — `compare::diverged`
/// reads line `i` as stage `i / seeds`, seed `i % seeds`, so a seed-major arm would be
/// refused as malformed at its first line. The two functions must keep the same shape;
/// `the_threaded_arm_prints_its_stages_in_the_same_order_as_the_scalar_one` holds them to
/// it.
///
/// Only `shard`'s seeds, for the reason [`arm_lines`] gives, and the same
/// `shard::merge` reassembles the shards: an arm that hashed every seed but merged them in
/// stride order would compare equal to itself and to nothing else.
fn threads_lines(
    seeds: u32,
    shard: shard::Shard,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<String>, String> {
    let mut blocks = vec![String::new(); stages().len()];
    for seed in shard.seeds(seeds) {
        let bytes = stage_bytes_threaded(seed, setting, workers)
            .map_err(|err| format!("seed {seed}: {err}"))?;
        for (block, (id, bytes)) in blocks.iter_mut().zip(bytes) {
            block.push_str(&format!("{id} {seed} {}\n", sha256_hex(&bytes)));
        }
    }
    Ok(blocks.concat().lines().map(str::to_owned).collect())
}

fn collect_arms(seeds: u32, tiers: Tiers) -> Result<Vec<Arm>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm(&[])?;
    let native = || {
        sharded_child(seeds, |shard| {
            let count = seeds.to_string();
            run_lines(Command::new(&exe).args([
                "hashgate-arm",
                "--seeds",
                &count,
                "--shard",
                &shard.to_string(),
            ]))
        })
    };
    let wasm32 = || {
        sharded_child(seeds, |shard| {
            let count = seeds.to_string();
            run_lines(
                node_harness(&wasm)?
                    .args(["hash", &count, "--shard", &shard.to_string()])
                    .args(stages()),
            )
        })
    };
    let mut arms = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    arms.extend(tier::arms(seeds, tiers)?);
    println!("hashgate: {} arms, tiers {}", arms.len(), tiers.as_str());
    println!(
        "hashgate: wasm artifact {} sha256 {}",
        wasm.display(),
        file_sha256(&wasm)?
    );
    Ok(arms)
}

/// One arm, as `shard::per_arm()` concurrent children, merged back into one arm's lines.
///
/// Each child goes through the existing `run_lines`, so each keeps its own `CHILD_TIMEOUT`:
/// the budget bounds **one shard of one arm**, and a shard that hangs is reported as the
/// shard it was rather than as the whole arm. The four arms themselves stay sequential —
/// only an arm's seeds are split — so a slow host slows the run rather than multiplying
/// it, and the merged arm is the same list of lines an unsharded one printed.
fn sharded_child<F>(seeds: u32, child: F) -> Result<Vec<String>, String>
where
    F: Fn(shard::Shard) -> Result<Vec<String>, String> + Sync + Send,
{
    let shards = shard::gathered(shard::concurrent(shard::per_arm(), child))?;
    shard::merge(seeds, &stages(), &shards)
}

#[cfg(test)]
mod tests;
