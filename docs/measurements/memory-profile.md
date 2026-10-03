# Motor heap under valgrind

Measured 2026-10-03 on host dlesieur42 (31.9 GB RAM, 20 CPUs) by `scripts/orch/memprofile.sh`,
which runs massif, dhat and memcheck inside the `ge-profile` image over a release build with line
tables. A run is `graph-cli tick --n <n> --ticks 2 --warm 1`: the scale model, its index and a live
force session. `grow` carries a session onto a bigger topology (`tick --grow`).

| run | n | massif peak heap B | dhat t-gmax B | B per node | dhat t-end B |
|---|---|---|---|---|---|
| tick-10000 | 10000 | 13779817 | 13779975 | 1377 | 633 |
| tick-100000 | 100000 | 124532553 | 124532711 | 1245 | 633 |
| tick-1000000 | 1000000 | 1180018511 | 1180225319 | 1180 | 633 |
| grow-10000 | 10000 | 27207850 | 27207850 | 2720 | 633 |

memcheck at n = 10000: definitely lost 0, indirectly lost 0, possibly lost 0, still reachable
633 bytes in 26 blocks, 0 errors. The 633 bytes live at exit are the same at every n, so they are
statics, not a per-graph leak.

What it shows:

- The force path is linear: about 1.2 KB per node at 1M nodes, 1.18 GB peak.
- `grow` peaks at twice `tick` because the session briefly holds the old and the new topology.
- No leak in the motor. The host freezes did not come from the motor's force path.

What it does not cover: the quadratic layouts (Kamada-Kawai, neato, the circle-packing fallback),
which allocate n² tables. Those are bounded by `graph_core::budget` instead
(`docs/decisions/memory-guard.md`).

Reproduce: `scripts/orch/memprofile.sh 10000 100000 1000000`, then read
`target/memprofile/summary.md`. It runs about 4 minutes and needs `GR_MEM` 12g.

Caveat: one generator (`crates/graph-cli/src/bench/scale.rs`) and the force engines only; a denser
graph has another bytes-per-node figure, and massif samples snapshots, so its peak can miss a short
spike (dhat's t-gmax is exact and agrees here to 0.02 %).

## Studio

Measured 2026-10-03 on the same host by `target/memprobe/synth.ts` under node 22 (node-slim,
`DRUN_MEM` 10g, `--max-old-space-size=4096`): the studio's own `syntheticRecords` and
`normaliseIngest`, the steps a worker takes before the motor sees a graph.

| nodes | links per node | step | JSON | peak RSS | time |
|---|---|---|---|---|---|
| 100 000 | 0 | synthetic | 16 MiB | 168 MiB | 67 ms |
| 100 000 | 2 | synthetic | 48.4 MiB | 276 MiB | 214 ms |
| 100 000 | 12 | synthetic | 202.2 MiB | 762 MiB | 830 ms |
| 200 000 | 12 | synthetic | 407.5 MiB | 1510 MiB | 1651 ms |
| 1 000 000 | 2 | synthetic | 492.7 MiB | 1912 MiB | 2.2 s |
| 200 000 | 2 | normalise | 97.4 MiB | 791 MiB | 1.5 s |
| 1 000 000 | 2 | normalise | 492.7 MiB | 3537 MiB | 9.3 s |

These threw `RangeError: Invalid string length` after allocating about 2 GiB: 1M × 3, 4, 5, 6 and
12; 300k, 400k and 500k × 12; 600k × 8. V8's longest string is 2^29 − 24 characters, and a 1M × 2
document is 517 M characters, 96 % of it.

What it shows: the old sliders reached graphs the page cannot build, and failed only after the
allocation. The caps in `packages/graph-studio/src/source/limits.ts` refuse before it: 2 000 000
links (nodes × links per node) for a generated graph, 2^28 characters for a document.

Caveat: one generator and one browser engine's string limit; a document with long ids reaches the
character cap at fewer nodes, and peak RSS under node is a proxy for a browser worker's heap.
