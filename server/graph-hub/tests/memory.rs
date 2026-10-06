//! Task 10: the terms of the §6 memory budget that rest on a measured number.
//!
//! Each measurement runs in its own child process (this test binary, re-run on one `#[ignore]`d
//! case): glibc keeps freed pages, so a second body in the same process would start above the first
//! one's peak. The child resets its high-water mark, does the work, and prints
//! `HUB_MEM peak=<bytes> size=<n>`; the parent divides and compares the ratio with the ceiling
//! `docs/measurements/hub-memory.md` records, which is where the budget arithmetic reads it from.
//!
//! The container half, every writer holding a parsed worst-case body at once inside a hub capped
//! at 1 GiB, is `container::peak_rss_at_every_cap_fits_one_gib`, run by `scripts/orch/hub-mem.sh`.

#[path = "memory/bodies.rs"]
mod bodies;
#[cfg(feature = "db-tests")]
#[path = "memory/container.rs"]
mod container;
#[cfg(feature = "db-tests")]
#[path = "memory/heads.rs"]
mod heads;
#[path = "memory/ledger.rs"]
mod ledger;
#[path = "memory/rss.rs"]
mod rss;
#[path = "support/mod.rs"]
mod support;

use graph_contract::hub::batch::read_batch;
use graph_contract::hub::{Limits, read_manifest};
use graph_contract::ingest::record_piece;
use graph_store::pool::LastSeen;
use std::hint::black_box;
use std::process::Command;
use support::fixtures::MANIFEST;

const PLUGIN: &str = "task";

/// Bodies per shape, at `max_body × k / SIZES` for `k` in `1..=SIZES`: five shapes, 100 bodies.
const SIZES: usize = 20;

/// `GRAPH_HUB_LAST_SEEN`'s default (`src/config/env.rs`).
const LAST_SEEN: u64 = 65_536;

/// What a child measured: its peak above the baseline, and the size it measured it at.
struct Measured {
    peak: u64,
    size: u64,
}

#[test]
fn f_w_is_measured_on_this_reader() {
    let max_body = Limits::DEFAULT.max_body as usize;
    let mut worst = 0.0_f64;
    for shape in bodies::SHAPES {
        let runs: Vec<Measured> = (1..=SIZES)
            .map(|k| one_body(shape, max_body * k / SIZES))
            .collect();
        let top = runs.iter().map(|run| run.peak).max().unwrap_or(0);
        let largest = runs.iter().map(|run| run.size).max().unwrap_or(1);
        let f_w = top as f64 / largest as f64;
        for run in &runs {
            println!("HUB_MEM shape={shape} size={} peak={}", run.size, run.peak);
        }
        println!("HUB_MEM shape={shape} f_w={f_w:.2} (peak {top} / largest body {largest})");
        worst = worst.max(f_w);
    }
    println!("HUB_MEM f_w={worst:.2}");
    let ceiling = ledger::value("f_w_ceiling");
    assert!(
        worst <= ceiling,
        "F_w {worst:.2} is over the recorded {ceiling}"
    );
}

#[test]
fn the_last_seen_map_entry_is_the_planned_size() {
    let run = child("last_seen_peak", &[]);
    assert_eq!(run.size, LAST_SEEN, "the map evicted before its cap");
    let entry = run.peak as f64 / run.size as f64;
    println!("HUB_MEM last_seen_entry={entry:.1} peak={}", run.peak);
    let ceiling = ledger::value("last_seen_entry_ceiling_bytes");
    assert!(
        entry <= ceiling,
        "a last-seen entry is {entry:.1} B, over the recorded {ceiling}"
    );
}

/// The hub's writer path for one body, minus the database: the raw bytes (`body::read`), the
/// lossy copy `read_batch` parses (dropped after the parse, as `routes/batches.rs` drops it), the
/// batch, its check, and the store's plan (`writer/plan.rs` `upserts_of`: a record and its
/// canonical text per upsert), all alive at the peak.
#[test]
#[ignore = "a child process of f_w_is_measured_on_this_reader"]
fn one_body_peak() {
    let shape = env("HUB_MEM_SHAPE");
    let size: usize = env("HUB_MEM_BYTES").parse().expect("a byte count");
    let limits = Limits::DEFAULT;
    let manifest = read_manifest(MANIFEST, PLUGIN).expect("the fixture manifest");
    let base = rss::baseline();
    let raw = bodies::body(&shape, size, limits.max_batch as usize).into_bytes();
    let batch = {
        let text = String::from_utf8_lossy(&raw).into_owned();
        read_batch(&text, &limits)
    };
    let batch = batch.unwrap_or_else(|error| panic!("a {shape} body of {size} B: {error:?}"));
    batch
        .check(PLUGIN, &manifest, &limits)
        .unwrap_or_else(|error| panic!("a {shape} body of {size} B: {error:?}"));
    let planned: Vec<_> = batch
        .upserts
        .iter()
        .map(|up| {
            let record = up.record(PLUGIN);
            let text = record_piece(&record);
            (record, text)
        })
        .collect();
    let peak = rss::peak();
    println!("HUB_MEM peak={} size={}", peak - base, raw.len());
    black_box((&raw, &batch, &planned));
}

/// A full last-seen map of the longest workspace ids (63 bytes, `check_workspace_id`); the ids
/// themselves are made before the baseline, since the map holds its own copies.
#[test]
#[ignore = "a child process of the_last_seen_map_entry_is_the_planned_size"]
fn last_seen_peak() {
    let ids: Vec<String> = (0..LAST_SEEN).map(|i| format!("{i:0>63}")).collect();
    let base = rss::baseline();
    let mut seen = LastSeen::new(LAST_SEEN as usize);
    for (seq, ws) in (1..).zip(&ids) {
        seen.raise(ws, 1, seq);
    }
    let peak = rss::peak();
    println!("HUB_MEM peak={} size={}", peak - base, seen.len());
    black_box((&ids, &seen));
}

fn one_body(shape: &str, size: usize) -> Measured {
    child(
        "one_body_peak",
        &[
            ("HUB_MEM_SHAPE", shape.to_owned()),
            ("HUB_MEM_BYTES", size.to_string()),
        ],
    )
}

/// Run `case` alone in a fresh copy of this test binary and read its `HUB_MEM` line.
fn child(case: &str, vars: &[(&str, String)]) -> Measured {
    let exe = std::env::current_exe().expect("this test binary");
    let output = Command::new(exe)
        .args([
            case,
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .envs(vars.iter().map(|(name, value)| (*name, value.as_str())))
        .output()
        .expect("a child test process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{case} {vars:?}: {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
        .lines()
        .find_map(|line| {
            // libtest prints `test <case> ... ` on the same line, before the case's own output.
            let (_, rest) = line.split_once("HUB_MEM peak=")?;
            let (peak, size) = rest.split_once(" size=")?;
            Some(Measured {
                peak: peak.parse().ok()?,
                size: size.trim().parse().ok()?,
            })
        })
        .unwrap_or_else(|| panic!("{case} {vars:?} printed no HUB_MEM line:\n{stdout}"))
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset: run through the parent case"))
}
