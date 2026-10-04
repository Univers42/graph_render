# P4f — where the wasm32 `extend` timer goes, and the biggest share removed

`docs/measurements/perf-p4e-extend.md` measured the `columns` path at 1M and found the two wasm
arms missed the 30 ms budget by about 1.9×, and it named the next measurement rather than
guessing at it: *"time `encodeBatch` on its own. That is the one quantity A1 really asks for and
this document does not have; isolating it needs a second timer path in
`harness/wasm-stream-bench.mjs`."*

This document is that measurement, and then the fix the measurement chose.
`harness/wasm-stream-bench.mjs` gained `--split`, which puts `encodeBatch` in a timer of its own,
so `extend − encode` is everything else that one call does; a CPU profile of the batch loop then
says what "everything else" is.

**The finding: the JS encoder is the wasm32 premium.** On both engines `encodeBatch` is about
four fifths of the timed `extend` — 31.18 ms of 37.33 on Barnes-Hut, 29.52 of 37.14 on particle
mesh — and the copy into linear memory plus the whole wasm32 motor together are the remaining
6.15 and 7.62 ms. P4e's A1 guessed the walk and `JSON.stringify`; the truth is that the *encoder*
is the cost, and the wasm motor was never where the premium was. Removing the encoder's wasted
work took 40 % off it and **20 % off both wasm sums**, which is not the 25 ms the budget needed:
**both wasm arms remain a miss**, and the next cost is named below.

## Method

One stream, four arms, 3 rounds, median of 3 medians — the same method as P4e, unchanged.
`target/bench/stream-1m.jsonl` is P4e's file, replayed byte for byte: **11 lines, 497 729 524
bytes**, line 0 at 900 000 nodes. Line 0 builds the graph untimed; each of the ten batches after
it is timed as `extend` then `grow`, in two timers that never contain each other, with `tick(1)`
outside them.

| arm | command |
|---|---|
| native | `GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut\|particle-mesh --path columns` |
| wasm32, split | `scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs --from target/bench/stream-1m.jsonl --engine barnes_hut\|particle_mesh --path columns --split` |

