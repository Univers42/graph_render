//! The ceilings table (`docs/`): which declared `scale_ceiling`s a phase actually measured.

use super::*;

/// Every reason the ceilings table is not an honest record of what was measured: a row the
/// ledger does not have, a row whose measured cell is not a number, a row whose measured
/// cell contradicts the ceiling its ledger row declares, and a table row written so this
/// reader cannot tell which column is which.
///
/// A ledger row the table says nothing about is deliberately **not** a finding: this
/// phase measured some ceilings and not others, and the count [`ceiling_coverage`]
/// prints is where the ones it did not are named. Turning "not yet measured" into a
/// silent pass would be the dishonest shape; turning it into a hard failure for the
/// twenty-odd layouts this phase never ran would make the flag unusable.
pub fn ceiling_findings(rows: &[Capability], doc: &str) -> Vec<String> {
    let (table, mut found) = ceiling_table(doc);
    for (id, measured) in &table {
        let Some(row) = rows.iter().find(|row| row.id == id) else {
            found.push(format!(
                "{id}: the table names a row the ledger does not have"
            ));
            continue;
        };
        if !is_measured(measured) {
            found.push(format!(
                "{id}: the table's measured cell is `{measured}`, not a number (declared {})",
                row.scale_ceiling
            ));
            continue;
        }
        // **A measurement may not sit above the ceiling its row declares.** `measured` is
        // "the largest N this phase ran", so it is *below* the declared ceiling by
        // construction — barnes_hut's declared 200 000 against a measured 4 000. A cell
        // above it is not a better measurement: it is either a mislabelled column or a
        // declaration that is wrong, and the cell used to be checked for being digits and
        // nothing else, so either read as "measured" and `--ceilings-measured` exited 0.
        let measured: u64 = measured.parse().unwrap_or(row.scale_ceiling);
        if measured > row.scale_ceiling {
            found.push(format!(
                "{id}: measured {measured} is above the declared ceiling {}, so the cell \
                 or the declaration is wrong",
                row.scale_ceiling
            ));
        }
    }
    found
}

/// The `measured` cell of every row of every ceilings table, by id, with the rows this
/// reader could not place.
///
/// The columns come from **the header's own cell names**, not from positions. `cells[1]`
/// and `cells[3]` happened to be `id` and `measured` in the one table this phase wrote,
/// which means a table whose columns are reordered, or a row written without its leading
/// pipe, silently reads a different column — and a plain integer in the wrong column was
/// accepted as a measurement. A header is matched by name; a separator row ends the table;
/// and a line that carries table syntax without the leading pipe is reported rather than
/// read.
fn ceiling_table(doc: &str) -> (Vec<(String, String)>, Vec<String>) {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut found: Vec<String> = Vec::new();
    let mut columns: Option<(usize, usize)> = None;
    for line in doc.lines() {
        if !line.contains('|') {
            columns = None;
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if !line.starts_with('|') {
            columns = None;
            found.push(format!(
                "a table row is written without its leading pipe: `{line}`"
            ));
            continue;
        }
        if is_separator(&cells) {
            continue;
        }
        if cells.contains(&"id") {
            match header_columns(&cells) {
                Some(found) => columns = Some(found),
                None => found.push(format!(
                    "a ceilings table has an `id` column and no `measured` one: `{line}`"
                )),
            }
            continue;
        }
        let Some((id_at, measured_at)) = columns else {
            continue;
        };
        let (Some(id), Some(measured)) = (cells.get(id_at), cells.get(measured_at)) else {
            found.push(format!("a table row is short of its own columns: `{line}`"));
            continue;
        };
        if id.is_empty() {
            continue;
        }
        rows.push(((*id).to_owned(), (*measured).to_owned()));
    }
    (rows, found)
}

/// The `(id, measured)` column indices of a header row, or `None` when it names `id` but no
/// `measured` column.
fn header_columns(cells: &[&str]) -> Option<(usize, usize)> {
    let id = cells.iter().position(|cell| *cell == "id")?;
    let measured = cells.iter().position(|cell| *cell == "measured")?;
    Some((id, measured))
}

/// A markdown alignment row: every non-empty cell is a run of dashes, optionally with
/// colons. "Every", not "any": a data row whose declared cell is negative (`-100`) is not a
/// separator, and treating it as one would drop the row.
fn is_separator(cells: &[&str]) -> bool {
    cells
        .iter()
        .filter(|cell| !cell.is_empty())
        .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
}

/// A measured cell: a run of digits and nothing else. `not measured` is a cell this
/// function refuses on purpose — it is how the table says "declared, not run".
fn is_measured(cell: &str) -> bool {
    !cell.is_empty() && cell.chars().all(|c| c.is_ascii_digit())
}

/// How many ledger rows the table measures, and how many it leaves reasoned.
pub fn ceiling_coverage(rows: &[Capability], doc: &str) -> (usize, usize) {
    let (table, _) = ceiling_table(doc);
    let measured = rows
        .iter()
        .filter(|row| {
            table
                .iter()
                .any(|(id, value)| id == row.id && is_measured(value))
        })
        .count();
    (measured, rows.len() - measured)
}

/// [`CEILINGS_DOC`]'s own text, or why it could not be read. A missing or unreadable doc
/// is an `Err` a caller reports as a finding: `--check` reads it, not only the flag that
/// names it.
pub fn read_ceilings_doc() -> Result<String, String> {
    read_ceilings_doc_at(&crate::runner::workspace_root().join(CEILINGS_DOC))
}

/// The same, from a path a caller chooses — the test points it at a doc the tree does not
/// have, which is how a missing doc becomes a finding rather than an absent check.
pub fn read_ceilings_doc_at(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// `--ceilings-measured`: reads [`CEILINGS_DOC`] and reports what it measures.
pub(super) fn check_ceilings(rows: &[Capability]) -> bool {
    let doc = match read_ceilings_doc() {
        Ok(doc) => doc,
        Err(err) => {
            println!("  {CEILINGS_DOC}: {err}");
            return false;
        }
    };
    let found = ceiling_findings(rows, &doc);
    let (measured, reasoned) = ceiling_coverage(rows, &doc);
    for problem in &found {
        println!("  {problem}");
    }
    println!(
        "ceilings measured: {measured} of {} rows; {reasoned} still reasoned",
        rows.len()
    );
    found.is_empty()
}
