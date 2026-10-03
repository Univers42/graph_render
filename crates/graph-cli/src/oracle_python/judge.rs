//! What a harness's result says about the run: each layout's verdict against its ceiling,
//! the byte-for-byte section when the harness reports one, the coverage of the cases it
//! emitted, and whether the run was one an honest arm wrote.
//!
//! Three of the four are about a differential that could otherwise pass over nothing, which
//! is why they are here rather than inlined in `verdict`: each names the shape of result it
//! reads, and a differential whose result carries none of the keys is untouched by all of it.

use serde_json::{Value, json};

/// Each layout's verdict against its ceiling: `(all pass, ledger function entries)`. A
/// layout with no compared case fails: a differential over nothing proves nothing, and so
/// does one that read a case and judged none of it — see [`covered`].
pub(super) fn judge(
    ceilings: &[(&str, &str, f64)],
    result: &Value,
) -> Result<(bool, serde_json::Map<String, Value>), String> {
    let mut pass = true;
    let mut functions = serde_json::Map::new();
    for &(id, key, allowed) in ceilings {
        let row = &result["layouts"][key];
        let cases = row["cases"].as_u64().unwrap_or(0);
        let worst = row["worst"]
            .as_f64()
            .ok_or(format!("{key}: no measured worst"))?;
        let within = cases > 0 && worst <= allowed && covered(key, row);
        let verdict = if within { "ok" } else { "FAIL" };
        println!("  {id}: {cases} cases, worst {worst:.3e}, ceiling {allowed:.0e}: {verdict}");
        pass &= within;
        functions.insert(
            id.into(),
            json!({
                "cases": cases, "declared": 0, "unexplained": u64::from(!within),
                "worst": worst, "ceiling": allowed,
            }),
        );
    }
    Ok((pass, functions))
}

/// Whether a differential with analytically determined cases agrees with them **byte for
/// byte**, when its harness reports that section.
///
/// A tolerance over the small cases is weaker than the truth they carry, so the harness
/// renders both arms at the oracle's own printed precision and compares the strings; a
/// result with a `closed` section and `closed_exact` false is a failure, not a note. The
/// differentials with no closed cases say nothing about it and this returns true.
pub(super) fn closed_cases(result: &Value) -> bool {
    if !result.get("closed_exact").is_some_and(Value::is_boolean) {
        return true;
    }
    let exact = result["closed_exact"].as_bool().unwrap_or(false);
    let cases = result["closed"].as_object().map_or(0, serde_json::Map::len);
    println!(
        "  closed cases: {cases} compared byte for byte: {}",
        if exact { "ok" } else { "FAIL" }
    );
    exact && cases > 0
}

/// Whether every case a harness read for `key` was compared, for a differential that states
/// how many it emitted.
///
/// `cases` are the ones the harness judged, split into `exact` (the two arms returned the
/// same bytes) and `ties` (the reference's order inside a class of equal keys is its own, so
/// the case is judged against the rule instead). A run that read a case and judged none of
/// it would otherwise report a shorter, passing table, and one that counted a case in both
/// halves a pass it did not earn. Differentials over a seeded sweep carry no `emitted` key
/// and keep the flat "cases > 0" rule.
fn covered(key: &str, row: &Value) -> bool {
    let Some(emitted) = row.get("emitted").and_then(Value::as_u64) else {
        return true;
    };
    let (cases, exact, ties) = (
        row["cases"].as_u64().unwrap_or(0),
        row["exact"].as_u64().unwrap_or(0),
        row["ties"].as_u64().unwrap_or(0),
    );
    let ok = cases == emitted && exact + ties == cases;
    if !ok {
        println!(
            "  {key}: {cases} compared of {emitted} emitted ({exact} byte-equal, {ties} on \
             the rule), so the run skipped a case or counted one twice"
        );
    }
    ok
}

/// Whether the result is one an honest run wrote. A harness's `--break` names the case it
/// broke in `broken`, and such a run's mismatches are the control's own, so it can never
/// be the pass the ledger records. A result with no `broken` key is every other
/// differential's and says nothing.
pub(super) fn unbroken(result: &Value) -> bool {
    match result.get("broken") {
        None | Some(Value::Null) => true,
        Some(broken) => {
            println!("  {} was deliberately broken: not a verdict", broken);
            false
        }
    }
}
