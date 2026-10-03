//! Ledger metadata for the last five SciGraphs layouts, the five that are natively 3D and
//! were the last `missing` rows in `docs/measurements/scigraphs-coverage.md`.
//!
//! Kept apart from `registry.rs` for the house line cap, and because these five are one
//! subject: they carry a z column, so a snapshot of any of them is labelled 0.4 rather
//! than 0.3 (`docs/decisions/contract-3d-verdict.md`, condition 1) and no recorded 2D
//! digest moves.
//!
//! **They are two kinds of thing, and the metadata says which per row.** [`SPHERE`],
//! [`HELIX`] and [`CUBE`] are closed forms over `(num_nodes, scale)` that read no graph at
//! all; [`HIERARCHICAL_3D`] reads the graph and [`SPRING_3D`] iterates. The three closed
//! forms owe no seed and say so; the two that draw or read structure say what they compare.
//!
//! The five rows themselves live in the children, by kind: [`basic`] holds the three
//! graph-free closed forms `SPHERE`, `HELIX` and `CUBE`, [`spiral3d`] the conical spiral
//! `SPIRAL_3D`, and [`graph`] the two that read the graph, `HIERARCHICAL_3D` and
//! `SPRING_3D`. What stays here is what all five share — [`BASIC_3D_CEILING`] and
//! `DEGRADATION` — plus the re-exports `registry.rs` imports, so the six names it uses are
//! unchanged by the split. Module-level visibility is unchanged too: the re-exports are
//! `pub(super)`, exactly as the consts were before they moved, and the children are private
//! modules, so the consts are `pub` inside them and no wider outside.
//!
//! **Six rows now, not five.** `SPIRAL_3D` joined for sg-spiral3d, so the three graph-free
//! closed forms are four: `SPIRAL_3D` takes `(num_nodes, scale)` and reads no graph exactly
//! as the other three do. [`BASIC_3D_CEILING`]'s own doc below was measured against three
//! and is unchanged by the fourth — the bound is what binds at 1 M nodes, which is the
//! snapshot's memory, not the layout's.

mod bipartite_3d;

pub(super) use bipartite_3d::BIPARTITE_3D;

/// The node count the graph-free 3D placements were measured at, and why it is this one.
///
/// **The measurement covers `sphere`, `helix` and `cube` — the three it was taken on.** Two
/// rows now sit under this constant that it was not measured for, and each says so itself:
/// `SPIRAL_3D` (`three_d/spiral3d.rs`, "Ponytail (UNMEASURED ceiling)") and `BIPARTITE_3D`.
/// Read the figure below as what it is, a measurement of three layouts at one size.
///
/// `graph-cli bench` refuses a size past a layout's registered `scale_ceiling`, and its own
/// cap is 1 000 000 nodes (`bench/scale.rs:33-34`, ten components of 100 000), so 1 M is
/// the largest size a measurement of any layout in this file can be taken at. It is a
/// **measured lower bound, not the wall**: those three layouts are `O(n)` in three `f64`
/// columns with no graph and no iteration, so what binds at 1 M nodes is the 48 bytes a
/// node costs in the snapshot's own three columns plus the 32 in the geometry's, not the
/// layout — and wasm32's 4 GiB would put the true wall several times higher.
///
/// Measured, `--release`, `--repeat 3`, medians, at 1 000 000 nodes and 1 549 929 edges on
/// this host:
///
/// ```sh
/// scripts/orch/gr cargo run -q --release -p graph-cli -- bench \
///   --layout layout.basic3d.sphere,layout.basic3d.helix,layout.basic3d.cube,layout.hierarchical3d \
///   --n 1000000 --repeat 3
/// ```
///
/// ```text
/// n=1000000
///   layout.basic3d.sphere           57.44 ms  stress-1 0.4531  (edges=1549929)
///   layout.basic3d.helix            12.69 ms  stress-1 0.4605  (edges=1549929)
///   layout.basic3d.cube              4.77 ms  stress-1 0.4836  (edges=1549929)
///   layout.hierarchical3d          306.94 ms  stress-1 0.4259  (edges=1549929)
/// ```
///
/// `layout.force.spring3d` is the exception and takes [`SPRING_CEILING`] instead: it is the
/// dense `O(50 n^2)` kernel, not a closed form. Measured beside its 2D sibling at the same
/// two sizes — `bench --layout layout.force.spring,layout.force.spring3d --n 10000,16000
/// --repeat 3`: 9 307.48 ms against 9 347.65 ms at 10 000 nodes, and 23 261.70 ms against
/// 23 443.23 ms at 16 000 — so the third column costs about 0.8% rather than 50%, and the
/// 2D arm's ceiling stands for both.
///
/// Ponytail (scale_ceiling): every digit here is one host's median at one size; another host
/// moves every one, and the bracketing is what carries the claim rather than the digits.
/// Nothing above 1 000 000 nodes was run, so the figure understates the wall.
pub const BASIC_3D_CEILING: u64 = 1_000_000;

/// The z-bearing degradation string, in the shape `registry/closed_form.rs:18` gives: what
/// happens past the ceiling on wasm32, what happens natively, and the explicit promise
/// that it is a refusal — never a wrap, never a truncation.
///
/// **The 3D addition is one clause and it is not optional.** Past the ceiling natively the
/// snapshot can also fail to build its z column, and the wire name it fails under is
/// `node.z` (`graph-contract/src/geometry/columns.rs:14-37`) — the same refusal a
/// non-finite x would get, at a different column.
const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes, and with \
SnapshotError::Length { column: \"node.z\" } if the z column does not match the node count — a \
refusal, never a wrap or a truncation. None of these six layouts refuses for any input of \
its own: every branch of each reference function is total, so there is no graph past which \
this module's own answer stops existing";

mod basic;
mod graph;
mod spiral3d;

pub(super) use basic::{CUBE, HELIX, SPHERE};
pub(super) use graph::{HIERARCHICAL_3D, SPRING_3D};
pub(super) use spiral3d::SPIRAL_3D;
