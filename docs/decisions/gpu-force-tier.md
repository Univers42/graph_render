# GPU force tier: the particle mesh's tick on WebGPU

Status: accepted, 2026-10-03, under full autonomy; the user may veto. It closes
`docs/decisions/compute-tiers.md` open decision 2 (a GPU tier that is not bit-identical) for the
force layout only.

Revised 2026-10-03, same day: the first version (commit 501b6b08) moved Barnes-Hut's charge walk to
the GPU and set the mesh aside ("one GPU force path, not two"). The one path is now the particle
mesh's. Why is under Context; the BH walk is no longer pursued.

## Context

- The goal is 1M nodes live in the browser: a tick ≤ 100 ms there and ≤ 25 ms natively
  (`prompts/perf-plan.md`). WGSL has no `f64`, so no GPU pass can be bit-identical to the CPU tiers;
  `compute-tiers.md:113` allows such a tier once CPU plus threads are shown not to hold the budget.
- They are shown not to, on the fastest CPU path, the particle mesh:
  - native, 8 workers, 1M: 107 ms a tick (`docs/measurements/perf-pm-stencil.md:68`);
  - browser, 1M: about 158 ms a step (`docs/measurements/perf-live-cadence.md`, branch
    perf-live-cadence), collide and deposit leading;
  - the tick is memory-bound (working set about 100 MB); wasm simd128 gave nothing
    (`perf-pm-simd.md`). Byte identity caps every remaining CPU lever.
  - Barnes-Hut is further off: 1 635 ms serial native at 1M (`perf-p2-pm.md:30`).
- The mesh is the better GPU shape and the better force:
  - every pass is a scatter, an FFT or a per-node gather over regular memory; the BH walk is a
    divergent per-node scan of about 10³ cells;
  - `mb-fidelity` (`docs/measurements/perf-mb-fidelity.md`) puts the mesh's charge error at 3.6e-3
    to 2.0e-1 rms against the exact sum, under Barnes-Hut θ = 0.9's on every one of its six sets.
- The obstacle the first version named stays: whether `gm-chromium` exposes a WebGPU adapter. The
  hardware arm (`GM_GPU=1`, `--use-angle=vulkan`, `deploy/nav/gpu.py:31-33`) reaches the host's RX
  6600 for WebGL2; WebGPU has not been asked for yet.

## Decision

1. **A new id, `layout.force.particle_mesh.gpu`,** reached only by explicit request: an SDK option,
   or a studio toggle the user sets. The motor never substitutes it (`compute-tiers.md`).
   - Degradation: **per-device reproducible only**. Two runs on one device, driver and browser
     give the same bytes. There is no native = wasm equality, so the tier is not in graph-core's
     `LAYOUTS` and not in the 4-way hash gate.
   - Its gate: the same-device repeat is byte-equal; each pass agrees with the CPU mesh within a
     stated f32 bound on fixtures graph-cli emits (the repo's oracle pattern: one generator, both
     arms load the same file); the settled layout's stress is within a stated factor of the CPU
     mesh's; and a negative control (a broken kernel stage) turns it red.
2. **Where it lives.** WGSL and its TypeScript host in the SDK, `crates/graph-sdk-js/src/gpu/`. The
   motor stays pure Rust and unchanged; the WebGPU types the SDK needs are declared locally, so the
   fingerprinted root `package.json` and lockfile do not move.
3. **Order-free arithmetic, so a device repeats itself.**
   - Bounds: min/max reductions, exact in any order.
   - Deposit: cloud-in-cell weights as fixed-point `atomicAdd` on `i32`. Integer addition is
     associative, so the scheduling order cannot reach the density. The scale is the largest power
     of two that keeps `n` unit charges in one cell under `i32::MAX`. Three weights go through
     `round` (correctly rounded) and the fourth is the scale minus those three, so every node
     deposits exactly one unit and the total charge is exact.
   - FFT (`P ≤ 1024`), the kernel spectrum and the field read: per-thread work in a fixed order.
     The twiddles and the kernel spectrum are computed on the CPU with `libm` and uploaded: WGSL's
     f32 `sin` and `cos` are only good to 2⁻¹¹ absolute.
   - The source's order is not the device's order: WGSL lets an implementation reassociate and
     fuse (WGSL §15.7.5) and flush subnormals (§15.7.2). One compiled pipeline on one device still
     repeats itself, and that is all the tier claims.
   - Collide: cell counts by integer atomics, a scan, then each cell's members sorted by node index,
     so the resolve sums in a fixed order.
   - Link, center, integrate: per-node gathers, as the CPU kernels already are (D10).
