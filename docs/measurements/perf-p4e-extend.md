# P4e — the 1M measurement of the columnar extend, eight arms

`docs/decisions/extend-columns.md` added a `GMX1` batch, `Topology::extend_columns`, the export
`gm_graph_extend_columns`, and — in this slice — `encodeBatch`, `Motor.extendColumns`, the studio's
switch and both benches' `--path columns` arm. **Condition 18** is this document: the same 1M
stream, the same eight arms, 3 rounds, median of the 3 medians, and an answer to A1 and A2 from
the numbers rather than from an argument.

**Verdict:** PLACEHOLDER-VERDICT

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
waited 60 s and re-checked inside one 45-minute budget for the whole run. Raw output, one file per
process and one per gate check, under `target/wf/p4e-step/`.

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

So a native/wasm comparison *within* one path is like for like only on `json` (both read the line),
and the `columns` pair differs by one host-side encode. The wasm `columns` column is the
**pessimistic** one — it charges the JS encoder — and the native `columns` column is the
**optimistic** one — it does not charge the Rust encoder. Both are what a host on that side
actually pays, which is why both are in the table rather than the flattering pair only.

## The eight arms

PLACEHOLDER-TABLE

PLACEHOLDER-ROUNDS

## A1 and A2, answered

PLACEHOLDER-A1A2

## The verdict

PLACEHOLDER-VERDICT-TABLE

## Reproducing

```sh
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --n 1000000 --stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl
GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut --path columns
scripts/orch/node-slim.sh node harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut --path columns
```

The gate-row sizes are the same commands at `tick --n 20000 --stream 2000 --batches 3` and the
stream file `target/bench/gate-stream.jsonl`; the three rounds are the same eight commands
repeated, each preceded by a `free -g` / `/proc/loadavg` check.
