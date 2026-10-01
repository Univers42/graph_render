# Perf P2 — the Barnes-Hut tick, one thread, byte-identical

Measured 2026-10-01 on branch `perf-p2` at `7a11a55`, host dlesieur42 (20 cores, load average
1.4–5 from other jobs), image `ge-profile` (valgrind 3.24.0, hyperfine 1.19.0). Same model,
stage and commands as `docs/measurements/perf-p1-baseline.md`, which every "before" column
here comes from. The targets are that page's "Restated targets", P2 row.

```sh
scripts/orch/profile.sh 100000 3 2         # -> target/profile/n100000/
scripts/orch/profile.sh 1000000 3 2        # -> target/profile/n1000000/  (about 45 min)
scripts/orch/gr cargo run -q --release -p graph-cli -- tick --n 10000 --ticks 3 --warm 2
scripts/orch/gr cargo test -p graph-core --test tick_alloc                       # expect 0
scripts/orch/gr -e GM_MUTATE_TICK_ALLOC=1 cargo test -p graph-core --test tick_alloc   # expect 101
```

Every change on this branch is byte-identical: the 65 Barnes-Hut goldens in graph-core's
lib tests and the 4-way hash gate hold on the final tree (`docs/reports/perf-p2.md` §3).

**Caveat:** callgrind counts are exact, but its cache is valgrind's model (D1 32 KiB, LL
24 MiB, no prefetcher): a miss count compares builds, it does not predict hardware stalls.
Wall-clock rows ran on a shared host and each prints its load average; the A/B verdicts below
come from interleaved runs of two binaries under the same load, not from rows taken hours apart.

## One Barnes-Hut tick, native, single thread

`graph-cli tick --n N --ticks 3 --warm 2`, tick median. "Levers 2–5" is commit `2d0a5a5`,
"final" is `7a11a55`.

| n | P1 baseline | levers 2–5 | final | final vs P1 | target | whole run, hyperfine (P1 → final) |
|---:|---:|---:|---:|---:|---|---|
| 10 000 | 19.63 ms | 10.60 ms | 8.51 ms | 2.3× | – | – |
| 100 000 | 296.43 ms | 163.54 ms | 112.10 ms | 2.6× | ≤ 150 ms, **met** | 1.410 s → 0.698 s ± 0.005 |
| 1 000 000 | 6338.10 ms | 3790.88 ms | 1580.75 ms | 4.0× | ≤ 3.2 s, **met** | 29.46 s → 9.637 s ± 0.029 |

From 100k to 1M the tick now grows 14.1× for 10× the nodes, against 21× at P1: most of the
memory penalty the baseline found at 1M is gone, and what is left is the `n log n` walk.

## Where the instructions go (callgrind, inside `ForceSession::step`)

5 ticks (2 warm, 3 measured); Ir per node per tick = total Ir / 5 / n; per-pass shares are
inclusive (`target/profile/nN/callgrind.{log,txt}`).

| | 100k P1 | 100k final | 1M P1 | 1M final | target |
|---|---:|---:|---:|---:|---|
| Ir, total | 12.13 G | 4.36 G | 143.59 G | 49.77 G | – |
| Ir per node per tick | 24.3 k | 8.7 k | 28.7 k | 9.95 k | ≤ 15 k at 100k, **met** |
| D1 read misses | 250.0 M | 34.5 M | 3 119.9 M | 414.3 M | – |
| LL read misses | 707 208 | 1 148 723 | 392 400 706 | 81 301 566 | ≤ 40 M at 1M, **missed** (4.8× fewer than P1) |
| charge pass, of which the walk (`charge::node_delta`) | 58.9 % | 55.2 %, walk 46.7 % | 59.6 % | 55.5 %, walk 47.5 % | – |
| collide pass, of which the walk (`collide::node_delta`) | 39.3 % | 39.6 %, walk 31.8 % | 38.8 % | 40.0 %, walk 32.4 % | – |
| both quadtree builds (`Quadtree::build`, flatten included) | 4.8 % | 14.2 % | 4.7 % | 13.7 % | – |
| link pass | 1.7 % | 4.8 % | 1.4 % | 4.2 % | – |
| walk stack push/pop | 16.4 % | 0 (no stack) | 16.7 % | 0 | – |

The build's and the link's shares grew because everything around them shrank: the build is
618 M instructions at 100k, about what it cost at P1 plus the flatten.

The 100k LL read misses rose from 0.71 M to 1.15 M. The tick's working set grew by the
preorder arena (a 48-byte `Cell` per tree node, twice) and now straddles the model's 24 MiB
LL at 100k; at 100k that is a few hundred thousand misses against 34.5 M D1 misses, and at
1M, where the baseline's misses were, the trend is the opposite (table above).

## The levers, in the order the baseline set

