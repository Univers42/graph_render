//! `graph-cli scigraphs-conformance`: the judge. It reads what the Python arm measured and
//! checks it against the pinned baseline, then records the verdict.
//!
//! **Three checks per row, and each can fail on its own.**
//!
//! 1. **The motor bytes.** `sha256(motor/<NAME>.f64)` against [`BASELINE`]. Every layout is
//!    deterministic (D1–D10), so these bytes are a constant of the tree, and a one-ULP change
//!    to one coordinate is a gate failure naming one row — which is what `--break` measures.
//! 2. **The reference bytes.** `sha256(ref/<NAME>.f64)` against the same table, so a row
//!    whose reference silently stopped being the reference is a failure rather than a new
//!    number nobody compared to anything.
//! 3. **The shape.** The measured Procrustes median against the pinned ceiling. Without it
//!    the gate would be a hash gate and would say nothing about whether the layout is close;
//!    with it, a layout that drifts while keeping its bytes would still be caught, and it
//!    cannot: hence both.
//!
//! A row that could not be measured keeps its cell and fails: `not run: …` is a verdict,
//! not a skip. A row with no motor layout (`GRAPHVIZ_DOT`) is judged on the reference half
//! only, and its motor cells in the matrix say `not run` rather than carrying a zero.

use super::ROWS;
use super::baseline::BASELINE;
use crate::evidence::Stamp;
use serde_json::{Value, json};
use std::path::Path;

/// One row's outcome: did every row pass, and what did each say.
#[derive(Debug)]
pub struct Outcome {
    pub pass: bool,
    /// One line per row, in [`ROWS`] order, for the log.
    pub lines: Vec<String>,
}

/// The verdict for one fixture directory: `(all rows passed, the ledger record, one line per
/// row)`.
pub fn judge(dir: &Path) -> Result<bool, String> {
    let outcome = run(dir)?;
    for line in &outcome.lines {
        println!("  {line}");
    }
    Ok(outcome.pass)
}

/// The judge, without the printing, so a test can hold it to a fixture directory of its own.
pub fn run(dir: &Path) -> Result<Outcome, String> {
    let stamp = Stamp::take()?;
    let manifest = read(&dir.join("conformance-manifest.json"))?;
    let metrics = read(&dir.join("metrics.json"))?;
    cross_check(&manifest, &metrics)?;
    // **Two shas maps, one table.** The motor's bytes are digested by the emit that wrote them
    // into `conformance-manifest.json`; the reference's are digested by the arm that wrote
    // them, which ran afterwards and cannot edit a file Rust already fingerprinted. Both maps
    // are keyed by the same relative paths, so the merged view is the one the baseline is
    // stated in and neither half has to know where the other came from.
    let digests = digests(&manifest, &metrics);
    let measured = rows_of(&metrics)?;
    // **The names, not the count.** 32 differently-named rows is the same length as the 32
    // the matrix wants, and a count check would wave it through to a `procrustes_median: not
    // measured` abort that names none of them.
    let missing: Vec<&str> = ROWS
        .iter()
        .map(|row| row.name)
        .filter(|name| !measured.contains_key(*name))
        .collect();
    if !missing.is_empty() || measured.len() != ROWS.len() {
        return Err(format!(
            "metrics.json has {} rows and is missing {missing:?}",
            measured.len()
        ));
    }
    let mut pass = true;
    let mut functions = serde_json::Map::new();
    let mut lines = Vec::new();
    for row in ROWS.iter() {
        let base = BASELINE
            .iter()
            .find(|b| b.name == row.name)
            .ok_or_else(|| format!("{}: no baseline pinned", row.name))?;
        let got = &measured[row.name];
        let (ok, why) = one_row(row.name, base, got, &digests, dir)?;
        pass &= ok;
        lines.push(format!("{}: {why}", row.name));
        functions.insert(
            row.name.into(),
            json!({
                "motor_id": row.motor,
                "reference": row.reference_arm(),
                "bitwise_f64": cell(got, "bitwise_f64"),
                "bitwise_f32": cell(got, "bitwise_f32"),
                "max_ulp": cell(got, "max_ulp"),
                "max_gap": cell(got, "max_gap"),
                "procrustes_median": cell(got, "procrustes_median"),
                "procrustes_max": cell(got, "procrustes_max"),
                "coordinates": cell(got, "coordinates"),
                "ceiling": base.procrustes_ceiling,
                "tier": base.tier,
                "cause": base.cause,
                "unexplained": u64::from(!ok),
            }),
        );
    }
    stamp.still_current()?;
    let body = json!({
        "rows": ROWS.len(),
        "pass": pass,
        "tolerance": false,
        "baseline": "docs/measurements/scigraphs-conformance.md",
        "functions": functions,
    });
    crate::evidence::record(&stamp, "scigraphs-conformance", body)?;
    Ok(Outcome { pass, lines })
}

