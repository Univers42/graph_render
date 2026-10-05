//! The ceilings the memory cases assert, read from `docs/measurements/hub-memory.md`, the one place
//! they are written: a table row `` | `<name>` | <number> | … | ``.

use std::path::PathBuf;

/// The number in `name`'s row. Panics naming the file when the row or its number is missing, so a
/// missing measurement fails the case instead of passing it.
pub fn value(name: &str) -> f64 {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/measurements/hub-memory.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let row = format!("| `{name}` |");
    text.lines()
        .find_map(|line| {
            let rest = line.strip_prefix(&row)?;
            rest.split('|').next()?.trim().parse().ok()
        })
        .unwrap_or_else(|| panic!("{} has no `{name}` row with a number", path.display()))
}
