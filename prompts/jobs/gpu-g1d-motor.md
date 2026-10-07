# Job gpu-g1d-motor (agent build: the GPU tier's live input: three graph-core accessors, four wasm exports, the SDK views)

Why: the GPU force tier must tick a **live** session, not only a `.gmfx` fixture. It needs the
session's simple graph and its velocities, and neither reaches the SDK today. The verdict is
`docs/decisions/gpu-g1d.md`, which rules on Amendment 4 sections A and B in
`prompts/jobs/gpu-g1d-verdict.md`. This job builds sections A and B and the SDK's read views over
them, and nothing else. The GPU handle, the studio and the browser gates are later slices.

Rule for every edit: if an edit fails twice, read the whole file and write it once.

## Facts (develop, 2026-10-07; re-check each before you rely on it)

**graph-core.**
- `crates/graph-core/src/layout/force/session.rs` is 298 lines.
  - Its child modules are declared at `:46-57` (`mod carry; … mod warm;`, then `#[cfg(test)] mod tests;`).
  - `ForceSession` keeps the simulation in `self.sim` (`barnes_hut::sim::Sim`).
  - `self.sim.graph` is the `SimpleGraph` (`force/mod.rs:71-80`), with `pub(crate)` fields
    `lo: Vec<u32>`, `hi: Vec<u32>`, `strength: Vec<f64>`.
  - `self.sim.vx` and `self.sim.vy` are the velocity columns (`Vec<f64>`, one per node).
  - The position readers `xs()`/`ys()` are at `session.rs:222-230`.
- `crates/graph-core/src/layout/force/session/mesh_probe.rs:99-128` is the pattern for a child
  module reading `self.sim.graph`. `mesh_probe()` returns `lo`/`hi`/`strength` clones.
- Test helpers to copy: `session/mesh_probe/tests.rs:1-30` (the gate's model, a mesh session,
  `bits`).

**graph-wasm.**
- `crates/graph-wasm/src/session.rs` is 296 lines.
  - `column()` (`:218-247`) is the pattern for a `(ptr, len)` pair: C3 says an empty column reads
    `0`, and the `to_wire` conversion applies.
  - `with()` (`:263-272`) is `pub(crate)`.
- `crates/graph-wasm/src/exports/session.rs:217-242` holds the two exports to mirror, with
  `errors::clear()` and `refuse(code, 0)`.
- `crates/graph-wasm/src/exports/mod.rs:25-33` declares the export modules.
- Native tests live under `crates/graph-wasm/src/session/tests/` (`fixture.rs` is the shared setup).
- `ABI_VERSION` stays `2`: an added export never bumps it (`docs/contract/wasm-abi.md:31`).

**Docs.** `docs/contract/wasm-abi.md`:
- the two `gm_force_session_column_*` rows are in the exports table;
- the lifetime row "A force session's column addresses" is at `:198`.

**SDK.**
- `crates/graph-sdk-js/src/wasm.ts:55-56` and `:78-79` hold `RawExports` and `EXPORT_NAMES`.
- `crates/graph-sdk-js/test/stub-module.mjs:18-30` holds `REQUIRED_EXPORT_NAMES`, and a guard
  test fails when it is behind `EXPORT_NAMES`.
- `crates/graph-sdk-js/src/force-columns.ts:28-38` (`checkedColumn`) and `:115-131` (`#column`)
  are the pattern for a checked view.

## What to build

