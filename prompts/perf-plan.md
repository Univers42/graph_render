# Plan — 1M nodes, live, multi-threaded, measured (7 phases)

## Context

The user wants the motor and the studio to handle **1,000,000-node graphs in real time** and to show a
graph **growing live** while an AI process runs. That means multi-threading done properly, the right
algorithms and data structures, and frontend memoization that works. Every claim has to come from
official profilers with numbers. The user's rule: **fix the algorithms and data structures first. The
GPU is not a cover for bad choices.** Each change lands atomically and is measured on its own. The
ceilings are raised only once everything is measured.

### Where we stand (measured, from `docs/measurements/`)

| Layer | Fact | Source |
|---|---|---|
| Motor BH (the only scalable force) | 22 ms/tick @10k, **454 ms/tick @100k** native scalar (superlinear); 7 threads = 4.29× @100k | phase09-bench.md, phase11-threads.md |
| wasm vs native | 2–8× slower; frame-budget crossover: native 10k, wasm 4k | phase09-crossover.md |
| Other force layouts | FA2/spring/FR/graphopt O(n²); KK/neato O(n²) memory; ceilings 500–16k | registry.rs |
| Memory | ~166 B/node vs 33 B budget | phase09-ceilings.md |
| Browser threads | none. wasm runs serially; no SAB, no COOP/COEP | app/vite.config.ts |
| Renderer | Canvas2D only, full repaint per frame; **10k = 3.8 fps**, 20k timed out; perf-fps row has never passed | studio-s7.md |
| Data path | snapshot copied 4× before it is drawn; sha256 on every run; live loop `.slice()`s xs/ys every frame | session.ts:94, calls.ts:45, frame.ts:77 |
| React | store notifies every listener, no selectors; **0 useMemo / 0 React.memo**; O(n) Legend/Search/Inspector on every render; restyle uncached, and it runs even when nothing changed | store.ts, Shell.tsx:66, pipeline.ts:234 |
| Live/streaming | only BH has a resumable session; no delta API; Topology is immutable | live-force-session.md |

### User decisions (2026-10-01)
- Browser threads: build **both** (a) SAB + wasm threads and (b) one wasm instance per Worker, measure them, then the user picks.
- Renderer: a feature-detected **WebGPU → WebGL2 → Canvas2D** chain, with every fallback tested.
- Compute: **CPU first**, deterministic, 4-way hash gate kept. A GPU compute tier is proposed only if Phase 7 misses the target.
- Live feed: the host JS API `applyDeltas(batch)`, a JSONL replay, and an optional WebSocket/SSE source in `app/`.

### Design checks done before writing the phases (file:line evidence)

| Question | Answer | Evidence |
|---|---|---|
| Can a bulk quadtree build keep the 65 BH goldens bit-identical? | **Yes, if it splits on the same float midpoints** (not on a quantised Morton key). Root bounds are fixed before any insert (`cover` both corners), so the PR-quadtree shape does not depend on insert order. Accumulation walks children in slot order, and the walk depends only on shape, never on node ids. Coincident chains are most-recent-first, which means descending index. | `quadtree.rs:build`, `Builder::add`/`insert_leaf`; `charge.rs:98-100` |
| Can threads get faster without touching graph-core? | **Yes.** `Runner::run(kernel, workers, out)` is the seam. Today `exec_native.rs:52-68` spawns OS threads on every pass, gives each its own `Vec`, then `concat`s them. First rung: `split_at_mut` into `out` (no alloc, no copy). A persistent pool only if spawn time is still above 3% after that. | `exec_native.rs:35-70`, `exec/mod.rs` |
| Can a live session grow? | **Yes, without making Topology mutable:** rebuild the Topology for each delta batch (O(n+m)), then `ForceSession::from_positions` keeps the old positions and seeds the new rows. An append-only topology is needed only if a measured rebuild misses the budget. | `session.rs:139` |
| Where does the renderer seam go? | `view.ts` hard-wires `canvas2d/controller.ts`, and `ViewStats.backend` is the literal `"canvas2d"`. A `Backend` interface gets three real implementations (WebGPU, WebGL2, Canvas2D), which satisfies ladder rung 7. | `view.ts:13-20,40` |
| How is a gate row written? | `name|expect|cmd`. A negative control is a row whose cmd ends `; test $? -eq 1`. | `scripts/orch/rows/quick.rows` |