/// One row: the two shas and the Procrustes median. Returns `(passed, why)`, where `why`
/// names the first thing that was wrong, so the log says which check spoke.
///
/// **An empty pin never passes.** A baseline row with no sha means the matrix has no
/// measurement of that name, and comparing an empty string with an empty string would make
/// the absence of a measurement read as agreement — the one thing this gate must not do.
pub fn one_row(
    name: &str,
    base: &'static super::baseline::Baseline,
    got: &Value,
    sha: &Value,
    dir: &Path,
) -> Result<(bool, String), String> {
    // **The motor's bytes are always pinned and always checked, measured or not.** Every
    // layout is deterministic (D1-D10), so an empty pin is a row the matrix is still waiting on
    // and never a pass -- and this check sits *before* the unmeasurable branch on purpose.
    //
    // That ordering is the whole fix. An earlier version returned from inside the `not run`
    // branch, so `GRAPHVIZ_DOT` -- whose motor file is always the empty one, whose digest is
    // therefore a constant, and whose pinned `reference_sha256` was consequently never
    // compared -- passed whatever the reference arm did, or did not, produce.
    if base.motor_sha256.is_empty() {
        return Ok((false, "FAIL \u{2014} motor sha is not pinned".into()));
    }
    if let Some(why) = moved(sha, "motor", name, base.motor_sha256)? {
        return Ok((false, why));
    }
    if !on_disk(dir, "motor", name, base.motor_sha256) {
        return Ok((
            false,
            "FAIL \u{2014} motor/<name>.f64 is not the file that was hashed".into(),
        ));
    }
    // The reference's bytes are pinned **unless the row declares them unreproducible**, which
    // is the one thing this matrix found about a reference rather than about the motor: two
    // runs of the pinned Graphviz over the same DOT give different `fdp` coordinates. That row
    // is gated on its motor bytes and its measured shape, and the log says so.
    if base.reference_note.is_empty() {
        if base.reference_sha256.is_empty() {
            return Ok((false, "FAIL \u{2014} reference sha is not pinned".into()));
        }
        if let Some(why) = moved(sha, "ref", name, base.reference_sha256)? {
            return Ok((false, why));
        }
        if !on_disk(dir, "ref", name, base.reference_sha256) {
            return Ok((
                false,
                "FAIL \u{2014} ref/<name>.f64 is not the file that was hashed".into(),
            ));
        }
    }
    if let Some(reason) = got["metrics"].as_str() {
        // A row that cannot be measured is pinned as such: it passes when the baseline says
        // `reference-absent` **and** it still says `not run`, and both pairs of bytes above
        // still match. A row that stops being measurable is a failure; a row that was never
        // measurable is a fact about the tree, and this is the only place in the judge that
        // lets one pass.
        if base.cause != "reference-absent" || !reason.starts_with("not run") {
            return Ok((false, format!("FAIL \u{2014} {reason}")));
        }
        return Ok((true, format!("ok \u{2014} {reason}")));
    }
    let median = got["procrustes_median"]
        .as_f64()
        .ok_or("procrustes_median: not measured")?;
    if median > base.procrustes_ceiling {
        return Ok((
            false,
            format!(
                "FAIL — procrustes median {median:.3e} over ceiling {:.3e}",
                base.procrustes_ceiling
            ),
        ));
    }
    let note = if base.reference_note.is_empty() {
        String::new()
    } else {
        format!(" [reference not pinned: {}]", base.reference_note)
    };
    Ok((
        true,
        format!(
            "{note}ok — {} f64, {} f32 of {} coordinates, median {median:.3e} <= {:.3e}",
            got["bitwise_f64"], got["bitwise_f32"], got["coordinates"], base.procrustes_ceiling
        ),
    ))
}

