//! The pipeline (`prompt.md` §3): pure stages in a fixed order, each stage's output hashed
//! on its own, so a cross-target divergence names the stage it started in.
//!
//! Pure means (D8): a stage reads an immutable topology and its own parameters and
//! returns new geometry. [`Stage::run`] is an associated function that receives `&Topology`
//! and `&Params` and nothing else: there is no `self` to carry state from one run to the
//! next, no `&mut` into anything another stage sees, and no clock or I/O in graph-core to
//! reach for.

use crate::arena::CapacityError;
use crate::index::{Topology, index_model};
use crate::layout::{self, Geometry};
use crate::records::{EdgeRecord, NodeRecord};
use core::fmt;
use graph_contract::binary::Snapshot;
use graph_contract::snapshot::SnapshotError;

mod topology;

pub use topology::{gate_node_count, seeded_model};

/// One stage of the pipeline that turns a topology into geometry.
pub trait Stage {
    /// Its parameters. `Default` is the stated default a hashed snapshot is pinned to.
    type Params: Default;
    /// The capability id it is registered and hashed under, e.g. `layout.grid`.
    const ID: &'static str;
    /// Runs the stage: the same topology and parameters give the same geometry, bit for
    /// bit, on every target.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError>;
}

/// Why a stage produced nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageError {
    /// The topology did not fit the `u32` index space.
    Capacity(CapacityError),
    /// A float column held NaN or ±∞, whose bits wasm32 does not pin (D9).
    NonFinite {
        /// Which column.
        column: &'static str,
    },
    /// A parameter outside what the stage accepts.
    Param {
        /// The parameter.
        name: &'static str,
        /// What it must be.
        rule: &'static str,
    },
    /// The stage's geometry broke a rule of the snapshot contract.
    Snapshot(SnapshotError),
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity(err) => err.fmt(f),
            Self::NonFinite { column } => write!(f, "non-finite value in column {column}"),
            Self::Param { name, rule } => write!(f, "parameter {name}: {rule}"),
            Self::Snapshot(err) => write!(f, "snapshot: {err}"),
        }
    }
}

/// What one pipeline run produced, stage by stage.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineRun {
    /// The topology stage's bytes.
    pub topology: Vec<u8>,
    /// The layout stage's id.
    pub layout: &'static str,
    /// The layout stage's output, as the snapshot its bytes are hashed from.
    pub snapshot: Snapshot,
}

impl PipelineRun {
    /// Each stage's id and the bytes hashed for it, in pipeline order.
    pub fn stages(&self) -> [(&'static str, Vec<u8>); 2] {
        [
            ("topology", self.topology.clone()),
            (self.layout, self.snapshot.to_bytes()),
        ]
    }
}

/// Runs the topology stage over the records, then `S` at `params`.
pub fn run_pipeline<S: Stage>(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    params: &S::Params,
) -> Result<PipelineRun, StageError> {
    run_with(nodes, edges, S::ID, |t| S::run(t, params))
}

/// The pipeline driver: indexes the records, writes the topology's bytes, runs `layout`
/// over the finished topology, and turns its geometry into a snapshot. The layout gets a
/// shared borrow of a topology no stage can change.
pub fn run_with(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    id: &'static str,
    layout: impl FnOnce(&Topology) -> Result<Geometry, StageError>,
) -> Result<PipelineRun, StageError> {
    let topology = index_model(nodes, edges).map_err(StageError::Capacity)?;
    let mut bytes = Vec::new();
    topology::encode(&topology, &mut bytes)?;
    let geometry = layout(&topology)?;
    Ok(PipelineRun {
        topology: bytes,
        layout: id,
        snapshot: layout::snapshot(&topology, geometry)?,
    })
}

/// The topology stage's bytes for `t`: what the stage hash sees, for the tests that prove
/// a topology built another way (`Topology::extend`) equal to `index_model`'s.
#[cfg(test)]
pub(crate) fn topology_bytes(t: &Topology) -> Result<Vec<u8>, StageError> {
    let mut bytes = Vec::new();
    topology::encode(t, &mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::grid::{Grid, GridParams};
    use crate::weights::REFERENCE_DEGREE;

    fn seeded(seed: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
        seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE)
    }

    #[test]
    fn the_pipeline_hashes_each_stage_on_its_own() {
        let (nodes, edges) = seeded(7);
        let run = run_pipeline::<Grid>(&nodes, &edges, &GridParams::default()).expect("runs");
        let [(first, topology), (second, layout)] = run.stages();
        assert_eq!((first, second), ("topology", "layout.grid"));
        assert_eq!(topology, run.topology);
        assert_eq!(layout, run.snapshot.to_bytes());
        assert_eq!(run.snapshot.parts().node_ids.len(), gate_node_count(7));
        let again = run_pipeline::<Grid>(&nodes, &edges, &GridParams::default()).expect("runs");
        assert_eq!(again, run, "pure: the same inputs, the same run");
    }

    #[test]
    fn a_parameter_moves_only_its_own_stage_and_the_model_moves_both() {
        let (nodes, edges) = seeded(9);
        let honest = run_pipeline::<Grid>(&nodes, &edges, &GridParams::default()).expect("runs");
        let wide = GridParams { spacing: 2.0 };
        let spaced = run_pipeline::<Grid>(&nodes, &edges, &wide).expect("runs");
        assert_eq!(spaced.topology, honest.topology);
        assert_ne!(spaced.snapshot, honest.snapshot);
        let (heavier, _) = seeded_model(9, gate_node_count(9), REFERENCE_DEGREE + 1);
        let reweighted = run_pipeline::<Grid>(&heavier, &edges, &GridParams::default());
        let reweighted = reweighted.expect("runs");
        assert_ne!(reweighted.topology, honest.topology);
        assert_eq!(
            reweighted.snapshot, honest.snapshot,
            "the grid ignores weights"
        );
    }

    #[test]
    fn a_stage_error_stops_the_run_and_every_error_says_what() {
        let (nodes, edges) = seeded(3);
        let refused = StageError::Param {
            name: "spacing",
            rule: "finite and above 0",
        };
        assert_eq!(
            run_with(&nodes, &edges, "x", |_| Err(refused)),
            Err(refused)
        );
        let cases = [
            (refused.to_string(), "parameter spacing: finite and above 0"),
            (
                StageError::NonFinite { column: "weight" }.to_string(),
                "non-finite value in column weight",
            ),
            (
                StageError::Snapshot(SnapshotError::CurveDegree).to_string(),
                "snapshot: edge.degree",
            ),
        ];
        for (message, needle) in cases {
            assert!(message.contains(needle), "{message:?} lacks {needle:?}");
        }
    }
}
