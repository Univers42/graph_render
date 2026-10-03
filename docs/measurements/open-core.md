# open-core — the 1M open's string arena: pre-sized tables, and the span in the key

Measured 2026-10-02 on branch `open-core` at `d0a321a`, host dlesieur42 (20 cores, load
average 10–11 from other jobs), image `gm-chromium` (Chromium 154.0.8037.57, viewport
1920x1080, DPR 1). WebGL2 runs on SwiftShader, Chrome's CPU rasteriser: there is no GPU in
the container.

```sh
scripts/studio.sh build                       # both arms, before and after the change
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/open.py 1000000 webgl2
# the two arms are interleaved; app/dist is root-owned out of the build container, so the
# arm under test is staged through one:
docker run --rm -v "$PWD:/w" -w /w gm-chromium sh -c \
  "rm -rf app/dist && cp -a target/dist-$ARM app/dist"
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8                       # PASS
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- \
  hashgate --seeds 8                                                                 # its negative control: exit 1
```

Byte-identical, and the gate says so: `hashgate --seeds 8` passes on the final tree
(`4-way equal on 8/8 seeds`) and its negative control still fails with exit 1. Reserving
capacity moves a table's internal layout and nothing else — the hashes handed to
`raw_entry_*` are still FNV-1a over the string, in the same order, and an `IndexMap`'s
iteration order is insertion order, so no output row moved.

**Caveat:** SwiftShader is a CPU rasteriser sharing the host with other jobs, so these
numbers rank builds on this host and say nothing about a real GPU. `open.py` is a 0.5 ms
sampling probe: its self rows under a few samples are noise, and its `find` row is
inlined into its callers, so the probe's cost is charged to the mangled
`indexmap::inner::Core<…>::get_hash` frame rather than to `StringArena::find`. Every
"A against B" below is an interleaved pair under the same load: six pairs for A against C,
three for A against B.

## The two id passes

The driver's graph is `shape: random`, `seed: 1`, and `degree 2` past 5 000 nodes
(`deploy/perf/drivers/hook.js`), so this is 1 000 000 nodes and 1 999 996 edges.

`index_model` and `check_ids` walk the same strings twice over: one pass interns and
probes `StringArena`, the other resolves endpoints. Inclusive, in the worker's profile.

| Arm | Runs | `index_model` median | `check_ids` median | Sum of medians | Sum of means | Open, median |
|---|---:|---:|---:|---:|---:|---:|
| A — before (`d0a321a`) | 6 | 1809.0 ms | 892.0 ms | 2701.0 ms | 2810.5 ms | 14.09 s |
| B — A + `with_capacity` at both call sites | 3 | 1738.5 ms | 733.4 ms | 2471.9 ms (−8.5%) | 2511.4 ms (−10.6%) | – |
| C — B + the span in the key | 6 | **1613.4 ms** | **583.5 ms** | **2196.9 ms (−18.7%)** | 2360.2 ms (−16.0%) | **13.24 s** (−6.0%) |

The target was −20% on the sum of the two inclusive times. The measured fall is −18.7% on
medians and −16.0% on means: short of the target, and these are the numbers.

## What moved the numbers

| Row (self time, median of 6) | A | C |
|---|---:|---:|
| the probe — `get_index_of` / `raw_entry_v1` from `find` and `intern` | 1577.7 ms | **1102.5 ms** |
| `hashbrown::RawTable::reserve_rehash` | 250.1 ms | absent in 5 of 6 runs |
| `IndexMap<Interned, _>` — the `node_ids` / `edge_ids` probes | 276.6 / 163.2 ms | 285.9 / 251.1 ms |

Pre-sizing alone (arm B) removes the rehashes of a table that grew from empty, and that is
most of what it wins: the arena and both id sets now start at their final order of
magnitude, so `reserve_rehash` — 250 ms of self time, and the 271 ms row the profile this
job started from named — stops appearing. It does not touch the probe, which is where the
time actually is.

