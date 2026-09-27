//! `graph-cli codegen`: writes the contract's JSON Schemas and TypeScript declarations to
//! their committed files. `--check` writes nothing and exits 1 if any committed file
//! differs from what the contract types generate now.

use crate::runner::workspace_root;
use graph_contract::codegen::outputs;
use std::path::Path;
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

/// Brings every generated file under `root` up to date (or, with `check`, only counts
/// what is not) and returns the number of files that were stale.
fn sync(root: &Path, check: bool) -> Result<usize, String> {
    let mut stale = 0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_counts_stale_files_and_a_write_makes_them_current() {
        let dir = std::env::temp_dir().join(format!("gm-codegen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(sync(&dir, true), Ok(3), "missing files are stale");
        assert!(!dir.exists(), "--check writes nothing");
        assert_eq!(sync(&dir, false), Ok(0));
        assert_eq!(sync(&dir, true), Ok(0));
        let (name, _) = &outputs()[1];
        std::fs::write(dir.join(name), "edited by hand").expect("writable");
        assert_eq!(sync(&dir, true), Ok(1));
        std::fs::remove_dir_all(&dir).expect("temp dir removable");
    }
}
