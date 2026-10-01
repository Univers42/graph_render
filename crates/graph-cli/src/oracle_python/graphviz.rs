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
        "neato" => Some(super::neato::NEATO),
        "circo" => Some(super::circo::CIRCO),
        "patchwork" => Some(super::patchwork::PATCHWORK),
        "fdp" => Some(super::fdp::FDP),
        "sfdp" => Some(super::sfdp::SFDP),
        _ => None,
    }
}

/// The engine names, for `--engine`'s own value parser.
pub(super) const ENGINES: [&str; 7] = [
    "twopi",
    "osage",
    "circo",
    "patchwork",
    "neato",
    "fdp",
    "sfdp",
];

#[cfg(test)]
mod tests {
    use super::{ENGINES, by_engine};

    /// Every engine the table holds is found by its own name, and the name the fixtures and
    /// the result file use is the one the CLI parses, so the two cannot drift.
    #[test]
    fn every_engine_is_found_by_its_own_name() {
        for name in ENGINES {
            let differential = by_engine(name).unwrap_or_else(|| panic!("{name} is an engine"));
            assert_eq!(
                differential.name, name,
                "{name} runs another engine's differential"
            );
        }
    }

    /// An engine there is no differential for is refused by the lookup, which is the negative
    /// control the `negctl-*-unknown-engine` gate rows run against: read as a pass, a typo in
    /// a gate row would compare nothing and say so with exit 0.
    #[test]
    fn an_unknown_engine_is_not_found_by_the_lookup() {
        assert!(
            by_engine("nosuch").is_none(),
            "an engine there is none for must be refused"
        );
    }

    /// The ceiling is one number per engine, and every ceiling is a positive power of ten:
    /// the shape the measurements files state them in.
    #[test]
    fn every_graphviz_ceiling_is_a_positive_power_of_ten() {
        for name in ENGINES {
            let differential = by_engine(name).expect("found");
            for &(id, _, ceiling) in differential.ceilings {
                assert!(ceiling > 0.0, "{id}: ceiling {ceiling}");
                assert_eq!(
                    ceiling.log10().fract(),
                    0.0,
                    "{id}: {ceiling} is not a power of ten"
                );
            }
        }
    }
}