## The 7 phases

Rules for every phase:
- Each phase is its own branch and worktree (`wt-new.sh`).
- The merge floor must be green, and the phase report goes in `docs/reports/`.
- Every number comes from an official tool, with its command and artifact under `docs/measurements/`.
- **Hash-neutral first:** a change that alters bytes gets a new layout id or a recorded golden update, never a silent change.
- Ceilings stay put until P7.

### P1 — Instrument the baseline (official tools, Docker)
- **Goal:** a profile that names the hot spots, so later phases fix measured problems, not guessed ones.
- **Changes:**
  - Add `perf`, `valgrind` (dhat, cachegrind) and `hyperfine` to a new `docker/profile.Dockerfile` (FROM ge-rust, Debian packages).
  - Add `scripts/orch/profile.sh` to run them.
  - Add a per-pass timer to `graph-cli bench` (`--passes`: build tree, charge, link, collide, integrate).
  - Add a counting `GlobalAlloc` to graph-cli's test build, used for allocations per tick.
  - Add 100k and 1M fixtures to the synthetic generator (it is clamped at 50k now, `synthetic.ts:28`; on the Rust side, through `graph-cli` emit).
  - Add CDP tracing plus a React Profiler hook to `deploy/perf`, with drivers for 50k, 200k and 1M.
- **Exit:**
  - `docs/measurements/perf-p1-baseline.md`: a flamegraph per pass at 100k and 1M, allocations per tick, cache misses, and studio fps, frame time and long tasks at 10k, 50k and 200k.
  - Every later target is stated against these numbers.

### P2 — Motor DSA and kernels, CPU, single thread
- **Goal:** cut the work itself before any thread or GPU touches it.
- **Changes, in order** (each one its own commit and measured):
  1. Quadtree bulk build: a top-down partition of an index array on the same midpoints, nodes in one arena, rebuilt once per tick and not twice (`quadtree.rs`, `barnes_hut/sim`). It must stay byte-identical against the 65 goldens.
  2. Zero allocations per steady tick: `step.rs:348,379`, `partition.rs:118`, `charge.rs:147-160`. The counting allocator test asserts 0.
  3. Kernels:
     - Compute each square root once (`collide.rs:132,164`).
     - Link force, one pass per edge as a gather with no duplicate (`step.rs:419-432`), allowed only if it is byte-equal; otherwise a new id.
     - Lay out SoA by tree order for cache locality, with positions ordered by Morton key inside the tree arena only; the wire order is unchanged.
  4. O(n²) and allocation-heavy layouts:
     - FA2 becomes a BH-approximated gather (`forceatlas2/state.rs:130-144`), under a new id `layout.force.forceatlas2_bh`. The exact version stays as the oracle arm.
     - Yifan Hu no longer clones the graph per level (`yifan_hu.rs:151,155`).
     - Pivot MDS Gram matrix (`pivot_mds.rs:156-165`).
     - Sugiyama ordering allocations (`ordering.rs:96-246`).
     - Betweenness, closeness and louvain scratch reuse.
  5. Ingest and snapshot: `graph-wasm/src/ingest.rs:61-72`, `build.rs:173`, `layout/mod.rs:121-146`, `binary.rs:87-90`. Pre-size everything and drop the intermediate copies.
- **Gate:**
  - Goldens and hashgate-8 must stay green.
  - A new row `alloc-per-tick|0|…` has a negative control knob `GM_MUTATE_TICK_ALLOC=1` that must turn it red.
- **Exit:** BH ≤ 120 ms/tick at 1M native single-thread (baseline extrapolates to about 5 s), and ≤ 166 → ≤ 80 B/node. Targets are re-stated against the P1 numbers.

