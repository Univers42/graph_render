# P4h — headroom on the wasm32 live-growth budget: the JS encode is a floor, and the arena's double probe is a trap

`docs/measurements/perf-p4g-wasm.md` met both wasm arms on a quiet host with 1.1–2.4 ms of
headroom — 28.86 ms (Barnes-Hut) and 28.36 ms (particle mesh) against a 30 ms budget — and named
the next cost as `extend − encode`: 8.40 ms and 9.28 ms, the largest single item left in either
wasm timer. This slice was asked to find **5 ms** of headroom on both arms.

**It is a miss, and it is a miss for a reason worth more than the milliseconds.** Three findings,
each measured rather than argued:

1. **The JS encoder is a floor at ~16 ms.** It is 52 % of the wasm `extend`, and four different
   arrangements of its lookups are within 3 % of each other. This is the largest single item in
   either wasm arm and this slice could not move it.
2. **The arena's double probe — the cost P4g and this brief both named as next — is a trap.**
   Removing one of its two probes per new id (25 000 fewer probes of a million-entry map per
   batch) made the **native** columns path **8 % slower** in 3 of 3 interleaved rounds, while
   helping wasm by 1–1.5 %. A control run on the record path, which the change does not touch,
   was flat over the same rounds. **Reverted.**
3. **The host cannot resolve a 1–2 ms effect.** The *same base binary* measured wasm Barnes-Hut
   at a 27.98 ms median in one interleaved run and 32.38 ms in another, an hour apart, at
   matching load, with a sibling job's `gate.sh` observed at 86 % CPU. Nothing below ~4 ms is
   measurable here, which is most of what this budget is made of.

The shipped tree is base plus one change: `relink` computes each edge's link geometry once
instead of once per moved endpoint. It is proved output-neutral by a mutation control and by
`hashgate`, `force-gate` and `scigraphs-conformance`; **its effect is below this host's
resolution and this document does not claim one.**

Base: **`c28ac2d12e8cce7336d0509e1fc3e1362dbfbb61`**, snapshotted as `target/wf/p4h-step/snap-base`
(`sha256 549f8004…`, the same bytes in every run below) before the first edit.

## Method

Unchanged from P4f/P4g: one stream, four arms, interleaved by pair. `target/bench/stream-1m.jsonl`
— 11 lines, 497 729 524 bytes, line 0 at 900 000 nodes; each of the ten batches after it timed as
`extend` then `grow`, in two timers that never contain each other, with `tick(1)` outside them.

| arm | command |
|---|---|
| native | `GR_MEM=12g timeout 3000 scripts/orch/gr cargo run -q --release -p graph-cli -- tick --stream 10000 --batches 10 --from target/bench/stream-1m.jsonl --layout barnes-hut\|particle-mesh --path columns` |
| wasm32, split | `scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs --from target/bench/stream-1m.jsonl --engine barnes_hut\|particle_mesh --path columns --split` |

`round.sh` walks the four arms one at a time behind a host gate (`free -g` available ≥ 12 GB and
1-minute load < 14, else wait 60 s and re-check), and `pairs.sh` alternates whole snapshots by
round — base r1, final r1, base r2, … — so drift hits both sides alike. Every arm runs from a
frozen snapshot (`snap.sh`: the release `graph-cli`, the release `graph_wasm.wasm`, `harness/`
and the SDK sources), so no rebuild happens between two arms and no git operation is involved.
All 24 processes of every run below exited 0 and the gate cleared on its first check every time.

**Caveat: the gate's threshold cannot see what matters here.** Load < 14 admits a host running at
2.3, and this session's entire signal is 1–4 ms. The load *ranges* are therefore reported beside
every pair rather than summarised, and the resolution limit is measured, not asserted — see
"What this host can and cannot resolve".

## The profile — both engines, the batch window only

`node --cpu-prof --cpu-prof-interval 100` over a plain (not `--split`) 1M run, then
`target/wf/p4h-step/prof.mjs`, which files every sample whose stack holds `batch` into
`extendColumns`, `grow` or `tick` and sums self time by function. Line 0's build, the warm ticks
and the snapshot fall outside `batch` and are dropped.

### wasm32, Barnes-Hut — `extendColumns` 36.07 ms/batch

