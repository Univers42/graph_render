# Job fix-memory-3d (agent build: follow-up of fix-tree-registry L-25)

Your worktree is cut from `fix-tree-registry`, which is not on develop yet; do not merge develop.

Read `prompts/jobs/fix-common.md` first. Source: `docs/measurements/fix-tree-registry.md`, row L-25
and the "L-25 rung" note.

fix-tree-registry widened the doc of the 3D rows' `scale_ceiling` instead of measuring it: no 3D
layout has a bytes-per-node figure, because `crates/graph-core/tests/memory.rs` has no 3D arm, so the
memory half of the 3D ceiling is borrowed from the 2D rows.

1. **Measure.** Add one `#[ignore]`d measurement per 3D layout family to `memory.rs`, in the shape
   of `hierarchy_layout_pipeline_memory_per_node` (same allocator, same `print_measurement`, same
   node counts): `layout.hierarchical3d`, and one arm for the closed forms (`layout.basic3d.sphere`;
   helix, cube and spiral share its allocation shape — say so in a comment only if the numbers for
   two of them agree within 5 %). Run them with the command in the file header and paste the
   output into `docs/measurements/fix-memory-3d.md`.
2. **Use the number.** In `crates/graph-core/src/registry/three_d.rs`, replace the borrowed memory
   argument in each 3D row's `scale_ceiling` doc with the measured bytes per node and the run that
   produced it. Change a ceiling value only if the measurement proves it wrong; if it does, stop and
   report under "decisions needed" with the number, since `capabilities --check` reports it.
3. `memory.rs` stays under 300 lines: put the 3D arms in a child module `tests/memory/three_d.rs`
   (`mod three_d;` in `memory.rs`), so they share the one counting allocator of that binary.

Paths: `crates/graph-core/tests/memory.rs`, `crates/graph-core/tests/memory/three_d.rs`,
`crates/graph-core/src/registry/three_d.rs` (doc comments only),
`docs/measurements/fix-memory-3d.md`.

Done when: the measurement block is pasted, every 3D row's doc cites it, fmt/clippy/test green.
