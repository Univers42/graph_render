# Phase 9 — wasm32 arm (graph-core compiled to wasm32, driven through the JS SDK)

node v22.23.3 · layout.force.barnes_hut · repeat 3 · module init 2.2 ms (once, not per tick)

`tick ms` is `run ms / 112`: the ABI's `gm_run` settles the layout in one call, so a
single tick is not observable from outside. It is the same per-tick-equivalent the
native arm derives from its own one-shot run, so the two crossover cells are computed
one way. It is not a measured single tick.

| n | build ms | run ms (112 ticks) | tick ms (derived) |
|---:|---:|---:|---:|
| 220 | 2.07 | 34.746 | 0.310 |
| 1000 | 7.89 | 248.228 | 2.216 |
| 2000 | 15.70 | 588.677 | 5.256 |
| 4000 | 30.73 | 1328.918 | 11.865 |
| 10000 | 79.67 | 4021.332 | 35.905 |

Largest n whose derived tick fits 16.67 ms: **4000**
