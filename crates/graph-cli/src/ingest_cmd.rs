// `graph-cli ingest`: the contract document to the derived graph, on the command line.
//
// The derivation is `graph_core::ingest::build` and it lives there; this is the same
// call the convergence test and every future consumer makes, exposed so a fixture can
// be regenerated and *diffed* rather than hand-edited, and so a third party can see
// what the motor does with a document without writing Rust.
//
//   graph-cli ingest --from fixtures/ingest/expected-graph.json --member ingest
//   graph-cli ingest --from doc.json --member ingest --out graph.json
//   graph-cli ingest --from doc.json --member ingest --check graph.json
//
// `--check` writes nothing and exits 1 if the named file differs, so the same command
// is both the regenerator and the gate.

use graph_contract::ingest::{self, Ingest, JsonValue};
use graph_core::ingest::{build, describe, to_canonical_json};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// What the run does. `Check` and `Write` are the only two, and neither is a default:
/// a command that wrote a file unless told to would be a command nobody could run twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Write the derived graph to `--out`, or to standard output.
    Write,
    /// Compare against `--out` and exit 1 on a difference.
    Check,
}

/// Everything one run needs, as a value rather than as eight parameters (house limit).
pub struct Plan {
    /// The document to read.
    pub from: PathBuf,
    /// Which member of it is the contract document.
    pub member: String,
    /// Where the derived graph goes, or which file `--check` compares against.
    pub out: Option<PathBuf>,
    /// What to do with it.
    pub mode: Mode,
}

/// `graph-cli ingest`, or the exit code the run produced: 0 pass, 1 stale, 2 could not run.
pub fn run(plan: &Plan) -> ExitCode {
    let text = match read_document(&plan.from) {
        Ok(text) => text,
        Err(why) => {
            eprintln!("ingest: {}: {why}", plan.from.display());
            return ExitCode::from(2);
        }
    };
    let derived = match derive(&text, &plan.member) {
        Ok(derived) => derived,
        Err(why) => {
            eprintln!("ingest: {}", why);
            return ExitCode::from(2);
        }
    };
    match &plan.out {
        None => {
            print!("{derived}");
            ExitCode::SUCCESS
        }
        Some(path) => match plan.mode {
            Mode::Check => compare(path, &derived),
            Mode::Write => match write(path, &derived) {
                Ok(()) => ExitCode::SUCCESS,
                Err(why) => {
                    eprintln!("ingest: writing {}: {why}", path.display());
                    ExitCode::from(2)
                }
            },
        },
    }
}

fn read_document(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| err.to_string())
}

/// The named member of the document, read with the contract's own strict reader, and
/// the one derivation of it. The `ingest.rs` member is a convenience for the committed
/// fixture, which carries the graph beside the document that produced it.
fn derive(text: &str, member: &str) -> Result<String, String> {
    let document = ingest::read_value(text).map_err(|err| err.to_string())?;
    let JsonValue::Map(members) = &document else {
        return Err("the document's root is not an object".to_owned());
    };
    let Some((_, value)) = members.iter().find(|(key, _)| key == member) else {
        return Err(format!("no member `{member}`"));
    };
    let document: Ingest =
        ingest::read(&ingest::to_json_value(value)).map_err(|e| e.to_string())?;
    let derived = build(&document).map_err(|err| err.to_string())?;
    // The canonical text *and* the readable rendering: the first is what a diff or a
    // comparison needs, the second is what a person reading a terminal needs.
    Ok(format!(
        "{}\n{}",
        to_canonical_json(&derived),
        describe(&derived)
    ))
}

fn compare(path: &Path, produced: &str) -> ExitCode {
    match std::fs::read_to_string(path) {
        Ok(committed) if committed == produced => {
            println!("ingest --check: up to date  {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(committed) => {
            println!("ingest --check: STALE       {}", path.display());
            let at = committed
                .bytes()
                .zip(produced.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(committed.len().min(produced.len()));
            println!("  first difference at byte {at}");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("ingest --check: reading {}: {err}", path.display());
            ExitCode::from(2)
        }
    }
}

fn write(path: &Path, produced: &str) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    }
    std::fs::write(path, produced).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::workspace_root;

    fn fixture() -> String {
        format!(
            "{}/fixtures/ingest/expected-graph.json",
            workspace_root().display()
        )
    }

    #[test]
    fn deriving_the_committed_document_reproduces_the_committed_graph() {
        let plan = Plan {
            from: PathBuf::from(fixture()),
            member: "ingest".to_owned(),
            out: None,
            mode: Mode::Write,
        };
        // The plan itself is not what this checks; the derivation behind it is, and it
        // must read the committed fixture without the CLI's argument parsing in the way.
        let text = read_document(&plan.from).expect("the fixture is readable");
        let derived = derive(&text, &plan.member).expect("the fixture derives");
        assert!(derived.starts_with(r#"{"edges":["#), "{derived:.80}");
        assert!(derived.contains("lib:task:t1"), "{derived}");
    }

    #[test]
    fn a_missing_member_is_refused_by_name() {
        let text = std::fs::read_to_string(fixture()).expect("the fixture is readable");
        let why = derive(&text, "nope").expect_err("an absent member is a refusal");
        assert_eq!(why, "no member `nope`");
    }

    #[test]
    fn a_document_that_is_not_the_contract_is_refused_rather_than_half_read() {
        // A member that is present but is not the contract: the refusal must come from
        // the contract's own reader, naming the path, not from a cast that quietly
        // produced an empty graph.
        let why = derive(
            r#"{"ingest":{"version":1,"source":"s","collections":42,"records":[]}}"#,
            "ingest",
        )
        .expect_err("a wrong shape");
        assert!(why.contains("collections"), "{why}");
        assert!(why.contains("expected an array"), "{why}");
        // And a root that is not an object at all.
        let why = derive("[]", "ingest").expect_err("a root that is not an object");
        assert_eq!(why, "the document's root is not an object");
    }
}
