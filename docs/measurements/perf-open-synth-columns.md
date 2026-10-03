# Perf open synth columns: the studio's synthetic graph goes to the motor as GMC1, never as JSON

Measured 2026-10-03 on branch `perf-open-synth-columns`, from base `5f71999`. Host:
dlesieur42, i5-13600KF, 20 cores, 31 GB. Node from `scripts/orch/node-slim.sh`
(node:22-slim), Rust from `scripts/orch/gr`, the browser from
`scripts/studio-probe.sh` (gm-chromium, software rasteriser).

Why: opening a 400k synthetic graph in the studio took 4.4–6.9 s, and a CPU profile of the
open (`deploy/perf/open.py 400000 webgl2`) put about 3.3 s of it in the JSON round trip —
building the records and `JSON.stringify` about 0.78 s, staging the text into wasm memory
about 0.6 s, and `gm_build`'s parse, record strings and `index_model` about 1.98 s.
`docs/decisions/ingest-columns.md` set the bar: **if the measured median saving is under 3 s,
the studio stays on JSON** (lines 39 and 71). This branch exists to answer that with a
generator that does not pay the encoder's `Map` dedupe.

## Design

| Piece | Where | What changed |
|---|---|---|
| the assembler | `crates/graph-sdk-js/src/columns-assemble.ts` (new) | `assembleColumns(rows)`: one buffer sized before a byte is written, from a string table and columns the producer already holds. An ASCII fast path (size the blob at one byte per code unit, let `encodeInto` say whether that was enough, take the offsets as the running code-unit sums) and an exact path for a table with a wider code point. Typed-array views for the `f64` and `u32` columns behind a module-load endianness probe. |
| the general encoder | `crates/graph-sdk-js/src/columns.ts` | `encodeColumns` keeps its `Map` intern and its endpoint check, then builds a `ColumnRows` and calls the assembler. Its bytes do not change. |
| the draws | `packages/graph-studio/src/source/synthetic-draw.ts` (new) | Both generators write each edge into preallocated `from`/`to` `Uint32Array` and a `kind` `Uint8Array`, and each node's kind and group into `Uint8Array`s. The edge count is a formula over the loops, known before the first draw, and a mismatch throws. One draw order, shared by both wire formats. |
| the JSON half | `packages/graph-studio/src/source/synthetic.ts` | `syntheticRecords` turns the columns into an `IngestEdge[]`; `syntheticIngest` is unchanged. Same signature, same field values, same order. |
| the column half | `packages/graph-studio/src/source/synthetic-columns.ts` (new) | `syntheticColumns(spec)` → `{ rows, nodes, edgeCount }`. A fixed head of 34 table entries, then each node's id and label (the very strings `nodes` holds), then `e-${j}`. One sequential pass per column; no `Map`, no per-row object. |
| the session | `packages/graph-studio/src/motor/{session,documents,worker,local}.ts` | `Document.payload` is `{kind:"json";text}` or `{kind:"columns";bytes}`; `replace` dispatches on it. `buildColumns` added to `MotorLike`. `assemble` injected into `SessionDeps` like `digest`, because the SDK owns the encoder. |

The invariant the differential rests on: **row `r` of the node columns is dense index `r`**.
The generator knows every endpoint as a row, so it needs no id → row lookup and the encoder
needs no endpoint resolution — which is exactly the work `encodeColumns` spent a `Map` on.

## Gates

```
scripts/orch/gate.sh target/gate-synth-columns scripts/orch/rows/perf-open-synth-columns.rows
```

Every row of that file was also run individually by this job; the results are in the return
block. A timed gate was not run here — the orchestrator runs it.

## Bench: the open path under Node

```
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
scripts/orch/node-slim.sh env ARM=json     node --experimental-strip-types --no-warnings \
  --max-old-space-size=12288 deploy/perf/wasm-open.ts 400000 layout.forceatlas2.barnes_hut
scripts/orch/node-slim.sh env ARM=columns  node --experimental-strip-types --no-warnings \
  --max-old-space-size=12288 deploy/perf/wasm-open.ts 400000 layout.forceatlas2.barnes_hut
```