| function | ms/batch | share |
|---|---:|---:|
| `intern` (JS `Table.intern`) | 9.63 | 26.7 % |
| `StringArena::find` via `node_index` | 5.50 | 15.2 % |
| `OccupiedEntry::from_hash` (the probe inside `intern`) | 2.66 | 7.4 % |
| `measureBlob` | 2.34 | 6.5 % |
| `encodeBatch` | 1.68 | 4.7 % |
| `joinTable` | 1.33 | 3.7 % |
| `dlmalloc::System::realloc` | 1.23 | 3.4 % |
| `csr::append::append` | 1.18 | 3.3 % |
| `column` | 0.91 | 2.5 % |
| `ColumnsDoc::text` | 0.70 | 2.0 % |
| `extend::row_of` | 0.68 | 1.9 % |
| `Topology::extend_columns` | 0.66 | 1.8 % |
| `StringArena::intern` (self) | 0.59 | 1.6 % |
| `BuildHasher::hash` (FNV) | 0.42 | 1.2 % |
| `Core::insert_unique` | 0.35 | 1.0 % |

### wasm32, particle mesh — `extendColumns` 32.83 ms/batch

| function | ms/batch | share |
|---|---:|---:|
| `intern` (JS `Table.intern`) | 8.42 | 25.7 % |
| `StringArena::find` via `node_index` | 5.59 | 17.0 % |
| `OccupiedEntry::from_hash` (the probe inside `intern`) | 3.32 | 10.1 % |
| `measureBlob` | 1.67 | 5.1 % |
| `encodeBatch` | 1.11 | 3.4 % |
| `dlmalloc::System::realloc` | 1.07 | 3.3 % |
| `joinTable` | 1.05 | 3.2 % |
| `csr::append::append` | 0.96 | 2.9 % |
| `column` | 0.74 | 2.3 % |
| `StringArena::intern` (self) | 0.66 | 2.0 % |
| `Topology::extend_columns` | 0.65 | 2.0 % |
| `ColumnsDoc::text` | 0.59 | 1.8 % |

### `grow`, both engines

| function | BH ms/batch | share | PM ms/batch | share |
|---|---:|---:|---:|---:|
| `dlmalloc::System::realloc` | 2.42 | 36.0 % | 1.04 | 20.4 % |
| `link::edge_geometry` | 1.44 | 21.5 % | 1.10 | 21.5 % |
| `SimpleGraph::absorb` | 0.55 | 8.2 % | 0.49 | 9.7 % |
| `Grid::grow` (particle mesh only) | — | — | 0.59 | 11.6 % |
| `csr::append::append` | 0.37 | 5.4 % | 0.32 | 6.3 % |
| `carry::beside_carried` (via `place`) | 0.36 | 5.3 % | 0.30 | 5.9 % |
| `sort::small_sort_network` + `quicksort` | 0.38 | 5.6 % | 0.34 | 6.5 % |
| `rem_pio2_large` + `rem_pio2` | 0.30 | 4.5 % | 0.25 | 4.8 % |
| `grow::relink` (self) | 0.23 | 3.4 % | 0.32 | 6.3 % |
| `grow::absorb` | 0.23 | 3.4 % | 0.08 | 1.5 % |

`tick` is outside the budget and was profiled only to be reported: 2353 ms/batch (BH) and
518 ms/batch (PM), dominated by `barnes_hut::collide::node_delta` (890 ms),
`charge::node_delta` (730 ms) and `quadtree::Builder::add` (431 ms).

**Reading the tables.** Both engines agree, so the ranking is not one arm's artefact. The JS
`Table.intern` and the arena's probes are each about a quarter of `extendColumns` and together
**52–58 %** of it; nothing else is above 6.5 %. That is why the two things this slice tried were
one in each — and why the third-largest item on the list is 1.2 ms.

**The profile's wasm frames are inflated about 2×, and the split bench says so.** The JS items in
the two tables sum to 16.59 ms (BH) against the `encode` column P4g measured at **16.43 ms** — a
ratio of 0.99, so JS maps 1:1. The wasm items sum to 19.5 ms against `extend − encode` =
**8.40 ms**, a ratio of 2.3. Every wasm number here is read as *profile ÷ 2.3* when turned into
milliseconds of an arm, and no wasm profile number is quoted as an arm result.

## The largest share, measured four ways, and **not** taken: the JS encode

`intern` is 9.63 / 8.42 ms and it is **not** the well-formedness check. On a real batch
(`target/wf/p4h-step/encode-pieces.mjs`; 10 000 nodes, 14 976 edges, median of 9):

