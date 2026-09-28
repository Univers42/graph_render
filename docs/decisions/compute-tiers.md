# ADR — Compute tiers: tuned single thread, then SIMD, then threads, then (maybe) GPU

Status: **accepted as roadmap** (user, 2026-09-28). Implementation is Phase 11
(`prompts/phase-11-compute-tiers.md`), after Phase 9 has measured where each tier is needed.
Two sub-decisions stay open and are stop-and-ask items there (§ Open decisions).

## Context

The motor's central promise is **bit-identical output** across native and wasm32, run to run
(`prompt.md` §6). The project's premise is **speed**: real-time iteration where Python could
not. More hardware (SIMD lanes, threads, a GPU) buys speed, but every step up puts the first
promise at more risk. So the escalation has to be ordered, designed and labelled, never
improvised per call site.

The biggest wins are algorithmic, not hardware: Barnes–Hut (Phase 6), Pivot MDS (Phase 6), level
of detail and simplification (Phase 9) and size-based iteration budgets (Phase 9) cut the work
itself. A 1M-node graph can be folded to a few tens of thousands of active elements. Hardware
tiers come after those, and only where Phase 9's numbers show they are needed.

## Decision

Four execution tiers, in this order. Tiers 1a, 1b and 2 must produce **the same bytes**; tier 3
is a different capability.

| Tier | What | Determinism | Where it runs |
|---|---|---|---|
| 1a — tuned scalar | Algorithmic wins, SoA columns, reused buffers, no per-tick allocation | bit-identical native = wasm32 (the existing 4-way gate) | graph-core |
| 1b — SIMD | Vectorised **across independent outputs** (4 nodes per `f32x4`), never across the terms of one sum | bit-identical to 1a — proven by the hash gate with the SIMD build as extra arms | graph-core, `simd128` on wasm32, SSE2/NEON natively |
| 2 — threads | Partitioned **by output**: each worker owns a fixed slice of nodes (or edges, or points) and computes each element's full value alone, in the scalar order | bit-identical to 1a — node-partitioning never splits one sum (D3 bans splitting a sum, not partitioning outputs) | executor **outside** graph-core: graph-cli natively, the SDK (Web Workers) in a browser |
| 3 — GPU | WebGPU compute, for very large live force layouts and heavy bundling only | **per-device reproducible only** — GPU float allows FMA contraction and looser functions, and differs between devices | the SDK only, under its **own capability id** (e.g. `layout.force.barnes_hut.gpu`) |

### Rules that make the escalation cheap (they bind Phases 4–9 now)

1. **D10 — partitionable kernels.** Every per-tick loop in graph-core is a *gather*: element `i`
   reads only the start-of-step state and writes only `out[i]`, accumulating its terms in a fixed
   order (CSR order, or quadtree order). No scatter into another element's accumulator. This is
   what lets tiers 1b and 2 be bit-identical to 1a without a redesign.
   *Consequence for Phase 6:* d3's `forceLink` is a sequential Gauss–Seidel scatter — each link
   reads velocities already modified by the links before it (`d3-force@3.0.0 src/link.js`,
   `force(alpha)`). Phase 6 implements the **Jacobi/gather form** (per node, over its incident edges
   in CSR order, from start-of-tick state) from day one. That is a deliberate, recorded deviation
   from d3, and it is allowed because force layouts are gated on quality (stress), not identity.
   `forceManyBody` is already a per-node gather (`src/manyBody.js`, `tree.visit(apply)` per node).
2. **Kernels in graph-core, executors outside.** graph-core stays pure and single-threaded: it
   exposes range kernels (`fn step_range(state, range, out)`), and it never spawns a thread,
   detects hardware or reads a clock. Native threads (`std::thread::scope`) live in graph-cli and
   graph-wasm's host; browser workers live in the SDK. The dependency allow-list stays closed (no
   `rayon`).
3. **One SDK call, tier chosen by measured thresholds.** From Phase 4 on, `layout(id, params,
   options)` keeps a third argument for execution options, so adding `options.exec` later is
   additive. The API never changes when a tier is added.
4. **Tier selection is a pure function** of `(n, m, capability flags)`, with thresholds taken from
   Phase 9's measured crossovers (never guessed) and never from elapsed time (D8). Capability flags
   (SIMD available, thread count, WebGPU present) are **inputs**, detected by the host or SDK, never
   inside graph-core. Because tiers 1a/1b/2 are bit-identical, auto-selecting among them cannot
   change a single output byte. The host may always pick a tier explicitly.
