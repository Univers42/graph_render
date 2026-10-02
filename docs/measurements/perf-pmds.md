# perf-pmds: Pivot MDS at 100 000 and 1 000 000 nodes (2026-10-02)

## Problem

`layout.mds.pivot` took 6.8 s at 100 000 nodes natively and 10.1 s in wasm, and had not been run at
1 000 000. Three costs dominated, in this order:

1. The Gram matrix `Cᵀ C` was a dot product per column pair. Each pair strode through `C` by `k`
   (100 pivots), which caused about 1e9 cache misses at 100 000 nodes.
2. The adjacency was a `Vec` per node: a million heap blocks at 1M, grown one push at a time.
3. Each pivot's BFS ran over the whole graph with a `VecDeque`. It wrote an `f64` matrix of
   800 MB at 1M, one row stride apart per write.

## Fix

Every step below keeps the output bytes. Each one is its own commit on `perf-pmds`.

| Step | What changed | Where |
|---|---|---|
| Gram by rows | `C` is read once, in order. Four rows go into each upper-triangle cell per load and store, still one addition at a time in ascending row order. The lower triangle is the mirror, since `a * b == b * a` exactly. | `pivot_mds/matrix.rs` `gram`, `add_rows`, `add_block`, `mirror` |
| CSR adjacency | One offsets array plus one targets array, with rows sorted and deduplicated in place. O(n + m + Σ d log d), no hashing. | `spectral/neighbors.rs` `simple_neighbors` |
| Hop counts kept as `u32` columns | The centered matrix is stored as the counts (400 MB at 1M, not 800 MB), one pivot's column after another. A centered entry is rebuilt from its count by the same expression, 512 rows at a time. | `matrix.rs` `Centered` |
| Per-component, in-place BFS | Each component is renumbered to a dense local CSR. The walk is a flat queue over a slice of the hop matrix, and its "unvisited" mark is `u32::MAX` in that slice. The farthest-point choice reads a running `u32` minimum instead of an `f64` one with a `-1` mark. | `neighbors.rs` `component`; `pivot_mds.rs` `walk`, `pivot_hops` |

Sums start from `SUM_ZERO` (`-0.0`, what `Iterator::sum::<f64>()` starts from), so a sum accumulated
in place has the iterator sum's bytes, including a `-0.0` total.

## Time and memory

`graph-cli bench --layout layout.mds.pivot`, native release, x86_64, inside `ge-rust`,
`CARGO_BUILD_JOBS=3`. Peak memory is the container cgroup's `memory.peak`. The host was shared, at load
12–18, so the last two columns run the two builds interleaved, three times each.

| n | original | Gram by rows | + CSR | + `u32` columns | + in-place BFS |
|---:|---:|---:|---:|---:|---:|
| 10 000 | 229 ms | 37 ms | 43 ms | | |
| 100 000 | 6 838 ms | 496 ms | 357 ms | 287–506 ms, 154 MiB | 268–349 ms, 156 MiB |
| 1 000 000 | not run | 12 192 ms | 5 571 ms, 1 882 MiB | 5 461–5 753 ms, 1 500 MiB | 4 131–4 987 ms, 1 515 MiB |

wasm32 (`deploy/perf/wasm-open.ts 100000 layout.mds.pivot`, Node, the release artifact), layout only:

| n | original | Gram by rows | now |
|---:|---:|---:|---:|
| 100 000 | 10 138 ms | 1 182 ms | 808–1 137 ms |

Stress (`stress-1`) is the same in every column: 0.4693, 0.4772 and 0.4922 at 10k, 100k and 1M.

## Where the time goes now

Callgrind at 100 000 nodes, collection toggled on `pivot_mds::run`:

| Build | Ir |
|---|---:|
| Gram by rows, before CSR | 4.18 G |
| `u32` columns | 3.05 G |
| in-place BFS (now) | 2.61 G |

The 2.61 G breaks down as follows:

| Function | Ir | share |
|---|---:|---:|
| `gram` (`add_block`) | 1 313 M | 50% |
| the pivot walks | 739 M | 28% |
| `rows_into` (centered rows rebuilt) | 300 M | 11% |
| `Centered::new` | 85 M | 3% |
| `project` | 78 M | 3% |

At 1M the work is exactly ten times as large (30.16 G Ir against 3.05 G for the same build). The wall
time grows faster than that because of cache misses: 1.2 G D1 misses at 1M.

An attempt that failed: rewriting `add_block` as a chain of iterator `zip`s raised the count to 3.32 G
Ir, so it was reverted.

## Bytes

Each build's snapshot was hashed over 38 cases:

- `layout.mds.pivot` and `layout.spectral`;
- the 15 default seeds 0, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377 and 599;
- seeds 1 and 7 at `--nodes 3000` and `--nodes 20000`.

All 38 hashes are identical after every step.

Negative controls:

- Reassociating the Gram sum in `add_block` moved 2 of the 19 Pivot MDS hashes. The edit was
  `*cell + (a0 * b0[q] + a1 * b1[q]) + (a2 * b2[q] + a3 * b3[q])` in place of the left-to-right sum.
- Flipping the reference argmax to `>=` fails `in_place_walks_choose_the_reference_pivots_and_hops`.

Caveat: snapshot coordinates are `f32` (`docs/contract/binary-layout.md`), so the hashes only see
`f64` changes that survive rounding to `f32`. That is why the Gram negative control moved 2 hashes, not
19. Exactness in `f64` rests on unit tests that compare against the replaced code bit for bit:

- `gram_is_the_column_dot_product_bit_for_bit`
- `centered_rows_are_the_row_major_double_centering_bit_for_bit`
- `in_place_walks_choose_the_reference_pivots_and_hops` (a 9×13 grid plus 150 random-edge nodes,
  checked against the original global `VecDeque` BFS with its `f64` `covered` and `-1` mark)

Seeds 1 and 7 hash the same at a fixed `--nodes`, because the generator ignores the seed there.

## Reproduce

```sh
scripts/orch/gr cargo test -p graph-core pivot_mds
scripts/orch/gr cargo test -p graph-core neighbors
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.mds.pivot --n 10000,100000
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.mds.pivot --n 1000000 --past-ceiling
# the 38 hashes: for each layout, seed s and optional --nodes n
graph-cli snapshot --seed $s [--nodes $n] --layout $l --out-bin - | sha256sum
```

## What it does not do

- It is single-threaded. Pivot choice is sequential, because each farthest-point pick needs the
  previous walk. The Gram matrix splits by cell rows, and the components are independent (the 1M
  fixture has ten components of 100 000 nodes), so both can go to threads without changing a byte.
  That is P3.
- `gram` runs at the SSE2 baseline width, about 2.6 instructions per multiply-add. Wider vectors need
  a target feature, and the wasm32 build does not have one.
- At 1M, the peak of 1.5 GB is mostly not this layout. The hop matrix is 400 MB; the rest is the bench
  harness's graph build and the topology (P2 item 5).
- The scale ceiling is unchanged. Ceilings move in P7.
