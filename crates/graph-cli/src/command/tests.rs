//! What the flags refuse: the floor under `--seeds`, and the four values a gate row used
//! to be able to leave out or mistype and still exit 0 (RG-05, RG-51, RG-58).
//!
//! Every test here is a negative control on the parser itself: the bug each one covers
//! was a command that ran with nothing to compare (zero seeds, an unstated seed count, an
//! unwired oracle, an unstated member, a defaulted fixture directory) and printed a pass.

use super::*;
use clap::Parser;
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

#[test]
fn an_ingest_check_must_name_the_member_it_parses() {
    assert!(parses(&["ingest", "--from", "x.json", "--check", "x.json"]).is_err());
    let named = ["ingest", "--from", "x.json", "--member", "ingest", "--check", "x.json"];
    assert!(parses(&named).is_ok());
}

#[test]
fn the_two_oracle_subcommands_refuse_a_missing_fixture_directory() {
    assert!(parses(&["oracle-diff"]).is_err());
    assert!(parses(&["oracle-diff", "--fixtures", "d"]).is_ok());
    assert!(parses(&["oracle-layouts"]).is_err());
    assert!(parses(&["oracle-layouts", "--fixtures", "d"]).is_ok());
}