| piece | ms | note |
|---|---:|---|
| interning the whole batch, with `isWellFormed` | **6.01** | |
| interning the whole batch, without it | **5.95** | so the check is 0.06 ms |
| `measureBlob(strings, exactWidth)` | 1.08 | `isWellFormed` 0.18 + `utf8Length` 0.90 |
| `allAscii(strings)` | 0.00 | early exit on the first emoji |
| `joinTable` | 0.61 | the one copy |
| `placeTable` (join + one `encodeInto`) | 1.94 | |

So 6 of the encoder's ~16 ms is 142 863 `Map` operations and nothing else. Where they go was
measured next (`intern-variants.mjs`, `intern-domain.mjs`; every variant asserts the table it
produces is element-for-element the shipped one):

| variant | ms | identical |
|---|---:|---|
| **shipped: one shared `Map`** | **6.42** | ✓ |
| a small `Map` per field, abandoned past 8 / 64 / 512 entries | 6.30 / 6.27 / 6.31 | ✓ |
| a per-field `Map` holding the shared table's index, past 8 / 64 / 512 | 6.28 / 6.26 / 6.31 | ✓ |
| a per-field array scan, up to 4 / 8 / 16 / 64 entries | 6.22 / 6.43 / 6.56 / 6.61 | ✓ |
| a per-field array scan plus the previous row's value | 6.59 / 6.91 / 6.96 | ✓ |

**Nothing beats it, and the reason is in the shape of the data rather than in the timings.**
75 % of the lookups are hits (142 863 lookups, 35 313 distinct values), and 63 % of them name a
field whose domain is at most 337 values — `node.kind` 2, `node.group` 5, `node.database_id` 8,
`node.icon` 16, `edge.kind` 5, `edge.label` 1, `node.source` 300. Splitting the table so those
hit a 5-entry map should have been free money and bought 2 %, because **the cost is not the
table's size but the 142 863 operations themselves**: a `Map.get` on a `JSON.parse` string costs
the same at 5 entries as at 35 000, and a linear scan of the domain costs a `===` per entry
instead. There is no cheaper arrangement of these lookups; the only way down is to do fewer of
them, and the fields that repeat are the ones whose repeats are already a pointer compare.

Two smaller JS ideas, measured and dropped for the same reason:

- **Hoisting the width walk into `intern`.** Every distinct value is walked by `isWellFormed`
  when first interned and walked *again* by `measureBlob`'s `exactWidth`, which looks like a
  deleted pass over 1.7 MB. It is not: both cover the same 35 313 distinct strings, so the
  1.08 ms moves from `measureBlob` into `intern` and the total is unchanged. `allAscii` is not a
  factor either — the studio's own icons are non-ASCII, so it early-exits and costs nothing.
- **A last-value `===` cache per field.** The repetition is real but badly placed: `node.kind`
  repeats the previous row 91 % of the time and `edge.label` 100 %, but `node.database_id`
  (8 distinct) and `node.group` (5 distinct) repeat it **0 %** — they rotate — so the cache
  catches ~23 % of lookups, half of them the empty label whose `Map.get` is already nearly free.
  Measured 6.59 ms against 6.42.

**So the encoder is a floor, and the headroom this job was asked for is not in it.** What is
left there is one sentence: 6 ms of `Map` operations and 3 ms of measuring-and-placing, against
a table of 35 313 values that must be deduplicated in first-seen order for the GMX1 bytes to be
the bytes they are. `measureBlob` and `placeTable` are at their floor too — one `encodeInto` over
one join, one width per entry — which confirms P4g's work rather than repeating it.

## Attempt 1 — one probe of the arena per new id: **measured, reverted**

**What it was.** `check_nodes` asked `topology.node_index(id)` whether the graph already held the
id, and the append then interned the same text through `Entries::string` — **two probes of a
million-entry `IndexMap` per new id**, and the same for every edge id in `check_edges`. The
profile puts the two probes together at 8.16 ms of the 36.07 ms window, the second-largest item
after the encoder, and the brief named it the arena's double probe.

**What it did** (all of it reverted, and none of it in the tree):

1. `StringArena::intern_new` returned `(Interned, bool)` — the handle plus whether the arena
   already held the string — read out of the probe `intern` already made, so `intern` became
   `intern_new(..).map(|(h, _)| h)` and there was one implementation.
