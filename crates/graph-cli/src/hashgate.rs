//! The 4-way hash gate (`prompt.md` §7.1): for every seed, native run 1, native run 2,
//! wasm32 run 1 and wasm32 run 2 must produce the same SHA-256 (D7) — cross-target
//! and run-to-run in one check. Each run is its own process, so nothing (an allocator
//! address, a hash seed) can leak from one run into the next.
//!
//! The wasm arm is the real `graph_wasm.wasm` driven by `harness/wasm-run.mjs` under
//! Node, which hashes with its built-in crypto: two independent SHA-256
//! implementations, so a broken hasher cannot agree with itself and pass.

use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use std::process::{Command, ExitCode};

/// Stage name of the Phase-0 synthetic buffer.
const STAGE: &str = "synthetic";
/// Set by the negative control (`prompt.md` §7.2).
const MUTATE_ENV: &str = "GM_MUTATE_REFERENCE_DEGREE";

/// Runs all four arms over seeds `0..seeds` and compares them seed by seed.
pub fn run(seeds: u32) -> ExitCode {
    match collect_arms(seeds) {
        Ok(arms) => report(seeds, &arms),
        Err(err) => {
            eprintln!("hashgate: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// Body of the hidden `hashgate-arm` subcommand: one native run.
pub fn arm(seeds: u32) -> ExitCode {
    let degree = match reference_degree() {
        Ok(degree) => degree,
        Err(err) => {
            eprintln!("hashgate-arm: {err}");
            return ExitCode::from(2);
        }
    };
    let mut lines = String::new();
    for seed in 0..seeds {
        match graph_core::synthetic_snapshot(seed, degree) {
            Ok(bytes) => lines.push_str(&format!("{STAGE} {seed} {}\n", sha256_hex(&bytes))),
            Err(err) => {
                eprintln!("hashgate-arm: seed {seed}: {err}");
                return ExitCode::from(2);
            }
        }
    }
    print!("{lines}");
    ExitCode::SUCCESS
}

/// The native arm's reference degree. The negative control overrides it here and only
/// here: the wasm arm runs the compiled-in constant and never sees the variable, so a
/// wired mutation surfaces as exactly the cross-target divergence the gate must catch.
/// A set but unparseable value is an error, never a silent fallback to the constant —
/// that fallback would let a typo in the control pass as green.
fn reference_degree() -> Result<u32, String> {
    parse_reference_degree(std::env::var(MUTATE_ENV))
}

fn parse_reference_degree(value: Result<String, std::env::VarError>) -> Result<u32, String> {
    match value {
        Err(std::env::VarError::NotPresent) => Ok(graph_core::REFERENCE_DEGREE),
        Err(err) => Err(format!("{MUTATE_ENV}: {err}")),
        Ok(text) => text
            .trim()
            .parse()
            .map_err(|e| format!("{MUTATE_ENV}={text:?}: {e}")),
    }
}

type Arm = (&'static str, Vec<String>);

fn collect_arms(seeds: u32) -> Result<Vec<Arm>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm()?;
    let count = seeds.to_string();
    let native = || run_lines(Command::new(&exe).args(["hashgate-arm", "--seeds", &count]));
    let wasm32 = || run_lines(node_harness(&wasm).args(["synthetic", &count]));
    let arms = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    println!(
        "hashgate: wasm artifact {} sha256 {}",
        wasm.display(),
        file_sha256(&wasm)?
    );
    Ok(arms)
}

fn report(seeds: u32, arms: &[Arm]) -> ExitCode {
    println!("hashgate: stage={STAGE} seeds={seeds}");
    let diverged = match diverged(seeds, arms) {
        Ok(diverged) => diverged,
        Err(err) => {
            eprintln!("hashgate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    for (name, lines) in arms {
        println!(
            "  {name:<13} digest {}",
            sha256_hex(lines.join("\n").as_bytes())
        );
    }
    for &i in diverged.iter().take(3) {
        println!("  DIVERGED seed {i}:");
        for (name, lines) in arms {
            println!("    {name:<13} {}", lines[i]);
        }
    }
    let equal = seeds as usize - diverged.len();
    println!("  4-way equal on {equal}/{seeds} seeds");
    if diverged.is_empty() {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        println!("FAIL: {} of {seeds} seeds diverge", diverged.len());
        ExitCode::from(1)
    }
}

/// Seeds on which the four arms disagree — or why they cannot be compared at all.
/// Vacuous comparisons are refused: zero seeds, a missing arm, a short arm, or a line
/// that is not `stage seed <64 hex>` for its own seed would otherwise all "agree".
fn diverged(seeds: u32, arms: &[Arm]) -> Result<Vec<usize>, String> {
    if seeds == 0 {
        return Err("0 seeds: a gate over nothing proves nothing".into());
    }
    if arms.len() != 4 {
        return Err(format!("{} arms, need 4", arms.len()));
    }
    for (name, lines) in arms {
        if lines.len() != seeds as usize {
            return Err(format!(
                "{name} printed {} lines for {seeds} seeds",
                lines.len()
            ));
        }
        if let Some((i, bad)) = lines.iter().enumerate().find(|(i, l)| !well_formed(l, *i)) {
            return Err(format!("{name} line {i} is malformed: {bad:?}"));
        }
    }
    let first = &arms[0].1;
    Ok((0..seeds as usize)
        .filter(|&i| arms.iter().any(|(_, l)| l[i] != first[i]))
        .collect())
}

fn well_formed(line: &str, seed: usize) -> bool {
    let mut parts = line.split(' ');
    let prefix_ok = parts.next() == Some(STAGE) && parts.next() == Some(seed.to_string().as_str());
    let digest = parts.next().unwrap_or("");
    prefix_ok
        && parts.next().is_none()
        && digest.len() == 64
        && digest.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seed: usize, fill: char) -> String {
        format!("{STAGE} {seed} {}", fill.to_string().repeat(64))
    }

    fn arms(fills: [[char; 2]; 4]) -> Vec<Arm> {
        let names = [
            "native run 1",
            "native run 2",
            "wasm32 run 1",
            "wasm32 run 2",
        ];
        names
            .iter()
            .zip(fills)
            .map(|(n, f)| (*n, vec![line(0, f[0]), line(1, f[1])]))
            .collect()
    }

    #[test]
    fn agreeing_arms_have_no_divergence() {
        assert_eq!(diverged(2, &arms([['a', 'b']; 4])), Ok(vec![]));
    }

    #[test]
    fn one_arm_differing_on_one_seed_names_that_seed() {
        let mut fills = [['a', 'b']; 4];
        fills[3][1] = 'c';
        assert_eq!(diverged(2, &arms(fills)), Ok(vec![1]));
        fills[0][0] = 'd';
        assert_eq!(diverged(2, &arms(fills)), Ok(vec![0, 1]));
    }

    #[test]
    fn vacuous_comparisons_are_refused() {
        assert!(diverged(0, &[]).is_err());
        assert!(diverged(2, &arms([['a', 'b']; 4])[..3]).is_err());
        assert!(diverged(3, &arms([['a', 'b']; 4])).is_err());
        let mut bad = arms([['a', 'b']; 4]);
        bad[2].1[1] = format!("{STAGE} 1 xyzzy");
        assert!(diverged(2, &bad).is_err());
        let mut renumbered = arms([['a', 'b']; 4]);
        renumbered[1].1[1] = line(0, 'b');
        assert!(diverged(2, &renumbered).is_err());
    }

    #[test]
    fn the_mutation_variable_parses_strictly() {
        use std::env::VarError;
        assert_eq!(
            parse_reference_degree(Err(VarError::NotPresent)),
            Ok(graph_core::REFERENCE_DEGREE)
        );
        assert_eq!(parse_reference_degree(Ok(" 9 ".into())), Ok(9));
        assert!(parse_reference_degree(Ok("nine".into())).is_err());
        assert!(parse_reference_degree(Ok(String::new())).is_err());
    }

    #[test]
    fn report_exit_code_is_pass_fail_or_could_not_run() {
        let mut fills = [['a', 'b']; 4];
        assert_eq!(report(2, &arms(fills)), ExitCode::SUCCESS);
        fills[2][0] = 'c';
        assert_eq!(report(2, &arms(fills)), ExitCode::from(1));
        assert_eq!(report(0, &[]), ExitCode::from(2));
    }
}
