# Perf studio-fa2bh report — the studio opens on ForceAtlas2 over Barnes–Hut

Job `perf-studio-fa2bh`. Commit measured `735a12f`, host load 7.86 on 20 cores, Chrome 154.0.8037.57
in Docker, software raster. Every number below was produced by a command this job ran; nothing is
quoted from elsewhere.

## 0. What this job was

The studio's default layout was `layout.forceatlas2`, the dense O(n²) port, ceiling 14 000 nodes.
The same layout with the repulsion summed over a Barnes–Hut tree,
`layout.forceatlas2.barnes_hut`, is O(n log n) with a 250 000-node ceiling
(`docs/measurements/perf-fa2bh.md`). The default now names it; the exact layout stays registered,
stays in the catalog, and stays the escape hatch for anyone who wants the exact picture.

There are exactly two places in the tree that name the default, and both were switched.

## 1. What changed

| file | line | change |
|---|---|---|
| `packages/graph-studio/src/state/settings.ts` | 159 | `layout: "layout.forceatlas2"` → `"layout.forceatlas2.barnes_hut"`, with a comment naming the ceiling and the escape hatch |
| `deploy/perf/run.py` | 35 | `FORCE_LAYOUT` the same id, so the perf driver times what the studio actually opens; the comment above `LARGE_LAYOUT` now says "the exact forceatlas2" for the 66 s measurement it always meant |

Nothing else in the sources moved. `CLAUDE.md:268` names no id — it points at `settings.ts` — so the
architecture note stayed true. No file under `crates/`, `packages/graph-render/`, `src/` or `app/`
was touched except what `scripts/studio.sh wasm` stages into `app/public/`.

## 2. The five tests that read the default, and why each fix keeps its intent

Measured, not guessed: with the id reverted to the exact layout all 11 tests in the four files below
pass (`# fail 0`), and with the switch in place `tests/*.test.ts` reports `# fail 5`. The five rows,
and the one-line reason for each fix:

1. `tests/parity.test.ts:72` "a parameter's default is a value the action accepts" — the row resolves
   every action with its default arguments against the test's own catalog fixture, and
   `layout.run`'s `id` default is `state.settings.layout` (`src/actions/run.ts:50`). **Fix:** the
   fixture catalog lists `DEFAULT_SETTINGS.layout` first instead of a hardcoded id — the row is about
   a default being *accepted*, not about which layouts the motor registers.