4. **Resident.** Positions and velocities live on the GPU across ticks. The CPU uploads the graph
   once and reads positions back once per drawn frame (8 MB at 1M), until a WebGPU renderer can
   read the buffer in place (P5).
5. **Fallbacks, never silent.** No adapter: the CPU mesh runs and the studio says so. Device lost:
   the CPU mesh resumes from the last positions read back.

## Slices (each an OpenCode brief under `prompts/jobs/`)

| slice | what | gate |
|---|---|---|
| G0 | adapter probe in `gm-chromium`, hardware and software arms: adapter info and the limits G1 needs | exit 0 with an adapter, 2 without; `GM_GPU_BREAK=1` non-zero |
| G1a | `graph-cli emit-gpu-fixtures`: positions plus the CPU mesh's per-pass deltas (charge, link, collide) at 1k, 10k, 50k, start and settled | `--check` round-trip; fixtures fingerprinted |
| G1b | WGSL charge: bounds, deposit, FFT, kernel, field read; a browser harness against G1a | rms bound, repeat equal, broken-butterfly control red |
| G1c | link, center, collide, integrate; the resident tick | per-pass bounds, repeat equal, stress factor, 1M ms per tick |
| G1d | the SDK option and the studio toggle, fallbacks; devil verdict first (public surface) | `sdk:test`, `studio.sh check`, a 1M browser probe |

G1b waits on G0's answer: without an adapter in the container the tier cannot be gated (exit 2),
and it stops there.

## Consequences

- `prompts/perf-plan.md` P3 gains a GPU row; its numbers come from G1c's measurement.
- The first version's Barnes-Hut charge walk is not built. If the user wants Obsidian's exact
  algorithm on the GPU, that is a second tier and a second decision.
- Caveat: the f32 bounds are measured, not derived; G1b records one per fixture, and a bound that
  fails at 1M is the stop. Two floors are known in advance:
  - the deposit quantum is 2⁻¹¹ of a unit charge at 1M. With independent rounding that is about
    3·10⁻⁴ of a unit per occupied cell (estimated, rms), against a mean cell density near 1 at
    `P = 1024`: about ten times under the mesh's own best error against the exact sum (3.6e-3);
  - positions are f32. The seed spiral reaches 12·√n = 12 000 units at 1M
    (`barnes_hut/seed.rs:10`), where f32's spacing is 2⁻¹⁰, so a step under 2⁻¹¹ units there is
    lost. The settled-stress factor judges it; the way out is a two-float (hi + lo) position in
    integrate only.

## Not built: an f32 CPU mesh

- **Not as the GPU's oracle.** No CPU program can predict a device's bytes: WGSL's `x / y` is
  2.5 ULP, `sqrt` is inherited from `1 / inverseSqrt` (2 ULP; WGSL §15.7.4.1), and §15.7.5 allows
  reassociation and fusion. The f64 mesh stays the reference, with a bound. A broken stage is off
  by O(1), far above any f32 bound, so the tighter bound a mirror would give buys no detection.
- **Not as a CPU tier, for now.** It is deterministic (IEEE f32 with no FMA, `libm`'s f32
  functions), but it is a new id with its own hashes and a second numeric path through every mesh
  pass. Halving the bytes reaches only the memory-bound passes. The largest pass, the collide
  `Gather` (33.5 of 107 ms), is instruction-bound (`prompts/jobs/perf-pm-collide-soa.md:3-13`),
  and simd128 gave nothing (`perf-pm-simd.md`).
- **Reopened if G0 exits 2** (no adapter in the container). It is then the next lever for the
  browser's 100 ms, and it starts with a measurement, not a layout.