2. `Entries::remember` wrote that handle into the per-entry memo, so the append's
   `Entries::string` was a memo read. That is the second probe, gone.
3. The validate passes interned the id, which means a refusal between two rows leaves strings
   the batch never earned, so `Topology::unwind` truncated the arena — a new
   `StringArena::truncate` cutting `lookup`, `spans` and `text` together, because `intern`
   appends to all three in the same order and cutting the map anywhere but the end would
   renumber every handle after it.
4. `Topology::node_claimed` / `edge_claimed` — the half of `node_index` that follows the arena
   lookup. **Not a detail:** occupancy is *not* the refusal `node_index` gives, because a string
   interned only as another field (a label equal to an id) sits in the arena with no node behind
   it. Reading occupancy as the answer refuses batches the record path accepts, which is exactly
   what `extend_columns_matches_extend` caught during development.

**Why no output byte could have moved.** The *set* of strings a successful batch interns is
unchanged. Their *order* changes — the batch's ids in row order, then the remaining fields — and
nothing reads arena slot order: every reader goes dense row → handle → text
(`index/view.rs:55`, `slots.rs:32`), and `stage/topology.rs:50`'s `topology_bytes`, which the
stage hash and every refusal test compare, writes the strings themselves. A refusal was to stay
byte-identical too, and that is the part that needed a proof rather than an argument, because the
validate passes used to be clean *by construction* and now write.

**It had the test and the negative control, and both passed.** A new test refused a batch
**midway** — first row admitted, last row a duplicate of it — and named the arena's contents on
both sides, because `strings().len()` alone cannot catch a truncate that drops the wrong strings.
With the `truncate` removed, exactly two tests went red and the rest stayed green:

```
test index::extend::tests::refusals::a_columns_batch_refused_midway_interns_nothing ... FAILED
  assertion `left == right` failed: the arena is the length it was   left: 9  right: 8
test index::extend::tests::refusals::extend_refusal_leaves_topology_unchanged ... FAILED
  assertion `left == right` failed: node id in the graph: the columns arena   left: 9  right: 8
test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured
```

`9` against `8` is the admitted row's id still in the arena.

**And then it was measured, and it lost.** Against the same base, interleaved, 3 rounds, at load
1.3–2.6 — the quietest window of the whole session — on the native Barnes-Hut `columns` route
(`target/wf/p4h-step/ab.sh`, medians of 3):

| route | base | with the change | Δ |
|---|---:|---:|---:|
| `--path columns` — `Topology::extend_columns`, **the changed path** | 11.67 | **12.59** | **+0.92 ms, +7.9 %** |
| `--path json` — `service::extend`, the record path, **not touched by the change** | 28.65 | 28.36 | −0.29 ms, −1.0 % |

The control is the point: the same binary pair, the same rounds, the same load — and the route
the change does not go near is flat while the route it does is 8 % slower. A prescribed
single-fix run (`snap-fix1`, `ROUNDS="1 2"`, base r1 / fix1 r1 / …) reproduced it: native
`extend` 11.86 → 12.93 and 11.65 → 12.50, +0.9 ms, while wasm improved (particle mesh
−1.12 / −1.69 ms, Barnes-Hut −2.52 / +0.35).

**Five measurements out of five, and a control.** That is 25 000 *fewer* probes of a
million-entry map per batch making the native path slower, and the same code making the wasm32
path faster. The brief's constraint is explicit — *native arms must not get slower* — so the
change is out of the tree.

**What is and is not established.** That the double probe is *not* where the native extend time
is, and that removing it does not pay. What is **not** established is why: the change replaced
one read-only pass over the arena with one writing pass, which plausibly cost the validate loop
the loop-invariant code motion it had when `check_nodes` took `&Topology` and the arena could not
change under it — wasm32, already register-starved, would lose less of that than x86-64. That is
a hypothesis with a mechanism, not a measurement, and the next slice should test it before
touching the arena again: keep the validate pass read-only and find another way to stop the
append re-probing.

## What shipped — `relink` computes each edge's geometry once

`crates/graph-core/src/layout/force/session/grow.rs`, and the only source file this slice
changes.

**What it was.** `relink` walked every touched vertex's whole row and recomputed `edge_geometry`
for every edge in it. `absorb` puts **both** endpoints of every edge it adds into `touched`, so
an edge the batch adds was computed twice, and an old edge between two nodes that both gained an
edge was computed twice. `edge_geometry` is two degree reads, two divisions and two clamps — a
whole call spent writing the value the first call wrote. It is 1.44 / 1.10 ms of the `grow`
window, the largest item there on Barnes-Hut, and the function's own doc comment already named
the cost as degree-proportional rather than batch-proportional.

