# P4d — where `extend` spends its time at 1M, and what came out of cutting it

P4c measured `extend` + `grow` at 1M nodes (`docs/measurements/perf-p4-delta.md`) and missed
the 30 ms budget in all four arms, with **`extend` at 83–94 % of the sum**. It could not
split `extend` into parse and index, so it named the cost and left it. This slice splits it,
cuts the largest half, and re-measures the same four arms on the same input.

**Verdict: the 30 ms budget is still missed in all four arms, but the native miss is nearly
closed** — `extend` + `grow` at 1M went from 47.05 to **33.15 ms** on native Barnes-Hut
(1.57× over → **1.10× over**) and from 50.92 to **33.61 ms** on the native particle mesh
(1.70× → 1.12×). The wasm arms improved less: 73.88 → **54.29 ms** (2.46× → 1.81×) and
78.13 → **59.78 ms** (2.60× → 1.99×). The full table and the remaining cost are in "The
verdict".

The one-line finding: `extend` is **84.9 % parse and 9.6 % index** after these changes and
was 90.3 % / 9.1 % before, and the parse was walking every byte of a 1M-node batch
**four times** where two are unavoidable. Three changes took `extend` from 3.64 G to 2.09 G
instructions inside the timer, **−42.5 %**.

## Step 1 — per batch, or per topology?

Before changing anything: is `extend`'s cost per batch (parse, allocation) or does it scale
with the topology (hash-table growth, a column copy, a pass over `n`)?

`--n 100000 --stream 10000 --batches 5`, native Barnes-Hut, release, 3 rounds:

| run | extend median ms | grow median ms | sum median ms | 1-minute load |
|---|---:|---:|---:|---|
| 1 | 40.14 | 3.52 | 43.97 | 10.90 |
| 2 | 42.04 | 3.60 | 45.64 | 10.67 |
| 3 | 42.22 | 3.54 | 46.28 | 10.67 |
| **median of 3** | **42.04** | **3.54** | **45.64** | |

P4c's 1M figure, same arm, same build family: **43.25 ms**.

**Flat.** 42.04 ms at 100 000 nodes against 43.25 ms at 1 000 000 — **2.8 % lower at a tenth
of the topology**, on batch lines 1.6 % *smaller* (4.82 MB against 4.92 MB at batch 10). A
per-byte parse and a pass over the accumulated topology would move in opposite directions by
more than that; neither does. So the cost is **per batch**, and the profile below was taken
at 100k, where callgrind runs in minutes instead of tens of minutes.

Caveat: one host, wall clock, and the load is printed beside every row. The 2.8 % is inside
the run-to-run spread of the three rounds (40.14–42.22), so the honest reading is "the same
cost at a tenth of the topology", not a measured 2.8 % gain.

## Step 2 — the profile

`scripts/orch/profile.sh`'s recipe run by hand (not edited), `GR_IMAGE=ge-profile`,
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, its own `--target-dir`, valgrind callgrind
with `--cache-sim=yes --toggle-collect='*graph_wasm::service::extend*'`, `GR_MEM=12g`, over
`tick --stream 10000 --batches 5 --from target/bench/stream-100k.jsonl --layout
barnes-hut`. Raw output under `target/profile/p4d-before/` (HEAD) and
`target/profile/p4d-final/` (this slice). Instruction counts are exact and do not depend on
the host's load; the wall-clock table above and the one in "Step 4" do.

### Inclusive Ir, before and after

`service::extend` is the whole timer both arms print. Shares are of it.

