# Phase 9 — TypeScript oracle arm (d3-force, this repo's own `tick()`)

node v22.23.3 · repeat 3 · 112 ticks per layout · median over every timed tick

Same graph, same frozen force set and same golden-spiral seed as the native arm; not the
same algorithm (d3 integrates link/collide in place, graph-core gathers them), so this is a
timing arm, not a quality one. Settle is the mean of the whole 112-tick layout.

| n | links | tick ms (median) | settle ms |
|---:|---:|---:|---:|
| 220 | 327 | 0.510 | 65.6 |
| 1000 | 1532 | 3.555 | 410.3 |
| 2000 | 3068 | 9.560 | 1135.7 |
| 4000 | 6188 | 19.258 | 2189.9 |
| 10000 | 15459 | 58.564 | 6718.7 |

Largest n whose median tick fits 16.67 ms: **2000**