1. **graph-core**, new file `crates/graph-core/src/layout/force/session/columns.rs`, declared as
   `mod columns;` in `session.rs`. Three `&self` methods on `ForceSession`, each with a `///` doc
   naming what it borrows and that it is good until the next `&mut` call:
   - `pub fn simple_edges(&self) -> (&[u32], &[u32], &[f64])`: `(lo, hi, strength)` of `self.sim.graph`;
   - `pub fn vxs(&self) -> &[f64]` and `pub fn vys(&self) -> &[f64]`: `self.sim.vx`, `self.sim.vy`.

   No new type and no `&mut` method. Tests go in `session/columns/tests.rs`, attached by
   `#[cfg(test)] #[path = "columns/tests.rs"] mod tests;` at the end of `columns.rs`:
   - `simple_edges_are_the_probes_own`: on mesh sessions for gate seeds 3, 5 and 9, the three
     slices are bit-equal to `mesh_probe()`'s `lo`, `hi`, `strength`;
   - `a_fresh_session_is_at_rest`: every `vxs`/`vys` value is `0.0`, with length `n`;
   - `the_velocities_move_with_a_tick`: after `step(1)` some velocity is non-zero, and `vxs().len()` equals `xs().len()`;
   - `the_accessors_leave_the_next_tick_byte_identical`: build two identical mesh sessions, call
     all three accessors on one, `step(3)` both, and assert that `xs`, `ys`, `vxs` and `vys` are bit-equal.
2. **graph-wasm**, new file `crates/graph-wasm/src/session/handoff.rs`, declared beside the other
   `session.rs` child modules:
   - `pub fn velocity(id: u32, axis: u32, want_ptr: bool) -> Result<u32, Code>`: axis `0` is
     `vxs`, `1` is `vys`, any other axis is `Code::IndexOutOfRange`;
   - `pub fn edge(id: u32, column: u32, want_ptr: bool) -> Result<u32, Code>`: column `0` is
     `lo`, `1` is `hi`, `2` is `strength`, any other column is `Code::IndexOutOfRange`. With
     `want_ptr == false` it returns the element count `m`, which is the same for all three.

   Both follow `column()` exactly: C3 empty → `Ok(0)`, `to_wire`, `with`. New file
   `crates/graph-wasm/src/exports/session_handoff.rs`, declared in `exports/mod.rs`, holds four
   `#[unsafe(no_mangle)] pub extern "C"` exports. Each copies the doc and `// SAFETY:` line
   pattern of `gm_force_session_column_ptr`:
   - `gm_force_session_velocity_ptr(session, axis)` and `gm_force_session_velocity_len(session, axis)`;
   - `gm_force_session_edge_ptr(session, column)`;
   - `gm_force_session_edge_len(session)`, which answers `edge(session, 0, false)`.

   Native tests in new file `crates/graph-wasm/src/session/tests/handoff.rs`, declared in
   `session/tests.rs`:
   - `the_velocity_columns_are_the_sessions_own`: the length is `n`, and after a tick the values
     read through the address equal `ForceSession::vxs()` bit for bit (read through `with`);
   - `the_edge_columns_are_the_simple_graph`: the three columns' values equal `simple_edges()`;
   - `a_bad_axis_or_column_reads_zero` and `a_dead_session_reads_zero` (`InvalidSession`), through
     the exports, as `refusals.rs` does for the position columns.
3. **Docs.** In `docs/contract/wasm-abi.md`:
   - four rows after the `gm_force_session_column_len` row, in the table's own style:
     signature, meaning, refusals, the C3 empty rule, and the C7 lifetime;
   - the lifetime row at `:198` extended: the velocity addresses are good only until the
     session's next `gm_force_session_tick` or `gm_force_session_grow`, because a mesh tick swaps
     `sim.vx`/`sim.vy` with scratch; the edge addresses are good until the next grow;
   - no row names a GPU engine: `particle_mesh_gpu` never appears in this file (condition 3).
