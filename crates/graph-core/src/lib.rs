//! graph-core — the motor. Pure: no I/O, no async, no environment, no clock, no
//! wasm-bindgen. It compiles for `wasm32-unknown-unknown` and native in every phase,
//! and every transcendental goes through `libm` so both targets round alike (D1).
//!
//! Phase 1: the topology layer — string arena, dense indices, SoA columns, three CSR
//! adjacencies — and the 17 pure functions of the TypeScript oracle's `core/model`,
//! byte-compared against it by `harness/oracle-diff.mjs`.
//!
//! Phase 2: the pipeline. A [`Stage`] turns the immutable [`Topology`] into geometry;
//! [`run_pipeline`] runs the topology stage and one layout and hashes each on its own,
//! so a cross-target divergence names the stage it started in. Every layout is listed,
//! with its ledger metadata, in [`registry`]. [`seeded_model`] is the model the 4-way
//! hash gate runs the pipeline over; it takes the reference degree as a parameter so the
//! gate's negative control can perturb one arm.

mod arena;
mod columns;
mod csr;
mod diff;
mod edgekind;
mod ids;
mod index;
pub mod layout;
mod legend;
mod neighborhood;
mod records;
pub mod registry;
mod stage;
mod synthetic;
mod weights;

pub use arena::{CapacityError, Interned, StringArena};
pub use columns::{EdgeColumns, NodeColumns, NodeKind};
pub use csr::{Csr, Incident};
pub use diff::{Patch, diff_graph, edges_equal, is_empty_patch};
pub use edgekind::{EdgeKind, child_first_from_type, edge_kind_from_type};
pub use ids::{
    EdgeIdParts, RecordRef, hash_string, make_edge_id, make_note_node_id, make_record_node_id,
    make_tag_node_id, parse_node_id,
};
pub use index::{Stats, Topology, empty_model, index_model, nodes_equal};
pub use layout::Geometry;
pub use layout::grid::{Grid, GridParams};
pub use layout::sugiyama::{Sugiyama, SugiyamaParams};
pub use legend::{DatabaseCount, LegendCounts, TagCount, derive_legend};
pub use neighborhood::{Neighborhood, neighborhood, neighborhood_edges};
pub use records::{EdgeRecord, EdgeView, NodeRecord, NodeView};
pub use stage::{
    PipelineRun, Stage, StageError, gate_node_count, run_pipeline, run_with, seeded_model,
};
pub use synthetic::build_synthetic_model;
pub use weights::{REFERENCE_DEGREE, apply_degree_weights};
