//! The ceilings table (`docs/`): which declared `scale_ceiling`s a phase actually measured.

use super::*;

/// The `measured` cell of every row of the ceilings table, by id: the table's own first
/// column is the id, its third is what Phase 9 measured (`not measured` for a row it
/// declares but has not run).
fn ceiling_table(doc: &str) -> Vec<(String, String)> {
    doc.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let id = cells
                .get(1)
                .filter(|c| !c.is_empty() && **c != "id" && !c.starts_with('-'))?;
            let measured = cells.get(3)?;
            Some(((*id).to_owned(), (*measured).to_owned()))
        })
        .collect()
}

/// Every reason the ceilings table is not an honest record of what was measured: a row
/// the ledger does not have, and a row whose measured cell is not a number.
///
/// A ledger row the table says nothing about is deliberately **not** a finding: this
/// phase measured some ceilings and not others, and the count [`ceiling_coverage`]
/// prints is where the ones it did not are named. Turning "not yet measured" into a
/// silent pass would be the dishonest shape; turning it into a hard failure for the
/// twenty-odd layouts this phase never ran would make the flag unusable.
pub fn ceiling_findings(rows: &[Capability], doc: &str) -> Vec<String> {
    let table = ceiling_table(doc);
    let mut found: Vec<String> = Vec::new();
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
        }
    }
    found
}

/// A measured cell: a run of digits and nothing else. `not measured` is a cell this
/// function refuses on purpose — it is how the table says "declared, not run".
fn is_measured(cell: &str) -> bool {
    !cell.is_empty() && cell.chars().all(|c| c.is_ascii_digit())
}

/// How many ledger rows the table measures, and how many it leaves reasoned.
pub fn ceiling_coverage(rows: &[Capability], doc: &str) -> (usize, usize) {
    let table = ceiling_table(doc);
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

/// `--ceilings-measured`: reads [`CEILINGS_DOC`] and reports what it measures.
pub(super) fn check_ceilings(rows: &[Capability]) -> bool {
    let path = crate::runner::workspace_root().join(CEILINGS_DOC);
    let doc = match std::fs::read_to_string(&path) {
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