`ARM` is set with `env` on purpose: `node-slim.sh` forwards argv into the container and does
not forward the host environment, so `ARM=columns node …` silently runs the json arm.

One artifact (`graph_wasm.wasm`, 1 372 711 bytes). One process per run so each arm starts with
fresh linear memory, three runs per arm per size, arms alternated. `totalMs` is generate +
encode + build: the open path only. The document is the generator's own — `random`, seed 1,
degree 2 past 5000 nodes — so 400 000 nodes and 799 996 edges, and 1 000 000 nodes and
1 999 996 edges.

| nodes | arm | generate, 3 runs | encode, 3 runs | build, 3 runs | median total | document | wasm after build |
|---|---|---|---|---|---|---|---|
| 400 000 | `json` | 795, 552, 809 | 1108, 518, 736 | 8835, 3952, 4715 | **4989 ms** | 195 MiB | 1086 MiB |
| 400 000 | `columns` | 543, 458, 505 | 237, 208, 199 | 888, 820, 851 | **1447 ms** | 70 MiB | 263 MiB |
| 1 000 000 | `json` | 1574, 1389, 1698 | 1276, 1297, 1290 | 13586, 14946, 15856 | **17 632 ms** | 490 MiB | 2651 MiB |
| 1 000 000 | `columns` | 1568, 1621, 1548 | 531, 474, 572 | 3114, 3377, 3369 | **5729 ms** | 177 MiB | 730 MiB |

Medians, not means: the raw runs are in the table because the spread is the story. The
document is 2.8× smaller and wasm linear memory after the build is a quarter of the JSON arm's
at 400k and 0.28× at 1M — 1921 MiB saved at 1M.

**The median saving at 1M is 11 903 ms.** `docs/decisions/ingest-columns.md` set the bar at
3 s and the studio stayed on JSON when the general encoder came in 1774 ms short of it. This
arm clears it by a factor of four.

Where it comes from, at the 1M medians: the build 14 946 → 3377 ms (−11 569), the encode
1297 → 474 ms (−823), the generator itself unchanged (1389 → 1621 ms, +232, inside the spread).
The generator is the same draws with the same order; it costs a little more only because it
also fills the columns, which is work the encoder used to do instead.

## Browser open: before and after

`before` is the base commit `5f71999` built and served from its own tree; `after` is this
branch. Both arms ran the **same** probe harness (`scripts/studio-probe.sh` plus
`deploy/perf/` and `deploy/nav/` synced into the base tree), because the harness was reworked
under this job and a `before` run on the old probe would not be comparable.

```
scripts/studio.sh build
PERF_MEMORY=10g scripts/studio-probe.sh open 400000  webgl2
PERF_MEMORY=10g scripts/studio-probe.sh open 1000000 webgl2
```

Runs one at a time, three per arm per size, arms alternated.

| nodes | arm | open, 3 runs | median | load average at each run |
|---|---|---|---|---|
| 400 000 | before | 6.41, 4.90, 4.27 | **4.90 s** | 17.7, 22.7, 22.5 |
| 400 000 | after | 2.80, 2.04, 1.98 | **2.04 s** | 19.9, 22.1, 21.6 |
| 1 000 000 | before | 15.02, 12.27, 13.56 | **13.56 s** | 21.2, 18.3, 16.1 |
| 1 000 000 | after | 5.68, 4.92, 5.47 | **5.47 s** | 18.9, 16.7, 15.1 |

**Verdict against the bar: met at 1M (8.09 s median saving), missed at 400k (2.86 s).**
The bar is written about the 1M open, and 1M is where the format's advantage is
superlinear — the JSON arm's parse and per-record strings scale with document size, while the
columns arm's fixed head does not.