**What changed.** A bitset over the simple graph's edges; each edge is computed the first time any
touched row yields it. The **set** of edges is untouched — still the union of the touched rows —
so the condition under which an edge is recomputed is exactly as before. Only the multiplicity
drops, and with it every second computation of a doubly-touched edge.

**Why no output byte can move.** `relink` writes `link_distance`, `link_strength` and
`link_bias`, and the new code writes **the same value to the same index** for every edge the old
code wrote — `edge_geometry` is a pure function of `(strength[e], degree(lo[e]), degree(hi[e]))`
and both calls are the same call. Order is irrelevant because there is no accumulation: each
store is an assignment, so D2's fixed-order rule is not engaged.

**The test and its negative control, both run.** Inverting the dedup so that nothing is computed
is a one-token mutation, and it turns the two tests that compare a grown session against a
`carry` of the same topologies by `to_bits` red:

```
test layout::force::session::tests::grow::grow_equals_carry ... FAILED
test layout::force::session::tests::grow::grow_across_a_side_boundary_equals_carry ... FAILED
  panicked at crates/graph-core/src/layout/force/session/tests/grow.rs:84
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured
```

The mutation is not in the tree. `hashgate --seeds 8` is 4-way equal on 8/8, `force-gate
--seeds 4` is equal on 4/4 including the wasm32 arm, and `scigraphs-conformance` passes.

**Its cost, stated:** a `vec![0; m / 64]` per `grow` call — 128 KB at a million simple edges,
zeroed per batch. A batch that touches nothing still pays it; that is the Caveat on `Written`.

**Its measured effect: none that this host can see.** The expectation going in was ~0.4 ms
(roughly 60 % of the `edge_geometry` calls were second calls). The per-pair deltas below are
mixed in sign within every arm, which is what an effect below the resolution looks like. **This
change is justified by removing work the profile measured, not by a measured win, and it should
not be reported as one.**

## The `grow`-per-batch climb, and what it is

P4g recorded particle mesh's `grow` climbing from about 2.5 ms at 920 000 rows to 4.8 ms at
1 000 000 with a constant 10 000-row batch, and said nobody had said why. One plain 1M run per
engine (the profiling runs, which are plain runs with a sampler attached):

| batch | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| wasm BH `grow`, ms | 63.94 | 4.07 | 2.33 | 2.66 | 3.50 | 6.05 | 3.75 | 6.86 | 4.21 | 4.48 |
| wasm PM `grow`, ms | 15.39 | 2.67 | 2.82 | 3.19 | 3.69 | 4.72 | 4.52 | 4.42 | 4.71 | 5.04 |

Batch 1 is not a data point — it is the JIT warming and the columns' first touch of a 1M-row
graph, which is why neither arm's `sum p95` nor `sum max` is quoted. **From batch 2 the climb is
real on both engines, and it is not the mesh:** P4g already made the mesh O(batch).

**It is amortised reallocation, and it climbs because the arrays being reallocated get bigger.**
`dlmalloc::System::realloc` is the single largest item in Barnes-Hut's `grow` at 2.42 ms/batch —
36 % of the window — and it is not one site. Every batch, `ForceSession::grow` resizes eleven
`f64` per-row columns (`px`, `py`, `x`, `y`, `vx`, `vy`, `fx`, `fy` and the three link columns),
`SimpleGraph` pushes three more, and `csr::Append` pushes into `spans` and `values`. Each is a
`Vec::resize`/`push` past capacity, and each crossing a doubling boundary copies the whole array
— 8 MB for an `f64` column at a million rows. The number of crossings in a fixed ten-batch
window does not grow with `n`, but **the size of each crossing does**, so the per-batch average
climbs with `n`. Over the ten batches that is ~24 ms of `realloc` on Barnes-Hut alone.

**Particle mesh carries a second, genuinely O(n)-per-batch term that Barnes-Hut does not:**
`Grid::grow` (0.59 ms/batch, 11.6 % of its `grow`) resets `order` and `slot` to the identity over
**all n rows** every batch — two O(n) writes plus an O(n) `clone_from_slice` for a 10 000-row
batch. **Not taken**, and named with its line (`particle_mesh/collide.rs:98`) and its invariant:
only *permutation consistency* over all n rows is required before the tick's charge pass reads the
previous sort, so the appended tail alone could be written, for roughly 0.26 ms of arm time. It
was left because the two-fix budget went on the arena probe, which is the larger item.

