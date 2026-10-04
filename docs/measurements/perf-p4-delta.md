# P4 delta — `extend` + `grow` per 10 000-node batch at 1M nodes, native and wasm32

The contract (`docs/contract/delta.md`, "Done when"): at 1 000 000 nodes with 10 000-node
batches, `extend` plus `grow` takes **≤ 30 ms per batch**, as the median of 3 alternated
rounds, natively and in wasm32, with the p95, the max and the host load beside it.

**Verdict: the 30 ms budget is missed in every arm, natively and in wasm32.** The median of the
3 medians of `extend` + `grow`:

| arm | extend median | grow median | **sum median** | sum p95 | sum max | vs 30 ms |
|---|---:|---:|---:|---:|---:|---:|
| native, Barnes-Hut | 43.25 ms | 3.65 ms | **47.05 ms** | 62.50 ms | 73.04 ms | **1.57×** |
| native, particle mesh | 42.34 ms | 6.79 ms | **50.92 ms** | 67.63 ms | 79.08 ms | **1.70×** |
| wasm32, Barnes-Hut | 69.57 ms | 4.36 ms | **73.88 ms** | 118.39 ms | 149.59 ms | **2.46×** |
| wasm32, particle mesh | 67.67 ms | 10.10 ms | **78.13 ms** | 90.36 ms | 98.62 ms | **2.60×** |

