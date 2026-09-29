> **Status (2026-09-28):** NOT STARTED (after Phase 9). See docs/reports/STATUS.md.

# Phase 11 — Compute tiers: SIMD, threads, and (only if measured necessary) GPU

**Read `prompt.md` first**, then `docs/decisions/compute-tiers.md` (the ADR this phase implements),
`docs/measurements/phase09-{bench,crossover}.md` and `BENCHMARKS.md`. Phase 9's gate must be green;
Phase 10 need not be (this phase touches no ingest surface).

## Goal

Make the gated algorithms faster **without giving up bit-identical output**. The tiers come in the
ADR's order: tuned scalar (already done in Phases 6–9), then SIMD, then threads. A GPU tier is added
only if Phase 9's numbers prove the CPU tiers cannot hold the frame budget at a target size, and then
only under its own capability id.

## Why this phase is after Phase 9, not inside Phase 6

Each tier costs complexity and puts determinism at risk. Only a measurement justifies it
(`minimalism-ladder.md` performance override). Phase 9 produces the crossover N per arm and says
where the scalar motor stops fitting 16.67 ms per tick. This phase spends effort exactly there and
nowhere else. The kernels are already partitionable, because Phases 6 and 8 wrote them in gather
form (`prompt.md` §6 D10), so this phase adds executors and SIMD lanes, not a redesign.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/exec/{mod.rs,partition.rs,select.rs}      (range kernels' contract + pure tier selection)
crates/graph-cli/src/exec_native.rs                              (std::thread::scope executor — outside graph-core)
crates/graph-sdk-js/src/exec/{workers.ts,select.ts}              (browser executor + feature detection)
crates/graph-sdk-js/src/exec/gpu/*                               (ONLY after stop-and-ask 2 is approved)
harness/tier-equality.mjs                                        (wasm scalar vs wasm simd128 vs worker arms)
docs/measurements/phase11-{simd,threads,gpu}.md
docs/decisions/tier-thresholds.md                                (the threshold table and its derivation)
```

**MODIFY — exactly these:**
```
crates/graph-core/src/layout/force/*.rs, crates/graph-core/src/post/fdeb.rs   (SIMD-friendly SoA loops only; semantics unchanged)
crates/graph-core/src/{lib.rs,registry.rs}
crates/graph-wasm/src/lib.rs, crates/graph-wasm/Cargo.toml       (a +simd128 build variant)
crates/graph-cli/src/{main.rs,hashgate.rs,capabilities.rs,bench.rs}
harness/wasm-run.mjs                                             (extra hash arms)
crates/graph-sdk-js/{src/index.ts,src/wasm.ts,README.md,package.json}
docker/rust.Dockerfile                                           (ONLY if stop-and-ask 1 selects shared-memory threads: a pinned nightly for -Z build-std)
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. `rayon` or any new graph-core dependency. Parallel float
reductions (D3). `relaxed-simd`. FTZ/DAZ. Changing any algorithm's semantics. Anything in osionos.

## Steps

### 1. The N-way hash gate first

Extend `hashgate` from 4-way (native ×2, wasm32 ×2) to N-way. Add native-SIMD, wasm-`simd128` and
native-threaded arms at thread counts {1, 2, 3, 4, 7}. The odd counts matter: they give uneven slices,
and a boundary bug hides behind even splits. **All arms must hash equal, per stage.** Build it
before any tier, and prove it can fail: a negative control that splits one node's force sum across two
threads must go red.

### 2. `exec/partition.rs` — the range-kernel contract

Every per-step kernel has the shape `fn step_range(state: &State, range: Range<u32>, out: &mut
[f32])`. It reads only start-of-step state and writes only `out[range]`, accumulating in the
kernel's fixed order. The quadtree (or grid index) is built once per step, single-threaded and
deterministic, before the ranges run. Parallelise the build only if a measurement shows it is the
bottleneck (Amdahl), and then only with a fixed cell order. Partition boundaries are a pure function
of `(n, workers)`.

### 3. Tier 1b — SIMD

Vectorise **across outputs** (lane k computes node i+k with exactly the scalar sequence), never
across the terms of one sum. Try autovectorisable SoA loops first (safe code, verified with
`cargo asm` or by inspecting the wasm for `f32x4`). Use explicit `core::arch` intrinsics only where
that measurably fails. Scalar remainder loops handle `n % 4`. Transcendentals stay scalar `libm`.
Ship two wasm artifacts (scalar and `+simd128`); the SDK picks by `WebAssembly.validate` on a
probe module. Record in `phase11-simd.md`: speedup per N, artifact sizes, and the proof of equality
(the N-way hash).

### 4. Tier 2 — threads

**Native** (`graph-cli/src/exec_native.rs`): `std::thread::scope`, fixed slices, a barrier per step,
double-buffered state. **Browser**: take the path chosen at stop-and-ask 1:

- (a) shared memory: `SharedArrayBuffer`, COOP/COEP, and an atomics-enabled wasm build; or
- (b) independent instances: one wasm instance per Web Worker, each holding the topology, with
  positions sent by transfer each step and force slices returned.

Measure both, and ask with the numbers before choosing. Speedup is reported against scalar at N = 220,
10k, 100k and 1M, with the thread count stated. Report when threads *lose*, which is likely at small
N: the tier selection threshold exists for exactly that.

### 5. `exec/select.rs` — tier selection is a pure function

`select(n, m, caps, thresholds) -> Tier`. `caps` (SIMD, thread count, WebGPU) is an input supplied
by the host or SDK, never detected inside graph-core. `thresholds` comes from
`docs/decisions/tier-thresholds.md`, derived from Phase 9 and this phase's measurements, never
guessed. No clock (D8). The SDK exposes `layout(id, params, { exec: "auto" | "scalar" | "simd" |
"threads" })`; `"auto"` is the default. An explicit request for an unavailable tier is refused with
an error, never silently downgraded.

### 6. Tier 3 — GPU, only after stop-and-ask 2

Only if Phase 9 plus step 4 show that, at a target N, no CPU tier holds 16.67 ms per tick. Then
WebGPU compute in the SDK, registered as **separate capability ids**
(`layout.force.barnes_hut.gpu`, possibly `post.bundle.fdeb.gpu`) with `degradation: "per-device
reproducible only"`. Gate on: same device, two runs, equal hash; stress no worse than the CPU tier by
a stated margin (`phase06-stress.md`'s metric); speedup measured. `exec: "auto"` **never** selects
a GPU id. Only an explicit request for the GPU capability id reaches it.

The port starts from the same gather kernels (one GPU thread per node, fixed order). Three
GPU-specific facts shape it, and each gets measured rather than assumed:

- **Divergence.** Barnes–Hut's per-node tree walk diverges within a lockstep group. Measure the
  direct port first. If divergence dominates, a GPU-shaped tree or brute-force tiles at mid N is a
  different algorithm: register it as such, gate it on stress rather than identity, and say so.
- **Transfer.** Upload, readback and dispatch overhead make small N a loss. Measure the GPU
  crossover N and write it into `tier-thresholds.md`. Below it, the GPU id refuses or documents that
  it is slower; it never pretends otherwise.
- **Hybrid** (CPU threads build the quadtree while the GPU computes forces) only if the
  measurements show the tree build is the GPU's bottleneck. It is an optimisation, not a starting
  design.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0
docker run --rm -v "$PWD:/w" ge-rust sh -c \
  'RUSTFLAGS="-C target-feature=+simd128" cargo build -p graph-wasm --release --target wasm32-unknown-unknown'  # 0

# N-way equality: every CPU tier on every target, per stage
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000 --tiers all   # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_SPLIT_SUM=1 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8 --tiers threads                                     # NON-ZERO
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                     # NON-ZERO
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/tier-equality.mjs                      # 0

# no forbidden constructs — PRODUCT code only (see scripts/forbidden-constructs.sh for why
# the bare `grep -rnE` form cannot be used: it matches this repo's own doc comments, which
# name the forbidden constructs in order to forbid them)
docker run --rm -v "$PWD:/w" ge-rust bash scripts/forbidden-constructs.sh crates/graph-core/src  # 0
# its own negative control: the row can go red
docker run --rm -v "$PWD:/w" ge-rust bash scripts/forbidden-constructs.sh --self-test  # 0

# the numbers that justify each tier
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- bench --n 220,10000,100000,1000000 \
  --tiers scalar,simd,threads --repeat 5 --out docs/measurements/phase11-threads.md               # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check               # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                         # 0
docker build -t ge-check . && docker run --rm ge-check                                             # 0
```

## Ledger delta

To `gated`: `exec.simd` and `exec.threads.native`, and `exec.threads.wasm` if stop-and-ask 1 was
approved. Each is gated by N-way hash equality and a measured speedup. `layout.force.barnes_hut.gpu`
(and any other `.gpu` id) is only registered after stop-and-ask 2, and never reaches `gated` on a
cross-device claim.

## Ponytail requirements

- **Tier thresholds** come from measurements on one machine class. Failing input: a host whose core
  count or SIMD width differs. Direction: a slower-than-possible tier is picked, which is cosmetic and
  never changes output bytes. Escape hatch: explicit `exec`.
- **Worker-instance threads (option b)** copy state each step. Failing input: small N, where the copy
  dominates. Direction: slower than scalar. Escape hatch: the threshold, or `exec: "scalar"`.
- **GPU**: per-device reproducible only. Failing input: two different GPUs, or two drivers.
  Direction: different pictures for the same graph, which is **dangerous if persisted and compared**.
  Escape hatch: the CPU id.
- **No marker** on the range kernels or on the partition function: they are exact.

## Stop-and-ask

1. **Browser threads**: shared memory (SAB + COOP/COEP headers + atomics build, likely pinned nightly
   `-Z build-std`) versus independent worker instances (no headers, a per-step copy). Present both
   measurements, then ask. `prompt.md` §10 marks SAB as a human decision, because it touches the
   host's deployment config and can break embeds.
2. **A GPU tier that is not bit-identical.** Ask only if the numbers show the CPU tiers cannot hold
   the frame budget at a target N, and bring those numbers.
3. **Any tier that cannot be made hash-equal to scalar** → stop and report. Do not add a tolerance,
   and do not register it under the scalar id.
4. **A thread or SIMD speedup under 3%** at every N → it is noise (`minimalism-ladder.md`). Report it
   and do not ship the tier.
