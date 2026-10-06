# Job gpu-g1b (agent build: the WGSL charge pass, held to G1a's fixtures)

Why: the GPU force tier's second slice. The browser must run the particle mesh's charge pass
(bounds, deposit, FFT with the kernel, field read) and match the CPU's own per-node velocity
increments. Those increments are the `.gmfx` fixtures G1a landed. The decision record's gate for
this slice (`docs/decisions/gpu-force-tier.md:74`) is: "rms bound, repeat equal, broken-butterfly
control red".

Read these first, in full:
- `docs/superpowers/plans/2026-10-06-gpu-g1.md`, Task 2 (G1b), `:886-1207`. It is the design.
- `docs/decisions/gpu-g1.md`, conditions 6, 7 and 8. **They win over the plan.**
- `fixtures/gpu/README.md`. It is the normative file format, and its "bounds" section holds the
  corrected derivations.

**This brief wins over the plan wherever the two disagree.** The orchestrator found six points
where the plan, as written, cannot run or contradicts itself. Each is settled below as "Settled".

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- G1a is on develop:
  - `crates/graph-sdk-js/src/gpu.ts` exports `loadFixture`, `scaleFor`, `Fixture`, `Pass`;
  - the loader is `crates/graph-sdk-js/src/gpu/fixture.ts`;
  - `fixtures/gpu/mesh-1k-{start,settled}.gmfx` are committed;
  - `graph-cli emit-gpu-fixtures --out target/gpu-fixtures` writes all eight files: 1k, 10k, 50k
    and 1M, start and settled.
- `crates/graph-sdk-js/src/index.ts` does not export `gpu.ts`. Keep it that way: no public SDK
  surface changes in this slice.
- SDK tests live in `crates/graph-sdk-js/test/*.test.mjs`, not under `src/`. `npm run sdk:test`
  globs them. G1a's `gpu-fixture.test.mjs` already has `the_two_scales_agree`; do not repeat it.
  `gpu-fixture.control.mjs` shows the house shape for a negative-control script: exit 0 when the
  fault is caught, and the row inverts that.
- The probe harnesses:
  - `scripts/studio-probe.sh NAME ARGS...` runs `deploy/perf/NAME.py` in `gm-chromium`.
  - The repo is mounted at `/w`.
  - `GM_GPU=1` adds `/dev/dri`; `GM_GPU=1 GM_GPU_BREAK=1` is the no-device control.
  - It forwards **no other env var** into the container. Only argv crosses.
  - It refuses to start without `app/dist/index.html`.
- `deploy/perf/webgpu.py` (G0) is the shape to copy:
  - `candidates(arm)` at `:140-164`;
  - `first_adapter` at `:206-223`;
  - `refusal` at `:236-250`;
  - exit codes 0 pass, 2 harness, 3 refused at `:253-280`.
  - `docs/measurements/gpu-adapter.md` shows both arms answer on this host: an AMD RX 6600
    (RADV) as hardware, SwiftShader as software.
- The CPU solve, which the GPU arm must reproduce operation for operation:
  - `crates/graph-core/src/layout/force/particle_mesh/mesh.rs:151-215`, `solve` and `deposit`;
  - `deposit.rs:186-193`, the weights;
  - `frame.rs:175-177`, `cell`;
  - `fft.rs`: `line` at `:134`, `forward`, and `inverse(buffers, &kernel.spectrum, cells)`.
    The kernel multiply happens inside `inverse`, and the passes run over `cells` lines, not
    always `P`.
  - `kernel.rs:66-76`: the spectrum is pre-scaled by `1/P²`;
  - `charge.rs:38-50`: the read is scaled by `params.charge * alpha`. The fixture's deltas are
    at `alpha = 1`.
- **Settled (1): the browser cannot import `.ts`.** Chromium does not strip types, so the plan's
  `import("/crates/graph-sdk-js/src/gpu.ts")` fails, `.ts` MIME or not.
  - Emit plain JS with the pinned `typescript` (5.7.3):
    `scripts/orch/node-slim.sh npx tsc -p crates/graph-sdk-js/tsconfig.json --noEmit false --rewriteRelativeImportExtensions --outDir target/gpu-js`.
  - The page imports `/target/gpu-js/gpu/charge.js`.
  - `deploy/serve.py` is therefore **not** changed.
  - Run this command before writing any kernel. If it does not emit runnable `.js` with rewritten
    imports, stop and report.

