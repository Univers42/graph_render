# Job p13-3d-seam (agent build, let a layout emit 3D)

Why: p13-3d landed the 3D contract (`Dim` at byte 14, `SnapshotParts.z: Option<Vec<f32>>`, `gm_dim`,
`label_for`), but `graph_core::layout::Geometry` (`crates/graph-core/src/layout/mod.rs:37-44`) has only
`nodes`/`edges`/`notes`, and `snapshot()` hard-wires `version: label_for(Dim::D2)` and `z: None`
(`mod.rs:60-71`). So no layout can emit 3D. p12-t3 (sphere, helix, cube, 3D spring and hierarchy) and
p12-t4a/t4b (3D arms of existing layouts) are blocked on this (`docs/measurements/p12-t3.md` on branch
p12-t3 has the evidence).

Do:
a. Add `pub z: Option<Vec<f32>>` to `Geometry` (the contract's type and meaning: one z per node, in
   node order; `None` = 2D). Add `Geometry::planar(nodes, edges, notes)` (z `None`) and use it at every
   construction site: `git grep -n 'Geometry {' crates` (about 36 files, including
   `graph-core/src/post/**` and `graph-wasm/src/post/**`). One constructor, so the next field does not
   touch 36 files again.
b. `snapshot()`: `version: label_for(if z.is_some() { Dim::D3 } else { Dim::D2 })`, `z: geometry.z`. A z
   whose length is not the node count, or holds a non-finite value, is refused like any other misfit
   geometry (D9): reuse the snapshot's existing checks; if the contract already refuses it, test that.
   Update the two comments at `mod.rs:56-61` and `:66-70` to say what is now true.
c. Every 2D byte is unchanged: emit the golden snapshot list before and after
   (`binary/tests/pinned.rs` stays green unedited; `hashgate --seeds 8` hashes equal to develop's).
d. Tests: a unit test that a `Geometry` with z produces a 0.4 snapshot that round-trips (binary and
   canonical JSON) with the same z; one with a wrong-length z is refused; one with NaN in z is refused.
   A negative control: `GM_MUTATE_NODE_Z=1` roundtrip already exists (`quick.rows`
   `negctl-dim-z-mismatch`): keep it red.
e. Post passes (`post/**`) receive 2D geometry today. If a post pass would now see a 3D `Geometry`,
   it must either carry z through untouched or refuse with a named error, never drop it silently: state
   which, per pass, in one line each.

Checks (paste each last line): fmt --check, clippy -D warnings, `cargo test --workspace --no-fail-fast`,
wasm32 build of graph-core and graph-wasm, `hashgate --seeds 8` and the `GM_MUTATE_REFERENCE_DEGREE=9`
negctl (exit 1), `roundtrip --seeds 100`, the `negctl-dim-z-mismatch` row, `codegen --check`.

Paths you may touch: `crates/graph-core/**`, `crates/graph-wasm/**`, `crates/graph-cli/**` (only for
construction sites and tests), `docs/measurements/p13-3d-seam.md`. No contract change: if one looks
needed, stop and say so.

Done when: no `Geometry {` literal outside the constructor, the checks pass, and the return block lists
(a)-(e) one line each.
