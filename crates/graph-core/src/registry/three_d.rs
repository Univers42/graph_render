//! Ledger metadata for the last eight SciGraphs layouts, the eight that are natively 3D.
//! Seven were the last `missing` rows in `docs/measurements/scigraphs-coverage.md`; the
//! eighth, `layout.random.3d`, came from `p12-t4a` under Option A
//! (`docs/decisions/3d-ids.md`) and is not on that matrix.
//!
//! Kept apart from `registry.rs` for the house line cap, and because these eight are one
//! subject: they carry a z column, so a snapshot of any of them is labelled 0.4 rather
//! than 0.3 (`docs/decisions/contract-3d-verdict.md`, condition 1) and no recorded 2D
//! digest moves.
//!
//! **They are two kinds of thing, and the metadata says which per row.** [`SPHERE`],
//! [`HELIX`], [`CUBE`] and [`SPIRAL_3D`] are closed forms over `(num_nodes, scale)` that
//! read no graph at all; [`RANDOM_3D`] reads no graph either but *draws*, [`HIERARCHICAL_3D`]
//! reads the graph and [`SPRING_3D`] iterates. The four `(num_nodes, scale)` forms owe no
//! seed and say so; the four that draw or read structure say what they compare.
//!
//! The eight rows themselves live in the children, by kind: [`basic`] holds the three
//! graph-free closed forms `SPHERE`, `HELIX` and `CUBE`, [`spiral3d`] the conical spiral
//! `SPIRAL_3D`, [`bipartite_3d`] the two-plane `BIPARTITE_3D`, [`random3d`] the seeded
//! `RANDOM_3D` and the `RANDOM_3D_LAYOUT` shim, and [`graph`] the two that read or iterate
//! the graph, `HIERARCHICAL_3D` and `SPRING_3D`. What stays here is what they mostly share
//! — [`BASIC_3D_CEILING`] and `DEGRADATION`, which `SPRING_3D` and `RANDOM_3D` each
//! override for themselves — plus the re-exports `registry.rs` imports, so the eight names
//! it uses are unchanged by the split. Module-level visibility is unchanged too: the
//! re-exports are `pub(super)`, exactly as the consts were before they moved, and the
//! children are private modules, so the consts are `pub` inside them and no wider outside.
//!
//! **Eight rows now, not five.** `SPIRAL_3D` joined for sg-spiral3d, so the graph-free
//! closed forms are five, and `BIPARTITE_3D` joined after it, so three of the eight read the
//! graph; `RANDOM_3D` reads none but draws. [`BASIC_3D_CEILING`]'s own doc below says which of the seven rows
//! under it were measured and which were inherited: four carry a timing at 1 000 000 nodes,
//! six carry a measured peak, 919.3 bytes a node at 100 000 nodes, from
//! `crates/graph-core/tests/memory/three_d.rs`, and `RANDOM_3D` is the seventh's exception on
//! both counts.

mod bipartite_3d;

use super::bench_cap::MAX_BENCH_NODES;

pub(super) use bipartite_3d::BIPARTITE_3D;

