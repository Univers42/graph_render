# GPU force tier: Barnes-Hut's charge walk on WebGPU

Status: accepted, 2026-10-03, under full autonomy; the user may veto. It closes
`docs/decisions/compute-tiers.md` open decision 2 (a GPU tier that is not bit-identical) for the
force charge pass only.

## Context

- The user asks for 1M nodes, live, on Obsidian's algorithm (`docs/decisions/obsidian-force.md`).
  That means Barnes-Hut θ for the many-body force.
- A Barnes-Hut tick at 1M costs 1 635 ms serial native. Charge plus collide take 95.5 % of it
  (`docs/measurements/perf-p2-pm.md:7,30`). Even ideal scaling on 16 threads leaves about 100 ms
  native, and wasm is slower again (`docs/measurements/perf-p3-wasm-threads.md:76`). The browser
  budget is ≤ 100 ms a tick, and a 30 fps feel wants ≤ 33 ms.
- WGSL has no `f64`, so a GPU pass cannot be bit-identical to the CPU tiers.
  `compute-tiers.md:113` allows such a tier only once CPU plus threads are shown not to hold the
  budget. The numbers above show that for Barnes-Hut at 1M.
- The walk already has the shape a GPU needs:
  - The tree is a preorder arena. A cell's subtree is `k..skip`, so a walk is a forward scan with
    no stack (`crates/graph-core/src/layout/force/quadtree/preorder.rs:4-10`).
  - Each node reads 40-byte `Body` records front to back and writes only its own delta
    (`barnes_hut/charge.rs:1-12,74-83`).

## Decision

1. **A new id, `layout.force.barnes_hut.gpu`.** It is reached only by explicit request: the SDK
   option, or a studio toggle the user sets. The motor never substitutes it on its own
   (`compute-tiers.md`).
   - Degradation: **per-device reproducible only**. The same device, driver and browser give the
     same bytes run to run. There is no native = wasm equality, so the tier stays out of the 4-way
     hash gate.
   - Its own gate has two rows:
     - two runs on one device are equal;
     - the `mb-fidelity` bound passes, with f32 slack stated in the record.
2. **Slice G1 moves only the charge walk to the GPU.**
   - The CPU builds the tree and the bodies with the existing code (`charge.rs:61-70`, `prepare`).
   - Bodies (`comx, comy, open, count, skip, start`) and positions upload as f32 structure-of-arrays.
   - One compute invocation runs per node. It scans the arena in the same order as the CPU walk
     and accumulates in that order, with no atomics, so the result is fixed on a given device.
   - The deltas are read back as f32 → f64 and merged on the CPU (`step::merge`).
   - Link, center, collide and integrate stay on the CPU in f64.
3. **Slice G2 runs only if G1 measures upload plus readback above 30 % of its GPU tick.** G2
   keeps the positions resident on the GPU and builds the tree there:
   - Morton keys come from the same quadrant descent as the CPU build, then a radix sort, then
     cells from the runs of the sorted keys.
   - Integration also moves to the GPU.
4. **Fallbacks.**
   - No adapter: the CPU tier runs, and the studio says so.
   - Device lost: the CPU tier resumes from the last positions it read back.
   - Neither fallback is silent.

## Unknowns, each a stop in G1's brief

- Whether `gm-chromium` (`deploy/chromium.Dockerfile`, hardware arm `--use-angle=vulkan`,
  `deploy/nav/gpu.py:31-33`) exposes a WebGPU adapter headless. If `requestAdapter()` returns
  null, the tier's gate cannot run (exit 2), and G1 cannot be called gated.
- The SDK option is public surface. G1 starts with a devil verdict, before any code.
- Whether f32 accumulation over about 10³ terms per node keeps the walk inside the fidelity
  bound. `mb-fidelity` measures this. It is not assumed.

## Consequences

- `prompts/perf-plan.md` P3 gains a GPU row. Slices are queued after `perf-mb-fidelity` reports,
  because G1's acceptance bound is that gate's number.
- The particle mesh's GPU variant is not pursued while the Barnes-Hut tier is open: one GPU force
  path, not two.
