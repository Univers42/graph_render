//! The Graphviz-armed differentials behind **one** subcommand pair, so a second engine is
//! a name in this list rather than a third command.
//!
//! `emit-graphviz-fixtures --engine <name>` and `oracle-graphviz --engine <name>`. The
//! default is `twopi`, so `oracle-twopi` — the command this replaced — is an alias that
//! still runs the twopi differential over the same files and records the same
//! `oracle-twopi` ledger record.

use super::Differential;
use clap::builder::TypedValueParser;
use std::path::PathBuf;

/// The fixture directory one engine's emit writes and its ingest reads, so the default
/// follows `--engine` instead of being a literal that can be wrong.
pub(super) fn default_dir(engine: &str) -> PathBuf {
    PathBuf::from(format!("target/{engine}-fixtures"))
}

/// `--engine`, parsed from [`ENGINES`] so the flag and the lookup cannot drift.
pub(super) fn engine_parser() -> impl TypedValueParser {
    clap::builder::PossibleValuesParser::new(ENGINES)
}

/// The Graphviz engine each name runs, and the differential that compares against it.
///
/// One list, so `--engine` cannot name a subcommand that exists and a differential that
/// does not: the parser and the lookup read the same two words.
pub(super) fn by_engine(engine: &str) -> Option<Differential> {
    match engine {
        "twopi" => Some(super::twopi::TWOPI),
        "osage" => Some(super::osage::OSAGE),
        "patchwork" => Some(super::patchwork::PATCHWORK),
        _ => None,
    }
}

/// The engine names, for `--engine`'s own value parser.
pub(super) const ENGINES: [&str; 3] = ["twopi", "osage", "patchwork"];