Keeping the span in the map key (arm C) is what moves the probe. The key was an
`Interned`, so a candidate's text cost a dependent read of `spans[slot]` before the read of
`text[start..start + len]`; the key is now the `(start, len)` pair itself, so the bucket
lands on the string directly and one random read per probe disappears. 475 ms of self time
on the probe row, and both arms hash the same.

The two `node_ids` / `edge_ids` probe rows did not move: one is flat (276.6 → 285.9 ms) and
the other is worse by 88 ms in the median (163.2 → 251.1 ms). Nothing in this change
touches that path, and the arm's variance is wide (their per-run means are 298.8 and
286.2 ms), so this is recorded as "no win", not as a regression to chase.

`check_ids` gains more than `index_model` because it probes the node-id arena twice per
edge — four million endpoint lookups on this graph — and it is the arm whose arenas were
sized exactly (every node id is distinct or the call fails), so it keeps the whole rehash
saving.

## What this cost

The lookup's bucket holds a 4-byte hash and the key: 8 bytes with an `Interned` key, 12
with a `Span` key, so the map's entries vector grows by 4 bytes per interned string. This
graph interns about 4M of them (1M node ids, ~1M distinct labels, ~2M edge ids), which is
+16 MB on its entries vector. Byte-safe because a bucket's key is only ever compared
through the hash that was computed from the string it names.

**Caveat:** that +4 bytes per string is arithmetic on the struct layouts, not a measurement
— no heap figure was taken in the browser, and `docs/measurements/p1-topology-memory.md`
still holds the committed memory numbers, which this change makes slightly worse.

## What is left

The probe is still the top self row (1102 ms), and the two `IndexMap<Interned, _>` rows
(≈540 ms between them) did not move: `admit_edge` resolves each endpoint with a
`strings.find` and then a second lookup in `node_ids`, and a `Vec<u32>` from arena slot to
dense node index, built while the nodes are admitted, would collapse the pair into one
lookup and a read. That is the other option this job listed; the section below is that
option, measured.

Not measured here: a native (non-wasm) arm, the browser's heap high-water mark, and any
graph other than the perf driver's synthetic million.

## Option (a): the arena slot table

Measured 2026-10-03 on the same host (dlesieur42, 20 cores, load average 5–7 from other
jobs), arm C of the table above as "before" — `eff5104`, which is the tree this section's
code change sits on top of — and the same tree plus the change as "after". Same image
(`gm-chromium`, Chromium on SwiftShader, viewport 1920x1080, DPR 1), same graph
(`shape: random`, `seed: 1`, degree 2 past 5 000 nodes: 1 000 000 nodes, 1 999 996 edges),
three interleaved before/after pairs on one host.

```sh
scripts/studio.sh build                       # once per arm: build, then
mkdir -p target/dist-$ARM && cp -r app/dist/. target/dist-$ARM/
# the arms are interleaved; app/dist is root-owned out of the build container, so the
# arm under test is staged through one:
docker run --rm -v "$PWD:/w" -w /w gm-chromium sh -c \
  "rm -rf app/dist && cp -a target/dist-$ARM app/dist"
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/open.py 1000000 webgl2
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8                       # PASS
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- \
  hashgate --seeds 8                                                                 # its negative control: exit 1
```

### The change

`admit_edge` resolved each endpoint twice: `strings.find` hashed the id and landed in the
arena's lookup, then `node_ids.get_index_of` hashed the resulting handle again and landed
in a second table — six hash probes per input edge on this graph, one per node id, and two
per endpoint. `index_model` now carries a local `Vec<u32>` from arena slot to dense node
index, filled in as nodes are admitted, and resolves an endpoint with the arena's probe
plus one array read. It is a local of `index_model`: no new `Topology` field, no new public
signature, nothing on the wire. A slot that no node claims holds `u32::MAX`, which
`next_index` never hands out, so "interned but not a node id" — a label, a source, a
dropped edge's id — is still not a node and an edge pointing at it is still dropped.

### The numbers

