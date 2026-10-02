# perf-heap: the wasm heap grows ahead (2026-10-02)

## Problem

Opening a graph in the studio spent most of its time in `gm_build`, and the cost grew faster than
the node count: 4.1 s at 50k, 20.8 s at 100k, 81.9 s at 200k. The motor's native ingest of the same
graph takes well under a second, so the time was not in the parser.

On wasm32 Rust's `System` allocator is dlmalloc. When a request does not fit, dlmalloc calls
`memory.grow` for the shortfall only, in whole 64 KiB pages. V8 answers each grow by detaching the
old `ArrayBuffer` and running a garbage collection over the JS heap. A build that grows the memory
thousands of times pays for thousands of collections, and each one costs more as the JS heap grows
with the graph. That makes the build superlinear.

## Fix

`crates/graph-wasm/src/heap.rs` is a `GlobalAlloc` that forwards to `System`. After any call that
grew the memory, it allocates `headroom(heap)` bytes and frees them at once. dlmalloc never returns
memory to wasm, so the block stays in its free top chunk and serves the next requests without
another grow. No new dependency; graph-core is unchanged.

`headroom(heap) = clamp(heap / 8, 1 MiB, 64 MiB)`. A 2 GiB load then grows the memory 132 times
instead of 32768 (unit model `a_two_gib_load_grows_the_memory_a_hundred_times_not_thirty_thousand`).

## Measurements

Synthetic random graph, `syntheticRecords({seed: 1, degree: 2})`, Node 22 in `node-slim`, one
sample per cell, host load 3–5. MiB is `WebAssembly.Memory.buffer.byteLength` after the stage.

### Build time and memory, one choice of headroom per column (100k nodes, `layout.mds.pivot`)

| Artifact | build ms | layout ms | toBytes ms | wasm MiB after open |
|---|---:|---:|---:|---:|
| develop (no headroom) | 20696 | 10045 | 39 | 323 |
| quarter, cap 256 MiB | 1068 | 9735 | 3 | 385 (+19%) |
| **eighth, cap 64 MiB (kept)** | **1275** | 9733 | 3 | **342 (+6%)** |

The quarter builds 16% faster than the eighth and holds 43 MiB more. The plan's target is 1M nodes
inside wasm32's 4 GiB address space, so memory wins: the eighth is kept.

### The studio worker's open path, develop vs quarter headroom

Same generator, each step of the worker's open path timed (`build` = `gm_build`, `layout` = the
whole stage).

| n | layout | build before → after (ms) | layout before → after (ms) | toBytes before → after (ms) |
|---:|---|---:|---:|---:|
| 100k | `layout.mds.pivot` | 20830 → 1059 | 11293 → 9915 | 39.7 → 2.8 |
| 100k | `layout.force.barnes_hut` | 21455 → 1069 | 20432 → 21907 | 39.1 → 2.9 |
| 200k | `layout.mds.pivot` | 81864 → 2764 | 26849 → 25117 | 97.8 → 7.7 |
| 200k | `layout.force.barnes_hut` | 83961 → 2149 | 47427 → 47186 | 78.3 → 6.8 |

The layout times do not move (one sample, ±7%): the layouts allocate their buffers up front and
were never grow-bound. Their wasm cost is algorithmic and is tracked separately (Pivot MDS at 200k,
Barnes–Hut in wasm without threads).

## What it does not do

- wasm memory never shrinks. The reserved block counts against the module until it is reused, up to
  an eighth of the peak (at most 64 MiB).
- A workload that grows by one huge block at a time (one large `Vec` reserve) gains nothing; that
  case already made one grow.
- The 64 MiB cap means that past 512 MiB the heap grows in 64 MiB steps. Each step is still one GC.

## Reproduce

```sh
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
mkdir -p target/wf && cp target/wasm32-unknown-unknown/release/graph_wasm.wasm target/wf/after.wasm  # develop's: before.wasm
scripts/orch/node-slim.sh env WASM=<artifact> node --max-old-space-size=8000 \
  --experimental-strip-types --no-warnings deploy/perf/wasm-open.ts 100000 layout.mds.pivot
```

The unit model and its bounds: `scripts/orch/gr cargo test -p graph-wasm heap`.
