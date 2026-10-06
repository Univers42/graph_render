# Job gpu-g1c-tick (build: the resident GPU tick — link, charge, centre, collide, integrate — its ms per tick at 1M, and its stress against the CPU mesh)

Why: the perf plan's GPU tier (1M nodes live in the browser, ≤ 100 ms per tick,
`docs/decisions/gpu-force-tier.md:13`). G1b put charge on the device and the link slice put link on
it, each as a one-shot probe checked against the fixture's per-pass column. This slice turns the
passes into one **resident tick**: buffers allocated once, the passes encoded in graph-core's
order every tick, one positions readback per tick. It then measures the tick's speed and the
quality of its layout.

Read first, in full:
- the plan's Task 3, `docs/superpowers/plans/2026-10-06-gpu-g1.md:1209-1405`;
- the ruling `docs/decisions/gpu-g1.md`, conditions 7-11;
- graph-core's tick, which is the order you copy exactly:
  `crates/graph-core/src/layout/force/particle_mesh.rs:175-208`. Also `motion.rs` (`merge`,
  `center`, `integrate`, `velocity_decay`, pins), `barnes_hut/sim.rs` (`center_shift`, the f64
  fold) and `particle_mesh/frame.rs`;
- this branch's per-pass probes: `crates/graph-sdk-js/src/gpu/charge.ts` (G1b) and `link.ts` (the
  link slice), with their buffers, pipelines and readback.

**Base.** Branch `gpu-g1c-tick` starts from `gpu-g1c`, which is G1b's tip plus the link slice.
- G1b stopped on its 1M `maxAbs` guard: 476× over at settled, 256× at start, while `rmsRel` stays
  under 1e-4. A separate analysis decides whether that guard is mis-derived. It is not yours: do
  not touch `bounds.ts` or G1b's two red `sdk:test` cases.
- The collide slice is being built in parallel. Its `runCollide` arrives in your branch later,
  when the orchestrator merges it and tells you. Until then, the tick runs with collide off, and
  that is reported as such (see step 4).

