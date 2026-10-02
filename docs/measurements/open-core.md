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
lookup and a read. That is the other option this job listed; it was not tried, because the
job sanctioned one, and it is where the next 20% would have to come from.

Not measured here: a native (non-wasm) arm, the browser's heap high-water mark, and any
graph other than the perf driver's synthetic million.