| | before Ir | % of extend | after Ir | % of extend | delta |
|---|---:|---:|---:|---:|---:|
| **`service::extend`** | **3,636,918,934** | 100.00 % | **2,092,815,000** | 100.00 % | **−42.5 %** |
| `read_records` (parse) | 3,284,804,553 | 90.32 % | 1,777,439,447 | 84.93 % | −45.9 % |
| ↳ the validating walk (`Scan::value` → `value_text`) | 1,439,756,655 | 39.59 % | 871,771,982 | 41.66 % | −39.5 % |
| ↳ ↳ `Scan::string` (the lexical byte loop) | 571,926,005 | 15.73 % | 250,608,505 | 11.97 % | −56.2 % |
| ↳ ↳ `Scan::key_seen` (duplicate keys) | 291,896,102 | 8.03 % | 145,948,051 | 6.97 % | −50.0 % |
| ↳ `read_all<NodeRecord>` | 854,258,152 | 23.49 % | 473,930,917 | 22.65 % | −44.5 % |
| ↳ `read_all<EdgeRecord>` | 1,182,055,474 | 32.50 % | 624,652,857 | 29.85 % | −47.2 % |
| ↳ `element::node` | 594,971,621 | 16.36 % | inlined into `read_all<NodeRecord>` | | |
| ↳ `element::edge` | 807,556,677 | 22.21 % | inlined into `read_all<EdgeRecord>` | | |
| ↳ `Element::read` (the members pass) | 938,882,017 | 25.82 % | gone — folded into the walk | | |
| ↳ `Element::quoted` (the string re-lex) | 151,454,273 | 4.16 % | gone | | |
| ↳ `from_utf8` and the root shape checks | ≤ 14,241,306 | ≤ 0.39 % | ≤ 14,241,306 | ≤ 0.68 % | |
| **`Topology::extend`** (index) | **331,278,739** | **9.11 %** | **201,379,591** | **9.62 %** | **−15.8 %** |
| ↳ `Load::of_batch` | 6,732,687 | 0.19 % | 6,732,687 | 0.32 % | ±0 |
| ↳ `StringArena::intern` | 112,099,797 | 3.08 % | 112,099,797 | 5.36 % | ±0 |
| ↳ `StringArena::find` (the id probes) | 36,558,048 | 1.01 % | 19,130,213 | 0.91 % | −47.7 % |
| ↳ `admit_node` | 84,669,410 | 2.33 % | 83,520,201 | 3.99 % | −1.4 % |
| ↳ `admit_edge` | 49,705,083 | 1.37 % | 49,150,189 | 2.35 % | −1.1 % |
| ↳ `AppendCsr::append` | 9,137,039 | 0.25 % | 9,137,039 | 0.44 % | ±0 |
| program total inside the window | 3,717,696,906 | | 2,173,592,477 | | −41.5 % |

Three things this table settles that P4c could not:

1. **`extend` is 90 % parse.** P4c's arm could not separate them; it is `read_records`, and
   the index half was never the problem.
2. **`from_utf8` is not a cost** — under 0.4 % of the timer. Worth knowing, because it is
   the one step that looks expensive and is not.
3. **The index half was already cheap.** At 6.6 % of the timer before these changes, no
   amount of work on `Topology::extend` could have reached a 30 ms budget on its own. The
   brief's fourth hypothesis — both endpoints hashed twice — is real and is fixed below, but
   it was worth 1 % of the timer, not 30 %.

`check_nodes`, `check_edges`, `append_nodes`, `append_edges` and `Load::of_batch` are all
inlined into `Topology::extend` at this optimisation level, so callgrind reports no separate
row for them; the `Topology::extend` line is their sum. `Load::of_batch` is the one that
does escape, because `of_batch` is a distinct `fn`.

### Top 15 by exclusive Ir

Before:

| # | Ir | % | function |
|---:|---:|---:|---|
| 1 | 571,926,005 | 15.38 % | `Scan::string` |
| 2 | 205,648,941 | 5.53 % | `Scan::value'2` |
| 3 | 193,311,667 | 5.20 % | `Scan::value` |
| 4 | 115,809,957 | 3.12 % | `_int_malloc` |
| 5 | 106,863,961 | 2.87 % | `Scan::members`' element callback |
| 6 | 82,234,066 | 2.21 % | `Scan::key_seen` |
| 7 | 68,458,657 | 1.84 % | `__memcmp_avx2_movbe` |
| 8 | 38,667,533 | 1.04 % | `free` |
| 9 | 32,651,559 | 0.88 % | a `call_mut` closure frame |
| 10 | 31,181,226 | 0.84 % | `_int_free_chunk` |
| 11 | 29,981,395 | 0.81 % | `__memcpy_avx_unaligned_erms` |
| 12 | 26,322,760 | 0.71 % | `malloc` |
| 13 | 25,899,310 | 0.70 % | `_int_realloc` |
| 14 | 25,789,433 | 0.69 % | `malloc_consolidate` |
| 15 | 22,116,037 | 0.59 % | `unlink_chunk` |

