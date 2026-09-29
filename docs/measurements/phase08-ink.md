# Phase 8 — ink saved by the edge bundlers, and their scale ceilings

Produced by `graph-cli ink`, native, release, in the Docker toolchain (`gr cargo run --release -p
graph-cli -- ink ...`). One run per row, no warm-up, no repeat (Ponytail: timings drift by tens of
percent with host load; the ceilings below leave that margin).

```sh
gr cargo run --release -p graph-cli -- ink --fixture hairball            # exit 0 = every bundler reduced the cells
gr cargo run --release -p graph-cli -- ink --nodes 1600                  # the scale sweep
```

Ink is the count of cells of a 128 x 128 raster that any drawn segment crosses (`post/ink.rs`).
Ponytail: a resolution, not a truth. It saturates on a dense drawing (13 104 cells is the whole
drawn area here), where a bundler that tightens curves still reads as 0% — the FDEB rows from 400
nodes up are that saturation, not evidence that FDEB stopped bundling. Length is reported beside it
because bundling never shortens a path.

## Fixture `hairball` (42 nodes, 178 edges), circular layout

| capability | cells | length | ink reduction | pairs | unbundled |
|---|---|---|---|---|---|
| straight (layout only) | 8871 | 211.029 | — | — | — |
| post.bundle.fdeb | 8395 | 212.470 | 5.37% | 994 | 61 |
| post.bundle.mingle | 2261 | 371.313 | 74.51% | 133 | 31 |

Grid layout, same fixture: straight 6764 cells; MINGLE 4648 (31.28%); FDEB 8232 (−21.70%). FDEB on
a lattice layout spreads curves over cells the straight edges did not touch, so a raster
reduction is not guaranteed for it there; the default layout of the command is the circular one,
which is the case bundling exists for. The `post_composability` test asserts on the long-span
fixture, where both reduce cells on the grid.

## Scale sweep (hairball generator, circular layout)

| nodes | edges | FDEB ms | FDEB pairs | MINGLE ms | MINGLE ink reduction |
|---|---|---|---|---|---|
| 100 | 430 | 2.6 | 5 341 | 31.4 | 82.37% |
| 200 | 864 | 10.3 | 20 891 | 122.6 | 82.93% |
| 400 | 1 730 | 39.4 | 81 252 | 336.4 | 83.66% |
| 800 | 3 464 | 157.0 | 319 504 | 1 113.9 | 83.65% |
| 1 600 | 6 930 | 623.2 | 1 268 360 | 4 179.5 | 84.31% |
| 3 200 | 13 864 | 2 582.7 | 5 051 209 | 16 510.8 | 84.23% |

Both are quadratic in edges. The budget for a one-shot redraw is one second.

- `scale_ceiling` **post.bundle.fdeb = 6 900 edges** (623 ms at 6 930; extrapolated 1 s near 8 800,
  so the ceiling keeps about 25% margin). FDEB is `O(m^2)` to build the pair list; the reference's
  spatial pruning is not ported (Ponytail in `post/fdeb.rs`).
- `scale_ceiling` **post.bundle.mingle = 3 000 edges** (1 114 ms at 3 464, 336 ms at 1 730;
  quadratic interpolation gives about 0.85 s at 3 000).
