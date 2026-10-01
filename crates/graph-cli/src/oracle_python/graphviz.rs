//! One differential per Graphviz engine, so `oracle-graphviz` is a single command that takes
//! `--engine` rather than one command per engine.
//!
//! `layout.twopi` landed first with its own `oracle-twopi` subcommand and its own harness, and
//! both keep working. This table is what they now share: `oracle-twopi` is a two-line alias
//! for `oracle-graphviz --engine twopi`, and the fixture names, the result file name and the
//! ledger record all come from the [`Differential`] each row holds — so a second engine costs
//! one row here and one module beside it, not a new command, a new arm and a new record.

use super::Differential;
use super::circo::CIRCO;
use super::twopi::TWOPI;

/// One engine: the name the oracle is invoked with, and the differential it drives.
pub(crate) struct Engine {
    /// The engine's name as Graphviz spells it: the `-Tplain` binary and the fixture stem.
    pub name: &'static str,
    pub differential: Differential,
}

/// Every Graphviz-engine differential, sorted by name so an unknown engine's error lists them
/// in a stable order.
pub(crate) const ENGINES: [Engine; 2] = [
    Engine {
        name: "circo",
        differential: CIRCO,
    },
    Engine {
        name: "twopi",
        differential: TWOPI,
    },
];

/// The engine called `name`, or the list of the ones there are.
pub(crate) fn engine(name: &str) -> Result<&'static Engine, String> {
    ENGINES
        .iter()
        .find(|engine| engine.name == name)
        .ok_or_else(|| {
            format!(
                "{name}: no Graphviz-engine differential; try one of {:?}",
                names()
            )
        })
}

/// The engine names, for an error message.
fn names() -> Vec<&'static str> {
    ENGINES.iter().map(|engine| engine.name).collect()
}

#[cfg(test)]
mod tests {
    use super::{ENGINES, engine};

    /// Every engine the table holds is found by its own name, and the name the fixtures and
    /// the result file use is the one the CLI parses, so the two cannot drift.
    #[test]
    fn every_engine_is_found_by_its_own_name() {
        for row in ENGINES {
            assert_eq!(engine(row.name).expect("found").name, row.name);
            assert_eq!(row.differential.name, row.name);
        }
    }

    /// An engine there is no differential for is refused and the error **names** the ones
    /// there are. The refusal is the negative control: read as a pass, a typo in a gate row
    /// would compare nothing and say so with exit 0.
    #[test]
    fn an_unknown_engine_is_refused_and_names_the_ones_there_are() {
        let Err(err) = engine("osage") else {
            panic!("an engine there is no differential for must be refused");
        };
        assert!(err.starts_with("osage: "), "{err}");
        for row in ENGINES {
            assert!(err.contains(row.name), "{err} does not name {}", row.name);
        }
    }

    /// The ceiling is one number per engine, and every ceiling is a positive power of ten:
    /// the shape the measurements file states them in.
    #[test]
    fn every_graphviz_ceiling_is_a_positive_power_of_ten() {
        for row in ENGINES {
            for &(id, _, ceiling) in row.differential.ceilings {
                assert!(ceiling > 0.0, "{id}: ceiling {ceiling}");
                let exponent = ceiling.log10();
                assert_eq!(
                    exponent.fract(),
                    0.0,
                    "{id}: ceiling {ceiling} is not a power of ten"
                );
            }
        }
    }
}
