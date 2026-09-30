# Tier-1 layouts: measured scale ceilings

Measured 2026-09-29, p12-t1 tree, native release, one host, `seed 0` synthetic model
(`graph_core::seeded_model`, concatenated components past 100 000 nodes).

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.random \
  --layout layout.circular.ring --layout layout.spiral --layout layout.bipartite \
  --n 220,10000,100000,1000000 --past-ceiling --repeat 3
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.yifan_hu \
  --n 220,10000,100000 --past-ceiling --repeat 1
```

| layout | 220 | 10 000 | 100 000 | 1 000 000 | ceiling |
|---|---|---|---|---|---|
| `layout.random` | 0.00 ms | 0.03 ms | 1.01 ms | 9.17 ms | 1 000 000 |
| `layout.circular.ring` | 0.01 ms | 0.13 ms | 1.32 ms | 13.23 ms | 1 000 000 |
| `layout.spiral` | 0.01 ms | 0.28 ms | 3.78 ms | 32.03 ms | 1 000 000 |
| `layout.bipartite` | 0.03 ms | 0.88 ms | 9.39 ms | 271.95 ms | 1 000 000 |
| `layout.force.yifan_hu` | 25.60 ms | 3 097.19 ms | 60 907.89 ms | not run | 100 000 |

Ponytail: the four closed forms were run to 1 000 000, the largest size `bench` accepts, so their
ceiling is a lower bound (they are linear; the grid's 4 600 000 memory projection is untouched
here). Yifan Hu's 100 000 is one timing (`--repeat 1`, a run takes a minute), not a median, and is
about the 60 s budget rather than under it.