/// Whether the file on disk still hashes to what the manifest says it does.
///
/// **Both halves are re-hashed here, not trusted.** The manifest and `metrics.json` are
/// written by the arms that produced the files, so comparing a recorded digest against a
/// pinned one would only prove that the arms agree with themselves. `file_sha256` over the
/// actual bytes is what makes the comparison mean anything, and it is what catches a `.f64`
/// deleted or edited after the arm wrote its digest.
fn on_disk(dir: &Path, dir_name: &str, name: &str, recorded: &str) -> bool {
    let path = dir.join(dir_name).join(format!("{name}.f64"));
    // **A missing file is a failed row, not an abort.** `file_sha256` reports the read error,
    // and propagating it would turn a deleted fixture into exit 2 — "could not run", which a
    // reader cannot tell from an image that was never built. A deleted coordinate file is a
    // row that no longer matches its pin, and that is a FAIL with the row named.
    crate::runner::file_sha256(&path).is_ok_and(|actual| actual == recorded)
}

/// Whether one half's digest moved off the pinned value. `Ok(None)` means it matched.
///
/// `dir` is the directory the file is written under — `motor/` or `ref/` — and `label` is the
/// word the log calls that half. They are the same thing today and they are separate arguments
/// because the one place they differ is a word, and conflating them once made the judge ask for
/// `reference/SPRING_3D.f64`, a file no arm has ever written.
fn moved(sha: &Value, dir: &str, name: &str, expected: &str) -> Result<Option<String>, String> {
    let label = if dir == "motor" { "motor" } else { "reference" };
    let relative = format!("{dir}/{name}.f64");
    let recorded = sha[&relative]
        .as_str()
        .ok_or_else(|| format!("{relative}: nothing records a sha256 for it"))?;
    if recorded == expected {
        return Ok(None);
    }
    Ok(Some(format!(
        "FAIL — {label} bytes are not the pinned ones (sha {recorded})"
    )))
}

/// The two shas maps merged into one flat `path -> digest`, the metrics' half winning on a
/// shared key. A collision would mean one file is listed in both under the same name, and the
/// later digest is the one the comparison used.
fn digests(manifest: &Value, metrics: &Value) -> Value {
    let mut out = manifest["sha256"].as_object().cloned().unwrap_or_default();
    for (key, value) in metrics["sha256"].as_object().into_iter().flatten() {
        // **The motor's digests come from the manifest alone.** `metrics.json` is written by
        // the other arm, and a key there named `motor/<NAME>.f64` would otherwise silently
        // replace a digest this crate computed from the bytes it wrote.
        if key.starts_with("motor/") {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    Value::Object(out)
}

/// A metric cell, carried through verbatim — including the `not run: …` string, which is a
/// value and not a hole.
fn cell(got: &Value, key: &str) -> Value {
    got.get(key).cloned().unwrap_or(Value::Null)
}

/// The manifest, the result and the tree are the same tree, and the result was computed from
/// these fixtures. Without both, a stale metrics file reads as this tree's answer.
fn cross_check(manifest: &Value, metrics: &Value) -> Result<(), String> {
    let stamp = Stamp::take()?;
    if manifest["fingerprint"] != metrics["fingerprint"]
        || manifest["fingerprint"].as_str() != Some(stamp.fingerprint())
    {
        return Err("fixtures, metrics and tree are not the same tree: re-emit and re-arm".into());
    }
    if manifest["sha256"]["conformance.jsonl"] != metrics["sha256"]["conformance.jsonl"] {
        return Err("the metrics were computed from other fixtures than these".into());
    }
    Ok(())
}

fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn rows_of(metrics: &Value) -> Result<serde_json::Map<String, Value>, String> {
    metrics["rows"]
        .as_object()
        .cloned()
        .ok_or_else(|| "metrics.json: no rows object".to_string())
}

#[cfg(test)]
mod tests;