After:

| # | Ir | % | function |
|---:|---:|---:|---|
| 1 | 250,608,505 | 11.53 % | `Scan::string` |
| 2 | 146,347,174 | 6.73 % | `Scan::value_text` |
| 3 | 115,894,432 | 5.33 % | `Element::fill`'s member callback |
| 4 | 66,948,837 | 3.08 % | `_int_malloc` |
| 5 | 51,542,069 | 2.37 % | `__memcmp_avx2_movbe` |
| 6 | 43,418,620 | 2.00 % | `Scan::value_text'2` |
| 7 | 41,117,033 | 1.89 % | `Scan::key_seen` |
| 8 | 38,698,144 | 1.78 % | a `call_mut` closure frame |
| 9 | 31,067,826 | 1.43 % | `free` |
| 10 | 25,776,615 | 1.19 % | `malloc_consolidate` |
| 11 | 24,747,164 | 1.14 % | `_int_free_chunk` |
| 12 | 24,186,340 | 1.11 % | `Element::span` |
| 13 | 22,335,716 | 1.03 % | `__memcpy_avx_unaligned_erms` |
| 14 | 21,041,617 | 0.97 % | `malloc` |
| 15 | 19,911,662 | 0.92 % | `StringArena::intern` |

The old #5 (`Scan::members`, 2.87 %) and the old #9 are the same work as the new #3, at 5.33 %
— the walk is the same walk, called once instead of twice, with the field mapping folded in.

### dhat — allocations

Same command, `valgrind --tool=dhat`, whole run (dhat has no toggle, so this covers the
head build, the five batches and the engine, not `extend` alone):

| | before | after |
|---|---:|---:|
| total blocks | 1,811,452 | 1,046,523 |
| total bytes | 631,933,117 | 461,218,398 |
| at t-gmax | 142,754,207 B in 105,877 blocks | 142,881,607 B in 105,878 blocks |
| allocation sites | 801 | 798 |

**−764,929 blocks, −42.2 %.** That is almost exactly the per-element key vector:
`Element::read` built a fresh `Scan` for every element, and `Scan::keys` is a `Vec` that
grows 1→2→4→8→16 as the walk pushes each key — so ~5 allocations per element, and 25 800
elements per batch across 6 `extend`s in this run. The merged walk reuses one `Scan`, so its
`keys` vector is grown once and never again. 129,000 elements × 5 growth steps ≈ 645,000, and
the two large `after`-vanished sites in the `before` profile (236,439 and 228,495 blocks,
224 B each) are that vector.

Per-batch allocations on the **record fields themselves are unchanged**, and cannot be
changed from inside this slice: one `String` per string field per record, ~135,000 for a
10 000-node / 15 000-edge batch (6 per node, 5 per edge). `NodeRecord`/`EdgeRecord` own their
strings (`crates/graph-core/src/records.rs`, outside this slice's paths) and
`service::extend` is fixed at `read_records` → `Topology::extend(&[NodeRecord], &[EdgeRecord])`
(`crates/graph-wasm/src/service.rs`, also outside). The profile agrees: the malloc family is
still ~190 M Ir after, against ~286 M before, and what fell is the key vector.

## Step 3 — the three changes

Each is a separate commit-range change, each re-timed natively. One round is enough while
iterating, as the brief says; the numbers below are **one round each at 100k**, and the
instruction counts are exact.

### 1. The root's members come out of the validating walk (`scan.rs`, `scan/walk.rs`)

`Document::new` walked the whole text twice: once to refuse (`scan.value(0)`) and once to
locate the root's members (`root_members`, which re-walked *both arrays in full* to count
them). The second walk could not find anything the first had not already validated.
`Scan::root` now does both in one pass, with the same refusals at the same offsets and the
same trailing-bytes check at the same point.

