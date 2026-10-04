//! What the flags refuse: the floor under `--seeds`, and the three values a gate row used
//! to be able to leave out or mistype and still exit 0 (RG-05, RG-58).
//!
//! Every test here is a negative control on the parser itself: the bug each one covers
//! was a command that ran with nothing to compare (zero seeds, an unstated seed count, an
//! unwired oracle, an unstated member) and printed a pass.

use super::*;
use clap::Parser;
use hashgate::shard::Shard;
use std::ffi::OsStr;

/// The crate's own command enum behind a `Parser`. `Command` derives `Subcommand`, which
/// is what `main`'s private `Cli` mounts it as, so the tests reach the same parser the
/// binary runs without duplicating the enum or reaching into `main`.
#[derive(Parser)]
struct T {
    #[command(subcommand)]
    command: Command,
}

/// `T::try_parse_from` with the program name supplied, so each test reads as argv alone.
fn parses(args: &[&str]) -> Result<T, clap::Error> {
    let mut argv = vec!["t"];
    argv.extend_from_slice(args);
    T::try_parse_from(argv)
}

#[test]
fn seed_count_accepts_one_and_refuses_zero() {
    let parser = seed_count();
    let cmd = clap::Command::new("t");
    assert!(parser.parse_ref(&cmd, None, OsStr::new("0")).is_err());
    assert_eq!(parser.parse_ref(&cmd, None, OsStr::new("1")).unwrap(), 1);
    let top = MAX_SEEDS.to_string();
    assert_eq!(
        parser.parse_ref(&cmd, None, OsStr::new(&top)).unwrap(),
        MAX_SEEDS as u32
    );
    let over = (MAX_SEEDS + 1).to_string();
    assert!(parser.parse_ref(&cmd, None, OsStr::new(&over)).is_err());
}

/// The finding itself, at the subcommand: `hashgate-arm --seeds 0` printed nothing and
/// exited 0, so a gate row over it read as a pass over nothing.
#[test]
fn no_gate_subcommand_takes_zero_seeds() {
    assert!(parses(&["hashgate-arm", "--seeds", "0"]).is_err());
    assert!(parses(&["hashgate", "--seeds", "0"]).is_err());
    assert!(parses(&["roundtrip", "--seeds", "0"]).is_err());
    assert!(parses(&["hashgate-arm", "--seeds", "1"]).is_ok());
}

#[test]
fn the_emit_fixtures_seed_count_must_be_stated() {
    assert!(parses(&["emit-fixtures"]).is_err());
    assert!(parses(&["emit-fixtures", "--seeds", "8"]).is_ok());
}

#[test]
fn stress_names_an_oracle_that_is_wired() {
    assert!(parses(&["stress", "--oracle", "d33"]).is_err());
    assert!(parses(&["stress", "--oracle", "d3", "--seeds", "8"]).is_ok());
}

/// The write path names its member too: clap 4.6 has no "required if `--check` is
/// present", so the flag carries no default on either path and both must state the key.
#[test]
fn an_ingest_check_must_name_the_member_it_parses() {
    assert!(parses(&["ingest", "--from", "x.json", "--check", "x.json"]).is_err());
    let named = [
        "ingest", "--from", "x.json", "--member", "ingest", "--check", "x.json",
    ];
    assert!(parses(&named).is_ok());
    let write = [
        "ingest", "--from", "x.json", "--member", "ingest", "--out", "g.json",
    ];
    assert!(parses(&write).is_ok());
}

/// The arm's shard, as the parser read it, or why the parser refused the line.
///
/// The clap error is a string because it is compared by text: `assert_eq!` on
/// `Result<_, clap::Error>` cannot work, and what this test claims is that the flag parsed,
/// not how clap phrases a refusal.
fn arm_shard(args: &[&str]) -> Result<Shard, String> {
    match parses(args).map_err(|e| e.to_string())?.command {
        Command::HashgateArm { shard, .. } => Ok(shard),
        Command::HashgateArm { .. } => unreachable!(),
    }
}

/// `--shard` defaults to the whole run, so an unflagged `hashgate-arm` still hashes every
/// seed; and a shard that names a slice of a run that does not exist is refused at the
/// parser, not by the merge discovering a hole in the arm afterwards.
#[test]
fn hashgate_arm_defaults_to_the_whole_run_and_refuses_an_impossible_shard() {
    assert_eq!(arm_shard(&["hashgate-arm", "--seeds", "4"]), Ok(Shard::WHOLE));
    assert_eq!(
        arm_shard(&["hashgate-arm", "--seeds", "4", "--shard", "2/3"]),
        Ok(Shard { index: 2, count: 3 })
    );
    assert_eq!(
        arm_shard(&["hashgate-arm", "--seeds", "4"]).map(|s| s.to_string()),
        Ok("0/1".to_owned())
    );
    for bad in ["1/0", "3/3", "x/2", "2"] {
        assert!(arm_shard(&["hashgate-arm", "--seeds", "4", "--shard", bad]).is_err(), "{bad}");
    }
}

#[test]
fn the_two_oracle_subcommands_still_accept_the_default_fixture_directory() {
    // RG-51's `--fixtures` half is **not** fixed here: making it required would fail the
    // shipped row `scripts/orch/rows/develop-full.rows`'s `oracle-layouts` (`-- oracle-layouts`,
    // no `--fixtures`), and the rows file is outside this job's paths. Pinned so the flag
    // stays optional until that row is updated; the finding is tracked, not forgotten.
    assert!(parses(&["oracle-diff"]).is_ok());
    assert!(parses(&["oracle-diff", "--fixtures", "d"]).is_ok());
    assert!(parses(&["oracle-layouts"]).is_ok());
    assert!(parses(&["oracle-layouts", "--fixtures", "d"]).is_ok());
}
