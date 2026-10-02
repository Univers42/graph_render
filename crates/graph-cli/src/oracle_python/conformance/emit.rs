//! `emit-conformance-fixtures`: the graphs both arms read, and the motor's own coordinates
//! over them.
//!
//! **The `f64` file is raw little-endian doubles and nothing else.** A JSON number would
//! round-trip every coordinate through a decimal printer on its way to the comparison, and
//! the one thing this matrix measures is whether two doubles are the same double. The `f32`
//! file beside it is the same values narrowed the way the snapshot narrows them, so the two
//! files cannot disagree about a row `--break` perturbed.
//!
//! Coordinates are concatenated in fixture order, three per node, `x y z`. The Python arm
//! slices the file by each fixture's node count from `conformance.jsonl`, so the file has no
//! header of its own and a length that is a function of the fixture set alone.

use super::fixtures::{self, Fixture};
use super::motor::{self, Ran};
use super::{ROWS, Row};
use crate::evidence::{FINGERPRINTED, Stamp};
use crate::runner::file_sha256;
use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// Writes the whole fixture set, every row's motor coordinates and the manifest.
pub fn write(out: &Path) -> Result<String, String> {
    let stamp = Stamp::take()?;
    let set = fixtures::all()?;
    std::fs::create_dir_all(out.join("motor")).map_err(|e| format!("{}: {e}", out.display()))?;
    let names: Vec<&str> = set.iter().map(|f| f.name).collect();
    let broken = motor::break_one();

    let mut files = serde_json::Map::new();
    let path = out.join("conformance.jsonl");
    write_lines(&path, &set.iter().map(fixtures::line).collect::<Vec<_>>())?;
    files.insert(
        path.file_name().unwrap().to_string_lossy().into(),
        Value::String(file_sha256(&path)?),
    );

    let mut rows = Vec::new();
    let mut coordinates = 0;
    for row in ROWS.iter() {
        let ran = runs(row, &set);
        let is_broken = broken.as_deref() == Some(row.name);
        let (bytes, count) = motor::raw_f64(&ran, is_broken);
        let stem = format!("motor/{}.f64", row.name);
        write_bytes(&out.join(&stem), &bytes)?;
        let sha = file_sha256(&out.join(&stem))?;
        files.insert(stem.clone(), Value::String(sha.clone()));
        let narrow = format!("motor/{}.f32", row.name);
        write_bytes(&out.join(&narrow), &motor::raw_f32(&bytes))?;
        let narrow_sha = file_sha256(&out.join(&narrow))?;
        files.insert(narrow, Value::String(narrow_sha));
        coordinates += count;
        rows.push(motor::row_line(
            row,
            &ran,
            &names,
            Some(sha),
            count,
            is_broken,
        ));
    }

    write_lines(&out.join("motor.jsonl"), &rows)?;
    let manifest = serde_json::json!({
        "rows": ROWS.len(),
        "fixtures": names.len(),
        "fixture_names": names,
        "coordinates": coordinates,
        "sha256": files,
        "fingerprint": stamp.fingerprint(),
        "fingerprinted": FINGERPRINTED,
        "break": broken,
    });
    write_lines(&out.join("conformance-manifest.json"), &[manifest])?;
    stamp.still_current()?;
    Ok(format!(
        "emit-conformance-fixtures: {} fixtures, {} rows, {coordinates} motor coordinates{}",
        names.len(),
        ROWS.len(),
        match &broken {
            None => String::new(),
            Some(name) => format!(" (one bit of {name} flipped)"),
        }
    ))
}

/// One row over the whole fixture set, in fixture order, skipping nothing: a fixture a
/// layout refuses is a gap in the row and the row says which, so the count of coordinates
/// the Python arm compares is the count the motor arm produced.
fn runs(row: &Row, set: &[Fixture]) -> Vec<Ran> {
    let Some(id) = row.motor else {
        return set
            .iter()
            .map(|_| Err("no motor layout".to_string()))
            .collect();
    };
    set.iter().map(|fixture| motor::run(id, fixture)).collect()
}

fn write_lines(path: &Path, rows: &[Value]) -> Result<(), String> {
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
    write_bytes(path, text.as_bytes())
}

/// Writes bytes and flushes them, so a reader that sees the file sees all of it.
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?,
    );
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|e| format!("{}: {e}", path.display()))
}
