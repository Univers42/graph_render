//! What a layout publishes, and how a run's parameter buffer reaches it
//! (`docs/decisions/layout-params.md`).
//!
//! [`LayoutParams`] is two things in one `&'static`: the [`ParamSpec`] list a caller reads
//! through `gm_layout_params`, and the function that turns a buffer's values into a run.
//! Keeping them in the same `Capability` literal is what stops a layout from publishing
//! parameters the dispatcher has no path for — the two cannot be added apart.
//!
//! Ponytail (the empty list): a layout registered with [`NONE`] is not one that *cannot*
//! be drawn differently. `layout.force.barnes_hut`, `layout.force.yifan_hu` and
//! `layout.force.particle_mesh` all take a `ForceParams` with twelve fields, and publish
//! none of them, because each of the three reads a different subset: publishing the union
//! would advertise three knobs that do nothing on one of them, and publishing the
//! intersection needs a per-layout subset the one-struct-one-table macro cannot express.
//! Direction: those three layouts are drawn at `ForceParams::default()` and only there.
//! Escape hatch: the live force session takes all twelve over its own ABI
//! (`docs/decisions/live-force-session.md`), and a per-layout subset list is the change
//! that would put the rest on this one.

use super::capability::Capability;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::params::{ParamSpec, ParamsError, ParamsView};

pub use super::tunable::Tunable;

/// Runs a layout at values already checked against its schema. The `&Capability` is what
/// lets one function serve every layout that publishes nothing: it is the layout's own
/// `run`, which is how an empty list still draws.
pub type RunValues = fn(&Capability, &Topology, &[f64]) -> Result<Geometry, StageError>;

/// One layout's published parameters and the run that honours them.
#[derive(Debug, Clone, Copy)]
pub struct LayoutParams {
    /// The published parameters, in the order a buffer carries them. Empty means the
    /// layout takes none.
    pub specs: &'static [ParamSpec],
    /// The run at a buffer's values.
    pub run: RunValues,
}

impl LayoutParams {
    /// The list a layout with no published parameters carries — see the module doc for
    /// why three layouts that *do* take parameters are on it.
    pub const NONE: LayoutParams = LayoutParams {
        specs: &[],
        run: no_params,
    };
}

/// Every layout registered at its own defaults: the run the hash gate is pinned to.
fn no_params(
    capability: &Capability,
    topology: &Topology,
    values: &[f64],
) -> Result<Geometry, StageError> {
    if !values.is_empty() {
        return Err(StageError::Param {
            name: "params",
            rule: "this layout publishes no parameters",
        });
    }
    (capability.run)(topology)
}

/// A [`Stage`] whose parameters are published, run at a buffer's values.
fn stage_values<S: Stage>(
    _capability: &Capability,
    topology: &Topology,
    values: &[f64],
) -> Result<Geometry, StageError>
where
    S::Params: Tunable,
{
    let params = <S::Params as Tunable>::tuned(values).ok_or(StageError::Param {
        name: "params",
        rule: "one value per published parameter",
    })?;
    S::run(topology, &params)
}

/// Circle packing is not a [`Stage`] — it has its own `run_with` — so it gets its own
/// tuner rather than a `Stage` impl invented for the sake of one line.
fn packing_values(
    _capability: &Capability,
    topology: &Topology,
    values: &[f64],
) -> Result<Geometry, StageError> {
    let params = crate::layout::circle_packing::CirclePackingParams::tuned(values).ok_or(
        StageError::Param {
            name: "params",
            rule: "one value per published parameter",
        },
    )?;
    crate::layout::circle_packing::run_with(topology, &params)
}

macro_rules! published {
    ($name:ident, $stage:ty) => {
        /// This layout's published parameters and the run at their values.
        pub const $name: LayoutParams = LayoutParams {
            specs: <$stage as Stage>::Params::PARAMS,
            run: stage_values::<$stage>,
        };
    };
}