Neither reallocation term is fixable by reserving: the final size is not known, and a `Vec`'s
amortised doubling is what makes the steady state cheap.

## What this host can and cannot resolve

The base binary is byte-identical in every run (`sha256 549f8004…`), so it is a fixed ruler. It
was used as one, twice, an hour apart:

| interleaved run | base wasm BH, median of 3 | base wasm PM | load range |
|---|---:|---:|---|
| run A (both fixes attempted) | **27.98** | 28.34 | 2.17–3.15 |
| run D (the shipped tree) | **32.38** | 30.89 | 2.17–2.76 |

**4.4 ms on the same bytes, at overlapping load.** The `encode` column — which no code in
either tree touches — moved 15.90 → 16.97 ms over the same interval, so the whole host was
~8–12 % slower. Directly after run D a foreign process was found holding 86 % of a core:
`harness/wasm-tick-bench.mjs --repeat 5`, whose parent is another worktree's
`scripts/orch/gate.sh scripts/orch/rows/develop-full.rows`. That is a legitimate concurrent job,
not something to kill, and it is why `round.sh`'s `load < 14` gate is the wrong instrument for a
1 ms budget.

Two consequences, and both are load-bearing for everything above:

1. **No number from this session is comparable across runs.** Only within-run, pair-by-pair
   comparisons are used, and only pairs whose load ranges overlap are counted.
2. **Fix 2's effect (~0.4 ms expected) and any future fix worth less than ~4 ms cannot be
   verified on this host at all** — not with more rounds, because the drift is between-run. The
   right instrument is a quieter or pinned-frequency machine, or a micro-benchmark like the
   `edge_geometry` one rather than the 1M arm.

## Per pair, base → final, with each side's own load range

The shipped tree, 3 pairs, `target/wf/p4h-step/{base,final}/`. `wasm` rows are `--split`, so
`encode` is the JS encoder and `extend − encode` is the wasm motor. All 24 processes exited 0 and
every pair's two load ranges overlap.

### Pair 1 — base load 2.25–2.76, final load 2.51–2.86

| arm | encode | extend | grow | **sum** | Δ sum |
|---|---:|---:|---:|---:|---:|
| native Barnes-Hut | — | 13.44 → 13.51 | 2.86 → 2.99 | 16.60 → **16.62** | +0.02 |
| native particle mesh | — | 13.71 → 13.39 | 3.50 → 3.57 | 17.52 → **17.52** | 0.00 |
| wasm Barnes-Hut | 16.97 → 16.38 | 27.18 → 27.40 | 3.74 → 3.83 | 30.78 → **31.54** | +0.76 |
| wasm particle mesh | 15.30 → 15.63 | 27.11 → 25.33 | 4.02 → 4.10 | 30.93 → **29.61** | −1.32 |

### Pair 2 — base load 2.29–2.53, final load 2.24–2.30

| arm | encode | extend | grow | **sum** | Δ sum |
|---|---:|---:|---:|---:|---:|
| native Barnes-Hut | — | 13.67 → 14.19 | 2.99 → 3.04 | 16.65 → **17.79** | +1.14 |
| native particle mesh | — | 13.56 → 13.43 | 3.55 → 3.73 | 17.51 → **17.78** | +0.27 |
| wasm Barnes-Hut | 17.09 → 16.62 | 28.17 → 28.25 | 3.77 → 3.85 | 32.38 → **32.11** | −0.27 |
| wasm particle mesh | 15.30 → 15.10 | 25.80 → 26.04 | 4.05 → 4.12 | 30.02 → **30.14** | +0.12 |

### Pair 3 — base load 2.31–2.46, final load 2.36–2.60

| arm | encode | extend | grow | **sum** | Δ sum |
|---|---:|---:|---:|---:|---:|
| native Barnes-Hut | — | 13.83 → 14.43 | 2.89 → 3.07 | 17.62 → **18.14** | +0.52 |
| native particle mesh | — | 13.62 → 12.05 | 3.42 → 3.41 | 17.72 → **15.58** | −2.14 |
| wasm Barnes-Hut | 17.05 → 15.82 | 28.66 → 25.13 | 3.69 → 3.60 | 32.84 → **27.88** | −4.96 |
| wasm particle mesh | 15.33 → 14.23 | 26.41 → 23.92 | 4.04 → 3.99 | 30.89 → **28.00** | −2.89 |