| Arm | Runs | `index_model` median | `check_ids` median | Sum of medians | Sum of means | Open, median |
|---|---:|---:|---:|---:|---:|---:|
| before — `eff5104` | 3 | 1572.1 ms | 514.9 ms | 2087.0 ms | 2077.5 ms | 10.43 s |
| after — + the slot table | 3 | **1359.9 ms** | 511.4 ms | **1871.3 ms (−10.3%)** | 1873.3 ms (−9.8%) | **10.29 s** (−1.3%) |

Per-run, in the order they ran: `index_model` 1572.1 / 1536.9 / 1579.7 before and
1339.4 / 1359.9 / 1380.4 after; `check_ids` 518.6 / 510.4 / 514.9 and 517.8 / 511.4 / 511.1;
open 10.43 / 10.39 / 10.49 s and 10.20 / 10.29 / 10.32 s.

`index_model` falls **13.5% on the median** (13.0% on the mean) — past the 3% the job asked
for, so the change stays. `check_ids` is flat (−0.7%), as it must be: it is a different
pass in a different crate and this change does not touch it. The open time falls 1.3%, and
all three `after` opens are below all three `before` opens, so it looks like a small real
win rather than noise — but three runs on a loaded host with a 0.5 ms profiler attached is
thin evidence for a claim about the whole open, so read it as "did not get worse".

Cumulative against the original arm A (1809.0 / 892.0 ms, 14.09 s open), the sum of the two
inclusive medians is now −30.7% and the open time −27.0% — but those are two hosts' worth of
load apart and this section's own before/after pair is the only comparison on one host.

### What moved the numbers

| Row (self time, median of 3) | before | after |
|---|---:|---:|
| the probe — `Core<Span>::get_hash`, from `find` and `intern` | 1025.9 ms | 1046.1 ms |
| `IndexMap<Interned, _>` `get_index_of` — the `node_ids` / `edge_ids` second probe | 240.0 ms | below 140 ms (out of the printed top 20) |
| `IndexMap<Interned, _>` `get_hash` | 229.6 ms | 197.7 ms |

The row that did not move is the arena's own probe: `admit_edge` still hashes each endpoint
once, and `admit_node` still hashes each node id once to test whether a node already claims
it. What went away is the *second* hash: `get_index_of` on `node_ids` fell out of the top 20
printed self rows, and its neighbour lost 32 ms. The remaining `get_index_of` traffic is one
edge-id de-duplication probe per input edge; `admit_edge`'s own id check still spends two
probes and is now the largest surviving pair of them.

### What this cost

4 bytes per distinct string for the slot table, live only for the length of the build and
dropped with it — at most 16 MB on this graph's ~4M interned strings, and nothing at all
once `index_model` returns. The public `Topology` is unchanged in size: `Vec<u32>` is a
local, so `Topology`'s layout, and everything derived from it, is byte-identical.

Byte-identical output, and the gate says so: `hashgate --seeds 8` passes on this tree
(`4-way equal on 8/8 seeds`) and its negative control still fails with exit 1. Nothing about
the change touches a hash: it removes a lookup, it does not reorder or rehash anything.

**Caveat:** as above — SwiftShader on a shared host, a 0.5 ms sampling probe, three runs
per arm, and the self rows of a function the compiler inlines are charged to whichever
caller the sampler saw. The `get_index_of` row's after-arm value is bounded, not measured:
`open.py` prints the top 20 self rows (`open.py:125` hardcodes `20`, so its optional third
argument does nothing) and the row fell out of them. No heap figure was taken in the
browser, so the 16 MB above is arithmetic on the sizes, not a measurement.

### What is left

`admit_edge`'s own id de-duplication is the same shape this change just removed: a
`strings.find` on the edge id and then a `edge_ids.get_index_of` on the handle, 2M probes on
this graph. The same trick applies to it, with one wrinkle the node side did not have — an
edge dropped for a missing endpoint leaves its id slot unclaimed, so the table would be
sparse and would need `u32::MAX` for the ids that were never admitted. Not measured, not
tried.

Also unmeasured: a native (non-wasm) arm, the browser's heap high-water mark, and any graph
other than the perf driver's synthetic million.