Settled decisions (from the plan, the ruling, and the orchestrator; follow them, don't reopen them):
1. **Order.** Decay `alpha`, then link and its merge, charge, centre, collide, gravity (skipped
   at zero, as graph-core does), integrate with collide's merge inside it. A constant string
   `link,charge,centre,collide,integrate` is compared in `the_pass_order_is_the_meshes`.
2. **Link in a real tick.**
   - The link probe folded `alpha = 1` into its uploaded strength, and read positions only
     because the fixture starts at rest.
   - In the tick, link reads what graph-core's link reads: positions plus current velocities,
     if `barnes_hut/link.rs` does so. Check it.
   - `alpha` is a per-tick uniform. Every pass scaled by `alpha` takes it from that uniform.
3. **Centre on the CPU** (plan `:1259-1268`).
   - Read the positions back once per tick. Fold the mean in node order in f64, as
     `center_shift` does, then upload the two scalars.
   - The module doc says this is the one place the arm is not resident end to end.
4. **The frame on the CPU, from the same readback.**
   - Write `gpu/frame.ts`, a transcription of `particle_mesh/frame.rs`: bounds → `step`, `h`,
     origin, `cells`.
   - The test `the_frame_matches_every_fixture_header` checks it, bit for bit, against the
     header of all eight fixtures. Their `step`, `h`, `originX`, `originY` and `cells` are
     graph-core's own output.
   - Its `Caveat:` line says it is a second copy, held to graph-core by that test.
   - Do not add a wasm export for it: that would be a public ABI change, which needs its own
     verdict.
5. **Stress** (orchestrator notes; the plan's `stress.ts` is dropped).
   - Use graph-cli's `stress(x, y, source, target)` (`crates/graph-cli/src/bench.rs:256`), made
     `pub(crate)`.
   - Add a test-instrument subcommand `graph-cli gpu-stress <fixture> <positions.f32> --ticks T`.
     It rebuilds the fixture's graph and positions, runs the CPU mesh for the same `T` ticks
     from the same start, computes stress on both position sets, and prints
     `PASS|FAIL ratio=<gpu/cpu>`.
   - It requires `0.5 ≤ ratio ≤ 2.0` (the plan's estimate). Exit 0 pass, 1 fail, 2 could not run.
   - If `stress` is quadratic, run it at 10k and 50k only, and say so.
   - `crates/graph-cli/src/measure.rs:106` is a different quantity; don't use it.
6. **One-tick displacement.** The same subcommand with `--ticks 1` also compares the GPU's
   positions after one tick with the CPU's: `rmsRel` of the displacement from the start
   positions.
   - Pin a ceiling from measurement, rounded up to two significant digits, under a guard of
     `1e-3`.
   - This is the check that makes the tick's controls fail (step 7).
7. **Faults** are `--break <name>` on the harness, passed as `runTick`'s second argument, never
   in a request field:
   - `tick-decay` drops `velocity_decay`;
   - `tick-order` runs charge before link.
   - Each must fail the one-tick displacement check.
   - The plan's `negctl-gravity` needs gravity above zero, and the fixtures have none, so drop it
     and record why.
8. **Budget.** At 1M, more than 100 ms per tick in the browser (median over 20 ticks after one
   warm-up) is the plan's stop: report the number and do not tune around it. A stress ratio
   outside `[0.5, 2.0]` is a stop too, and the report says whether the cause is the f32 spacing
   or the collide window.

Steps (TDD: each test written and seen RED before its code):
1. `gpu/motion.ts` and `gpu/kernels/motion.wgsl.ts`: the velocity merge, the integrate (decay
   `1 − 0.42`, pins overwrite), and the centre's host fold with its upload.
   - `the_centre_shift_is_the_cpus_fold`: compares the fold over 10 000 positions with an f64
     left fold, and with the fixture's own positions. Each comparison is bit for bit.
2. `gpu/frame.ts` and its test (decision 4).
3. Resident stages.
   - Refactor `charge.ts` and `link.ts` so each exposes a stage. A stage is built once per graph
     over shared resident buffers (positions, velocities, CSR) and encodes its dispatches into a
     given command encoder.
   - Each probe (`runCharge`, `runLink`) keeps its behaviour and its report byte for byte. The
     probes become thin users of the stages; G1b's and the link slice's rows must stay as they
     are.
4. `gpu/tick.ts` with `runTick(request, fault)` and the public `gpu/tick-api.ts` `probeTick(request)`
   (exported from `gpu.ts`). `TickRequest`/`TickReport` are the plan's (`:1232-1255`), plus:
   - `collide: boolean` in the report, false until the collide slice is merged;
   - `displacementRelRms` (decision 6).
5. Harness: `deploy/perf/gpu-mesh.py --pass tick --ticks T`.
   - It prints `ms/tick` and writes the final f32 positions to `target/gpu-tick/<fixture>.f32`
     for `gpu-stress`.
   - Keep every file at most 300 lines, with new modules as needed.
6. The `graph-cli gpu-stress` subcommand and its unit test:
   `a_gpu_stress_of_the_cpu_s_own_positions_is_one`.
7. Measure:
   - hardware: `ms/tick` at 1k, 10k, 50k, then 1M;
   - the 1M run is alone under the GPU lock, after a check that `free -g` shows at least 12 GB
     available;
   - stress at 10k and 50k, at `T = 300`;
   - one-tick displacement on every non-1M fixture;
   - both controls.
   - Append `## G1c — the resident tick, measured` to `docs/measurements/gpu-g1.md`.
8. When the orchestrator tells you collide is merged: plug its stage into the tick in its place
   in the order, set `collide: true`, re-measure, and update the doc.

Rules:
- Toolchain: only the wrappers (`scripts/orch/gr`, `scripts/orch/node-slim.sh`,
  `scripts/studio-probe.sh`, `scripts/studio.sh`). Never a bare `cargo`, `node`, `npm` or
  `docker run`. Pass `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3` to cargo jobs.
- **GPU safety.** Wrap every `GM_GPU=1` run in
  `flock -w 3600 /home/dlesieur/goinfre/orch/queue/gpu.lock`, after a `free -g` check of at least
  12 GB available. Only the 1M runs in step 7 may use a 1M fixture.
- Paths you may touch:
  - new: `crates/graph-sdk-js/src/gpu/{tick,tick-api,motion,frame}.ts`, `stage` modules you
    need under `crates/graph-sdk-js/src/gpu/`, `crates/graph-sdk-js/src/gpu/kernels/motion.wgsl.ts`,
    `crates/graph-sdk-js/test/gpu-{tick,motion,frame}*.mjs`, `deploy/perf/gpu_mesh_*.py`,
    `crates/graph-cli/src/gpu_stress.rs` (and a child module dir);
  - edits: `crates/graph-sdk-js/src/gpu.ts`, `gpu/charge.ts`, `gpu/link.ts`, `gpu/buffers.ts`,
    `gpu/pipelines.ts`, `gpu/readback.ts` (refactor only), `deploy/perf/gpu-mesh.py`,
    `deploy/perf/gpu_mesh_page.py`, `crates/graph-cli/src/bench.rs` (visibility only),
    `crates/graph-cli/src/main.rs` (registration, additive), an append to
    `docs/measurements/gpu-g1.md`.
  - Nothing in `crates/graph-core`, `crates/graph-wasm`, `packages/`, `app/`, `src/`, `bounds.ts`
    or `bounds-link.ts`. Nothing exported from `crates/graph-sdk-js/src/index.ts`.
- House limits:
  - each file at most 300 lines, each function at most 40, at most 4 parameters, nesting at
    most 3;
  - no type assertions (`as X`, `as unknown`, `: any`, `eslint-disable`);
  - no `f16`;
  - every heuristic, estimate or timeout carries a `Caveat:` line.
- Git:
  - commit in this worktree only, as `git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -m updated`,
    with no trailer;
  - push only this branch;
  - never merge, rebase, or touch develop or main.
- If a fact here is wrong on your branch, or the work needs a path outside the list, stop and
  report it (`status: blocked`). Nobody can answer questions mid-run.

Done when `scripts/orch/gate.sh target/rows-gpu-g1c-tick /home/dlesieur/Documents/graph_render/scripts/orch/rows/gpu-g1c-tick.rows`
writes a `summary.txt` with every row PASS, except G1b's two `sdk:test` cases, which you name. The
branch must be committed and pushed. If the collide slice has not been merged yet, return
`status: blocked` naming step 8 as the only work left.

Return (under 80 lines):
- status;
- branch tip;
- files with line counts;
- the RED runs;
- `ms/tick` per size, with the 1M median;
- the stress ratios;
- the displacement ceilings;
- each row's result;
- decisions taken;
- deviations.