use crate::layout::circle_packing::CirclePackingParams;
use crate::layout::force::spring::{Spring, Spring3D};
use crate::layout::force::{DavidsonHarel, Drl, FruchtermanReingold, Graphopt, KamadaKawai, Lgl};
use crate::layout::forceatlas2::{ForceAtlas2, ForceAtlas2BarnesHut};
use crate::layout::grid::Grid;
use crate::layout::sugiyama::Sugiyama;

published!(GRID, Grid);
published!(SUGIYAMA, Sugiyama);
published!(SPRING, Spring);
published!(SPRING_3D, Spring3D);
published!(FORCEATLAS2, ForceAtlas2);
published!(FORCEATLAS2_BARNES_HUT, ForceAtlas2BarnesHut);
published!(FRUCHTERMAN_REINGOLD, FruchtermanReingold);
published!(KAMADA_KAWAI, KamadaKawai);
published!(GRAPHOPT, Graphopt);
published!(DAVIDSON_HAREL, DavidsonHarel);
published!(LGL, Lgl);
published!(DRL, Drl);

/// Circle packing's published parameters. Not a `published!` arm because it is the one
/// layout that is not a `Stage`.
pub const PACKING: LayoutParams = LayoutParams {
    specs: CirclePackingParams::PARAMS,
    run: packing_values,
};

impl Capability {
    /// This layout's published parameters, as the view every buffer is read through.
    pub fn params(&self) -> ParamsView<'static> {
        ParamsView::new(self.params.specs)
    }

    /// The layout at the values `bytes` carries: one little-endian `f64` per published
    /// parameter, in schema order.
    ///
    /// **An empty buffer is this layout's own `Default`**, which is the run the hash gate
    /// is pinned to and what every pre-ABI-2 caller sends — so "no opinion" is the empty
    /// buffer, not a buffer of defaults the caller had to assemble. A layout that
    /// publishes nothing therefore accepts only the empty buffer, and refuses anything
    /// else rather than dropping it.
    ///
    /// The refusal is graph-core's own ([`StageError::Param`], which names the
    /// parameter); the wasm boundary maps the same [`ParamsError`] to a wire code, so
    /// neither caller restates the rule.
    pub fn run_params(&self, topology: &Topology, bytes: &[u8]) -> Result<Geometry, StageError> {
        self.params_values(bytes)
            .map_err(|why| refuse(self.params.specs, why))
            .and_then(|values| self.run_values(topology, &values))
    }

    /// The values `bytes` carries, or the [`ParamsError`] that refused it.
    ///
    /// The same check [`run_params`](Self::run_params) makes, with the refusal left as
    /// itself instead of folded into a [`StageError`], because a boundary that has codes
    /// wants the reason and not a re-encoding of it: this is how `gm_run` answers
    /// `ParamOutOfRange` rather than letting every parameter refusal arrive as the
    /// `LayoutFailed` it shares with an algorithm that genuinely failed.
    pub fn params_values(&self, bytes: &[u8]) -> Result<Vec<f64>, ParamsError> {
        let view = self.params();
        if bytes.is_empty() {
            return Ok(view.defaults());
        }
        view.values(bytes)
    }

    /// The layout at `values`, already checked against the schema by
    /// [`ParamsView::validate`].
    pub fn run_values(&self, topology: &Topology, values: &[f64]) -> Result<Geometry, StageError> {
        (self.params.run)(self, topology, values)
    }
}

/// A refused buffer as the stage error the pipeline already speaks. The precise bounds
/// are in the schema, which is the one place they are stated; the rule here is the one
/// thing a caller needs to hear without the schema, and it says the value is not clamped.
fn refuse(specs: &'static [ParamSpec], why: ParamsError) -> StageError {
    match why {
        ParamsError::NotAccepted => StageError::Param {
            name: "params",
            rule: "this layout publishes no parameters",
        },
        ParamsError::Malformed => StageError::Param {
            name: "params",
            rule: "one little-endian f64 per published parameter, and no other bytes",
        },
        ParamsError::OutOfRange { index } => StageError::Param {
            name: specs.get(index).map_or("params", |spec| spec.name),
            rule: "inside the range the schema publishes, never clamped",
        },
    }
}

#[cfg(test)]
mod tests;
