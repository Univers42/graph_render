//! `graph-cli codegen`: writes the contract's JSON Schemas and TypeScript declarations to
//! their committed files. `--check` writes nothing and exits 1 if any committed file
//! differs from what the contract types generate now, **or** if the tree still holds a
//! generated file the generator no longer emits.

use crate::runner::workspace_root;
use graph_contract::codegen::outputs;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn run(check: bool) -> ExitCode {
    match sync(&workspace_root(), check) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(stale) => {
            println!("codegen --check: {stale} stale file(s); run `graph-cli codegen` and commit");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("codegen: {err}");
            ExitCode::from(2)
        }
    }
}

/// The two suffixes a committed generated file carries. An orphan is looked for by these
/// and nowhere else, so a hand-written document in the same directory is not a finding.
const GENERATED_SUFFIXES: [&str; 2] = [".schema.json", ".d.ts"];

/// Brings every generated file under `root` up to date (or, with `check`, only counts
/// what is not) and returns the number of files that were stale.
///
/// An **orphan** — a committed generated file `outputs()` no longer produces, left by a
/// renamed generator or a dropped schema — counts as stale in both modes and is never
/// deleted: removing a committed file is a person's decision, and the command's job is to
/// name it. The loop below used to enumerate the generator's *current* outputs only, so
/// such a file was never read and never counted: `codegen --check` exited 0 over it.
fn sync(root: &Path, check: bool) -> Result<usize, String> {
    let generated: BTreeSet<String> = outputs().iter().map(|(name, _)| name.clone()).collect();
    let mut stale = orphans(root, &generated)?;
    for (name, contents) in outputs() {
        let path = root.join(&name);
        if std::fs::read_to_string(&path).is_ok_and(|now| now == contents) {
            println!("  up to date  {name}");
            continue;
        }
        if check {
            stale += 1;
            println!("  STALE       {name}");
            continue;
        }
        let dir = path.parent().unwrap_or(root);
        std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(&path, &contents))
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("  wrote       {name}");
    }
    Ok(stale)
}

/// Every committed generated file in the directories the generator writes into that is
/// not one of `generated`, by workspace-relative path in byte order.
///
/// The directories come from the outputs themselves rather than from a hand-kept list, so
/// a sixth output is covered the day it is added. A missing directory is not an orphan:
/// a fresh checkout has none of them until the generator runs.
fn orphans(root: &Path, generated: &BTreeSet<String>) -> Result<usize, String> {
    let mut directories: BTreeSet<&Path> = BTreeSet::new();
    for name in generated {
        if let Some(dir) = Path::new(name).parent() {
            directories.insert(dir);
        }
    }
    let mut found: Vec<String> = Vec::new();
    for dir in directories {
        let listing = match std::fs::read_dir(root.join(dir)) {
            Ok(listing) => listing,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("reading {}: {err}", dir.display())),
        };
        for entry in listing {
            let entry = entry.map_err(|e| format!("reading {}: {e}", dir.display()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !GENERATED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
                continue;
            }
            let relative = owned_relative(dir, &name);
            if generated.contains(&relative) {
                continue;
            }
            println!("  ORPHAN      {relative}");
            found.push(relative);
        }
    }
    found.sort();
    Ok(found.len())
}

fn owned_relative(dir: &Path, name: &str) -> String {
    let mut relative = PathBuf::from(dir);
    relative.push(name);
    relative.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The orphan control: a committed generated file the generator no longer emits is
    /// counted and named, in `--check` as in a write, and is never deleted.
    #[test]
    fn a_committed_generated_file_the_generator_no_longer_emits_is_a_finding() {
        let dir = std::env::temp_dir().join(format!("gm-codegen-orphan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(sync(&dir, false), Ok(0), "a fresh tree has no orphan");
        let orphan = owned_relative(Path::new(codegen_dirs()[0].as_str()), "orphan.schema.json");
        let path = dir.join(&orphan);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("temp dir");
        std::fs::write(&path, "{ \"left behind\": true }").expect("writable");
        assert_eq!(
            sync(&dir, true),
            Ok(1),
            "the orphan is stale, not invisible"
        );
        assert!(
            path.exists(),
            "`--check` writes nothing, so it does not delete it either"
        );
        assert_eq!(
            sync(&dir, false),
            Ok(1),
            "and a write names it rather than removing it"
        );
        assert!(
            path.exists(),
            "removing a committed file is a person's decision"
        );
        std::fs::remove_dir_all(&dir).expect("temp dir removable");
    }

    /// The control's other half: a hand-written document beside the generated ones is not
    /// an orphan, so the finding cannot be satisfied by or spent on prose.
    #[test]
    fn a_hand_written_document_beside_them_is_not_an_orphan() {
        let dir = std::env::temp_dir().join(format!("gm-codegen-prose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(sync(&dir, false), Ok(0));
        let prose = dir
            .join(codegen_dirs()[0].as_str())
            .join("binary-layout.md");
        std::fs::write(&prose, "# hand written").expect("writable");
        assert_eq!(sync(&dir, true), Ok(0), "{prose:?} is not generated");
        std::fs::remove_dir_all(&dir).expect("temp dir removable");
    }

    /// The directories `outputs()` writes into, in the order the first output names them.
    fn codegen_dirs() -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for (name, _) in outputs() {
            if let Some(dir) = Path::new(&name).parent().and_then(Path::to_str)
                && !seen.iter().any(|seen| seen == dir)
            {
                seen.push(dir.to_owned());
            }
        }
        seen
    }

    #[test]
    fn check_counts_stale_files_and_a_write_makes_them_current() {
        let dir = std::env::temp_dir().join(format!("gm-codegen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            sync(&dir, true),
            Ok(graph_contract::codegen::outputs().len() as usize),
            "every committed generated file that is missing counts as stale"
        );
        assert!(!dir.exists(), "--check writes nothing");
        assert_eq!(sync(&dir, false), Ok(0));
        assert_eq!(sync(&dir, true), Ok(0));
        let (name, _) = &outputs()[1];
        std::fs::write(dir.join(name), "edited by hand").expect("writable");
        assert_eq!(sync(&dir, true), Ok(1), "one edited file is stale");
        std::fs::remove_dir_all(&dir).expect("temp dir removable");
    }
}
