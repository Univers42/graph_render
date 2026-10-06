# Job gpu-g1c-floor (build: the collide guard's input floor, Amendment 3, and the 1M link and collide ceiling rows)

Why: the perf plan's GPU tier (1M nodes live in the browser). At 1M the collide arm fails
`mesh-1m-start` on its guard, 7.505e-4 against 3.387e-4. An analysis showed why: the arm sits
exactly on the error that narrowing the positions to f32 causes, and the guard does not count that
error. The ruling is `docs/decisions/gpu-g1.md`, **Amendment 3**. Read it in full first; its
"Ruling" list is this job's acceptance criteria.

Read first, in full:
- `docs/decisions/gpu-g1.md`, Amendments 1 and 3;
- `crates/graph-sdk-js/src/gpu/bounds-collide.ts` (the guard, `maxContacts`, the verdict and the
  report), `crates/graph-sdk-js/src/gpu/collide.ts` (`gridFor`, `cellOf`, the `Grid`) and
  `crates/graph-sdk-js/src/gpu/fixture.ts`;
- the CPU pass you transcribe once more, in f64: `resolve` in
  `crates/graph-core/src/layout/force/particle_mesh/collide.rs:235-257`;
- `harness/gpu-collide-floor.py`, the numpy analysis behind the amendment. Its (b) column is the
  number your host floor must reproduce.

**Base.** The branch `gpu-g1c-floor` starts from `gpu-g1c` (97ffa0a5). That tip is develop with G1b
landed, plus the link and collide slices. `sdk:test` is fully green there (342/342): every row
below expects zero failures.

Facts measured by the orchestrator (hardware arm, `amd/rdna-2`, 2026-10-06, at gpu-g1c 73fcda18):

| pass | fixture | rmsAbs | rmsRef | rmsRel | maxAbs |
|---|---|---|---|---|---|
| link | `mesh-1m-settled` | 1.51605e-5 | 145.159 | 1.04441e-7 | 9.76562e-4 |
| link | `mesh-1m-start` | 5.95736e-5 | 801.34 | 7.43424e-8 | 4.88281e-4 |
| collide | `mesh-1m-settled` | 6.75245e-4 | 36.1809 | 1.8663e-5 | 0.209986 |
| collide | `mesh-1m-start` | 2.39858e-4 | 0.319593 | 7.505127e-4 | 1.61097e-3 |

The numpy floor, (b) of `harness/gpu-collide-floor.py`, as absolute rms: 6.893e-6 at
`mesh-1k-start`, 5.426e-5 at `mesh-50k-start`, 2.399e-4 at `mesh-1m-start`, 6.752e-4 at
`mesh-1m-settled`. Its `k_c`: 8, 8, 8, 119.

Steps (TDD: each test is written and seen RED before its code):
1. **The host walk, once.** Create `crates/graph-sdk-js/src/gpu/collide-host.ts`.
   - Move `maxContacts`, `bucketOf` and `readsBuckets` out of `bounds-collide.ts` into it,
     unchanged.
   - `bounds-collide.ts` re-exports `maxContacts`, so the test's import path keeps working.
   - Factor the grid walk that `maxContacts` does into one function that visits each node's
     contacts. `maxContacts` and the new function below both use it, so no walk is written
     twice.
2. **`hostCollide(posX, posY, grid)`** returns `{ x: Float64Array, y: Float64Array }`.
   - It is the CPU's `resolve` in f64, per node, over the node's contacts as the walk finds them:
     `l = dx² + dy²`; skip when `l` is NaN or `l ≥ d2`; `dist = √l`;
     `push = (reach − dist) / dist · 0.5`; add `dx·push` and `dy·push`.
   - The cell of every node comes from the positions it is given.
   - It does not port the coincidence jiggle. An exactly zero axis contributes its zero
     component.
   - Its `Caveat:` line says so, and why that is small: the jiggle is at most 5e-7 per axis.
3. **`collideFloor(posX, posY, grid)`** returns the floor's absolute rms over the `2n`
   components (the order `components()` uses): `hostCollide` on the positions narrowed with
   `Math.fround`, against `hostCollide` on the f64 positions.
4. **The guard.**
   - `collideGuard(k_c, reach, rmsRef, floorRms)` returns
     `1e-4 + (floorRms + k_c·5·2⁻²³·reach/2) / rmsRef`. At `rmsRef === 0` it keeps today's
     answer.
   - `collideVerdict`'s input gains `floorRms`. The guard failure text names the floor:
     `… over 1e-4 + (floor=<floorRms> + k_c=<k>·5·2⁻²³·<P>)/<rmsRef> = <guard>`. It still
     starts with `guard (n=…, state=…)`.
   - `collideReport` computes the floor next to `k_c`.
   - Update the module doc's guard section to Amendment 3's form, citing the amendment.