5. **Nothing non-deterministic hides under a deterministic id.** A GPU result is only ever
   returned under its own capability id with `degradation: "per-device reproducible only"`. It is
   never auto-substituted for the CPU id. Same rule as sampled betweenness (Phase 7).
6. **SIMD specifics.** No horizontal reductions that reorder a sum. No FMA: wasm `simd128` has none;
   `relaxed-simd` is banned (it permits FMA and implementation-defined results). Never set FTZ/DAZ.
   Transcendentals stay scalar through `libm` (D1). Autovectorised SoA loops are tried first (no
   `unsafe`); explicit intrinsics only where a measurement demands them. Two wasm builds (scalar,
   `+simd128`) are shipped, and the SDK picks by feature detection, which is safe because they are
   byte-identical.

### Why one rule serves both threads and the GPU

A GPU is a massively multi-threaded processor: thousands of small threads running the same code
on different data. The rule "one worker per output, which writes only its own output, summing in a
fixed order" (D10) is the same rule at both scales. A CPU thread takes a slice of nodes; a GPU
thread takes one node. If the kernels are written in gather form from the start, tier 2 → tier 3
is mostly a port, not a redesign. Turning d3's link scatter into gather is needed for tier 2
anyway, so the GPU costs no extra there.

Where the GPU still differs, and why it stays a separate id even when ported faithfully:

- **Uneven work.** GPU threads run in lockstep groups. Barnes–Hut's tree walk takes a different
  path per node, which diverges badly on a GPU. GPU force codes often use differently shaped trees,
  or brute-force tiles at mid sizes. That is a *different algorithm*, so it is gated on quality
  (stress no worse than the CPU tier) and speed, never on identity.
- **Transfer cost.** Data is copied to GPU memory and back. At small N that overhead dominates, the
  same small-N problem wasm has at 220 nodes. So the GPU has its own measured crossover, below
  which it is never selected.
- **Floating point.** FMA contraction and less precise functions are allowed. So the GPU tier is not
  bit-identical to the CPU, nor across devices.
- **Hybrid runs** are possible later: for example CPU threads rebuild the quadtree while the GPU
  computes forces. That is a later optimisation, decided by Phase 9 and Phase 11 measurements, not
  designed up front.

## Alternatives rejected

- **Parallel reduction for speed (rayon sums, SIMD horizontal adds).** Faster, but non-associative
  float addition makes output depend on thread count and lane width. That breaks the central
  guarantee for a constant factor.
- **GPU under the CPU id, with a tolerance.** It would silently turn "bit-identical" into
  "close enough" on exactly the largest graphs. Rejected for the same reason Phase 2 refused a
  round-trip epsilon.
- **Threads inside graph-core.** That puts `std::thread` into a crate that must build for
  `wasm32-unknown-unknown`, where spawning panics, and it would need `rayon`, which is off the
  closed allow-list.

## Open decisions (stop-and-ask in Phase 11, with Phase 9's numbers in hand)

1. **Browser threads.** Shared-memory wasm threads need `SharedArrayBuffer`, which needs COOP/COEP
   response headers. That touches the host's deployment config and can break embeds, so
   `prompt.md` §10 lists it as out of scope without a human decision. On stable Rust,
   `wasm32-unknown-unknown` std is also not built with atomics, so shared-memory threads likely need
   a pinned nightly `-Z build-std` (verify at Phase 11 start, do not assume). The non-SAB
   alternative is N independent wasm instances in Web Workers, each holding the topology and
   receiving positions by transfer each tick. It needs no headers, but pays a copy per tick.
   Phase 11 measures both before asking.
2. **Accepting a GPU tier that is not bit-identical**, only if Phase 9 shows CPU + threads cannot
   hold the 16.67 ms tick at a target N.

## Consequences

- Phases 6 and 8 write their kernels in gather form (D10). Phase 6's d3 differential remains a
  stress comparison, as it already was.
- Phase 9 measures a crossover N **per tier** (scalar, SIMD; threads natively), not one per arm
  only. It emits the threshold table that tier selection reads.
- Phase 11 extends the hash gate from 4-way to N-way. Every CPU tier on every target joins the same
  equality check, and a single differing arm names the tier that broke.
