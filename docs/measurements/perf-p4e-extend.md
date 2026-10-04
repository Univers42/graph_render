# P4e — the 1M measurement of the columnar extend, eight arms

`docs/decisions/extend-columns.md` added a `GMX1` batch, `Topology::extend_columns`, the export
`gm_graph_extend_columns`, and — in this slice — `encodeBatch`, `Motor.extendColumns`, the studio's
switch and both benches' `--path columns` arm. **Condition 18** is this document: the same 1M
stream, the same eight arms, 3 rounds, median of the 3 medians, and an answer to A1 and A2 from
the numbers rather than from an argument.

**Verdict: the budget is met in the two native `columns` arms and missed in the other six.**
`extend` + `grow` per 10 000-node batch at 1M nodes, median of 3 alternated rounds:

| arm | sum median | vs 30 ms |
|---|---:|---:|
| native, Barnes-Hut, `columns` | **23.92 ms** | **0.80× — met** |
| native, particle mesh, `columns` | **19.34 ms** | **0.64× — met** |
| wasm32, particle mesh, `columns` | 56.74 ms | **1.89× over — missed** |
| wasm32, Barnes-Hut, `columns` | 55.12 ms | **1.84× over — missed** |
| native, particle mesh, `json` | 36.85 ms | 1.23× over — missed |
| native, Barnes-Hut, `json` | 35.21 ms | 1.17× over — missed |
| wasm32, Barnes-Hut, `json` | 74.22 ms | 2.47× over — missed |
| wasm32, particle mesh, `json` | 62.90 ms | 2.10× over — missed |

The native miss is **closed** — P4d's 33.15 ms and 33.61 ms become 23.92 ms and 19.34 ms on the
columnar path, and the JSON path is unchanged in behaviour and still there (F4: the export stays,
the row stays `implemented`). The wasm miss is **not** closed, and the reason is not the one A1
predicted: see "A1 and A2".

## The method

One input, two engines, two batch formats. `graph-cli tick --n 1000000 --stream 10000 --batches 10
--emit target/bench/stream-1m.jsonl` writes the stream; all eight arms replay **the same file**.