5. **Ceiling rows.**
   - `CollideCeiling` gains `floorRms`. Fill every existing row from `collideFloor` on that row's
     fixture, rounded **down** to three significant digits.
   - Add `hardware:1000000:0` and `hardware:1000000:1` from the table above. `rmsRel` and `maxAbs`
     are rounded **up** to two significant digits. `k_c`, `rmsRef` and `floorRms` come from your
     host run on the 1M fixtures.
   - In `bounds-link.ts`, add `hardware:1000000:0` and `hardware:1000000:1` from the link rows of
     the table, rounded up to two significant digits, in that file's row shape.
6. **Tests** in `crates/graph-sdk-js/test/gpu-collide.test.mjs`, or a new
   `gpu-collide-floor.test.mjs` if that file would pass 300 lines:
   - `the_host_collide_reproduces_the_fixture`: on both committed 1k fixtures,
     `hostCollide(posX, posY, gridFor(posX, posY))` against `fixture.delta.collide` has `maxAbs ≤ 1e-12`.
   - `the_collide_floor_is_the_narrowing_alone`: on a toy of three nodes whose coordinates are exact
     in f32, the floor is exactly 0. On the committed `mesh-1k-start`, the floor is within 5% of
     6.893e-6.
   - `the_1m_start_fails_without_its_floor`: `collideVerdict` at the measured 1M-start numbers
     (`rmsRel` 7.505127e-4, `rmsRef` 0.319593, `k_c` 8, `reach` 32, `maxAbs` 1.61097e-3, `n` 1e6,
     `state` 0, an arm with no row) fails with `guard` at `floorRms: 0`. It passes at
     `floorRms: 2.399e-4`. This is the amendment's negative control.
   - Update `the_collide_guard_reads_the_measured_crowd` and
     `every_collide_ceiling_sits_under_its_guard` to the four-argument guard. The latter uses each
     row's `floorRms`.
7. **The host check over the emitted fixtures.** `harness/gpu-collide-host.mjs`, at most 120 lines,
   run as `node --experimental-strip-types harness/gpu-collide-host.mjs <dir> [--only 1k,10k,50k,1m]`.
   - For every `.gmfx` it prints one line:
     `PASS|FAIL <name> reproduce=<maxAbs> floorRms=<…> k_c=<…> rmsRef=<…>`.
   - It FAILs when `reproduce > 1e-12 · max(1, max|reference|)`.
   - Exit 0 all pass, 1 any fail, 2 could not run.
   - Running it over the 1M fixtures is allowed: it is host-only, with no GPU. Check first that
     `free -g` shows at least 12 GB available.
8. Append `## G1c — the 1M rows and the collide floor` to `docs/measurements/gpu-g1.md`:
   - the table above;
   - your host floor per fixture against the numpy (b) column;
   - the new guard per fixture;
   - each row's result.

Rules (beyond `scripts/orch/common.md`):
- Toolchain: only the wrappers (`scripts/orch/node-slim.sh`, `scripts/orch/gr`,
  `scripts/studio-probe.sh`, `scripts/studio.sh`). Never a bare `node`, `npm`, `cargo` or
  `docker run`.
- **GPU safety.** Wrap every `GM_GPU=1` run in `flock -w 3600 /home/dlesieur/goinfre/orch/queue/gpu.lock`,
  after a `free -g` check of at least 12 GB available. Never run a 1M fixture on the GPU: the 1M rows
  are the orchestrator's.
- Paths you may touch:
  - new: `crates/graph-sdk-js/src/gpu/collide-host.ts`, `crates/graph-sdk-js/test/gpu-collide-floor.test.mjs`,
    `harness/gpu-collide-host.mjs`;
  - edits: `crates/graph-sdk-js/src/gpu/bounds-collide.ts`, `crates/graph-sdk-js/src/gpu/bounds-link.ts`
    (the two rows only), `crates/graph-sdk-js/test/gpu-collide.test.mjs`, and an append to
    `docs/measurements/gpu-g1.md`.
  - Nothing else. Not the kernels, not `collide.ts`, not `pass-report.ts`, not `deploy/`, not
    `crates/graph-core`. Nothing exported from `crates/graph-sdk-js/src/index.ts`.
- House limits:
  - each file at most 300 lines, each function at most 40, at most 4 parameters, nesting at
    most 3;
  - no type assertions (`as X`, `as unknown`, `: any`, `eslint-disable`);
  - no `f16`;
  - every heuristic carries a `Caveat:` line.
- Git:
  - commit in this worktree only, as `git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -m updated`,
    with no trailer;
  - push only this branch (`git push -q origin HEAD`);
  - never merge, rebase, or touch develop, main or gpu-g1c.
- If a fact here is wrong on your branch, stop and report it (`status: blocked`). Report a
  floor more than 5% away from the numpy one, or a reproduction over `1e-12`, the same way.

Done when `scripts/orch/gate.sh target/rows-gpu-g1c-floor /home/dlesieur/Documents/graph_render/scripts/orch/rows/gpu-g1c-floor.rows`
writes a `summary.txt` with every row PASS, and the branch is committed and pushed. The 1M GPU rows
are in `gpu-g1c-floor-1m.rows`, which only the orchestrator runs.

Return (under 60 lines):
- status;
- branch tip;
- files with line counts;
- the RED runs;
- the host floor per fixture against numpy (b);
- the new rows;
- each row's result;
- deviations.
