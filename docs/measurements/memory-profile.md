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
