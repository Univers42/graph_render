# Perf P1 baseline — where the time goes, before any repair

Measured 2026-10-01 on develop `5936309` plus the P1 instruments, host dlesieur42 (20 cores,
load average 4–9 from other jobs), images `ge-profile` (valgrind 3.24.0, hyperfine 1.19.0, on
`ge-rust`) and `gm-chromium` (Chromium 154.0.8037.57). The plan is `prompts/perf-plan.md`;
every later phase states its target against the numbers on this page.

```sh
docker build -f docker/profile.Dockerfile -t ge-profile .
scripts/orch/profile.sh 10000 3 2          # -> target/profile/n10000/
scripts/orch/profile.sh 100000 3 2         # -> target/profile/n100000/
scripts/orch/profile.sh 1000000 3 2        # -> target/profile/n1000000/  (about 45 min)
scripts/orch/gr cargo test -p graph-core --test tick_alloc -- --nocapture
scripts/studio.sh build
scripts/studio-perf.sh --label p1-scale --cases 10000,50000,200000,1000000   # exit 1: a scale run
```

The model is graph-cli's scale model (`crates/graph-cli/src/bench/scale.rs`): `seeded_model`
at seed 0 with `REFERENCE_DEGREE`, and past its 100 000-node cap whole 100 000-node components
of it, so the 1M graph is ten disconnected components (easier than one 1M graph, so a lower
bound on cost). The stage is the Barnes-Hut force session (`ForceSession::step`, d3's charge +
link + collide), one thread. A fixture file is not used: `n1m.json` would be about 160 MB, and
the generator is the same bytes every run.

**Caveat:** callgrind counts are exact, but its cache is valgrind's model (D1 32 KiB, LL
24 MiB, no prefetcher), so a miss count compares builds, it does not predict hardware stalls.
Wall-clock rows ran on a shared host: `graph-cli tick` prints the load average at its start and
end, and a row is compared only with a row taken under similar load. The studio ran with
software raster in a container, so its frame rates compare run to run on this host and say
nothing about a desktop GPU.

## Motor: one Barnes-Hut tick, native, single thread

`graph-cli tick --n N --ticks 3 --warm 2` (`target/profile/nN/tick.md`, `hyperfine.json`).

| n | m | index ms | warm 2 ticks ms | tick min ms | tick median ms | tick max ms | whole run (hyperfine, 5 runs) |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10 000 | 15 474 | 4.5 | 33.8 | 17.52 | 19.63 | 20.48 | 0.102 s ± 0.005 |
| 100 000 | 154 978 | 77.2 | 448.9 | 279.39 | 296.43 | 314.79 | 1.410 s ± 0.030 |
| 1 000 000 | 1 549 780 | 1163.0 | 7896.1 | 5587.10 | 6338.10 | 6992.71 | 29.46 s ± 0.61 |

Ten times the nodes costs 15× the time from 10k to 100k, and 21× from 100k to 1M. An
`n log n` walk predicts about 12×. The instruction counts below grow 12.5× and 11.8×, so the
extra time at 1M is memory, not work.

## Where the instructions go (callgrind, inside `ForceSession::step`)

`target/profile/nN/callgrind.{out,txt}`: 5 ticks (2 warm, 3 measured) inside the
`--toggle-collect` window; inclusive, per pass. Ir per node per tick = total Ir / 5 / n.

| | 10k | 100k | 1M |
|---|---:|---:|---:|
| Ir, total | 0.97 G | 12.13 G | 143.59 G |
| Ir per node per tick | 19.4 k | 24.3 k | 28.7 k |
| charge pass (build, aggregate, walk) | 60.6 % | 58.9 % | 59.6 % |
| collide pass (project, build, walk) | ≈ 35 % | 39.3 % | 38.8 % |
| link pass | 2.2 % | 1.7 % | 1.4 % |
| both quadtree builds | 5.5 % | 4.8 % | 4.7 % |
| walk stack `Vec` push/pop (`vec/mod.rs`, `ptr/mod.rs` inlined in `visit_in`) | 15.9 % | 16.4 % | 16.7 % |
| D1 read misses | 15.6 M (4.6 %) | 250.0 M (5.8 %) | 3 119.9 M (6.1 %) |
| LL read misses | 3 | 707 208 | 392 400 706 |
| LL read misses, by pass | – | link 56 %, charge 10 % | collide 63 %, charge 33 %, link 2 % |