| # | Lever | Verdict | Evidence |
|---|---|---|---|
| 2 | Stackless walk over a preorder arena (`quadtree/preorder.rs`): each cell's bounds computed once in the flatten, a subtree is the run `k..skip` | kept, `2d0a5a5` | the walk stack was 16.4 % of Ir at 100k; it is gone. 100k 296 → 164 ms with levers 4 and 5 |
| 4 | `sqrt` hoisted out of the collide walk; the opening test as `w²/θ² >= l`, computed once per cell per tick (`Body::open`) | kept, `2d0a5a5` | byte-identical by construction: the same IEEE expression per visit (goldens) |
| 5 | Zero allocations per warm tick: reused `deltas`, `ranges()` over a fixed `partition`, the arena `clear()`-ed and refilled | kept, `2d0a5a5` | `tick_alloc.rs` counts 0 over 16 warm ticks at n = 2000 (was 176); its negative control counts 99 and fails, exit 101 |
| 1 | Queries in tree order: `step_range` and the serial passes walk `order()` (leaf by leaf in preorder), and `merge` writes each delta back to `order[k]` | kept, `7a11a55` | 100k 164 → 112 ms, 1M 3791 → 1581 ms; D1 read misses at 100k 156.5 M → 34.5 M. Byte-safe because each node receives exactly one delta, so the order the outputs are laid out in moves no float |
| 3 | Leaf coordinates stored in the arena instead of read through `x[order[k]]` | **rejected** | interleaved A/B at 100k: 7 % slower. The extra 16 bytes per point outweigh the gather that lever 1 already made sequential. Diff kept out of tree (`$GM_SCRATCH/logs/perf-p2-lever3-rejected.diff`, host-local) |
| 6 | Bulk (top-down) tree build | **skipped** | not byte-identical: the charge walk's jiggle hashes `tree.key(k)`, the insertion-order id of the pointer node (`quadtree.rs` `key`), which a bulk build cannot reproduce. A build that changes it changes bytes, so it belongs under a new id (plan rule), and the particle-mesh id replaces the tree outright |

## Heap (dhat, whole run)

| | P1 | final |
|---|---:|---:|
| 100k total | 237.8 MB in 1 341 679 blocks | 297.8 MB in 1 341 707 blocks |
| 100k peak (t-gmax) | 102.0 MB, during ingest | 122.6 MB, during the last tick |
| at exit, both sizes | 611 B | 611 B |
| 1M total | 3 521.7 MB in 24 705 943 blocks | 4 013.1 MB in 24 705 986 blocks |
| 1M peak (t-gmax) | 1 159.3 MB in 10 449 823 blocks | the same bytes and blocks: still the ingest, the session stays under it |

**A regression, recorded and not fixed here.** The peak moved from the ingest to the session:
the two preorder arenas (`Vec<Cell>`, 12.6 MB each at a doubled capacity of 2^18 cells), the
`Body` aggregate (12.3 MB) and the two pointer trees (`Vec<Shape>`, 8.4 MB each) are live
together with the string arena the index keeps (about 30 MB). The 28 extra blocks are the
reused buffers; no warm tick allocates. Sizing the arenas from `shape.len()` on first growth
removes the doubling slack only when the first tick is the largest, so it is not a fix; dropping
the pointer tree needs the bulk build of lever 6. Handed on to the particle-mesh slice, whose
grid has no per-node tree, and to P7's ceilings.

## Targets

| Target (baseline, restated) | Result |
|---|---|
| 1M tick ≤ 3.2 s | 1580.75 ms, met |
| 100k tick ≤ 150 ms | 112.10 ms, met |
| Ir per node per tick ≤ 15 k at 100k | 8.7 k, met |
| LL read misses at 1M ≤ 40 M over 5 ticks | 81.3 M, **missed**: 4.8× fewer than P1 (392.4 M), 2× over the target |
| 0 allocations per warm tick | 0, met; negative control red |
| `layout.force.particle_mesh`, 1M ≤ 150 ms | not in this slice: sub-slice `perf-p2-pm` |

## What is left in the exact tick

At 100k, 10.4 % of all instructions are slice bounds checks inside `charge::node_delta` and
6.4 % inside `collide::node_delta` (`core/src/slice/index.rs` lines attributed to them). The
tree build is now 14.2 % and 26 % of D1 read misses. Those are the next levers for the exact
force; threads (P3) multiply whatever is left.

At 1M the LL read misses (81.3 M, target ≤ 40 M) split, inclusive, into: both
`Quadtree::build`s 27.6 M, the rest of the two `prepare`s (aggregate, flatten) 13.1 M, the
charge walk 8.7 M, the collide walk 9.8 M, the link pass 10.2 M, and 11.9 M in the rest of `Sim::tick`. The builds' pointer chase is
the largest share, and lever 6 (the bulk build that would remove it) moves bytes.