## Verdicts

Counting only the three pairs above, whose load ranges all overlap.

| arm | Δ sum, per pair (ms) | sign agrees? | verdict |
|---|---:|---|---|
| native Barnes-Hut | +0.02, +1.14, +0.52 | 3/3 positive | **not slower beyond resolution** — +0.56 mean, and the `grow` column that fix 2 touches is +0.13 |
| native particle mesh | 0.00, +0.27, −2.14 | no | **no change resolvable** |
| wasm Barnes-Hut | +0.76, −0.27, −4.96 | no | **target missed** — 31.5 / 32.1 / 27.9 ms against ≤ 25 ms |
| wasm particle mesh | −1.32, +0.12, −2.89 | no | **target missed** — 29.6 / 30.1 / 28.0 ms against ≤ 25 ms |

**Both wasm arms miss by 3–7 ms, and no reading of these pairs closes any of it.** The deltas do
not agree in sign within any arm, which is the signature of an effect below the noise, and the
arms' own levels in this run (30–33 ms) are the contaminated ones: the same base tree measured
27.98 / 28.34 ms in run A. **P4 stands where P4g left it — met on a quiet host, not robustly —
and this slice did not move it.**

For the record, run A's per-pair `sum` (both fixes, load 2.17–3.15 base / 2.17–2.90 final) is
the measurement that says what the arena change would have bought and cost:

| arm | pair 1 | pair 2 | pair 3 |
|---|---:|---:|---:|
| native Barnes-Hut | 14.65 → 15.22 | 14.62 → 15.20 | 14.66 → 15.51 |
| native particle mesh | 15.26 → 15.87 | 15.14 → 15.97 | 15.12 → 15.93 |
| wasm Barnes-Hut | 28.72 → 28.29 | 27.69 → 27.78 | 27.98 → 27.79 |
| wasm particle mesh | 28.34 → 27.91 | 28.08 → 27.49 | 28.70 → 27.11 |

wasm −0.19 / −0.59 ms median, native +0.57 / +0.68 ms median: the trade that the brief's
"native arms must not get slower" rules out.

## What is left, named and measured

- **The JS encoder, ~16 ms, a floor** (the four arrangements above). The largest item in either
  wasm arm. Anything that moves it has to change what the encoder does, not how it looks up.
- **`Grid::grow`'s O(n) identity reset**, particle mesh only, ~0.26 ms of arm time, one loop:
  `particle_mesh/collide.rs:98`.
- **`absorb`'s degree scan** — `SimpleGraph::absorb` walks the *lower-degree* endpoint's row to
  reject a pair already present: 0.55 / 0.49 ms. Needed for "first pair wins".
- **`dlmalloc::realloc`**, 2.42 / 1.04 ms: amortised growth of eleven columns and two CSR arrays.
  Not fixable without knowing the final size.
- **The arena's remaining probe cost**, ~4.8 ms of arm time. Two probes are now one, and the
  one that is left is a hash plus a control-byte group plus, on a hit, the bucket and the text.
  `Span` is already the map's key precisely so a probe reads the string out of the bucket rather
  than resolving a handle first (`arena.rs:80-83`). **Do not attack this until attempt 1's
  regression is explained.**
- **The hash, and why it is still not taken.** P4g measured the byte-at-a-time FNV-1a at 28–30 %
  of `find` natively and a word-at-a-time form recovering 12–13 % of it. That is a *different
  function*, not a faster one: FNV-1a is `(h ^ b) * P` per byte and XOR does not distribute over
  multiply, so eight bytes cannot be folded into one multiply and keep the value. The value is
  pinned against the published 64-bit vectors by `crates/graph-core/src/arena/tests.rs:68`
  (guard F-35) — **a file outside this slice's allowed paths** — so changing `Fnv1a::write` is
  out of scope as well as low-value. P4g's caveat that wasm32's hash/probe ratio is unmeasured
  still stands.

## Caveats

- **Host load moves these numbers by more than the fixes do — measured, not assumed.** 4.4 ms on
  an identical binary between two interleaved runs, with a sibling job's gate observed at 86 % of
  a core. See "What this host can and cannot resolve".
