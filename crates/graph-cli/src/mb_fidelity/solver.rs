//! The solvers this gate grades, and how a `--require` word names one.

use std::str::FromStr;

/// A many-body solver, named the way `mb-fidelity --require` names it.
///
/// `pm` and `bh:<theta>` are the two engines graph-core has; a word this does not parse is
/// a refusal at the argument, not a solver that silently measured nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Solver {
    /// The particle mesh, `ForceSession::with_particle_mesh`. No `theta`: the field is
    /// solved on an FFT mesh, and its accuracy is set by the cell size.
    Mesh,
    /// Barnes-Hut at the given opening angle.
    Tree(f64),
}

impl Solver {
    /// The name a row and a `--require` word both use.
    pub fn label(self) -> String {
        match self {
            Self::Mesh => "pm".to_owned(),
            Self::Tree(theta) => format!("bh:{theta}"),
        }
    }

    /// The opening angle to hand `charge_deltas`. The mesh has none, and reads zero: the
    /// two solvers differ in the engine and in nothing else, so a mesh row and a tree row
    /// are two engines on one set rather than two argument values.
    pub fn theta(self) -> f64 {
        match self {
            Self::Mesh => 0.0,
            Self::Tree(theta) => theta,
        }
    }
}

impl FromStr for Solver {
    type Err = String;

    /// `pm`, or `bh:<theta>` with a finite `theta`.
    ///
    /// `bh:0` is legal and is the self-check's solver: opening nothing is what makes the
    /// tree's walk the exact sum in a different order. `bh:nan` is not, because a
    /// non-finite opening angle decides nothing and would grade as if it had.
    fn from_str(word: &str) -> Result<Self, Self::Err> {
        if word == "pm" {
            return Ok(Self::Mesh);
        }
        let Some(theta) = word.strip_prefix("bh:") else {
            return Err(format!("{word}: a solver is `pm` or `bh:<theta>`"));
        };
        let Ok(theta) = theta.parse::<f64>() else {
            return Err(format!("{word}: theta is not a number"));
        };
        if !theta.is_finite() {
            return Err(format!("{word}: theta is not finite"));
        }
        Ok(Self::Tree(theta))
    }
}