`Caveat:` the load average was 15–23 on a 20-core host for the whole session, so both arms
were measured on a loaded machine. The medians are interleaved, so the comparison is fair even
though neither number is absolute; treat the browser medians as good to about ±1 s and the
Node medians as the tighter measurement.

`Caveat, and what this number is not:` `open s` is the driver's `source.synthetic` dispatch,
which includes laying out the graph it just built with the studio's persisted layout — it is
not a pure ingest measurement. That is why the browser saving (8.09 s) is smaller than the
Node saving (11 903 ms): a large fixed layout sits inside the browser's span and dilutes the
ingest saving. The profile agrees about where the JSON went — in the before arm's worker,
`decode` (the `TextDecoder` over the JSON text) and `StringArena::intern` are both on the hot
list at 1M and both disappear from the after arm's top rows.

## Re-measured before landing, 2026-10-03

`PERF_MEMORY=10g scripts/studio-probe.sh open 1000000 webgl2`, SwiftShader, arms alternated, 3
rounds, medians only. Logs: `$GM_SCRATCH/orch/logs/open-1m-ab.log` and `open-1m-p4a-ab.log`.

| arm | tree | `open s`, 3 runs | median | 1-min load |
|---|---|---|---|---|
| before | develop `873f168e` | 10.25, 10.51, 11.11 | **10.51 s** | 5.4–6.5 |
| after | `perf-open-synth-columns` `e8c00beb` | 3.73, 3.73, 3.75 | **3.73 s** | 5.4–6.5 |
| after | `perf-open-synth-columns` `e8c00beb` | 4.77, 3.57, 4.17 | **4.17 s** | 11.7–12.2 |
| after + P4a | `perf-p4a-extend` `635abfb8` | 4.86, 3.97, 3.41 | **3.97 s** | 11.7–12.2 |

The open stack takes the 1M open from 10.51 s to 3.73 s, against P5's 12.45 s baseline. P4a's
append CSRs cost nothing measurable on the open: 3.97 s against 4.17 s in the same session, inside
the run-to-run spread.

On the `e8c00beb` arm the probe printed its open time and then exited 1:
`Profiler.stop ... Session with given id not found`. The open stack starts every new source in a
new worker (`motor/client.ts`, `loadFresh`), so the worker the probe attached to before the open is
gone afterwards. `deploy/perf/open.py` now reports that worker as retired and exits 0; the worker
that builds the graph is not profiled (its Caveat).

## What this does not do

- The documents and fixtures stay on JSON. `normaliseIngest` is the JSON reader's contract and
  the columns path would be a second spelling of it; only the generator, which already holds
  columns, moves.
- `encodeColumns` is untouched in behaviour and still on the SDK's index. Nothing outside
  `crates/graph-sdk-js` calls it, and `deploy/perf/open.py` never did.
- `assembleColumns` does not dedupe the table, so the blob carries a repeat for every repeated
  value (`"studio"`, `db-3`, a shared label). The contract allows it and the decoder's arena
  interns by content, so the cost is bytes in transit: 177 MiB against 70 MiB for a table that
  was deduped. A generator whose labels were all distinct would close that gap and the format
  would still win, by more.
- The generator's own cost is unchanged and is now *inside* the arm it is measuring. It was
  always building `IngestNode` objects (the studio UI reads weights off them); what it no
  longer builds is the `IngestEdge` per edge and the JSON text per document.
- Nothing here is a gate. `scripts/orch/rows/perf-open-synth-columns.rows` runs the studio's
  `check` and the SDK's tests; neither asserts a time. The differential
  (`packages/graph-studio/tests/synthetic-columns.motor.test.ts`) is what stands between the
  columns path and a silent regression: 64 specs decoding to exactly `syntheticRecords`,
  byte-identical snapshot bytes from either path, and a negative control that fails both
  comparisons when two edges' target rows are swapped in the bytes.
- The browser arm ran on SwiftShader, not a GPU, and the GPU arm was not requested.