### P3 — Threads (native and browser), deterministic
- **Goal:** linear speed-up on the gather kernels without changing a byte.
- **Changes:**
  - `Threads` writes into `split_at_mut` spans of `out` (no Vec<Vec>, no concat).
  - Measure spawn overhead; build a persistent pool (`std` only: parked workers plus a generation barrier) only if spawn time is ≥ 3%.
  - Parallel tree build: the top 2 levels are split serially, and each quadrant subtree is built by one worker and stitched in slot order. Shape-identical, so byte-identical.
  - Wire `select.rs:81-83` into production, which has no caller today.
  - Browser, both models, measured, then the user picks:
    - (a) SAB + wasm threads: nightly `-Z build-std` with `+atomics,+bulk-memory`, plus COOP/COEP in `app/vite.config.ts` and `deploy/`. **Stop-and-ask 1** is already answered with "build both".
    - (b) N wasm instances, one per Worker: positions broadcast as a transferable `Float64Array` per tick, and each worker returns its span.
- **Gate:**
  - Hashgate Threads arm at workers {1,2,3,4,7} at 100k and 1M.
  - Negative control `GM_MUTATE_SPLIT_SUM` (it exists in `partition.rs` tests) promoted to a row.
  - A browser parity row: (a) and (b) produce bytes equal to serial.
- **Exit:**
  - Native ≥ 5× on 8 cores at 1M, so ≤ 25 ms/tick.
  - Browser ≥ 3× over serial wasm at 200k.
  - A decision record `docs/decisions/browser-threads.md` with both numbers.

### P4 — Streaming deltas and the live session
- **Goal:** watch a graph grow while an AI process emits nodes and edges.
- **Changes:**
  - The delta format `{addNodes, addEdges, removeNodes, removeEdges, attrs}`, specified in `docs/contract/delta.md` and generated through codegen.
  - Motor: `ForceSession::apply(delta)` rebuilds the Topology and warm-starts through `from_positions`. A new node is seeded at the mean of its placed neighbours plus a counter jiggle (D8, no RNG). Reheat is bounded.
  - ABI: `gm_force_session_apply`, rows stay dense.
  - SDK `applyDeltas(batch)` coalesces deltas per animation frame.
  - The studio host API, a JSONL replay (`fixtures/stream-*.jsonl`), and an optional SSE/WebSocket source in `app/` (dev only).
- **Gate:**
  - A delta trace replays to identical bytes on native and wasm (a new hashgate arm `stream`).
  - Negative control: drop one delta and the gate must go red.
- **Exit:** 1M final size at 10k nodes per second ingested, with a rebuild of ≤ 30 ms per batch at 1M (measured). If it misses, an append-only CSR with periodic compaction becomes a sub-slice.

### P5 — Renderer chain WebGPU → WebGL2 → Canvas2D
- **Goal:** 60 fps drawing of 1M points, decoupled from the simulation's tick rate.
- **Changes:**
  - A `Backend` interface in `packages/graph-render/src/backend.ts`, chosen by feature detection: `navigator.gpu` and `requestAdapter`, then `getContext('webgl2')`, then 2D. Every failure falls through to the next backend.
  - GPU backends:
    - One positions buffer, updated with `writeBuffer`/`bufferSubData` and never rebuilt.
    - Instanced quads for nodes; edges as index pairs fetched in the vertex shader.
    - LOD: below a pixel threshold, nodes become points and edges are density-binned.
    - Labels: a culled top-K atlas.
  - Picking: GPU ID-buffer readback on WebGPU and WebGL2; on Canvas2D, an incrementally updated uniform grid, not one rebuilt every frame (`controller.ts:158-173`).
  - Canvas2D gets the cheap wins: the edge plan is cached, not rebuilt every frame (`edges.ts:231`), and the impostor cache is bounded with LRU (`impostors.ts:43`).
