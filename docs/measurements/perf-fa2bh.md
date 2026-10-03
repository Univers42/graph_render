# perf-fa2bh: ForceAtlas2 over a Barnes–Hut tree (2026-10-02)

## Problem

The studio's default layout is `layout.forceatlas2`, a port of networkx's dense all-pairs repulsion:
O(n²) per iteration. Its registered ceiling is 14 000 nodes, and a 4 000-node graph already takes
about a second to lay out natively and several in wasm.

## Fix

`layout.forceatlas2.barnes_hut` is the same layout with the repulsion summed over the shared
Barnes–Hut quadtree (`crates/graph-core/src/layout/forceatlas2/state/barnes_hut.rs`): one tree build,
one bottom-up mass pass and one stackless preorder walk per node per iteration, O(n log n). Every other
force (attraction, gravity, swing, traction, speed) is the exact layout's code. The exact layout keeps
its id and its bytes, and is this layout's oracle.

## Opening angle

The tree repulsion against the dense repulsion on the same state, partway through a dense run of the
test graph in `barnes_hut/tests.rs` (each node linked to two earlier ones). `node` is the rms over
nodes of |approx − exact| / |exact|; `total` is |approx − exact| / |exact| over the whole force field.

| θ | 1500 nodes, 30 iter | 1500 nodes, 100 iter | 5000 nodes, 100 iter |
|---|---|---|---|
| 0.3 | node .0004, total .0009 | .0012 / .0033 | .0004 / .0041 |
| 0.5 | .0016 / .0039 | .0024 / .0033 | .0016 / .0069 |
| **0.8 (kept)** | .0070 / .0140 | .0106 / .0135 | .0057 / .0404 |
| 1.0 | .0139 / .0190 | .0318 / .0753 | .0122 / .1470 |
| 1.2 (Gephi) | .0234 / .0266 | .0706 / .1874 | .0187 / .0704 |

The error shrinks monotonically with θ, and at θ = 0 the tree sums every pair exactly (< 1e-12,
`a_closed_angle_sums_every_pair_exactly`). It grows as the layout settles: a settled node's
repulsion nearly cancels, so the far-field error is a larger share of what is left. θ = 0.8 is the
loosest angle that stays near 1% per node there; Gephi's 1.2 does not. The bounds the unit test
holds are node < 2% and total < 5% (`a_far_cell_moves_the_force_by_a_few_percent_at_most`).

## Quality: different but not worse

`graph-cli` `stress/fa2.rs` runs both layouts on every 19th stress-gate seed (32 graphs, 2 to 591
nodes) and takes the tree layout's stress correlation minus the exact layout's, with the stress
gate's own verdict (no case below −0.05, median not below it):

```
fa2 stress: 31 cases, worst -0.02186, median +0.00197
```

(One seed has too few nodes for a correlation.)

## Time

`graph-cli bench`, native release, x86_64, inside `ge-rust`, `CARGO_BUILD_JOBS=3`, one sample per
cell, host load 8–19 (other sessions' jobs running).

| n | edges | barnes_hut ms | stress-1 | exact ms | stress-1 | speed-up |
|---:|---:|---:|---:|---:|---:|---:|
| 1 000 | 1 541 | 43.59 | 0.4089 | 81.05 | 0.4082 | 1.9× |
| 5 000 | 7 721 | 263.41 | 0.4205 | 2 007.74 | 0.4197 | 7.6× |
| 10 000 | 15 474 | 583.83 | 0.4111 | 8 241.42 | 0.4101 | 14.1× |
| 100 000 | 154 978 | 8 310.29 | 0.4539 | not run | | |
| 1 000 000 | 1 549 929 | 123 031.69 | 0.4486 | not run | | |

Each tenfold step costs 14.2× and 14.8× (n log n alone is 12.5× and 12.0×; the rest is the tree
outgrowing the cache). At 1M that is 1.23 s per iteration on one thread. The exact layout is not run
past 10 000: its 4.1× per doubling projects 100 000 to about 14 minutes.

The ceiling is 250 000: under the exact layout's 30-second budget, `100 000 · (30 / 8.31)^(1/1.17)`
is 300 000, rounded down because each cell is one sample on a loaded host (load 8–19).

## Reproduce

```sh
scripts/orch/gr cargo test --release -p graph-core forceatlas2
scripts/orch/gr cargo test --release -p graph-cli fa2 -- --nocapture
scripts/orch/gr cargo run -q --release -p graph-cli -- bench \
  --layout layout.forceatlas2.barnes_hut,layout.forceatlas2 --n 1000,5000,10000
scripts/orch/gr cargo run -q --release -p graph-cli -- bench \
  --layout layout.forceatlas2.barnes_hut --n 100000,1000000 --past-ceiling
```

## What it does not do

- It is not the exact layout's picture: FA2 is chaotic, so a 1% force difference becomes a
  different layout, not a perturbed one. It is checked as not worse, never as equal.
- The 1 000-node speed-up is small: the tree costs a build and a walk per iteration, which a few
  hundred nodes do not repay.
- It is single-threaded; the walk is a per-node gather, so it is the shape the thread executor needs.