- Instructions: the second walk was 620,158,021 Ir, **17.0 % of the whole timer**, gone.
- Native extend median at 100k: **42.04 → 36.03 ms** (one round, load 9.88 against the
  10.67 of the step-1 median — the closest match available, and the only two 100k
  wall-clock readings taken at a comparable load).

### 2. One walk per record, and no second lexical pass (`scan/walk.rs`, `element.rs`, `element/table.rs`, `ingest.rs`)

Each element used to be walked three times: once as a value inside `Scan::elements`' array
walk, once by `Scan::members` to find its fields' spans, and once more per string field by
`Element::quoted`, which re-lexed bytes the walk had just read. `Scan::records` now walks
the array and each element's members **once**, handing each member over as a
`scan::Field { key, value, text }` — key, span, and the string's own unescaped text. The
record reader reads that table; it never re-lexes and never re-walks.

- Instructions: `Element::read` (938,882,017 Ir, 25.8 %) and `Element::quoted`
  (151,454,273 Ir, 4.2 %) both disappear, at the cost of the field mapping folded into the
  walk. The parse half fell **45.9 %**.
- Native extend median at 100k: measured at **43.27 ms**, which is *worse* than change 1's
  36.03 ms and is **not** what the instructions say — those rounds ran at a 1-minute load of
  **17.4**, above this slice's own 14 gate, after the host had spent ten minutes between 20
  and 50. The wall-clock reading is reported because it was taken; the instruction count is
  what the change is claimed on. This is the one number in this document that the host, not
  the code, decided.

### 3. The check resolves the endpoints, so the append does not (`index/extend.rs`)

`check_edges` had to look up both endpoints of every edge to decide whether they named a
node; `append_edges` then called `endpoints` and looked both up again — 2 extra arena
probes per edge, 30,000 per batch. `check_edges` now returns the resolved dense rows, and
the row for a *batch* node is arithmetic (`node_count + its position in the batch`) rather
than a probe, because it is in no id table yet. `append_edges` takes the pairs.

- Instructions: `StringArena::find` **36,558,048 → 19,130,213 Ir, −47.7 %**;
  `Topology::extend` 331,278,739 → 201,379,591 Ir, **−15.8 %**.
- Native extend median at 100k: folded into the step-4 measurement below rather than
  measured alone — one round against a −15.8 % instruction delta on a 9.6 % share of the
  timer would have been inside the noise, and the honest per-change attribution here is the
  instruction count.

### What was not changed, and why

- **One `String` per field per record** (~135,000 allocations per batch, ~9 % of the timer).
  `NodeRecord`/`EdgeRecord` own their strings and `service::extend`'s signature is fixed;
  both files are outside this slice's paths. This is the largest remaining cost that is not
  removable from here.
- **The remaining validating walk.** `Document::new` must read the whole text before any
  shape check, because a syntax fault in `edges[4 000 000]` is published ahead of a missing
  `version`. Folding the *record* pass into it would reorder those refusals, which
  `ingest/differential.rs` is right to call a change. It is 41.7 % of the timer and it stays.
- **`Scan::string`'s byte loop** is still the single largest leaf at 11.5 %. It is now called
  twice per string byte (once per walk) where it was called four times. A `slice::position`
  form would likely beat the bounds-checked `matches!` loop, and is the obvious next cut —
  but it is a leaf rewrite of the one function `canonical_json::parse` is mirrored against,
  and it is not needed to make the budget.

## Step 4 — the re-measurement at 1M