4. **SDK.**
   - Add the four exports to `RawExports` and `EXPORT_NAMES` in `src/wasm.ts`, and to
     `REQUIRED_EXPORT_NAMES` in `test/stub-module.mjs`.
   - **Velocities, with the positions' D9 gate.** These are the verdict's conditions 2 and 7 in
     `docs/decisions/gpu-g1d.md`.
     - Extend `src/force-columns.ts`'s `ForceColumns`:
       - `readVelocities(): { vxs: Float64Array; vys: Float64Array }` uses the velocity exports,
         is checked like `checkedColumn`, and has its own cached views. Name the column
         "the velocity column (axis N)" in the errors.
       - `assertFinite()` also scans both velocity columns. A non-finite velocity is
         `TamperedGeometryError`, worded like the position one.
       - `forget()` drops the velocity views too. The mesh tick swaps `sim.vx`/`sim.vy` with
         scratch (`particle_mesh/motion.rs:184`, `:192`), so a view is stale after a tick, as
         the position views are.
       - `read()` stays exactly as it is.
     - Add one public method to `src/force.ts`'s `ForceSession`, after `positions()`:
       `velocities(): { readonly vxs: Float64Array; readonly vys: Float64Array }`.
       - It is `requireLive()` + `this.#own((columns) => columns.readVelocities())`.
       - Its doc is at most 8 lines: zero-copy and writable, the D9 gate on the next read and the
         next tick, stale after a tick or a grow.
       - `force.ts` must stay at or under 300 lines.
   - **The simple graph.** New file `src/force-handoff.ts`, not exported from `index.ts`, with one function:
     `simpleEdges(loaded: Loaded, id: ForceSessionId): { lo: Uint32Array; hi: Uint32Array; strength: Float64Array }`.
     - Copy the three columns out (`.slice()`), because a grow can move them.
     - `m = 0` answers three empty arrays, not a refusal.
     - A non-zero length with a zero address, a misaligned address or an out-of-bounds range is
       `AbiContractError`.
     - A refusal becomes `InvalidSessionError` or `ForceSessionRefusedError`, as `ForceColumns.#column` does.
   - **Tests**, in new file `test/force-handoff.test.mjs`. It needs the built wasm, so copy
     `test/motor.test.mjs`'s loading and its skip-free failure:
     - `the_edges_are_the_simple_graph`: build a 4-node graph with one duplicated edge (the
       second copy has a different strength) and one self-loop. Expect `m` = the distinct
       non-loop edges, the first strength kept, and `lo < hi` on every edge;
     - `a_fresh_session_is_at_rest`: `velocities()` is all `0`, with length `n`;
     - `velocities_move_after_a_tick`;
     - `a_non_finite_velocity_written_through_the_view_is_refused`: write `NaN` through
       `velocities().vxs[0]`, then require `TamperedGeometryError` from the next `velocities()`
       **and** from the next `tick(1)`.

## Constraints

- graph-core: no new dependency and no `unsafe`. At most 40 lines per function, 4 parameters,
  300 lines per file and nesting depth 3.
- No `#[allow]` and no `#[doc(hidden)]`. No type assertions in the TS (`as X`, `as unknown`, `any`).
- The motor's output must not move: `hashgate --seeds 8` keeps its digests.

Paths you may touch:
- `crates/graph-core/src/layout/force/session.rs` (one `mod` line), `…/session/columns.rs`, `…/session/columns/tests.rs`;
- `crates/graph-wasm/src/session.rs` (one `mod` line), `…/session/handoff.rs`,
  `…/session/tests.rs` (one `mod` line), `…/session/tests/handoff.rs`,
  `…/exports/mod.rs` (one `mod` line), `…/exports/session_handoff.rs`;
- `docs/contract/wasm-abi.md`;
- `crates/graph-sdk-js/src/wasm.ts`, `crates/graph-sdk-js/src/force-columns.ts`, `crates/graph-sdk-js/src/force.ts`
  (the one method), `crates/graph-sdk-js/src/force-handoff.ts`,
  `crates/graph-sdk-js/test/stub-module.mjs`, `crates/graph-sdk-js/test/force-handoff.test.mjs`.

Nothing else.

Done when `scripts/orch/rows/gpu-g1d-motor.rows` passes. Run its rows by name filter as you go;
the orchestrator runs the whole file.

Return: the files changed, each test name, every deviation from this brief.