| Finding | Evidence |
|---|---|
| The charge walk is the hot spot | 59–61 % of instructions and 63–66 % of D1 read misses at every size (`charge.rs` `node_delta_with`, `quadtree.rs` `visit_in`) |
| Collide is a second full tree walk for a short-range force | 35–39 % of instructions; at 1M it causes 246 M of the 392 M LL read misses. Its tree is rebuilt over the projected positions every tick (`collide.rs` `prepare`), and `sqrt(d2)` is taken on every internal node visited (`collide.rs:132`) |
| One instruction in six is the walk's stack | `visit_in` pushes `(u32, Bounds)`, 40 bytes, per child and recomputes each quadrant's bounds on the way down (`quadtree.rs:visit_in`) |
| At 1M the tick is memory-bound | the LL read misses go from 707 k to 392 M (555×) for 10× the nodes, while the instructions grow 11.8×. Queries run in node-index order, which has no spatial locality, and leaf points are read through the index chain (`x[head]`) |
| The bulk quadtree build (plan P2.1) is the smallest lever | both builds are 4.7–5.5 % of instructions |
| The link pass is cheap in work and dear in misses at 100k | 1.7 % of instructions, 56 % of LL misses at 100k: endpoint gathers by edge order |
| A warm tick still allocates | 176 allocations over 16 warm ticks at n = 2000, about 11 per tick, from `partition`'s range list and the walk stacks (`crates/graph-core/tests/tick_alloc.rs`, a ratchet at 11/tick) |

## Heap (dhat, whole run)

`target/profile/nN/dhat.{json,log}`. The whole run includes the model build and the index.

| n | total | peak (t-gmax) | blocks at peak | at exit |
|---:|---:|---:|---:|---:|
| 10 000 | 26.8 MB in 134 065 blocks | 11.0 MB | 104 478 | 611 B |
| 100 000 | 237.8 MB in 1 341 679 blocks | 102.0 MB | 1 045 048 | 611 B |
| 1 000 000 | 3 521.7 MB in 24 705 943 blocks | 1 159.3 MB | 10 449 823 | 611 B |

The peak is the ingest, not the session: at 100k the largest live blocks at the peak are the
input `EdgeRecord` vector (27.2 MB) and `NodeRecord` vector (16.8 MB), then the string arena and
its `IndexMap`, which grow by doubling (18 reallocations, 14.7 MB allocated for a 7.3 MB final
table, `arena.rs:96-97` from `index.rs:52`). About 10 blocks per node at the peak are the
per-record id strings. At 1M the peak is 1159 bytes per node: the input `EdgeRecord` vector
(337.2 MB) and `NodeRecord` vector (268.8 MB), then the arena (62.9 + 33.6 MB) and three
`IndexMap` bucket tables (58.7 + 29.4 + 29.4 MB) plus its hash table (37.7 MB). The arena and
the map each reallocate 20–23 times. Pre-sizing them from the record counts (plan P2.5) removes
the doubling; the records are the caller's. Reads over the 1M run: 96.1 GB.

## Studio (Chromium, software raster, DPR 1, 1920×1080)

`target/studio-perf/p1-scale/{report.json,table.md}`. Layout `layout.mds.pivot` from 5000
nodes up (`deploy/perf/run.py` `LARGE_FROM`); three wheel phases (zoom in, out, back) after
a 1.5 s settle. "Open" is `__perf.open` from the call to the graph drawn; "at open" is the
React work from navigation to that point, counted by `deploy/perf/react-hook.js`.

| nodes | edges | open ms | worst fps (phase) | fps zoom-in / out / back | JS per frame mean / p95 ms | long tasks | longest ms | React commits / renders during zoom | React at open |
|---:|---:|---:|---|---|---|---:|---:|---|---|
| 10 000 | 19 996 | 420 | 3.9 (zoom-in) | 3.9 / 6.5 / 59.8 | 2.30 / 3.5 | 21 | 1 501 | 0 / 0 | 20 commits, 4 327 renders |
| 50 000 | – | 3 592 | 18.2 (zoom-in) | 18.2 / 24.6 / 28.0 | 4.97 / 7.7 | 47 | 80 | 0 / 0 | 16 commits, 3 632 renders |
| 200 000 | – | 22 604 | 1.6 (zoom-in) | 1.6 / 2.3 / 2.1 | 29.79 / 32.4 | 159 | 1 062 | 0 / 0 | 16 commits, 3 632 renders |
| 1 000 000 | – | not run: `__perf.open` did not return within the 180 s CDP timeout | | | | | | | |