- **Gate:**
  - Per-backend parity screenshots against Canvas2D at 2k: pixel diff within a tolerance stated with a `Caveat:` line.
  - Fallback rows that force-disable WebGPU, then WebGL2, and assert the next backend draws (`studio-smoke` with `STUDIO_BACKEND=…`).
  - Negative control: a broken backend must not leave a blank canvas.
- **Exit:**
  - Measured fps: WebGL2 ≥ 60 at 200k and ≥ 30 at 1M; Canvas2D ≥ 30 at 20k (now 3.8 at 10k).
  - WebGPU is measured on the host browser. The chromium image uses software raster, so CI covers WebGL2 and Canvas2D only, and that gap is written down.

### P6 — Studio memoization and the data path
- **Goal:** the UI does work only when its inputs change.
- **Changes:**
  - The store gets selectors through `useSyncExternalStore(subscribe, select)` with equality (`store.ts:11-28`, `useStudio.ts:7-9`).
  - `React.memo` on panels; `ForcesPanel` reads alpha at a throttle, not every 16 ms (`ForcesPanel.tsx:114-116`).
  - Cached derivations:
    - Legend counts (`Legend.tsx:30-69`).
    - A prefix or trigram search index built once per graph (`Search.tsx:65,70`, `matches.ts:8-21`).
    - Inspector neighbours read from the CSR, not scanned (`pipeline.ts:242-251`).
    - Restyle keyed by (style, filter, graph) generation counters instead of `JSON.stringify` (`pipeline.ts:92-97,157,214-234`).
    - The filter computed once (`filter.ts:33-34`), and reveal restyles only the changed rows (`reveal.ts:24`).
  - Copies: the snapshot moves zero-copy as a transfer from the worker to the renderer (`calls.ts:45`, `session.ts:94-100`, `frame.ts:77-100`); `liveLoop.ts` stops `.slice()`ing; `liveSession.ts:39-41` gets an index map.
  - sha256 is computed lazily, only on export or replay (`session.ts:215`).
  - An in-worker layout cache keyed by (graph generation, layout id, params) with a bounded LRU and a `Caveat:` line.
- **Gate:**
  - A React Profiler row: an interaction at 50k commits ≤ N components (the number set from P1).
  - CDP: no long task over 50 ms during pan or zoom at 200k.
  - Negative control: a knob that disables the selector equality must turn the row red.
- **Exit:**
  - Studio interaction p95 ≤ 16 ms at 200k.
  - Zero main-thread copies of positions per frame (counted).

### P7 — Verify end to end, then raise the ceilings
- **Goal:** one green full gate on develop, then the ceilings move.
- **Changes:**
  - Full gate: hashgate-1000, oracles, mutants and every new row with its negative control.
  - `scale_ceiling` and the studio perf rows are raised only to measured values (registry, `deploy/perf/rows.py`, `hook.js` 20k → 1M).
  - `docs/reports/perf-final.md`: before and after tables from the same tools as P1.
  - If the 1M target is missed on CPU, write the GPU compute proposal (**stop-and-ask 2/3**: not bit-identical, so not hash-equal) as a decision record for the user. Do not build it.
- **Exit:**
  - 1M nodes: layout tick ≤ 25 ms native and ≤ 100 ms in the browser, drawn at ≥ 30 fps, with live deltas visible.
  - develop → main merge, after the user's standing go-ahead.

### Dependencies and parallel branches
- P1 → everything.
- P2 → P3 (the threads speed up the new kernels).
- P2 ∥ P5 ∥ P6: these touch the motor, render and studio layers respectively, so no file overlaps. The P5 and P6 numbers are taken on the P1 fixtures.
- P3 + P4 (P4 needs P2's warm start and P3's executor in the worker).
- P7 last.

### Verification (how anyone re-runs it)
- Merge floor: `scripts/orch/gate.sh <logdir> scripts/orch/rows/quick.rows`.
- Per-phase rows file: `scripts/orch/rows/perf-pN.rows`.
- Profiles: `scripts/orch/profile.sh <fixture>`.
- Studio: `scripts/studio.sh check`, `studio-smoke.sh` and `studio-perf.sh`, each with its `*_BREAK=1` negative control.