Settled points 2–6 (the orchestrator's rulings on the plan):
- **Settled (2): the fixture crosses by `fetch`, never as base64** (verdict condition 6).
  - The harness serves the repo root (`/w`) once, with `deploy/serve.py`'s `QuietHandler`, on
    `127.0.0.1`.
  - It writes a one-line `target/gpu-js/probe.html` and opens that page.
  - The page script `fetch`es `/target/gpu-fixtures/<name>.gmfx` and passes the `ArrayBuffer`.
- **Settled (3): the faults are argv, not env.** `studio-probe.sh` forwards no `GM_GPU_BREAK_*`.
  - `gpu-mesh.py` takes `--break <fault>` and hands it to the page.
  - The page calls `runCharge(request, fault)`, exported from `gpu/charge.ts`.
  - `gpu.ts`'s public `probeCharge(request)` calls `runCharge(request, undefined)`.
  - So no fault knob appears on the public `ChargeRequest` type.
- **Settled (4): the frame comes from the fixture header.** `h`, the origin, `P`, `cells` and
  `reach` are uploaded as uniforms.
  - The `bounds` kernel still runs. Its `min`/`max` over the `f32` positions must equal, bit for
    bit, the `min`/`max` the host takes over `Math.fround` of the fixture positions.
  - Report that as `boundsExact`, a field added to `ChargeReport`.
  - The rung choice from bounds to frame is G1c's, not this slice's. No TypeScript copy of
    `frame.rs`.
- **Settled (5): the ceilings.** The plan's `the_ceilings_are_ordered` (`B_max ≥ 2⁻¹¹·h`)
  contradicts its own 3.7e-5, because `2⁻¹¹·26.9 = 0.013`. Drop that test. The rule is:
  - **Guards** are derived, and a breach is a stop, not a re-tune:
    - `rmsRel ≤ 1e-4` at every fixture;
    - at the two 1M fixtures, `maxAbs ≤ |charge| · (2⁻¹¹/√3) / h²`, with `h` from the fixture
      header. The derivation is condition 7's. Below 1M the deposit quantum is not the
      dominant error, so there is no `maxAbs` guard there.
  - **Ceilings** are measured. `bounds.ts` holds one table keyed by `(arm, n, state)`: the
    measured `rmsRel` and `maxAbs` rounded **up** to two significant digits. The arm is held to
    its own row.
    - Caveat to write there: one device on one driver stack; a driver update re-measures, it
      does not widen.
  - The f32-spacing sentence is a `Caveat:`, not a test. It must read as written: a velocity
    error under 2⁻¹¹ is lost when integrated into a rim position at 12 000 units, so a ceiling
    under it gains nothing at 1M. It is not "unmeasurable": the delta itself is read in `f32`.
  - No `8e-7` and no `k = 4` anywhere (condition 8). The link ceiling is G1c's.
- **Settled (6): tests that test nothing are dropped.**
  - The plan's `a_fixed_point_round_trips` and
    `the_bounds_are_the_same_whichever_way_they_are_folded` would test a TypeScript mirror and
    `Math.min`, not the WGSL. Drop both.
  - The WGSL's exactness is held by `depositedUnits` on the device and by `negctl-weight`.
- `scripts/orch/rows/gpu-g1b.rows` is already on develop. Do not edit it. Its row names are the
  contract for the files and flags below.

Steps:
1. **Emit check.** Run the `gpu-js` row's command and confirm imports in `target/gpu-js/` end in
   `.js`.
2. **`gpu/types.ts`.** The local WebGPU declarations, only the members the code reads. No `f16`
   type. No import of `@webgpu/types`: the root `package.json` and lockfile must not move.
3. **RED: `test/gpu-wgsl.test.mjs`.**
   - Write `every_kernel_is_256_wide_and_f32`: every `@compute` entry in
     `src/gpu/kernels/*.wgsl.ts` is `@workgroup_size(256)`, and no kernel source mentions `f16`.
   - Put the scan function in `test/gpu-wgsl-scan.mjs` so the control can reuse it.
   - Write `test/gpu-wgsl.control.mjs`:
     - it rewrites one kernel's size to 64 and adds `enable f16;` to another, in memory;
     - it exits 0 only if the scan refuses both.
   - It must fail before the kernels exist.
4. **The kernels**, `src/gpu/kernels/*.wgsl.ts`, each exporting its WGSL as a string. All are
   `@workgroup_size(256)`, all are `f32`/`i32`/`u32`, and all except the deposit's atomics are in
   gather form:
   - `zero.wgsl.ts`: clears the `i32` density.
   - `bounds.wgsl.ts`: the `min`/`max` fold. Use two dispatches, or three past 65 535
     workgroups.
   - `deposit.wgsl.ts`: four `atomicAdd<i32>` per node.
     - The weights are fixed point at `scaleFor(n)`: three correctly rounded, the fourth
       `scale − w0 − w1 − w2`.
     - `cell` is a truncating cast and a `min` against `cells − 2`.
   - `fft.wgsl.ts`: one workgroup per line, `var<workgroup>` storage of `P` complex samples.
     - Each invocation strides `i, i+256, …`, and `if (i >= P)` guards `P = 128`.
     - Bit reversal is integer arithmetic.
     - Twiddles come from the fixture's table, narrowed to `f32` once on upload. Never use
       WGSL's `sin`/`cos`.
     - The inverse conjugates. Do not normalise.
     - Mirror `fft.rs` exactly: which lines each pass runs (`cells`, not `P`), the axis order,
       and the kernel multiply inside the inverse.
   - `read.wgsl.ts`: the CIC read in the order `(at, at+1, at+P, at+P+1)`, scaled by
     `charge * alpha`.
5. **Host: `buffers.ts`, `adapter.ts`, `charge.ts`, `bounds.ts`.**
   - `adapter.ts` checks the four limits in the plan's "Interfaces" section; a breach is a
     refusal.
   - `charge.ts` dispatches the stages in `mesh.rs` order. Then it does one
     `copyBufferToBuffer` per read-back column and `mapAsync`. It runs the whole pass twice for
     `repeatEqual`: byte-equal `density` and byte-equal delta.
   - `bounds.ts` holds:
     - the comparator: `rmsAbs`, `rmsRef`, `rmsRel` and `maxAbs` over the `2n` components, the
       reference narrowed with `Math.fround`;
     - the guard and ceiling tables of Settled (5);
     - `depositedUnits === n * scale` exactly.
   - `ChargeReport` is the plan's, plus `boundsExact`.
   - `test/gpu-bounds.test.mjs`, node-only:
     - `the_comparator_names_the_failing_case`: a report that breaches one ceiling has
       `pass: false` and names `(n, state)` and the check;
     - `every_ceiling_sits_under_its_guard`: each table row's `rmsRel` is ≤ 1e-4, and each 1M
       row's `maxAbs` is ≤ its guard.
   - `failures` names each breached check with one of these words: `rms`, `max`, `deposit`,
     `repeat`, `bounds`, `guard`.
   - Faults for `runCharge`, each changing one thing:
     - `butterfly` swaps the `hi` twiddle index;
     - `deposit` sends every node one cell off, weights unchanged;
     - `repeat` adds `+1` to one density cell in the second run, after its deposit and before its
       forward transform;
     - `weight` adds one quantum to the fourth weight, so each node deposits `scale + 1`;
     - `bounds` makes the fold skip the last block.
6. **`deploy/perf/gpu-mesh.py`.**
   - argv: `<arm> <dir> [--only 1k,10k,50k,1m] [--break <fault>]`.
   - It runs every `mesh-*.gmfx` in `<dir>` that `--only` keeps.
   - For each fixture it prints one line, `PASS <name> …` or `FAIL <name> <failures> …`, with
     every report field.
   - Exit 0 if all pass, 3 on any `FAIL` or any refusal, 2 if the harness could not run.
   - Under `hardware`, a software or fallback adapter is a refusal, as in `webgpu.py`.
7. **Measure, then fill the ceiling table.**
   - Run `charge-hardware` and `charge-software` once with the ceiling table empty: the guards
     only.
   - Write the measured rows into `bounds.ts`, then run every row.
   - If a guard is breached:
     - if `negctl-butterfly` is red, the kernel is wrong: fix it;
     - if it is not, stop and report both numbers. A 1M guard breach is the plan's stop.
8. **`docs/measurements/gpu-g1.md`**: append `## G1b — the charge pass, measured`. It holds:
   - the 14 measured rows (8 hardware, 6 software), with `rmsRel`, `maxAbs`, `rmsRef`,
     `depositedUnits` and the wall time per fixture;
   - each guard with its arithmetic;
   - the adapter strings;
   - each control's exit;
   - the Caveat line of Settled (5);
   - one line saying the 1M software rows were not run.

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/orch/node-slim.sh`; cargo only through
  `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr`. Never take
  `~/goinfre/orch/timed.lock`.
- Before any 1M browser run, check `free -g`: at least 12 GB must be available. Run one 1M probe
  at a time. Use `PERF_MEMORY=12g` only if the 1M row exits 137.
- No `as` type assertions in the SDK's GPU code (`no-assert`). No `any`. No `eslint-disable`.
  Each file is ≤ 300 lines and each function is ≤ 40 lines.
- Every heuristic, ceiling and fold-order choice carries a `Caveat:` line naming its failing
  input.
- Paths you may touch:
  - `crates/graph-sdk-js/src/gpu.ts` and `crates/graph-sdk-js/src/gpu/**`;
  - `crates/graph-sdk-js/test/gpu-*.mjs`;
  - `deploy/perf/gpu-mesh.py`;
  - `docs/measurements/gpu-g1.md`, by appending only.
- Nothing under `crates/graph-core`, `crates/graph-cli`, `fixtures/`, `app/`, `packages/`, `src/`
  or the root `package*.json` changes.

Done when:
- `scripts/orch/gate.sh target/rows-gpu-g1b scripts/orch/rows/gpu-g1b.rows` writes a
  `summary.txt` with every row PASS;
- the measured table is in `bounds.ts` and in `docs/measurements/gpu-g1.md`.

Return:
- the branch tip;
- the files, with line counts;
- the `fft.rs` pass structure you mirrored, with `file:line`;
- the 14 measured rows;
- each guard with its number;
- each row's result;
- every deviation.
