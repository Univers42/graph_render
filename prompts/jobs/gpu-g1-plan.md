# Job gpu-g1-plan (agent build, docs only: the implementation plan for the GPU tier's G1 slices)

Why: `docs/decisions/gpu-force-tier.md` (accepted 2026-10-03) moves the particle mesh's tick to
WebGPU as a new, explicitly requested tier, `layout.force.particle_mesh.gpu`. Its slice G0 landed
on 2026-10-05 (`docs/measurements/gpu-adapter.md`, `deploy/perf/webgpu.py`).
- **Hardware arm:** `GM_GPU=1` gives an AMD RDNA2 adapter (RX 6600), not a fallback. A 1M-`u32`
  compute pass with readback takes 8.7 ms.
- **Software arm:** SwiftShader, a fallback adapter. `maxComputeInvocationsPerWorkgroup` is 256
  there, against the hardware arm's 1024. The software arm has no `shader-f16`.

The next slices are G1a, G1b, G1c and G1d (the decision's table). This job writes no code. It
reads the CPU mesh and the repo's fixture and oracle patterns, and writes the plan the build jobs
will execute, with exact file:line facts. A plan that guesses is worse than none: every fact
carries its `path:line`, and an unknown is written as one.

Read, in full:
- `docs/decisions/gpu-force-tier.md`;
- `docs/decisions/compute-tiers.md`;
- `docs/measurements/gpu-adapter.md`;
- `crates/graph-core/src/layout/force/particle_mesh/` (every file), and how the force session runs
  a tick through `graph_core::exec::Runner` (`crates/graph-cli/src/bench/tick/passes.rs` is a
  `Runner` that wraps every pass);
- the fixture pattern: `graph-cli emit-spectral-fixtures` (`crates/graph-cli/src/oracle_python/cli.rs:17`,
  `cli/run.rs:15`) and how its `--check` / differential works;
- `deploy/perf/webgpu.py` (the browser harness pattern G1b reuses);
- `prompt.md` §6 (determinism D1–D10) and the house limits in `CLAUDE.md`.

Write `docs/superpowers/plans/2026-10-06-gpu-g1.md`, in the shape of
`docs/superpowers/plans/2026-10-05-dag-lanes-layout.md`: header, Global Constraints, Review Focus,
then one task per slice, G1a to G1d. Each task names:
- **Files:** exact paths to create or modify, with line ranges for each modification.
- **Interfaces:** exact Rust and TypeScript signatures, including what graph-core must expose so
  graph-cli can run one pass alone. If graph-core needs a new `pub` item:
  - say which, and why no existing item serves;
  - mark the item for a risk verdict before code (house rule: a public surface change).
- **The fixture format (G1a):**
  - nodes and edges;
  - the mesh parameters (`P`, spacing);
  - positions at start and settled for n = 1k, 10k, 50k;
  - per pass (charge, link, collide), the velocity increment that pass adds from zero velocity.

  Name the generator. One generator, both arms load the same file: the repo's oracle rule.
  Name the files under `fixtures/` and their sizes, estimated and labelled as estimated.
- **The bound each G1b/G1c pass is held to:**
  - how it is measured (rms and max of the per-node difference, relative to what);
  - which number in the decision record it is checked against (the deposit quantum 2⁻¹¹, f32
    spacing 2⁻¹⁰ at 12,000 units).
- **The workgroup size:** 256 on both arms (the G0 finding). If a kernel needs more, say so.
- **Tests and gate rows**, with the negative control for each, in the files' own style. The rows
  file for each slice, ready to copy into `scripts/orch/rows/gpu-g1<x>.rows`.
- **Steps**, each one action, with the code a build job writes. Rust code must meet the house
  limits:
  - ≤ 40 lines per function;
  - ≤ 4 parameters;
  - ≤ 300 lines per file;
  - nesting ≤ 3;
  - `Caveat:` on every heuristic.
- **Where it stops:** G1b waits on G1a's fixtures, and a bound that fails at 1M is the stop
  (decision record, Consequences).

Then, in a last section "Brief for G1a", write the OpenCode job brief for G1a alone. Model it on
`prompts/jobs/dag-lanes.md`: Why, Facts with `path:line`, Rules, Paths you may touch, Done when,
Return.

Rules (beyond `scripts/orch/common.md`):
- You may run `git grep`, `sed -n`, read any file, and run
  `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core particle_mesh`
  to see the current tests pass. Do not run benches, probes or the hash gate.
- Paths you may touch: `docs/superpowers/plans/2026-10-06-gpu-g1.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-gpu-g1-plan scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS;
- the plan has the four tasks and the G1a brief, and `grep -c 'path:line\|:[0-9]' ` finds a
  `path:line` in every Facts bullet.

Return:
- the branch tip;
- for each slice, one line: files, the new `pub` items (or none), the bound, the rows;
- every unknown you could not resolve;
- every deviation.