| Finding | Evidence |
|---|---|
| The 10k frame cost is outside the frame callback | JS per frame is 2.3 ms, yet zoom-in runs at 3.9 fps with long tasks up to 1.5 s, and the same graph zoomed back runs at 59.8 fps. The cost follows the view, not n: Canvas2D raster of zoomed-in edges in the software rasterizer is the likely cause. That attribution is UNKNOWN until P5 traces it |
| At 200k the frame callback alone is over budget | 26–30 ms of JS per frame, every frame a long task; with raster, 1.6–2.3 fps. The renderer redraws every edge every frame (`canvas2d/controller.ts`, `edges.ts`) |
| Opening a graph grows faster than n | 0.42 s at 10k, 3.6 s at 50k, 22.6 s at 200k (×8.6 then ×6.3 for ×5 and ×4 the nodes), and 1M does not open in 180 s. The open path is the synthetic source, the worker's ingest and layout, the snapshot's copies and its sha256 (plan P6) |
| React is idle during interaction | 0 commits and 0 renders in every zoom phase at every size: pan and zoom never reach React |
| The P1 probes do not perturb the page | an A/B on the same build, develop's probes then P1's then develop's: perf-fps 29.0 / 29.0 / 28.3 fps at 120 nodes, 1.6 / 1.5 / 1.5 at 2000, perf-10k 3.9 fps in all three (`docs/reports/perf-p1.md` §3) |
| Small graphs got slower on develop since S7 | perf-fps at 120 nodes, DPR 2: 28–29 fps today against 45.5 fps in `studio-s7.md`, with develop's own probes. Cause UNKNOWN; P5/P6 |
| React's cost at open does not grow with n | 16–20 commits and 3 632–4 327 component renders from navigation to the graph drawn, the same at 50k and 200k. Its share of the open is UNKNOWN: the hook counts renders, it does not time them |

## Restated targets (replacing the plan's guesses)

The plan's P2 exit (≤ 120 ms per tick at 1M, one thread) needs 53× less time per tick than the
6338 ms measured. The exact d3 Barnes-Hut does about 28.7 k instructions per node per tick here;
53× would be about 540 per node, below the cost of the walk itself. So that target is withdrawn,
and real time at 1M is reached by three levers instead: a cheaper walk, threads, and an
approximate force under a new id.

| Phase | Target, against this page | Measured by |
|---|---|---|
| P2 kernels, one thread, byte-identical | 1M tick ≤ 3.2 s (2×), 100k ≤ 150 ms (2×); Ir per node per tick ≤ 15 k at 100k; LL read misses at 1M ≤ 40 M over the 5-tick window (10×); 0 allocations per warm tick | `profile.sh`, `tick_alloc.rs` |
| P2 levers, in measured order | (1) queries in spatial order (tree preorder) instead of index order, the 1M miss problem; (2) a stackless walk over a preorder arena with skip links and per-node bounds, the 16 % stack cost; (3) leaf coordinates stored in the arena, not read through `x[head]`; (4) `sqrt(d2)` hoisted out of the collide walk; (5) zero allocations per tick; (6) the bulk build last (≤ 5 %) | callgrind per pass |
| P2 new id (approximate) | `layout.force.particle_mesh`: charge on a grid by FFT convolution, O(n + G log G), quality-gated against Barnes-Hut (stress and neighbourhood preservation within a stated tolerance), not hash-equal to it. Target: 1M tick ≤ 150 ms on one thread | `tick`, `graph-cli stress` |
| P3 threads | ≥ 8× on 16 workers at 1M over the P2 single thread (the phase-11 tiers gave 4.29× on 7 workers at 100k); browser ≥ 3× over serial wasm at 200k | `bench --tiers`, hashgate Threads arm |
| P5 renderer | WebGL2 ≥ 60 fps at 200k and ≥ 30 at 1M, Canvas2D ≥ 30 at 20k, against the studio table above | `studio-perf.sh` |
| P6 studio | 0 React commits during pan/zoom (holds today, keep it); no long task over 50 ms during zoom at 200k; graph open at 1M within the evaluate timeout | `studio-perf.sh` |
| P7 end to end | 1M exact Barnes-Hut ≤ 400 ms per tick native on 16 workers; 1M particle-mesh ≤ 25 ms per tick native; drawn at ≥ 30 fps with live deltas. Rendering is decoupled from the tick rate, so a graph that settles at a few ticks per second still pans at frame rate | `tick`, `studio-perf.sh` |

## Deviations from the plan's P1

| Plan | Done instead | Why |
|---|---|---|
| `perf` and flamegraphs | callgrind (exact instructions and simulated misses per function, inclusive per pass) and dhat | `perf_event_paranoid` is 4 on this host and the Docker daemon is rootless, so `perf_event_open` is refused in every container (SKIP, not a pass). `callgrind.out` opens in kcachegrind for a call-graph view |
| cachegrind | callgrind `--cache-sim=yes` | cachegrind has no `--toggle-collect`, so it cannot be limited to `step()` |
| a `--passes` timer in `graph-cli bench` | the callgrind per-pass split | graph-core has no clock (D-rules), and callgrind's split is exact and noise-free |
| the counting allocator in graph-cli's tests | `crates/graph-core/tests/tick_alloc.rs` | its own test binary counts nothing but the session |
| CDP tracing and the React Profiler | a `longtask` PerformanceObserver and a DevTools-style commit hook | the Profiler API is a no-op in React's production build; the hook counts the build as shipped |
| a Rust-side 100k/1M fixture | `graph-cli tick --n` builds the scale model in process | the scale generator already exists (phase 9) and is the committed artefact; a 1M file is about 160 MB |