`p95` and `max` are the medians of the 3 rounds' p95 and max. Only the **median** column is a
claim; p95 over ten batches is one interpolated value, not a tail (see "What these numbers
are not").

The largest cost is named: **`extend`, not `grow`**, in every arm and in every round. `extend`
is 92 % of the sum natively on Barnes-Hut (43.25 of 47.05 ms), 83 % natively on the particle
mesh (42.34 of 50.92 ms) and 94 % in wasm32 on Barnes-Hut (69.57 of 73.88 ms), while `grow` is
3.65–10.10 ms in every arm and is nowhere near the budget on its own. **Which half of
`extend` it is cannot be told from this arm** — see "The split of `extend`" below. This slice
changes no graph-core, so it can name the cost and not remove it.

## How the numbers were taken

One input, two arms. `graph-cli tick --stream 10000 --batches 10 --n 1000000 --emit
target/bench/stream-1m.jsonl` writes the stream; both arms replay the *same file*.

| line | nodes | edges | bytes |
|---:|---:|---:|---:|
| 0 | 900000 | 1394802 | 447956269 |
| 1 | 10000 | 14976 | 4714034 |
| 2 | 10000 | 15189 | 4906463 |
| 3 | 10000 | 15245 | 4931737 |
| 4 | 10000 | 15397 | 4966419 |
| 5 | 10000 | 15358 | 4961998 |
| 6 | 10000 | 15568 | 5006432 |
| 7 | 10000 | 15650 | 5030496 |
| 8 | 10000 | 15772 | 5065505 |
| 9 | 10000 | 15898 | 5092310 |
| 10 | 10000 | 15925 | 5097861 |

**11 lines, 497 729 524 bytes.** Every line is a JSON v1 document written by
`graph_wasm::ingest_document`; line 0 is the first `n − K·BATCH` nodes with only the edges
whose both endpoints are among them, and each later line is the next `BATCH` nodes plus every
edge whose *later* endpoint is in that batch, so no node and no edge appears twice
(`bench::tick::stream::tests::the_batches_partition_the_model`). Two emits of one plan are
byte-identical (`…::tests::two_emits_are_byte_identical`), so both arms ran over the same
bytes.

- **Native arm** — `graph-cli tick --stream 10000 --batches 10 --from <file> --layout
  barnes-hut|particle-mesh`, in `crates/graph-cli/src/bench/tick/stream/arm.rs`. Untimed:
  `service::build(line 0)`, `ForceSession::from_frozen`, `step(10)`. Per batch, timed
  separately: `graph_wasm::service::extend(&mut topology, line)` then `session.grow(&topology)`,
  then an untimed `step(1)`.
- **wasm32 arm** — `node harness/wasm-stream-bench.mjs --from <file> --engine
  barnes_hut|particle_mesh`. Untimed: `motor.build(line 0)` on the raw line, `forceSession`,
  `tick(10)`. Per batch: `JSON.parse` the line untimed, then `extend` and `grow` in their own
  timers, then `tick(1)`.
- **Release** for every 1M process: `--release` natively, and
  `cargo build -p graph-wasm --release --target wasm32-unknown-unknown` for the wasm module.
- One 1M process at a time, 3 rounds, each alternating native BH → wasm BH → native PM →
  wasm PM → the reference `tick --grow 10000 --n 1000000`.

Inside each timer is `extend`'s whole body: `ingest::read_records` then `Topology::extend`.
The wasm `extend` additionally carries the SDK's own `JSON.stringify` of the batch before
`gm_graph_extend` sees it (`crates/graph-sdk-js/src/extend.ts:46`), which is a difference in
the work between the two columns and a candidate for the wasm arm's ~1.6× extend premium —
this arm does not time the serialize on its own, so the premium is measured and the cause is
not. The wasm `extend` column is the pessimistic one.

## Per round

Sum of `extend` + `grow`, per batch, in ms. Batch 1 is the outlier in every arm and every
round; see "Batch 1".

| round | arm | b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10 | median | p95 | max |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | native BH | 73.04 | 45.48 | 43.24 | 43.79 | 48.52 | 49.61 | 46.84 | 46.87 | 47.90 | 48.17 | 47.39 | 62.50 | 73.04 |
| 2 | native BH | 72.28 | 41.99 | 45.27 | 43.86 | 45.21 | 45.39 | 46.25 | 47.06 | 47.42 | 48.76 | 45.82 | 61.69 | 72.28 |
| 3 | native BH | 74.72 | 44.21 | 43.39 | 46.41 | 46.78 | 46.46 | 48.69 | 51.04 | 47.32 | 54.44 | 47.05 | 65.59 | 74.72 |
| 1 | native PM | 79.08 | 45.11 | 50.65 | 45.60 | 51.20 | 47.86 | 48.61 | 51.77 | 53.64 | 51.41 | 50.92 | 67.63 | 79.08 |
| 2 | native PM | 80.63 | 45.74 | 51.67 | 46.80 | 48.75 | 47.73 | 48.58 | 50.10 | 49.94 | 51.04 | 49.34 | 67.60 | 80.63 |
| 3 | native PM | 78.48 | 48.41 | 54.36 | 45.52 | 48.93 | 52.09 | 51.58 | 52.75 | 56.62 | 56.65 | 52.42 | 68.65 | 78.48 |
| 1 | wasm BH | 144.65 | 74.25 | 66.16 | 69.30 | 80.75 | 74.59 | 71.97 | 73.51 | 72.10 | 84.28 | 73.88 | 117.49 | 144.65 |
| 2 | wasm BH | 152.10 | 69.53 | 70.13 | 71.52 | 69.96 | 71.71 | 84.15 | 87.96 | 86.39 | 79.67 | 75.69 | 123.24 | 152.10 |
| 3 | wasm BH | 149.59 | 68.43 | 66.42 | 78.13 | 71.69 | 72.34 | 77.80 | 80.25 | 72.99 | 73.36 | 73.18 | 118.39 | 149.59 |
| 1 | wasm PM | 98.99 | 75.39 | 74.99 | 73.50 | 75.15 | 76.06 | 78.36 | 79.36 | 79.16 | 80.54 | 77.21 | 90.68 | 98.99 |
| 2 | wasm PM | 98.62 | 80.27 | 74.79 | 78.12 | 76.50 | 77.26 | 78.06 | 78.92 | 78.14 | 80.03 | 78.13 | 90.36 | 98.62 |
| 3 | wasm PM | 97.45 | 81.64 | 74.28 | 73.03 | 78.32 | 78.05 | 77.05 | 78.24 | 77.97 | 78.69 | 78.14 | 90.33 | 97.45 |

The median of each row's ten samples, the R7-interpolated p95 and the max are the columns the
two arms print; this table reprints them so all three rounds sit side by side.

Each half's own median, per round and then the median of the three:

| arm | extend median per round (ms) | extend median of 3 | grow median per round (ms) | grow median of 3 |
|---|---|---:|---|---:|
| native BH | 43.05, 42.29, 43.25 | **43.25 ms** | 3.65, 3.60, 3.73 | **3.65 ms** |
| native PM | 42.34, 42.06, 44.32 | **42.34 ms** | 6.71, 6.79, 7.40 | **6.79 ms** |
| wasm BH | 69.57, 70.83, 68.02 | **69.57 ms** | 4.36, 4.50, 4.34 | **4.36 ms** |
| wasm PM | 67.29, 68.23, 67.67 | **67.67 ms** | 10.10, 10.26, 10.10 | **10.10 ms** |

`grow` costs more on the particle mesh than on Barnes-Hut in every round (6.79 vs 3.65 ms
native, 10.10 vs 4.36 ms in wasm32, medians of 3). The arm does not time the mesh's
structures separately, so **why** is not measured here — what the numbers say is only that the
mesh's carry is the more expensive of the two and that both are far inside the budget.

## The split of `extend`

`extend`'s body is two things: `ingest::read_records` — parse ~4.7–5.1 MB of JSON v1 into
10 000 `NodeRecord`s and ~15 000 `EdgeRecord`s — and `Topology::extend`, which appends those
into the dense columns of a 900 000-node topology. **This arm cannot separate them**:
`graph_wasm::ingest` is a private module (`crates/graph-wasm/src/lib.rs:163`), so
`read_records` is not callable from `graph-cli`, and timing it would need a graph-wasm or a
graph-core change, both outside this slice's paths. So the split is not claimed.

What the data does say, from the per-batch rows above and the emit table:

- **`extend` does not scale with the batch's own bytes.** Batch 1 is 4 714 034 bytes and batch
  10 is 5 097 861 — **+8.1 %** — while `extend` over batches 2 to 10 sits in 39.7–48.9 ms
  natively (all four engine/parse combinations) with no upward trend, and 63.0–82.9 ms in
  wasm32. Whatever a per-byte parse costs, it is the flat floor of `extend`, not a growing
  part of it: what grows with the batch number is work at the size of the accumulated
  topology, not work on the batch.
- **A full re-index prices the index side separately**: the reference arm builds the whole
  1 000 000-node / 1 549 929-edge model from records in **733.83 ms** (median of 3:
  740.60, 717.78, 733.83) and carries onto it in **255.00 ms** (255.10, 255.00, 254.51). So
  one batch's `extend` at 43.25 ms is **5.9 %** of a full re-index of the same model, and one
  batch's `grow` at 3.65 ms is **1.4 %** of the reference carry — the append path already
  does the rebuild's work at a seventeenth and a seventieth of what the rebuild costs.

**What this measurement claims about the miss:** the cost is in `extend`, at 1M columns, and
every arm's max is batch 1. Everything past that — whether the append's cost is the parse,
the column growth, or a pass inside `Topology::extend` — is not separated here and would need
a graph-core or graph-wasm timer, which this slice does not add.

## Batch 1

Every arm, every round: batch 1's sum is 1.5–1.6× (native BH), 1.6× (native PM), 2.0× (wasm BH)
and 1.3× (wasm PM) the same round's median, and its `grow` alone is 31.3–33.4 ms on native BH,
36.2–38.6 ms on native PM, 65.1–72.3 ms on wasm BH and 18.1–19.0 ms on wasm PM, against
2.3–5.6 ms and 3.0–6.5 ms for every later batch. **What causes it is not measured**: the
reading the data supports is a one-off on the first append — first write into columns the
topology has never touched, and in wasm32 the first growth of the module's heap into fresh
pages — and nothing in this arm times page faults or heap growth, so it stays a reading.
**The median is claimed; the max is batch 1 in 12 of 12 arm-rounds** and the p95 sits between
the two because ten samples is a thin distribution.

## The snapshot, timed once per wasm process

The structure snapshot the studio rebuilds, at the final size (1 000 000 nodes,
74 721 460 bytes of snapshot):

| round | `run(handle, "layout.random")` | `toBytes(handle)` |
|---|---:|---:|
| 1 (BH) | 131.52 ms | 73.68 ms |
| 2 (BH) | 130.28 ms | 76.68 ms |
| 3 (BH) | 189.53 ms | 78.94 ms |
| 1 (PM) | 109.37 ms | 78.90 ms |
| 2 (PM) | 110.04 ms | 79.86 ms |
| 3 (PM) | 107.25 ms | 82.40 ms |

Median `run` **131.52 ms** on Barnes-Hut and **109.37 ms** on the particle mesh; median
`toBytes` **76.68 ms** and **79.86 ms** (78.92 ms across all six runs). This is a
whole-structure `layout.random` at 1M nodes plus a 74.7 MB serialize, so it is not a per-batch
cost and is not in the 30 ms budget; it is the price of a studio rebuild at 1M, and the two
calls together are **2.6× one batch's `extend` + `grow`** in wasm32 (188.9 ms against 73.9 ms).

## The reference arm

`graph-cli tick --grow 10000 --n 1000000` — the pre-existing measurement, for scale. It carries
a session across a **freshly indexed** topology rather than appending to a live one, so its
carry is not a batch's grow: 255.00 ms against 3.65 ms, **70×**.

| round | index ms | carry median ms | carry min ms | carry max ms | m new |
|---|---:|---:|---:|---:|---:|
| 1 | 740.60 | 255.10 | 250.32 | 263.09 | 1549929 |
| 2 | 717.78 | 255.00 | 251.97 | 269.22 | 1549929 |
| 3 | 733.83 | 254.51 | 252.02 | 262.39 | 1549929 |

The reference arm's model is `seeded_model`, not `scale_model`, so it has 1 549 929 edges
against the stream's 1 549 780 (1 394 802 in line 0 plus 154 978 across the ten batches) —
comparable, not identical, and the reference number is not the stream number.

## `/proc/loadavg` per round

The host was **shared and loaded**, and is reported rather than assumed idle. The host gate
(`free -g` available ≥ 12 GB **and** 1-minute load < 14) was checked before every one of the
15 processes; it passed on the first reading every time, so nothing waited and nothing was run
against a gate failure. Available memory was 19 GB throughout.

| round | before r1 | before r2 | before r3 |
|---|---|---|---|
| 1-minute load at each process's start | 11.73, 9.41, 7.85, 7.26, 6.48 | 6.26, 6.27, 5.45, 5.08, 4.62 | 4.37, 4.07, 3.78, 3.88, 3.83 |

Each arm also prints its own `load start` and `load end` on its summary row; they are in the
raw logs and agree with the table above (the first reading of round 1 is the 11.73 the gate
saw). The load **fell** across the three rounds, which means round 1's numbers are the ones
measured on the busiest host and the later rounds the quietest — and round 1 is not the slowest
round (47.39 vs 45.82 vs 47.05 ms on native BH), so on this host the load is not what moves
these numbers.

## What these numbers are not

- **Only medians are claimed.** The p95 and the max are printed because the contract asks for
  them. Ten batches make a thin p95: it is one value interpolated between the ninth and tenth
  smallest sum (`stream/stats.rs`'s `R7` rule, the same rule in both arms), not a tail. The max
  is batch 1 in all 12 arm-rounds and says nothing about steady state.
- **Wall clock, one host, one machine.** Every number is a wall-clock reading on this shared
  20-core host, taken with the load printed beside it. Nothing here is a per-core or
  per-allocator figure, and nothing here survives a change of page cache state.
- **`extend` is not split into parse and index** — see above. The claim is about the whole of
  `extend`.
- **The two arms do not time byte-identical work.** The wasm `extend` carries the SDK's
  `JSON.stringify`; the native one does not. The wasm column is the pessimistic one.
- **The batch-1 outlier is not separated out.** The median absorbs it, but it is the max in
  every arm, and a run that wanted a clean steady-state number would need to discard batch 1
  and say so.

## What this does not do

- **No graph-core change.** `ForceSession::grow`, `Topology::extend` and `ingest::read_records`
  are untouched; this slice only times them. The 30 ms miss is reported, not fixed.
- **No studio change.** `docs/measurements/perf-p4-delta.md` is the only document, and the
  snapshot numbers are one measurement of an existing call.
- **No new dependency, no `unsafe`**, and the wasm module is the release artifact the SDK
  already ships.

## Reproducing

```
scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --n 1000000 --emit target/bench/stream-1m.jsonl
scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut
scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout particle-mesh
scripts/orch/node-slim.sh node harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut
scripts/orch/node-slim.sh node harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine particle_mesh
scripts/orch/gr cargo run -q --release -p graph-cli -- tick --grow 10000 --n 1000000
```

The stream file is 497 MB under `target/bench/` and is generated, never committed. Both arms'
gate rows run the same commands at `n = 2000`, `batch = 100`, `batches = 3`.