- **The profile inflates wasm frames about 2×**, established by comparing the profile's JS total
  (16.59 ms) with P4g's measured `encode` (16.43 ms) and its wasm total (19.5 ms) with
  `extend − encode` (8.40 ms). Sampled at 100 µs; inlined JS frames fold into their caller and
  wasm frames are named only because the release module has a name section.
- **The four intern variants were measured on one batch of one stream, warm** — the same
  `JSON.parse` strings, so their hashes are cached after the first pass. Use them for the ratio
  between arrangements, not as an arm number; the arm's own `intern` is 9.63 ms where this bench
  says 6.42.
- **`tick` is outside the budget** and was profiled only to be reported: 2353 / 518 ms per batch
  is real work the P4 sum does not contain.
- **A p95 over ten batches is one interpolated value, not a tail**, on both sides
  (`stream/stats.rs`'s R7 rule in both benches).
- **The split arm does twice the encode work.** Under `--split` each batch runs `encodeBatch` once
  for the `encode` timer and again inside the timed `extend`. The `extend` column is measured
  around its own call and is unaffected; the *process* does twice the GC work, so the split arm is
  not a control for the default arm.

## Reproducing

```sh
# base = c28ac2d12e8cce7336d0509e1fc3e1362dbfbb61
git -C . rev-parse HEAD

# the module and the 1M stream (P4e's file)
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo run -q --release -p graph-cli -- \
  tick --n 1000000 --stream 10000 --batches 10 --emit target/bench/stream-1m.jsonl

# a wasm arm, split: `encode` beside `extend` and `grow`
scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-stream-bench.mjs \
  --from target/bench/stream-1m.jsonl --engine barnes_hut --path columns --split

# freeze this tree's artifacts, then measure base and final interleaved by round
bash target/wf/p4h-step/snap.sh target/wf/p4h-step/snap-final
STEP=target/wf/p4h-step bash target/wf/p4h-step/pairs.sh \
  target/wf/p4h-step/snap-base target/wf/p4h-step/snap-final
scripts/orch/node-slim.sh node target/wf/p4h-step/read-round.mjs target/wf/p4h-step/base
scripts/orch/node-slim.sh node target/wf/p4h-step/read-round.mjs target/wf/p4h-step/final

# one fix alone, against the snapshot before it — how attempt 1 was isolated
bash target/wf/p4h-step/snap.sh target/wf/p4h-step/snap-fix1
STEP=target/wf/p4h-step ROUNDS="1 2" bash target/wf/p4h-step/pairs.sh \
  target/wf/p4h-step/snap-base target/wf/p4h-step/snap-fix1

# the control that killed attempt 1: the changed route against the untouched one
bash target/wf/p4h-step/ab.sh target/wf/p4h-step/snap-base target/wf/p4h-step/snap-final 3

# the profile, both engines (no --split, or the untimed encode lands in "other")
scripts/orch/node-slim.sh node --experimental-strip-types --cpu-prof \
  --cpu-prof-interval 100 --cpu-prof-dir target/wf/p4h-step/prof-barnes_hut \
  harness/wasm-stream-bench.mjs --from target/bench/stream-1m.jsonl \
  --engine barnes_hut --path columns
scripts/orch/node-slim.sh node target/wf/p4h-step/prof.mjs \
  target/wf/p4h-step/prof-barnes_hut/*.cpuprofile --batches 10

# the encoder micro-benchmarks this document's tables come from
scripts/orch/node-slim.sh node --experimental-strip-types \
  target/wf/p4h-step/field-repetition.mjs target/bench/stream-1m.jsonl 1
scripts/orch/node-slim.sh node --experimental-strip-types \
  target/wf/p4h-step/encode-pieces.mjs target/bench/stream-1m.jsonl 1 9
scripts/orch/node-slim.sh node --experimental-strip-types \
  target/wf/p4h-step/intern-variants.mjs target/bench/stream-1m.jsonl 1 11
scripts/orch/node-slim.sh node --experimental-strip-types \
  target/wf/p4h-step/intern-domain.mjs target/bench/stream-1m.jsonl 1 11

# the proof, and its negative control
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo test -p graph-core --lib layout::force::session::tests::grow
```

Every 1M process is preceded by the host gate in `round.sh` (`free -g` available ≥ 12 GB and
1-minute load < 14, else wait 60 s and re-check) — which, as measured above, is too loose to
resolve this budget. `target/wf/p4h-step/` is generated output and is not committed.