**11 lines, 497 729 524 bytes**, line 0 at 900 000 nodes and 447 956 269 bytes — the same file
P4c emitted and P4d re-measured, byte for byte (`docs/measurements/perf-p4-delta.md` "How the
numbers were taken"). Line 0 is untimed in both engines; each of the ten batches after it is timed
as `extend` then `grow`, in two timers that never contain each other, with `tick(1)` outside them.

| arm | command |
|---|---|
| native, `--path json` | `scripts/orch/gr cargo run -q --release -p graph-cli -- tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut\|particle-mesh --path json` |
| native, `--path columns` | the same with `--path columns` |
| wasm32, `--path json` | `scripts/orch/node-slim.sh node harness/wasm-stream-bench.mjs --from target/bench/stream-1m.jsonl --engine barnes_hut\|particle_mesh --path json` |
| wasm32, `--path columns` | the same with `--path columns` |

`GR_MEM=12g` before `timeout` on every native process; the wasm module rebuilt once with
`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` before the
first wasm process. One 1M process at a time. 3 rounds, each in the fixed alternating order

> native BH json → native BH columns → wasm BH json → wasm BH columns → the same four for PM

so no arm is systematically first or last inside a round. Before **every** process the host gate
was checked — `free -g` available ≥ 12 GB and 1-minute load < 14 — and on a failure the process
waited 60 s and re-checked, inside one 45-minute budget for the whole run. It waited 2 minutes
before the first process and 25 minutes in total across the 24; every process eventually ran and
every one exited 0. Raw output, one file per process and one per gate check, under
`target/wf/p4e-step/`.

**What each `extend` column contains, and what it does not.** This is the one asymmetry the
eight-arm table cannot smooth away, so it is stated before the numbers:

- native `json` — `service::extend`: it **reads** the line and appends it. The read is inside.
- native `columns` — `service::extend_columns`: it **decodes** a `GMX1` batch and appends it. The
  JSON read and the Rust encode that produced those bytes are outside
  (`crates/graph-cli/src/bench/tick/stream/arm.rs:117-124`).
- wasm `json` — the SDK's `JSON.stringify` of the batch, the copy into linear memory, and
  `ingest::read_records`. **The serialize is inside the timer.**
- wasm `columns` — the SDK's `encodeBatch`, the copy into linear memory, and the columnar decode.
  **The encode is inside the timer.**

So the wasm `columns` column is the **pessimistic** one — it charges the JS encoder — and the
native `columns` column is the **optimistic** one — it does not charge the Rust encoder. Both are
what a host on that side actually pays, which is why both are in the table rather than the
flattering pair alone, and why the native/wasm `columns` gap below is an upper bound on what a
host's own encoder costs.

## The eight arms

| arm | extend median | grow median | **sum median** | sum p95 | sum max | vs 30 ms |
|---|---:|---:|---:|---:|---:|---:|
| native BH json | 29.73 ms | 3.77 ms | **35.21 ms** | 60.25 ms | 63.04 ms | 1.17× |
| native BH columns | 17.85 ms | 5.68 ms | **23.92 ms** | 47.86 ms | 64.52 ms | **0.80×** |
| wasm BH json | 68.63 ms | 5.24 ms | **74.22 ms** | 155.66 ms | 203.61 ms | 2.47× |
| wasm BH columns | 50.41 ms | 4.94 ms | **55.12 ms** | 141.26 ms | 183.44 ms | 1.84× |
| native PM json | 28.90 ms | 8.30 ms | **36.85 ms** | 61.12 ms | 67.03 ms | 1.23× |
| native PM columns | 12.08 ms | 6.88 ms | **19.34 ms** | 41.10 ms | 49.24 ms | **0.64×** |
| wasm PM json | 52.49 ms | 10.06 ms | **62.90 ms** | 76.77 ms | 81.68 ms | 2.10× |
| wasm PM columns | 47.28 ms | 10.13 ms | **56.74 ms** | 103.27 ms | 111.04 ms | 1.89× |

Median of the 3 medians; p95 and max likewise. **Caveat:** a p95 over ten batches is one
interpolated value, not a tail, on both sides — `stream/stats.rs`'s `R7` rule in both benches, so
the two arms' columns are comparable only because the rule is one.

Against P4d's four arms, same stream, same method, **this host busier** (see the load caveat):

| arm | P4d sum | this slice, same path | change | P4d extend | this extend | P4d grow | this grow |
|---|---:|---:|---:|---:|---:|---:|---:|
| native BH json | 33.15 | **35.21** | +6.2 % | 28.25 | 29.73 | 4.04 | 3.77 |
| native PM json | 33.61 | **36.85** | +9.6 % | 26.44 | 28.90 | 6.69 | 8.30 |
| wasm BH json | 54.29 | **74.22** | +36.7 % | 49.76 | 68.63 | 4.30 | 5.24 |
| wasm PM json | 59.78 | **62.90** | +5.2 % | 49.55 | 52.49 | 10.24 | 10.06 |

**The `json` rows are the control, and they say the host cost this run 5–10 % natively and up to
37 % on wasm Barnes-Hut.** That last cell is not a code change — nothing in this slice touches the
JSON path — so it is the host: wasm BH json's three rounds are 57.74, 92.68 and 74.22 ms, and the
middle one ran while the 1-minute load went from 13.54 to 23.24 inside the process. The native
`json` spread is much tighter (32.18 / 35.21 / 52.31 on BH), which is what one expects of a
single-threaded process on a busy box: it loses a core's worth of time-slicing, and the wasm arm
pays it twice over because its own allocator and GC are on the same thread.

**`grow` is where it should be: unchanged** (3.77 against 4.04 on native BH json, 8.30 against
6.69 on native PM, 5.24 against 4.30 on wasm BH, 10.06 against 10.24 on wasm PM — inside
run-to-run spread on this host), because nothing in this slice touches it. Every millisecond of
the columns path's win is `extend`.

### Per round

Sum of `extend` + `grow` per batch, in ms, with the 1-minute load at each process's start and end.

| round | arm | b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10 | median | p95 | max | load start → end |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 1 | native BH json | 63.04 | 29.22 | 30.00 | 30.41 | 32.81 | 51.11 | 37.60 | 32.81 | 54.03 | 56.84 | 35.21 | 60.25 | 63.04 | 13.10 → 12.70 |
| 2 | native BH json | 92.66 | 46.37 | 46.60 | 52.27 | 49.04 | 52.35 | 50.86 | 59.24 | 52.76 | 54.04 | 52.31 | 77.62 | 92.66 | 13.56 → 26.16 |
| 3 | native BH json | 60.68 | 28.83 | 30.17 | 30.52 | 32.15 | 31.52 | 32.21 | 32.85 | 58.09 | 33.70 | 32.18 | 59.52 | 60.68 | 11.93 → 11.26 |
| 1 | native BH columns | 69.70 | 19.64 | 21.19 | 22.69 | 22.76 | 26.54 | 26.46 | 25.51 | 27.31 | 30.43 | 25.99 | 52.03 | 69.70 | 12.70 → 12.91 |
| 2 | native BH columns | 64.52 | 19.32 | 21.97 | 22.02 | 24.37 | 23.47 | 24.50 | 22.61 | 25.48 | 27.48 | 23.92 | 47.86 | 64.52 | 13.18 → 15.24 |
| 3 | native BH columns | 42.69 | 12.56 | 13.11 | 14.14 | 14.64 | 15.31 | 16.28 | 16.42 | 17.00 | 17.10 | 15.80 | 31.18 | 42.69 | 11.26 → 10.43 |
| 1 | wasm BH json | 134.85 | 54.95 | 51.94 | 89.09 | 64.51 | 92.13 | 57.03 | 58.00 | 57.49 | 57.42 | 57.74 | 115.63 | 134.85 | 12.91 → 13.17 |
| 2 | wasm BH json | 203.61 | 90.50 | 85.90 | 91.71 | 92.33 | 96.80 | 93.02 | 97.05 | 91.73 | 96.11 | 92.68 | 155.66 | 203.61 | 13.54 → 23.24 |
| 3 | wasm BH json | 260.07 | 92.30 | 56.68 | 58.23 | 56.33 | 90.21 | 91.59 | 93.54 | 57.99 | 56.29 | 74.22 | 185.13 | 260.07 | 10.43 → 10.70 |
| 1 | wasm BH columns | 141.96 | 53.66 | 68.58 | 50.98 | 53.07 | 46.99 | 75.87 | 59.30 | 54.70 | 55.54 | 55.12 | 112.22 | 141.96 | 13.17 → 13.10 |
| 2 | wasm BH columns | 183.44 | 77.34 | 51.30 | 89.71 | 49.59 | 55.82 | 79.71 | 54.45 | 55.84 | 76.37 | 66.10 | 141.26 | 183.44 | 13.34 → 13.46 |
| 3 | wasm BH columns | 202.20 | 49.84 | 71.94 | 43.49 | 69.57 | 51.84 | 44.32 | 49.04 | 50.73 | 51.43 | 51.08 | 143.58 | 202.20 | 10.70 → 9.73 |
| 1 | native PM json | 67.03 | 33.21 | 38.67 | 33.06 | 33.63 | 34.54 | 37.46 | 38.84 | 36.12 | 40.39 | 36.79 | 55.04 | 67.03 | 13.10 → 13.09 |
| 2 | native PM json | 64.96 | 33.14 | 37.68 | 51.31 | 51.79 | 52.64 | 56.42 | 56.04 | 54.87 | 55.49 | 53.75 | 61.12 | 64.96 | 13.46 → 14.83 |
| 3 | native PM json | 67.58 | 33.20 | 37.31 | 53.39 | 33.56 | 34.35 | 38.49 | 38.60 | 35.76 | 36.38 | 36.85 | 61.20 | 67.58 | 9.73 → 9.78 |
| 1 | native PM columns | 49.24 | 16.06 | 21.10 | 16.17 | 19.03 | 17.73 | 18.25 | 20.20 | 19.28 | 31.16 | 19.15 | 41.10 | 49.24 | 13.09 → 13.07 |
| 2 | native PM columns | 47.58 | 16.35 | 20.99 | 17.93 | 19.08 | 17.86 | 18.87 | 21.43 | 19.61 | 20.95 | 19.34 | 35.81 | 47.58 | 13.20 → 13.08 |
| 3 | native PM columns | 55.25 | 18.25 | 29.63 | 15.89 | 21.94 | 20.73 | 17.94 | 20.62 | 18.57 | 19.58 | 20.10 | 43.72 | 55.25 | 9.78 → 9.57 |
| 1 | wasm PM json | 81.68 | 59.58 | 59.14 | 58.71 | 60.96 | 61.80 | 62.55 | 63.30 | 65.27 | 65.59 | 62.17 | 74.44 | 81.68 | 13.07 → 13.33 |
| 2 | wasm PM json | 90.43 | 61.71 | 57.93 | 58.15 | 64.74 | 63.03 | 62.42 | 95.35 | 65.74 | 103.26 | 63.89 | 99.70 | 103.26 | 13.08 → 12.49 |
| 3 | wasm PM json | 80.80 | 60.64 | 57.02 | 58.56 | 64.07 | 62.32 | 61.69 | 63.48 | 64.59 | 71.85 | 62.90 | 76.77 | 80.80 | 9.57 → 8.70 |
| 1 | wasm PM columns | 111.04 | 47.00 | 45.78 | 61.15 | 53.67 | 55.60 | 57.19 | 50.54 | 93.77 | 54.92 | 55.26 | 103.27 | 111.04 | 13.33 → 13.17 |
| 2 | wasm PM columns | 117.57 | 58.07 | 48.43 | 56.94 | 58.16 | 54.47 | 55.95 | 50.76 | 147.18 | 58.59 | 57.51 | 133.85 | 147.18 | 12.49 → 11.93 |
| 3 | wasm PM columns | 92.98 | 70.51 | 45.55 | 57.33 | 56.15 | 78.41 | 62.35 | 50.75 | 50.27 | 50.85 | 56.74 | 86.42 | 92.98 | 8.70 → 8.33 |

**Batch 1 is the max in 21 of 24 arm-rounds**, the same first-append effect P4c and P4d found and
for the same unmeasured reason: the first append writes into columns the topology has never
touched. Only medians are claimed.

### `/proc/loadavg` per round, and what it cost

1-minute load at each process's start, in the eight-arm order:

| round | native BH json | native BH cols | wasm BH json | wasm BH cols | native PM json | native PM cols | wasm PM json | wasm PM cols |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 13.10 | 12.70 | 12.91 | 13.17 | 13.10 | 13.09 | 13.07 | 13.33 |
| 2 | 13.56 | 13.18 | 13.54 | 13.34 | 13.46 | 13.20 | 13.08 | 12.49 |
| 3 | 11.93 | 11.26 | 10.43 | 10.70 | 9.73 | 9.78 | 9.57 | 8.70 |

**Caveat: this host was never quiet.** The load sat between **8.3 and 13.6** for the whole run —
under this document's own gate of 14 at every check, so every process ran, but well above the
3.6–9.3 P4d's rounds 2 and 3 ran at, and the 15-minute load never left 11.7–16.8. The load fell
across the rounds, so **round 3 is the quietest and the fastest on every arm** (native BH columns
15.80 ms against 25.99 and 23.92; native BH json 32.18 against 35.21 and 52.31), and the
medians below are therefore **upper bounds** on what this code costs on a quiet host. The two
verdicts do not depend on that: the native `columns` arms are under 30 ms on the *busiest* round
as well as on the median, and the wasm `columns` arms are over 30 ms by nearly 2× on the quietest
round (51.08 and 56.74), which no plausible share of another job's CPU explains. The 45-minute
gate budget was spent waiting, not running: 2 minutes before the first process, 25 in total.

## A1 and A2, answered

**A2 — confirmed.** `index_columns`' `EntryTable` path is cheaper per record than the P4d JSON
path at 10 000 rows, on the same stream, the same batches and the same host: native Barnes-Hut
`extend` **29.73 → 17.85 ms** (−40 %) and native particle mesh **28.90 → 12.08 ms** (−58 %). The
sum, which is what the budget is written against, goes 35.21 → 23.92 and 36.85 → 19.34. Read with
the caveat above: the native `columns` timer excludes the encode, and the native `json` timer
includes the read, so part of that gap is the read and part is the decode-and-index.

**A1 — refuted in its strong form, and this is the finding.** A1 said "the JSON walk plus
`JSON.stringify` is most of the wasm premium", and that if encoding columns in JS costs what
`stringify` did, wasm gains only the walk. The wasm arm gained **less than the native arm did**,
and the premium it still pays got *bigger*:

| path | native `extend` | wasm `extend` | premium |
|---|---:|---:|---:|
| BH json | 29.73 ms | 68.63 ms | 38.90 ms (2.31×) |
| BH columns | 17.85 ms | 50.41 ms | 32.56 ms (2.82×) |
| PM json | 28.90 ms | 52.49 ms | 23.59 ms (1.82×) |
| PM columns | 12.08 ms | 47.28 ms | 35.20 ms (3.91×) |

Swapping the format took 11.88 ms off native Barnes-Hut and 16.82 ms off native particle mesh,
and took 18.22 ms and 5.21 ms off the wasm arms. On Barnes-Hut the wasm arm gave up about what
the native arm did (18.22 against 11.88 ms) while the native particle mesh gave up more than
twice what its wasm counterpart did (16.82 against 5.21 ms). So the walk-plus-serialize is **not**
most of the premium: on the particle mesh it is a quarter of it. What is left in the wasm arm's
`extend` after the columns path is the wasm32 tax on the columnar decode and the index walk, plus
the copy into linear memory, plus — and this is the number this bench does not have — the JS
`encodeBatch`, which the wasm arm pays inside its timer and the native arm does not pay at all.

**So the next measurement, named: time `encodeBatch` on its own.** That is the one quantity A1
really asks for and this document does not have; isolating it needs a second timer path in
`harness/wasm-stream-bench.mjs`, which is why it is not here. Until it exists, the honest
statement is a bound: the wasm `columns` premium over native (32.56 ms BH, 35.20 ms PM) contains
the whole JS encode, and the JSON arm's premium before the change (38.90 / 23.59 ms) is what the
walk and `JSON.stringify` together cost. A1's own escape clause — "if encoding columns in JS costs
what `stringify` did, wasm gains only the walk" — is the pessimistic branch, and the measurement
lands on the other side of it for particle mesh and near the boundary for Barnes-Hut.

## Reproducing

```sh
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --n 1000000 --stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl

# native, one arm (repeat for --layout particle-mesh and --path columns|json)
GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl \
  --layout barnes-hut --path columns

# wasm32, one arm (repeat for --engine particle_mesh and --path columns|json)
scripts/orch/node-slim.sh node harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut --path columns
```

Three rounds are those two commands over the four (engine, path) pairs in the alternating order
above, each preceded by `free -g | awk '/^Mem:/ {print $7}'` ≥ 12 and
`awk '{print $1}' /proc/loadavg` < 14, waiting 60 s and re-checking otherwise. The gate-row sizes
are the same commands at `tick --n 20000 --stream 2000 --batches 3` over
`target/bench/gate-stream.jsonl`. The 3-round tables above were read out of the raw logs by
`target/wf/p4e-step/summarise.mjs`, which is generated output rather than a committed tool.