The fixed alternating order inside each round is **native BH → wasm BH → native PM → wasm PM**,
so no arm is systematically first or last. One 1M process at a time. Before **every** process the
host gate was checked — `free -g` available ≥ 12 GB and 1-minute load < 14 — and on a failure the
process waited 60 s and re-checked, inside a 45-minute budget for the run. All 12 processes ran
and every one exited 0; the gate cleared on the first check in 10 of 12 cases and after a wait in
2 (round 1's wasm arms, at 11.87 and 9.20). Raw output, one file per process and one per gate
check, under `target/wf/p4f-step/before-pristine/` and `.../after/`.

**Caveat: this host was busy and it varied.** The 1-minute load at the twelve process starts ran
from 3.05 to 11.87 and fell across the run, so the later rounds are the quieter ones. Both
before and after were taken the same way, back to back on the same host, which is what makes the
two tables comparable to each other even though neither is a quiet-host number. Nothing here
claims an absolute cost; it claims a difference.

### Before

| round | arm | encode median | extend median | grow median | sum median | sum p95 | sum max | load start → end |
|---:|---|---:|---:|---:|---:|---:|---:|---|
| 1 | native BH | — | 15.47 | 5.13 | 21.50 | 40.71 | 52.85 | 4.96 3.86 4.72 → 14.21 6.59 5.61 |
| 2 | native BH | — | 12.36 | 3.59 | 17.41 | 31.99 | 41.67 | 7.89 7.43 6.10 → 5.83 6.96 6.00 |
| 3 | native BH | — | 12.07 | 3.56 | 15.84 | 30.73 | 41.99 | 6.71 6.95 6.07 → 5.64 6.64 6.00 |
| 1 | wasm BH | 36.40 | 45.73 | 5.35 | 50.77 | 99.35 | 118.93 | 11.87 7.41 5.96 → 10.50 7.79 6.17 |
| 2 | wasm BH | 29.73 | 37.24 | 4.29 | 41.29 | 87.91 | 117.85 | 5.83 6.96 6.00 → 5.18 6.64 5.94 |
| 3 | wasm BH | 31.18 | 37.33 | 4.37 | 41.46 | 88.57 | 120.20 | 5.64 6.64 6.00 → 4.64 6.24 5.89 |
| 1 | native PM | — | 14.33 | 7.30 | 22.43 | 41.17 | 54.82 | 10.50 7.79 6.17 → 9.20 7.64 6.15 |
| 2 | native PM | — | 12.85 | 7.71 | 20.69 | 46.72 | 55.15 | 5.18 6.64 5.94 → 7.36 7.07 6.09 |
| 3 | native PM | — | 12.48 | 6.88 | 19.37 | 39.47 | 52.40 | 4.64 6.24 5.89 → 4.54 6.17 5.87 |
| 1 | wasm PM | 29.52 | 37.14 | 10.37 | 48.24 | 56.69 | 62.10 | 9.20 7.64 6.15 → 7.89 7.43 6.10 |
| 2 | wasm PM | 25.12 | 37.24 | 10.33 | 47.55 | 59.92 | 63.34 | 7.57 7.12 6.11 → 6.71 6.95 6.07 |
| 3 | wasm PM | 29.81 | 36.01 | 10.32 | 46.83 | 58.04 | 63.28 | 4.54 6.17 5.87 → 4.35 6.04 5.84 |

Median of the three per-round medians, with `extend − encode` — the copy into linear memory plus
the wasm32 motor, which is all the encoder does not explain:

| arm | encode | extend | grow | **sum** | **(b)+(c) = extend − encode** | encode's share of `extend` | vs 30 ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| native BH columns | — | 12.36 | 3.59 | **17.41** | — | — | 0.58× **met** |
| native PM columns | — | 12.85 | 7.30 | **20.69** | — | — | 0.69× **met** |
| wasm BH columns | **31.18** | 37.33 | 4.37 | **41.46** | **6.15** | **84 %** | 1.38× **missed** |
| wasm PM columns | **29.52** | 37.14 | 10.33 | **47.55** | **7.62** | **79 %** | 1.58× **missed** |

The native arms carry no `encode` column and therefore no `(b)+(c)`: the native arm's timer does
not contain a host-side encoder at all, because the bytes it hands
`service::extend_columns` were written by a Rust encoder outside the timer (P4e, "What each
`extend` column contains"). That asymmetry is the reason the wasm `columns` column is the
pessimistic one, and it is why the two `columns` columns were never going to agree.

## What the split says

`encodeBatch` is **31.18 ms of BH's 37.33 ms `extend`** and **29.52 ms of PM's 37.14 ms**. What is
left over — the `gm_alloc`, the `Uint8Array.set` and the whole `gm_graph_extend_columns` call
inside wasm32 — is 6.15 ms and 7.62 ms.

So the brief's expectation ("the wasm arm must lose about 25 ms of `extend`") is answerable, and
the 25 ms is *not* in the motor. It is in `encodeBatch`, and it is there on **both** engines, which
is the part P4e's A1 did not anticipate: this is not a wasm32 tax at all. The native arm pays
this cost too — it simply does not pay it inside its timer.

## What the profile says

The name section is present: the release `graph_wasm.wasm` carries a custom section named `name`
of **208 795 bytes** (read by walking the section table in `target/wf/p4f-step/sections.mjs`), so
wasm functions are attributable and the mangled Rust names below are real.

`node --cpu-prof` profiles the *whole process*, and at 1M that is tens of seconds of untimed
Barnes-Hut `collide`/`charge` against about half a second of `extend` — the whole-process profile
answers a different question. So the profiler window is the batch loop only: started by hand
through `node:inspector` after line 0's build and the warm ticks, stopped after the last extend,
with `session.grow` and `session.tick` skipped (they are the layout, they are untimed in both
benches, and `extend_columns` does not read positions). `JSON.parse` and the read of each line
are inside that window and are listed as harness cost, not as `extend`.

**Caveat:** the interval is 100 µs over ten batches, so the small shares are a handful of
samples each and only the large ones are worth reading. Self times are aggregated by name —
V8 makes one profile node per call path, so the same Rust function appears several times and an
un-aggregated top-10 reports `collide` four times.

### Top 10 by self time inside the timed batches — wasm32, Barnes-Hut

| # | function | samples | share | ms, all 10 batches |
|---:|---|---:|---:|---:|
| 1 | `Table.intern` (`columns.ts:115`) — **JS** | 922 | 14.46 % | 160.39 |
| 2 | `replay` (`prof-extend.mjs:56`) — harness `JSON.parse` | 832 | 13.05 % | 144.73 |
| 3 | *(garbage collector)* | 618 | 9.69 % | 107.50 |
| 4 | the wasm export boundary (`interface:578`) | 540 | 8.47 % | 93.94 |
| 5 | `graph_core::arena::StringArena::find` — **wasm** | 498 | 7.81 % | 86.63 |
| 6 | `writeExact` (`columns-assemble.ts:263`) — **JS** | 460 | 7.21 % | 80.02 |
| 7 | `graph_core::arena::StringArena::intern` — **wasm** | 322 | 5.05 % | 56.01 |
| 8 | `assembleColumnsAs` (`columns-assemble.ts:109`) — **JS** | 210 | 3.29 % | 36.53 |
| 9 | `tableOffsets` (`columns-assemble.ts:164`) — **JS** | 190 | 2.98 % | 33.05 |
| 10 | `write` (`node:string_decoder:94`) — harness readline | 152 | 2.38 % | 26.44 |

### Top 10 — wasm32, particle mesh

| # | function | samples | share | ms, all 10 batches |
|---:|---|---:|---:|---:|
| 1 | `Table.intern` (`columns.ts:115`) — **JS** | 607 | 12.82 % | 98.24 |
| 2 | `replay` (`prof-extend.mjs:56`) — harness `JSON.parse` | 605 | 12.78 % | 97.92 |
| 3 | the wasm export boundary (`interface:578`) | 484 | 10.23 % | 78.33 |
| 4 | *(garbage collector)* | 469 | 9.91 % | 75.91 |
| 5 | `graph_core::arena::StringArena::find` — **wasm** | 364 | 7.69 % | 58.91 |
| 6 | `writeExact` (`columns-assemble.ts:263`) — **JS** | 341 | 7.20 % | 55.19 |
| 7 | `graph_core::arena::StringArena::intern` — **wasm** | 190 | 4.01 % | 30.75 |
| 8 | `tableOffsets` (`columns-assemble.ts:164`) — **JS** | 181 | 3.82 % | 29.29 |
| 9 | `assembleColumnsAs` (`columns-assemble.ts:109`) — **JS** | 140 | 2.96 % | 22.66 |
| 10 | `write` (`node:string_decoder:94`) — harness readline | 135 | 2.85 % | 21.85 |

### (b) against (c)

The three costs the `extend` timer holds, as shares of the profiled window:

| group | BH | PM |
|---|---:|---:|
| **(a) the JS encode** — `columns.ts`, `columns-batch.ts`, `columns-assemble.ts`, `encoding` | **40.01 %** | **39.74 %** |
| **(b) the staging copy** — `gm_alloc`, `gm_free`, `Uint8Array.set` | **0.25 %** | **0.11 %** |
| **(c) the wasm32 motor** — every `graph_wasm.wasm` frame | **25.05 %** | **22.92 %** |
| harness: the line read and `JSON.parse`, untimed in the bench | 16.06 % | 15.99 % |
| ungrouped (the export boundary, GC, `dlmalloc`) | 18.63 % | 21.23 % |

**(b) is 0.25 % and 0.11 % — the copy into linear memory is not worth optimising and this
document does not try.** `staging.ts`'s `copied` does one `gm_alloc`, one `Uint8Array.set` and
one `gm_free` for the whole document, and a document-sized memcpy is memcpy. Encoding straight
into the reservation instead would buy the 0.25 % and cost the assembler its single sized
buffer; on this measurement that trade is a loss. The brief offered it as an option; the profile
declines it.

Inside `(b)+(c)` — the 6.15 ms and 7.62 ms the split leaves — the profile's own split is
0.25 : 25.05, so **essentially all of it is (c), the wasm32 motor, and none of it is the copy.**
That is the useful form of the answer: the motor is cheap, the copy is free, and the encoder is
the whole premium.

## What was removed, and why it was the biggest

**(a), the JS encode, at 40 % of the window and 84 % / 79 % of the timed `extend`.** Nothing else
was a candidate: (b) is 0.25 % and (c) is bounded above by the 6–8 ms the split leaves, so even
deleting the entire wasm32 motor could not reach the 25 ms the budget needs. Three changes, each
at the root of what the profile named, and **the GMX1 bytes are unchanged**:

1. **`Table.intern` built a field path it never read** (`columns.ts:115`). Every one of the 16
   fields of every node and every edge was interned through
   `intern(\`nodes[${row}].id\`, node.id)` — a template literal, formatted and allocated, 160 000
   times a batch — and the string was used *only* if the encoder refused the value. It was the
   single largest frame in the profile on both engines. `intern` now takes the section and the
   column as the caller's own literals and builds the path in `fieldPath` on the throw path only,
   so a clean batch allocates none of them. The refusal is byte-identical: `nodes[2].id` is still
   what `ColumnsEncoderError.field` says.

2. **The assembler joined the whole string table and then, on this data, threw the join away**
   (`columns-assemble.ts:109`). `assembleColumnsAs` did `rows.strings.join("")`, sized the buffer
   at `text.length`, and asked `encodeInto` whether that was enough — and the studio's data says
   no, because its icons are `🌿` and `📈`. So a megabyte of string per batch was allocated,
   encoded into a buffer that could not hold it, discarded, and then the table was walked again
   entry by entry. The choice is now made first, by a code-unit scan that allocates nothing
   (`allAscii`), and the exact path never joins at all. This is why `writeExact` was 7.2 % of the
   window: it was the path that ran, and it was not the path that did the work.

3. **The exact path measured every entry twice** (`columns-assemble.ts:263`). `tableOffsets`
   walked the table with `utf8Length` to build the offsets, and then the placement loop called
   `utf8Length` *again* on each entry to check that `encodeInto` had written what it measured.
   One pass now: `measureBlob` returns the widths, and `placeBlob` places each entry into the slot
   its own measured width names, so the two passes cannot disagree and only one of them exists.

`encodeBatch`'s three `Float64Array.from(nodes, (node) => node.weight)` calls each allocate an
intermediate JavaScript array of every value before copying it into the typed array; they are now
one `column()` helper that writes into the `Float64Array` it allocates. Same values, same order,
no intermediate.

All of it is inside `crates/graph-sdk-js/**`. Nothing in Rust, the ABI, the GMX1 format or any
public signature changed.

### The bytes are pinned, and the pin has a negative control

Three tests, all written before or with the change and all observed red first:

- `test/extend-columns.test.mjs` **"the GMX1 bytes of a fixed batch are a pinned literal"** — a
  three-node, three-edge batch carrying a multi-byte id, a `🌿` icon, a `-0` weight, a subnormal,
  an absent optional and an edge onto a node the batch does not carry. Its 440 bytes are pinned as
  a hex literal *and* a SHA-256, so a changed byte is one diff line. **Negative control:** editing
  one byte of the literal (`3d` → `3e`, the blob-length word) makes this test fail and the other
  seven in the file stay green — verified.
- `test/columns-table.test.mjs` — `Table`'s own contract, four tests, all red against the old
  two-argument `intern`.
- `test/columns-blob.test.mjs` — the blob placement, including **"a non-ASCII table is never
  joined into one string"**, which pins the wasted copy as *behaviour* (it counts `Array.prototype.join`
  calls) rather than as a profile.

The whole SDK suite is 173 tests, green, and `sdk:typecheck` and `sdk:lint --max-warnings=0` are
both clean.

### The next share, and why it was left

After (a), the largest remaining cost inside `extend` is `StringArena::find` and
`StringArena::intern` in `crates/graph-core/src/arena.rs` — **7.81 % and 5.05 % of the window on
BH**, together about an eighth of it. Both are the FNV-1a hash in `arena.rs:47-52` walking an id
byte at a time, once per lookup.

It was left alone in this slice. The arena's handles do not come from the hash: `intern` hands
out the `IndexMap` insertion index (`arena.rs:150` asserts `slot.index() == spans.len()`), so the
hash decides bucket placement only and no dense index or output byte follows from its value. A
cheaper hash is output-neutral by construction, and the hash gate is what has to confirm it.
Whether the cost is the hash or the cache misses of probing a million-entry table is the next
thing to measure.

### After

The same four arms, the same stream, the same gate, the same order, run back to back with the
fixed encoder. Every process exited 0.

| round | arm | encode median | extend median | grow median | sum median | sum p95 | sum max | load start → end |
|---:|---|---:|---:|---:|---:|---:|---:|---|
| 1 | native BH | — | 13.41 | 3.60 | 16.31 | 34.09 | 45.83 | 3.59 3.95 4.95 → 4.14 4.05 4.94 |
| 2 | native BH | — | 12.72 | 3.76 | 16.98 | 31.84 | 41.88 | 2.83 3.69 4.75 → 2.50 3.49 4.64 |
| 3 | native BH | — | 11.74 | 3.56 | 15.33 | 32.31 | 42.38 | 2.40 3.26 4.47 → 2.27 3.12 4.37 |
| 1 | wasm BH | 18.37 | 29.73 | 4.36 | 33.97 | 84.44 | 122.58 | 4.14 4.05 4.94 → 3.26 3.84 4.83 |
| 2 | wasm BH | 18.74 | 28.10 | 4.33 | 33.00 | 83.78 | 123.91 | 2.50 3.49 4.64 → 2.61 3.38 4.54 |
| 3 | wasm BH | 18.95 | 27.92 | 4.44 | 32.64 | 83.45 | 123.10 | 2.27 3.12 4.37 → 2.55 3.07 4.30 |
| 1 | native PM | — | 11.52 | 6.75 | 18.92 | 35.91 | 48.28 | 3.26 3.84 4.83 → 3.07 3.78 4.80 |
| 2 | native PM | — | 12.49 | 6.83 | 19.18 | 38.33 | 50.91 | 2.61 3.38 4.54 → 2.52 3.33 4.52 |
| 3 | native PM | — | 11.84 | 7.24 | 19.07 | 37.20 | 50.51 | 2.55 3.07 4.30 → 2.47 3.04 4.27 |
| 1 | wasm PM | 18.20 | 27.66 | 9.95 | 37.90 | 50.52 | 56.86 | 3.07 3.78 4.80 → 2.83 3.69 4.75 |
| 2 | wasm PM | 18.16 | 27.35 | 10.27 | 37.62 | 50.28 | 56.69 | 2.52 3.33 4.52 → 2.40 3.26 4.47 |
| 3 | wasm PM | 18.06 | 27.78 | 10.18 | 37.93 | 51.07 | 57.52 | 2.47 3.04 4.27 → 2.59 3.03 4.25 |

### Before against after, median of 3 medians

| arm | encode | extend | grow | **sum** | sum vs 30 ms | change |
|---|---:|---:|---:|---:|---:|---:|
| native BH columns | — | 12.36 → 12.72 | 3.59 → 3.60 | **17.41 → 16.31** | 0.58× → **0.54× met** | −6 % |
| native PM columns | — | 12.85 → 11.84 | 7.30 → 6.83 | **20.69 → 19.07** | 0.69× → **0.64× met** | −8 % |
| wasm BH columns | **31.18 → 18.74** | 37.33 → 28.10 | 4.37 → 4.36 | **41.46 → 33.00** | 1.38× → **1.10× missed** | **−20 %** |
| wasm PM columns | **29.52 → 18.16** | 37.14 → 27.66 | 10.33 → 10.18 | **47.55 → 37.90** | 1.58× → **1.26× missed** | **−20 %** |

`encodeBatch` lost **12.44 ms (BH) and 11.36 ms (PM) — 40 % and 38 % of itself** — and that is
almost exactly what `extend` lost (9.23 and 9.48 ms), the difference being run-to-run spread.
**The native arms moved 6–8 %, and they do not run the JS encoder at all** — the bytes their
`service::extend_columns` receives were written by a Rust encoder outside the timer, which is
what "What each `extend` column contains" above says, so the 6–8 % is the host and nothing
else: the after run sat at 2.27–4.14 where the before run sat at 3.05–11.87. It is the reason
the native columns are in the table as a control, and it bounds how much of the 20 % to believe —
the *paired* change, encode against itself in the same process, is the number to trust, and it
is 40 %.

### Verdicts

| arm | sum median | vs 30 ms | verdict |
|---|---:|---:|---|
| native, Barnes-Hut, `columns` | 16.31 ms | 0.54× | **met** |
| native, particle mesh, `columns` | 19.07 ms | 0.64× | **met** |
| wasm32, Barnes-Hut, `columns` | 33.00 ms | 1.10× | **MISSED** — 3.00 ms over |
| wasm32, particle mesh, `columns` | 37.90 ms | 1.26× | **MISSED** — 7.90 ms over |

**Both wasm arms are still a miss, and they are recorded as one.** The encoder was the whole
premium and removing its waste took 20 % off both, which is real and reproducible; it is not the
25 ms the budget needed.

**Next cost, named.** A profile of the fixed encoder, same window, same 100 µs interval:

| group | before | after |
|---|---:|---:|
| (a) the JS encode | 40.01 % | **32.90 %** |
| (b) the staging copy | 0.25 % | 0.22 % |
| (c) the wasm32 motor | 25.05 % | **28.66 %** |
| harness (read + `JSON.parse`, untimed) | 16.06 % | 17.28 % |

and inside what is left of (a), the top frame is still `Table.intern` — **12.48 %, down from
14.46 %**, but now it is the `Map.get` and the *inherent* cost of the format: the GMX1 string
table is deduped in first-seen order (D4), so 160 000 probes a batch are what makes the bytes a
pure function of the document. That is not waste and there is no cheaper way to do it.
`measureBlob` (4.24 %) and `placeBlob` (2.23 %) are now one pass each, and `join` is gone from
the profile entirely.

So the next cost is **(c), the wasm32 motor — `StringArena::find` at 9.43 % and
`StringArena::intern` at 5.79 %**, which is the FNV-1a in `crates/graph-core/src/arena.rs:47-52`
walking an id byte at a time, once per lookup. It was not taken in this
slice; "The next share" above says why a different hash cannot move a handle.

One smaller, safe item is left on the table and is named here rather than taken: `utf8Length`
(`columns-blob.ts`) walks a string with a code-point iterator, which allocates a one-character
string per code unit, and it is what `measureBlob`'s 4.24 % is. A `charCodeAt` walk computes the
same number for every input including a lone surrogate. It is worth perhaps 1–2 ms of the
remaining 18; against a 3.00 ms and 7.90 ms gap, and at the cost of another full 12-process
round to measure honestly, it was not worth taking blind.

## Reproducing

```sh
# the module, and the 1M stream (P4e's file; emitted once if absent)
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --n 1000000 --stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl

# the wasm arm, split: `encode` beside `extend` and `grow`. `extend - encode` is (b)+(c).
scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut --path columns --split

# a native arm
GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl \
  --layout barnes-hut --path columns

# the pinned GMX1 bytes, and the SDK suite
scripts/orch/node-slim.sh node --test --experimental-strip-types \
  crates/graph-sdk-js/test/extend-columns.test.mjs
scripts/orch/node-slim.sh node --test --experimental-strip-types \
  'crates/graph-sdk-js/test/*.test.mjs'
```

Three rounds are those two commands over the four arms in the fixed alternating order above, each
preceded by `free -g | awk '/^Mem:/ {print $7}'` ≥ 12 and `awk '{print $1}' /proc/loadavg` < 14,
waiting 60 s and re-checking otherwise. The "before" table was taken by putting the encoder from
the tip this slice started from back in place (`git show 050715b6:crates/graph-sdk-js/src/…`,
through `target/wf/p4f-step/swap-sdk.sh`) and running the identical commands, so before and after
differ in the encoder and in nothing else. The profile is
`node --experimental-strip-types target/wf/p4f-step/prof-extend.mjs target/bench/stream-1m.jsonl
barnes_hut target/wf/p4f-step/prof/extend-bh.cpuprofile`, read by `read-prof.mjs`. All of
`target/wf/p4f-step/` is generated output and is not committed.

## Caveats

- **The host was busy and it varied.** The 1-minute load at the twelve process starts ran from
  3.05 to 11.87 and fell across the run; the later rounds are the quieter ones. Before and after
  were taken back to back, in the same order, with the same gate, which is what makes them
  comparable to each other. Neither is a quiet-host number, and the medians here are upper bounds
  on what this code costs on an idle machine.
- **The split arm does twice the encode work.** Under `--split` each batch runs `encodeBatch`
  once for the `encode` timer and again inside the timed `extendColumns`. The `extend` column is
  measured around its own call and is unaffected; the *process* does twice the GC work, so the
  split arm is not a control for the default arm.
- **The profile samples at 100 µs over ten batches.** A frame at 1 % is about ten samples. The
  large shares are solid; the small ones, `(b)`'s 0.25 % especially, are at the edge of what this
  profile can resolve and are reported as "not worth optimising" rather than as an exact figure.
- **`(b)` is measured by name, not by subtraction.** The staging copy shows up in the profile as
  `gm_alloc`/`gm_free`/`.set` frames; 0.25 % is that count, not `6.15 ms − something`.
- **A p95 over ten batches is one interpolated value, not a tail**, on both sides —
  `stream/stats.rs`'s `R7` rule in both benches.
