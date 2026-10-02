# Perf P1 report — instrument the baseline

Shape: `prompt.md` §12. Plan: `prompts/perf-plan.md` P1. Branch `perf-p1`, worktree
`$GM_SCRATCH/wt/perf-p1`, base develop `5936309`. The measurements themselves are in
`docs/measurements/perf-p1-baseline.md`; this report is about what the phase changed and
what was checked.

## 0. What this phase was

Official profilers pointed at the force tick and the studio, so every later phase fixes a
measured hot spot and states its target against a number. No motor code changed: every
graph-core and graph-cli edit is a new instrument, and no hash moves.

## 1. Authorization compliance

| Plan item (P1 Changes) | File | Status |
|---|---|---|
| profilers in a Debian image FROM ge-rust | `docker/profile.Dockerfile` (new) | done; valgrind and hyperfine, **perf omitted** (deviation 1) |
| a script that runs them | `scripts/orch/profile.sh` (new) | done |
| a per-pass timer in `graph-cli bench` | `crates/graph-cli/src/bench/tick.rs` (new), `bench.rs`, `command.rs`, `main.rs` (one line each) | **deviation 2**: a `tick` subcommand times single ticks; the per-pass split comes from callgrind |
| a counting `GlobalAlloc` in graph-cli's test build | `crates/graph-core/tests/tick_alloc.rs` (new) | **deviation 3**: in graph-core's tests, as its own test binary |
| 100k and 1M in the synthetic generator | `packages/graph-studio/src/source/synthetic.ts` (`MAX_NODES` 50 000 → 1 000 000), `tests/ingest.test.ts` | done; the Rust side reuses the phase-9 scale generator (deviation 4) |
| CDP tracing and a React Profiler hook in `deploy/perf`, drivers at 50k, 200k, 1M | `deploy/perf/react-hook.js` (new), `probes/frame.js`, `run.py`, `rows.py`, `drivers/hook.js`, `scripts/studio-perf.sh` (`--cases`) | **deviation 5**: a `longtask` PerformanceObserver and a DevTools-style commit hook |
| exit document | `docs/measurements/perf-p1-baseline.md` (new) | done |
| this report | `docs/reports/perf-p1.md` (new) | done |

Deviations, each named:

1. **perf is SKIP.** `perf_event_paranoid` is 4 on host dlesieur42 and the Docker daemon is
   rootless, so `perf_event_open` is refused in any container. Callgrind (exact instruction
   counts, simulated cache) and dhat stand in; there is no flamegraph SVG, `callgrind.out`
   opens in kcachegrind.
2. **No `--passes` timer.** graph-core has no clock (D-rules), and a timer around each pass
   would need one threaded through the session. Callgrind's inclusive per-pass split is exact.
3. **The counting allocator lives in graph-core's tests**, in its own binary, so it counts
   only the session it wraps.
4. **No new Rust fixture.** `graph-cli tick --n` builds the phase-9 scale model in process;
   that generator is already the committed artefact for `n1m.json`.
5. **No CDP trace, no React Profiler.** The Profiler API does nothing in React's production
   build, which is what the studio ships; the hook counts commits and renders in that build.
6. **`drivers/hook.js` `maxNodes` 20 000 → 1 000 000.** The plan raises it in P7, but the P1
   cases at 50k, 200k and 1M cannot run under a 20 000 cap. It only lets a case run; no gating
   row's threshold moved.
7. **`deploy/perf/run.py` keeps the cases measured before a timeout.** A 1M case that did not
   open used to fail the whole run as "could not run".

## 2. The ledger diff

None. No layout, analysis or post entry was added or changed; `capabilities --check` is in the
gate table below.

## 3. The gate table

Re-run at report time on the final tree, with `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3`:
`scripts/orch/gate.sh $GM_SCRATCH/gate-perf-p1 <quick.rows + capabilities + tick-alloc>`.

| Row | Expect | Exit | Time | Verdict |
|---|---|---:|---:|---|
| `fmt` | 0 | 0 | 1 s | PASS (after `cargo fmt` reflowed one closure in `bench/tick.rs`; the first run was red on it) |
| `clippy -D warnings` | 0 | 0 | 0 s, incremental | PASS |
| `cargo test --workspace --no-fail-fast` | 0 | 0 | 997 s | PASS: 1 591 passed, 0 failed, 12 ignored, over 20 result lines |
| `wasm32-core` | 0 | 0 | 3 s | PASS |
| `hashgate-8` | 0 | 0 | 4 s | PASS |
| `negctl-degree` (`GM_MUTATE_REFERENCE_DEGREE=9`, expects 1) | 0 | 0 | 4 s | PASS |
| `negctl-dim-z-mismatch` | 0 | 0 | 1 s | PASS |
| `force-gate-4` | 0 | 0 | 0 s | PASS |
| `negctl-force-gravity` | 0 | 0 | 1 s | PASS |
| `capabilities --check` | 0 | 1 | 0 s | **FAIL**: every gated row reads "hashgate ran 8 seeds, need 1000" and "no oracle record". The evidence is pinned to the tree, and this tree's only records are the 8-seed rows above. Per the merge floor (`CLAUDE.md`, 2026-09-29) the full gate runs once on develop; this row turns green there or becomes a repair task |
| `tick-alloc` (`cargo test -p graph-core --test tick_alloc`) | 0 | 0 | 1 s | PASS |

Studio, on the same tree:

| Command | Exit | Verdict |
|---|---:|---|
| `scripts/studio.sh check` | 0 | PASS: tsc ×4; graph-render 341/341, graph-studio 503/503, UI render 59/59, 0 skipped; eslint `--max-warnings 0`; vite build |
| `scripts/studio-smoke.sh` | 0 | PASS (400 nodes drawn, no row left unrun) |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 | PASS (the negative control fails as it must) |
| `scripts/studio-perf.sh --label p1-gate` | 1 | perf-idle, perf-js (2.7 ms), perf-edge-batch, perf-sprite-cache, perf-label-layout PASS; **perf-block FAIL** and **perf-fps FAIL**, both present on develop, see below |
| `STUDIO_PERF_BREAK=1 scripts/studio-perf.sh --label p1-break` | 1 | PASS: perf-edge-batch FAILs at budget 2 and 9 against 3 and 10 strokes |

**Do the P1 probes change what they measure?** An A/B on the same `app/dist`, back to back
at load average 2.7–3.2: develop's `deploy/perf` (from `5936309`, mounted over the worktree's),
then P1's, then develop's again (`target/studio-perf/ab-{dev1,p1,dev2}/`).

| Run | perf-fps 120 nodes, DPR 2 | perf-fps 2000 nodes | perf-js p95 | perf-10k DPR 1 / DPR 2 | perf-block |
|---|---|---|---|---|---|
| develop probes | 29.0 fps | 1.6 fps | 3.4 ms | 3.9 / 1.1 fps | FAIL |
| P1 probes | 29.0 fps | 1.5 fps | 3.0 ms | 3.9 / 1.1 fps | FAIL |
| develop probes again | 28.3 fps | 1.5 fps | 2.8 ms | 3.9 / 1.1 fps | FAIL |

The P1 probes move nothing beyond run-to-run spread, and both red rows are red with develop's
own probes:

- **perf-block** fails on the five 3D layouts (`layout.basic3d.{sphere,helix,cube}`,
  `layout.hierarchical3d`, `layout.force.spring3d`): the studio's worker refuses their
  snapshots ("snapshot refused (dimension-3d)"). They entered develop on 2026-10-01
  (`a39f968` … `d611dcb`), before this branch's base. The studio offers layouts it cannot draw.
- **perf-fps** at 120 nodes reads 28–29 fps, against 45.5 fps in `docs/measurements/studio-s7.md`.
  The drop is in develop, not in P1; its cause is UNKNOWN and goes to P5 and P6.

## 4. The 4-way hash table

No stage touched: graph-core's `src/` is unchanged (`git diff --stat 5936309 -- crates/graph-core/src`
is empty). `hashgate-8`, its negative control and `force-gate-4` are in the gate table.

## 5. Coverage

| Changed symbol | Exercised by |
|---|---|
| `graph-cli tick` (`bench/tick.rs` `Plan`, `run`, `HEADER`) | `bench::tests::tick::a_tick_row_has_one_cell_per_header_column`; the three `profile.sh` runs |
| `tick_alloc.rs` (the counter) | itself: it first asserts that one `Vec::with_capacity(1)` counts as one allocation |
| `MAX_NODES` (synthetic.ts) | `tests/ingest.test.ts`: the clamp at 10× the cap, and the degree clamp at 100 nodes |
| `react-hook.js`, `probes/frame.js`, `run.py` (`open_ms`, timeout handling), `rows.py` (`_frame_lines`) | the `p1-scale` run (`target/studio-perf/p1-scale/`), which wrote every new column and the 1M not-run row; the default gate run and the A/B in §3 |
| `profile.sh`, `profile.Dockerfile` | the 10k, 100k and 1M runs under `target/profile/` |

## 6. Caveat / Ponytail markers added

| File | Marker | Failing input, direction |
|---|---|---|
| `scripts/orch/profile.sh` | Caveat: valgrind's cache model, 50–100× slowdown, host load | a miss count read as a hardware prediction: wrong in either direction; a loaded host inflates wall time |
| `crates/graph-cli/src/bench/tick.rs` | Caveat: a tick's cost follows alpha | a short `--warm` measures the early, most expensive ticks: over-reports a settled layout's cost |
| `deploy/perf/react-hook.js` | Caveat: PerformedWork bit, function components only | forwardRef and lazy components are not counted, and a React that renamed the flag reads 0: an under-count, which can pass a budget row that should fail |
| `docs/measurements/perf-p1-baseline.md` | Caveat: cache model, shared host, software raster | studio fps say nothing about a desktop GPU |

## 7. What could not be verified

| Item | State |
|---|---|
| `perf` hardware counters and flamegraphs | SKIP (deviation 1) |
| The 1M studio case | NOT RUN: `__perf.open` did not return within 180 s, in a container capped at 4 GiB; whether memory contributed is UNKNOWN |
| What the 1.3–1.5 s long tasks at 10k are | UNKNOWN: they fall outside the frame callback (2.3 ms of JS per frame); raster is the likely cause, for P5 to trace |
| Edge counts of the studio's 50k and 200k graphs | not recorded by the probe (only the 10k case reads `stats()`) |
| React's share of the open time | UNKNOWN: the hook counts renders, it does not time them |
| WebGPU / hardware GPU numbers | not measured: the chromium image has software raster only |

## 8. Stop-and-ask items

None blocking. Two red rows that predate this branch are handed on rather than fixed here,
because they are outside P1's envelope: perf-block (the studio lists 3D layouts its 2D
renderer refuses) and the 120-node perf-fps drop since S7.

One decision is recorded in the baseline's restated targets rather than asked: the plan's P2 exit (≤ 120 ms per tick at 1M on one thread) is 53× under the measured 6338 ms
and below the cost of the exact walk itself, so it is replaced by a 2× single-thread target for
the exact Barnes-Hut plus an approximate particle-mesh force under a new layout id, quality-gated
against Barnes-Hut (allowed by the plan's own rule: a change that alters bytes gets a new id).
