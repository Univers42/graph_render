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
//! **Seven rows now, not five.** `SPIRAL_3D` joined for sg-spiral3d, so the graph-free
//! closed forms are four, and `BIPARTITE_3D` joined after it, so three of the seven read
//! the graph. [`BASIC_3D_CEILING`]'s own doc below says which of the six rows under it were
//! measured and which were inherited: four carry a timing at 1 000 000 nodes, and no 3D row
//! carries a bytes-per-node figure at all.

mod bipartite_3d;

use super::bench_cap::MAX_BENCH_NODES;

pub(super) use bipartite_3d::BIPARTITE_3D;

/// The node count four of the six rows under this constant were run at, and what the
/// other two inherited.
///
/// **Where the figure comes from.** [`MAX_BENCH_NODES`] read rather than written out, so
/// this ceiling and the radial one are the same number by construction instead of two
/// literals that happen to agree. `graph-cli bench` parses `--n` against it, so
/// 1 000 000 nodes is the largest size a measurement of any layout can be taken at.
///
/// **What was measured, and on which of the six rows.** A wall-clock `bench` run at
/// 1 000 000 nodes and 1 549 929 edges, pasted below, which covers **four** of the six:
/// the three graph-free closed forms and `layout.hierarchical3d`, the graph-reading one.
/// Every digit in that block is one host's median at one size.
///
/// **What was not measured — the honest limit of this constant.** No 3D row has a
/// bytes-per-node figure. `crates/graph-core/tests/memory.rs` has no 3D arm: it sweeps the
/// topology, the grid, the three hierarchy layouts and circle packing, so the 919 / 933 B
/// per node that `GRID_CEILING` and `HIERARCHY_LAYOUT_CEILING` derive 4 GiB from were
/// taken on 2D rows and are applied here as an argument, not as a result. The two rows
/// here with no timing of their own — `SPIRAL_3D` and `BIPARTITE_3D` — each say so in its
/// own `ponytail` (`three_d/spiral3d.rs`, `three_d/bipartite_3d.rs`).
///
/// **Why one number can still stand for six rows.** The four graph-free closed forms are
/// `O(n)` in three `f64` columns with no graph and no iteration; the two graph-reading ones
/// add only an `O(n + m)` pass over the same topology and snapshot substrate. What binds at
/// 1 M nodes is therefore the snapshot's own cost — 48 bytes a node across its three columns
/// plus the 32 in the geometry's — and not the layout, so wasm32's 4 GiB would put the true
/// wall several times higher.
///
/// `layout.force.spring3d` is the exception and takes [`SPRING_CEILING`] instead: it is the
/// dense `O(50 n^2)` kernel, not a closed form. Measured beside its 2D sibling at the same
/// two sizes — `bench --layout layout.force.spring,layout.force.spring3d --n 10000,16000
/// --repeat 3`: 9 307.48 ms against 9 347.65 ms at 10 000 nodes, and 23 261.70 ms against
/// 23 443.23 ms at 16 000 — so the third column costs about 0.8% rather than 50%, and the
/// 2D arm's ceiling stands for both.
///
/// The block the "four of the six" claim rests on, `--release`, `--repeat 3`, medians, at
/// 1 000 000 nodes and 1 549 929 edges on this host:
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
/// Ponytail (scale_ceiling): what it gets wrong — every digit in the block is one host's
/// median at one size, so another host moves every one of them; the bracketing, not the
/// digits, is what carries the claim. Nothing above 1 000 000 nodes was run, so the figure
/// understates the wall. Two of the six rows under it were never run at this size at all,
/// and no 3D row carries a bytes-per-node figure, so the memory half of the argument above
/// is borrowed from the 2D rows. Direction: too low, never too high. Escape hatch: raise
/// it, then measure the two rows that have so far only inherited it.
pub const BASIC_3D_CEILING: u64 = MAX_BENCH_NODES as u64;

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