/// The node count four of the seven rows under this constant were run at, and what the
/// other three inherited.
///
/// **Where the figure comes from.** [`MAX_BENCH_NODES`] read rather than written out, so
/// this ceiling and the radial one are the same number by construction instead of two
/// literals that happen to agree. `graph-cli bench` parses `--n` against it, so
/// 1 000 000 nodes is the largest size a measurement of any layout can be taken at.
///
/// **What was measured, and on which of the seven rows.** A wall-clock `bench` run at
/// 1 000 000 nodes and 1 549 929 edges, pasted below, which covers **four** of the seven:
/// the three graph-free closed forms and `layout.hierarchical3d`, the graph-reading one.
/// Every digit in that block is one host's median at one size.
///
/// **The memory half, measured on this tree rather than borrowed.** The 3D arms of
/// `crates/graph-core/tests/memory.rs`, in that file's child module `tests/memory/three_d.rs`,
/// run by the command in the file's header (`cargo test --release -p graph-core --test
/// memory -- --ignored --nocapture`). They cover six of the seven rows under this constant --
/// every one but `layout.random.3d`, which joined after those arms were written -- on the
/// same counting global allocator, through the same `print_measurement`, at the same three
/// node counts as the 2D rows. **At 100 000 nodes every one of those six peaks at 91 930 690
/// bytes — 919.3 B a node, the same number to the byte** across the four graph-free closed
/// forms, `layout.hierarchical3d` and `layout.bipartite_3d`. At 10 000 nodes five of the six
/// peak at 8 199 698 B (820.0 B) and `layout.hierarchical3d` at 8 290 770 B; at 1 000 nodes
/// five of the six peak at 887 903 B and `layout.basic3d.spiral` at 1 315 703 B, the one row
/// that moves at the smallest size. All three blocks are pasted at
/// `docs/measurements/fix-memory-3d.md`.
///
/// **What the measurement does to the ceiling: nothing.** 919.3 B a node is within 1 % of
/// the 919 B `GRID_CEILING` derives 4 GiB from (`registry/grid.rs:16-20`), which is the
/// figure this argument used to take on loan from the 2D rows — so the borrowed number
/// turns out to have been the right one to borrow. 4 GiB at 919.3 B a node is 4.67 M nodes,
/// the same two figures `GRID_CEILING` rounds down from, against a ceiling here of 1 M: this
/// ceiling is the bench cap rather than a memory bound, and it remains conservative in the
/// direction the Ponytail below names.
///
/// **The three rows here with no timing of their own — `SPIRAL_3D`, `BIPARTITE_3D` and
/// `RANDOM_3D` — still have none at 1 000 000 nodes**, and each says so in its own
/// `ponytail` (`three_d/spiral3d.rs`, `three_d/bipartite_3d.rs`, `three_d/random3d.rs`).
///
/// **Why one number can stand for seven rows — and it is now measured, not argued.** The five
/// graph-free closed forms are `O(n)` in three `f64` columns with no graph and no iteration;
/// the graph-reading ones add only an `O(n + m)` pass over the same topology and snapshot
/// substrate. The peaks agree to the byte, so the substrate is all that binds: not one of the
/// six measured rows reaches a byte above another, and `layout.random.3d` is the seventh,
/// unmeasured. What binds at 1 M nodes is therefore the snapshot's
/// own cost — 48 bytes a node across its three columns plus the 32 in the geometry's — and
/// not the layout, so wasm32's 4 GiB would put the true wall several times higher.
///
/// `layout.force.spring3d` is the exception and takes [`SPRING_CEILING`] instead: it is the
/// dense `O(50 n^2)` kernel, not a closed form. Measured beside its 2D sibling at the same
/// two sizes — `bench --layout layout.force.spring,layout.force.spring3d --n 10000,16000
/// --repeat 3`: 9 307.48 ms against 9 347.65 ms at 10 000 nodes, and 23 261.70 ms against
/// 23 443.23 ms at 16 000 — so the third column costs about 0.8% rather than 50%, and the
/// 2D arm's ceiling stands for both.
///
/// The block the "four of the seven" claim rests on, `--release`, `--repeat 3`, medians, at
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
/// Ponytail (scale_ceiling): what it gets wrong — every digit in the timing block is one
/// host's median at one size, so another host moves every one of them; the bracketing, not
/// the digits, is what carries the claim. The memory figures are a different animal: they
/// are byte counts, not timings, so they do not move with the host beyond the allocator's
/// size-class rounding — but they stop at 100 000 nodes, three orders of magnitude short of
/// the ceiling they justify, and peak is not held, so a caller that keeps more than one run
/// alive at once is outside every number above. Nothing above 1 000 000 nodes was run, so
/// the timing figure understates the wall. Three of the seven rows under it were never run at
/// this size at all. Direction: too low, never too high. Escape hatch: raise it, then
/// measure the two rows that have so far only inherited it, and sweep the memory arms up to
/// the ceiling's own size.
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
mod random3d;
mod spiral3d;

pub(super) use basic::{CUBE, HELIX, SPHERE};
pub(super) use graph::{HIERARCHICAL_3D, SPRING_3D};
pub(super) use random3d::RANDOM_3D_LAYOUT;
pub(super) use spiral3d::SPIRAL_3D;
