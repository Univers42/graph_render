use std::path::{Path, PathBuf};

/// Every line of this crate's own source that holds `needle`.
///
/// Review Focus 5 pins a negative — the compute crate's credential check appears nowhere in the hub
/// — and only a source grep can hold one: code that must not be called compiles perfectly well
/// whether or not anyone calls it.
///
/// Caveat: this greps `src/` and not `tests/`, so a test may name what it forbids.
pub fn grep_this_crate(needle: &str) -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    walk(&root, needle, &mut hits);
    // Sorted, so a failing assertion prints the same list on every run and in every order.
    hits.sort();
    hits
}

/// The grep's own recursion: one directory level at a time, since `src/` is two levels deep.
fn walk(dir: &Path, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, needle, hits);
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if line.contains(needle) {
                hits.push(format!("{}:{}", path.display(), number + 1));
            }
        }
    }
}