Exactly P4c's method (`perf-p4-delta.md` "Method"), on the same input: `tick --n 1000000
--stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl`, re-emitted here and
**byte-identical** to P4c's file (11 lines, 497,729,524 bytes, line 0 at 900,000 nodes and
447,956,269 bytes). `--release` natively; the wasm module rebuilt with
`cargo build -p graph-wasm --release --target wasm32-unknown-unknown` before the first wasm
process. Before **every** process the host gate was checked — `free -g` available ≥ 12 GB and
1-minute load < 14 — and the process waited rather than ran against a failure; the gate
waited 22 minutes before the first process and passed immediately for the other eleven.
One 1M process at a time. 3 rounds, each alternating native BH → wasm BH → native PM → wasm
PM. Raw output under `target/wf/step4/`.

| arm | extend median | grow median | **sum median** | sum p95 | sum max | vs 30 ms |
|---|---:|---:|---:|---:|---:|---:|
| native, Barnes-Hut | 28.25 ms | 4.04 ms | **33.15 ms** | 63.91 ms | 67.54 ms | **1.10×** |
| native, particle mesh | 26.44 ms | 6.69 ms | **33.61 ms** | 56.44 ms | 70.02 ms | **1.12×** |
| wasm32, Barnes-Hut | 49.76 ms | 4.30 ms | **54.29 ms** | 101.19 ms | 138.82 ms | **1.81×** |
| wasm32, particle mesh | 49.55 ms | 10.24 ms | **59.78 ms** | 72.59 ms | 80.17 ms | **1.99×** |

Median of the 3 medians; p95 and max likewise. Against P4c, same four arms:

| arm | P4c sum | this slice | change | P4c extend | this extend | P4c grow | this grow |
|---|---:|---:|---:|---:|---:|---:|---:|
| native BH | 47.05 | **33.15** | −29.5 % | 43.25 | **28.25** | 3.65 | 4.04 |
| native PM | 50.92 | **33.61** | −34.0 % | 42.34 | **26.44** | 6.79 | 6.69 |
| wasm BH | 73.88 | **54.29** | −26.5 % | 69.57 | **49.76** | 4.36 | 4.30 |
| wasm PM | 78.13 | **59.78** | −23.5 % | 67.67 | **49.55** | 10.10 | 10.24 |

`grow` is where it should be: **unchanged in all four arms** (4.04 against 3.65, 6.69 against
6.79, 4.30 against 4.36, 10.24 against 10.10 — inside run-to-run spread), because nothing in
this slice touches it. Every millisecond of the improvement is `extend`.

### Per round

Sum of `extend` + `grow` per batch, in ms.

| round | arm | b1 | b2 | b3 | b4 | b5 | b6 | b7 | b8 | b9 | b10 | median | p95 | max |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | native BH | 93.57 | 40.77 | 44.48 | 48.77 | 47.48 | 50.82 | 47.88 | 52.85 | 53.91 | 50.85 | 49.79 | 75.72 | 93.57 |
| 2 | native BH | 59.87 | 48.87 | 28.55 | 30.86 | 32.69 | 33.77 | 32.57 | 31.31 | 35.42 | 32.30 | 32.63 | 54.92 | 59.87 |
| 3 | native BH | 59.47 | 27.00 | 27.87 | 28.74 | 29.06 | 33.25 | 33.26 | 33.05 | 34.97 | 67.54 | 33.15 | 63.91 | 67.54 |
| 1 | native PM | 70.02 | 31.82 | 39.85 | 30.31 | 31.64 | 32.69 | 33.04 | 33.85 | 34.11 | 34.40 | 33.45 | 56.44 | 70.02 |
| 2 | native PM | 68.62 | 30.63 | 39.30 | 30.31 | 31.88 | 32.60 | 35.20 | 34.70 | 34.16 | 34.85 | 34.43 | 55.42 | 68.62 |
| 3 | native PM | 70.69 | 30.48 | 40.00 | 30.40 | 31.67 | 32.26 | 33.13 | 34.18 | 34.10 | 36.47 | 33.61 | 56.88 | 70.69 |
| 1 | wasm BH | 128.56 | 54.39 | 49.24 | 53.40 | 52.75 | 53.68 | 54.42 | 58.49 | 54.16 | 55.30 | 54.27 | 97.02 | 128.56 |
| 2 | wasm BH | 138.82 | 52.10 | 49.69 | 52.31 | 55.20 | 53.76 | 54.33 | 54.81 | 54.24 | 54.78 | 54.29 | 101.19 | 138.82 |
| 3 | wasm BH | 141.42 | 56.02 | 49.42 | 52.62 | 52.93 | 54.70 | 54.13 | 54.52 | 54.70 | 58.16 | 54.61 | 103.95 | 141.42 |
| 1 | wasm PM | 80.17 | 59.45 | 55.14 | 59.16 | 59.26 | 58.92 | 63.33 | 62.76 | 60.09 | 60.56 | 59.77 | 72.59 | 80.17 |
| 2 | wasm PM | 78.78 | 57.69 | 55.01 | 56.05 | 59.63 | 59.92 | 59.51 | 60.40 | 60.42 | 104.33 | 59.78 | 92.83 | 104.33 |
| 3 | wasm PM | 78.96 | 61.43 | 58.17 | 56.40 | 61.25 | 58.98 | 62.25 | 60.34 | 59.84 | 60.02 | 60.18 | 71.44 | 78.96 |

**Batch 1 is the max in 11 of 12 arm-rounds** (wasm PM round 2's max is batch 10), the same
one-off P4c found, and for the same unmeasured reason: the first append writes into columns
the topology has never touched. Only medians are claimed; the max is that one batch.

Round 1's native Barnes-Hut (49.79 ms) is the one arm-round worth distrusting: its
1-minute load was **9.28 at the start and 15.54 by the end** — above this document's own
gate — and its `extend` median is 43.54 against 28.25 and 28.00 in the two quiet rounds.
Rounds 2 and 3 ran at 8.04 and 3.60. The median of three is reported as measured; the two
quiet rounds are the honest picture of the native arm, and they are **32.63 and 33.15 ms**.

### `/proc/loadavg` per round

1-minute load at each process's start, in the order the four ran:

| round | native BH | wasm BH | native PM | wasm PM |
|---|---:|---:|---:|---:|
| 1 | 9.28 | 13.46 | 9.45 | 9.29 |
| 2 | 8.04 | 5.85 | 4.07 | 4.05 |
| 3 | 3.60 | 3.54 | 3.66 | 3.25 |

**The host was loaded and is reported rather than assumed idle.** It sat between 20 and 50
for the first half of this slice (which is why step 3's per-change wall-clock readings are
weak and the instruction counts carry the attribution), and fell to 3–4 by round 3.
Available memory was 21–22 GB throughout, and the gate passed on the first reading for 11 of
the 12 processes. The load **fell** across the rounds, so round 1 is the busiest and round 3
the quietest — and round 3 is not the fastest round (33.15 against 32.63 on native BH), so
on this host the load is not what moves the four arms apart.

### The wasm premium widened

P4c measured wasm `extend` at 1.61× native (BH) and 1.60× (PM). Here it is **1.76×** and
**1.87×**. That is the expected consequence of cutting native-side CPU: the work removed was
byte-at-a-time walking, and what is left in the wasm arm is dominated by costs the native arm
does not pay — the SDK's `JSON.stringify` of each batch before `gm_graph_extend` sees it, and
the bridge itself. **This arm does not time the serialize on its own, so the premium is
measured and the cause is not.** A closing move on the wasm side is therefore a
`JSON.stringify` question, not a parse question, and `crates/graph-sdk-js` was outside this
slice's paths.

## The remaining largest cost, named

After these changes the largest single item inside the timer is the **validating walk**
(`Scan::value_text`, 41.7 % of `extend`), and the largest single leaf is **`Scan::string`'s
byte-at-a-time loop** inside it (11.5 % of `extend`). Together they are the price of the
refusal order this ABI publishes, and they are what stands between this slice's 33 ms and the
30 ms budget natively.

## The verdict

**The 30 ms budget is missed in all four arms.** Median of the 3 medians of `extend` + `grow`
per 10 000-node batch at 1M nodes:

| arm | sum median | vs 30 ms | P4c | was |
|---|---:|---:|---:|---:|
| native, Barnes-Hut | 33.15 ms | **1.10× over** | 47.05 ms | 1.57× over |
| native, particle mesh | 33.61 ms | **1.12× over** | 50.92 ms | 1.70× over |
| wasm32, Barnes-Hut | 54.29 ms | **1.81× over** | 73.88 ms | 2.46× over |
| wasm32, particle mesh | 59.78 ms | **1.99× over** | 78.13 ms | 2.60× over |

**A miss is a miss.** The target this slice set itself — native `extend` ≤ 15 ms, so wasm
at about 1.6× would land ≤ 25 ms — was **not met**: native `extend` is 26.44–28.25 ms
against 15. The instruction count says why: −42.5 % of a body that was 90 % parse leaves the
two unavoidable walks, and one of those two is a whole validating pass over 4.9 MB.

What the slice did buy is real and measured: **−42.5 % instructions, −29.5 % wall clock
natively on Barnes-Hut and −34.0 % on the particle mesh**, with `grow` untouched and every
refusal, every record and every byte of every arm unchanged (`hashgate --seeds 8`,
`roundtrip --seeds 100`, `force-gate --seeds 4`, the ingest differential and all four
stream arms). The native miss went from 1.57× to 1.10×.

**The next cost, named:** the validating walk. Two ways forward, neither taken here. One is
`Scan::string`'s loop (11.5 % of the timer, a leaf rewrite of the function
`canonical_json::parse` is mirrored against). The other is removing the *second* walk — the
one `Document::new` does to refuse — which cannot be done without changing which refusal a
document with two faults gets, and `ingest/differential.rs` is the judge that says so. On the
wasm side the remaining cost is not in this motor at all: the SDK's `JSON.stringify`.

**Only medians are claimed.** The p95 and max are printed because the contract asks for
them; ten samples make a thin p95 (one interpolated value between the ninth and tenth
smallest sum, the same R7 rule in both arms), not a tail.

## What this slice changed

| file | what |
|---|---|
| `crates/graph-wasm/src/ingest/scan.rs` | `Document::new` locates the root on the validating pass; new `Field` |
| `crates/graph-wasm/src/ingest/scan/walk.rs` | new `root` and `records`; `value_text`; `object` yields `Field`; `rebase` |
| `crates/graph-wasm/src/ingest/element.rs` | field lists, the `Shape` trait and the two record builders |
| `crates/graph-wasm/src/ingest/element/table.rs` | **new**: the element's member table, filled by the walk |
| `crates/graph-wasm/src/ingest.rs` | `read_all` over `records`; `from_utf8` unchanged |
| `crates/graph-core/src/index/extend.rs` | `check_edges` resolves the endpoints; `append_edges` takes them; `row_of` |

Not touched: the wasm exports (`lib.rs`, `gm_*`), the SDK, `packages/`, `app/`, `harness/`,
`scripts/`, any `Cargo.toml`. No new dependency, no `unsafe`. `IndexSet`/`IndexMap` with
`FixedState` throughout; no clock in graph-core; no `HashMap`.

## What this measurement is not

- **Only medians are claimed.** p95 and max are printed because the contract asks for them;
  ten batches make a thin p95 (one interpolated value, not a tail).
- **Wall clock, one shared host.** Every number carries the 1-minute load beside it, and the
  host was loaded — see "The verdict". Nothing here survives a change of page-cache state.
- **Instruction counts, not the same thing as time.** The before/after profile deltas are
  exact; the per-change wall-clock attributions in step 3 are one round each and two of the
  three ran at loads this document's own gate would have refused.
- **The two arms do not time byte-identical work.** The wasm `extend` carries the SDK's own
  `JSON.stringify` of the batch; the native one does not. The wasm column stays the
  pessimistic one.