2. `tests/settings-portable.test.ts:40` "export, reset the panels, import: the export is
   byte-identical" and 3. `tests/settings-portable.test.ts:53` "resetting one panel restores only
   that panel's keys" — both act on `DRAWN`, whose `run.layoutId` was the old default. With the
   settings naming a different layout, `planOf` (`src/studio/pipeline.ts:225`) plans a relayout, the
   test's `refusingClient` refuses it, `arrange` throws before the settings are patched, and the
   reset silently does nothing — the rows then report the *unchanged* settings as the failure diff.
   **Fix:** `tests/drawn.ts:16` `RUN.layoutId: DEFAULT_SETTINGS.layout`, which is what the fixture
   means by "drawn" (a drawing under the studio's own default) and restores the coherence the two
   rows assume. No assertion in either test was weakened.
4. `tests/space-3d.test.ts:46` "a 3D snapshot is drawn: the run reaches the screen with its z column"
   — asserts `run.layoutId` on a start with no explicit layout. **Fix:** assert
   `DEFAULT_SETTINGS.layout`. The comment's claim that the id is "the catalog's first" was already
   only a coincidence of the old default matching the desk's first catalog entry; the studio asks for
   `settings.layout`, so the row now asserts what it means. The row's real subject, the z column
   reaching the screen, is untouched.
5. `tests/studio.motor.test.ts:72` "every action is logged with its command, its time and the digest
   of what it drew" — the regex `/400 nodes, 798 links · layout\.forceatlas2 \d+ ms/` stops matching
   once the log names `…forceatlas2.barnes_hut`. **Fix:** build the pattern from
   `DEFAULT_SETTINGS.layout` with its dots escaped. The row still requires the size, the exact id and
   a time in ms.

Left alone on purpose: `desk.ts:208`, `refusals.motor.test.ts:12`, `drawn.ts:21`, `recipe.test.ts`,
`registry.test.ts`, `console.test.ts`, `ui/*.test.tsx` list `layout.forceatlas2` in their own fake
catalog or fixtures and claim nothing about the default; `live-session.motor.test.ts:12`,
`refusals.motor.test.ts:82,98,126` and `session.motor.test.ts:35+` name the exact id explicitly and
test that id. `studio.motor.test.ts:69` and `session.motor.test.ts:19` assert the real wasm catalog
*contains* the exact id — still true, so still green.

## 3. Gates, each with its exit code

| command | exit | what it reported |
|---|---:|---|
| `scripts/studio.sh wasm` | 0 | staged `graph_wasm.wasm` (1 328 422 bytes) and `fixtures/` into `app/public` |
| `scripts/studio.sh check` | **0** | types (4 tsconfigs), `packages/graph-render` 402/402, `packages/graph-studio` 533/533, render tests 90/90, eslint `--max-warnings 0`, vite build |
| `scripts/studio.sh build` | 0 | `app/dist`, `built in 255ms` |
| `scripts/studio-smoke.sh` | **0** | 5/5 PASS: no exception, no console error, `store.error` null, no overlay, 400 nodes drawn |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | **1** (expected non-zero) | all 5 rows FAIL — the wasm import is broken on purpose, 0 nodes drawn |
| `scripts/studio-nav.sh` | **0** | 17/17 PASS, every camera row (drag, wheel, pinch, double-click, F/0/+/-/arrows/Escape, clamp, space+drag, middle-drag) |

The first `scripts/studio.sh check` run, before the fixes, exited 1 with `# fail 5` in
`packages/graph-studio`; the run above is after them.

## 4. The default open at 4000 nodes, before and after

Driver: `deploy/perf/run.py` under `scripts/studio-perf.sh`, headless, no studio server needed.
Both runs: `--cases 1000,2000,4000` (a scale measurement at DPR 1, so it **exits 1 by design** —
`perf-idle`, `perf-block` and `perf-fps` read NOT-RUN; the table is the result). Both were run alone
and sequentially on the same build, `--layout` naming the id so the two runs differ only in the
layout the graphs are laid out with — which is exactly what the default now selects.

`--layout` is honoured only together with `--cases` (`run.py:9`), so this is the one supported way
to A/B the two layouts without editing the file back and forth.

| nodes | layout | open ms | speed-up | JS mean ms | JS p95 ms | worst fps | React renders at open |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1000 | exact `layout.forceatlas2` | 315 | — | 0.710 | 2.3 | 12.6 | 739 |
| 1000 | `…barnes_hut` | **182** | 1.7× | 1.136 | 3.3 | 10.4 | 679 |
| 2000 | exact | 1191 | — | 0.767 | 4.3 | 26.3 | 672 |
| 2000 | `…barnes_hut` | **242** | 4.9× | 1.449 | 4.2 | 5.6 | 672 |
| 4000 | exact | 3187 | — | 7.641 | 20.7 | 59.6 | 672 |
| 4000 | `…barnes_hut` | **764** | 4.2× | 7.238 | 22.8 | 55.9 | 672 |

Source tables: `target/studio-perf/fa2bh-before/table.md`, `target/studio-perf/fa2bh-after/table.md`.

**The one number that is the point: the 4000-node open fell from 3187 ms to 764 ms, 4.2×.** The
1000-node case is only 1.7× because the fixed per-open cost dominates there.

Caveats, stated rather than smoothed over:

- Software raster in a container on a shared host, load 7.86/20. The ms columns are not a user's
  frame rate; `perf-p6.md` records the same row swinging 384 → 4643 ms across runs of identical code
  when other jobs were building. Both runs here were alone and sequential, and the direction and
  size of the win are far outside that spread, but a single sample per cell is a single sample.
- `perf-js` reads **4.3 ms FAIL before and 4.2 ms FAIL after** at 2000 nodes (budget ≤ 4 ms). It
  fails identically with and without the switch, so this job did not cause it and did not fix it,
  and it is reported here as the FAIL it is. `perf-p6.md` records this row at 4.6 ms FAIL for code
  that read 2.7 ms PASS when re-run alone on the same host; the ponytail note at
  `scripts/studio-perf.sh:20` names `perf-fps` specifically, and this job did not spend a third run
  chasing a row that did not move.
- JS mean/p95 at 4000 nodes are unchanged (7.6 → 7.2, 20.7 → 22.8): those columns time frames, and
  the frames draw the same picture either way. Only the open got cheaper.
- `worst fps` is noise here (10.4 vs 12.6 at 1000, 55.9 vs 59.6 at 4000): the raster, not the layout.
- `docs/measurements/perf-fa2bh.md` has no 4000-node row and no wasm table — its closest measured
  points are 1000 (43.59 ms BH vs 81.05 exact) and 5000 (263.41 vs 2 007.74), native single-threaded.
  That file is not in this job's paths, so it was not edited; this report is the browser/wasm-side
  measurement and does not overwrite it.

## 5. Consequences worth naming

- The default's node ceiling moves from 14 000 to 250 000. A graph the studio would have refused at
  open now draws. That is the intended win (the studio opens on a 4000-node graph for a reason), and
  it is also a behaviour change: nothing refuses between 14 001 and 250 000 any more.
- The layout the studio asks for at open is no longer the first entry of every hand-written test
  catalog. Four fixtures had encoded the coincidence and are fixed above; a fifth (`desk.ts:208`)
  still lists the exact id first and is left alone because it only feeds `layout.run`'s *choices*.
- The perf driver's `FORCE_LAYOUT` now matches the studio's default, so `deploy/perf/baseline.json`,
  which was recorded against the exact layout, is no longer comparable to a fresh run.
  `scripts/studio-perf.sh:84` does pass it, so this was checked rather than assumed: the only row
  that reads a baseline is `perf-fps` (`deploy/perf/rows.py:90`), and `--cases` runs no DPR 2 case,
  so that row read NOT-RUN in both runs here and the stale baseline changed no verdict. A later
  full gate run of `scripts/studio-perf.sh` should re-record the baseline.

## 6. Not run

- `deploy/perf/wasm-open.ts` (the wasm-only build/layout/`toBytes` timing) was not run: it times the
  motor with an explicit layout of its own and does not read the studio's default, so it would not
  have measured this switch.
- No browser/HUD check of the default open was needed for the gates above; `studio-smoke.sh` and
  `studio-nav.sh` drive the built studio in Chromium and both ran against this build.