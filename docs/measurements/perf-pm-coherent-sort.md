# Perf PM coherent sort: the collide grid's counting sort in last tick's order

Measured 2026-10-05 on branch `perf-pm-coherent-sort` (from develop `bc34c94c`). Host: dlesieur42,
i5-13600KF, 20 threads, 31 GB. Rust from `scripts/orch/gr`.

**Not kept.** The coherent sort is correct — every column is bit-identical to today's build — and
it is **slower**: the serial row's median went from **13.58 ms to 20.92 ms** per tick at 1M nodes on
8 workers, and the tick's median rose from 98.01 ms to 111.29 ms. The measurement is below, and the
revert is at the end.

## Why the sort looked worth attacking

`docs/measurements/perf-pm-stencil.md:67` put 14.04 ms of a 1M-node particle-mesh tick (8 workers)
outside every pass, on one thread. The collide grid's counting sort
(`particle_mesh/collide.rs::build`) is the largest serial piece: it walks the nodes in dense index
order while the buckets are in *spatial* order, so every count and every scatter is a random access
into the 8 MB `start` table. Nodes move little from one tick to the next, so walking the *previous
tick's* `order` should make the same accesses nearly sequential.

## Step 2: the breakdown (throwaway `std::time::Instant` timers)

`Grid::build`'s fill+count+prefix and its scatter, `Rows::sort` and `gravity::apply` were each
timed into a `static AtomicU64`, printed per tick from `graph-cli tick`. The patch is not in the
tree. `tick --layout particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`, three rounds, load
3.1–3.4:

| segment | ms/tick (median of 3) |
|---|---:|
| fill+count+prefix | **4.55** |
| scatter | **3.46** |
| `Rows::sort` | **4.11** |
| gravity | 0.00 |

`gravity` shows 0.00 because `sim.params.gravity` is 0 in this layout and the tick skips the force
at zero strength (`particle_mesh.rs:143`), so its segment never runs.

The stop rule asked for: below 3 ms for count+scatter, do not proceed. **count+scatter = 8.01 ms**,
so the work continued.

Note the totals do not add to the 13.3 ms serial row: the serial row is `total − covered`, and the
probe's own timers add work inside it. The segments are comparable to each other, which is what the
rule needs.

## The design (implemented, then reverted)

New `collide/sort.rs`, `Grid::build` calling it (collide.rs 282 → 271 lines, `build` shorter):

- **Buckets in slot order.** A Runner pass computes `bk[k] = bucket(cell(xy[order[k]]))` — the
  previous order's node at each slot. `bk` is a `Vec<u32>`, sized in `Grid::new`, resized in
  `Grid::grow`. It replaces `hash.rs`'s node-ordered `Buckets` kernel, which moved there.
- **Count.** `start[bk[k] + 2] += 1` over slots, then the same prefix sum.
- **Scatter in `k` order.** Read the cursor `c = start[bk[k] + 1]`, increment it, write
  `packed[c] = (order[k] as u64) << 32 | k as u64`. `packed` is a `Vec<u64>`.
- **Restore the per-bucket node order.** Sorting a bucket's `packed` run by value sorts it by node
  index, which is the contract `build` states. Insertion sort up to 32 entries, `sort_unstable`
  above; the keys are unique so stability is moot. A run already ascending is skipped.
- **Unpack.** `order[c] = hi(packed[c])` and `moved[lo(packed[c])] = c`, `moved` over `bk`'s spent
  buffer.
- **Invert.** A Runner pass sets `slot[i] = moved[old slot[i]]` into a spare `slot` buffer, then a
  `swap`.

**Why it is the same bytes:** each bucket gets the same set of nodes; the per-bucket sort puts them
in ascending node index, which is today's order; `start` holds the same counts; `slot` is `order`'s
inverse. The first build after `Grid::new` or `Grid::grow` starts from the identity, which is exactly
today's algorithm.

## Why it was slower: the attribution

A second throwaway probe (also not in the tree) timed the coherent sort's own four serial segments.
Median of 3, load 2.9–4.5, same command:

| segment | ms/tick | today's equivalent |
|---|---:|---:|
| count+prefix | **1.72** | 4.55 (step 2's fill+count+prefix) |
| scatter | **11.62** | 3.46 (step 2's scatter) |
| restore_order | **7.50** | — (new work) |
| unpack | **2.47** | — (new work) |

The premise was half right and the cost was larger than the saving:

- **The count did get faster**, 4.55 → 1.72 ms, and that is the coherence win working: walking the
  previous order makes the accesses into `start` nearly sequential.
- **The scatter got 3× slower**, 3.46 → 11.62 ms. Walking slots makes the scatter's *reads* coherent
  but its *write* is `packed[c]`, and `c` is still a bucket cursor reached through the same random
  `start` table — plus a second random table (`packed`) that today's scatter never touched, and a
  read of `order[k]`. The design traded one random scatter for a worse one.
- **`restore_order` costs 7.50 ms** and did not exist before: one pass over all 2²¹ buckets calling a
  sort on each. The length guard (`if run.len() > 1`) was added after the first bench round and only
  recovered ~0.7 ms (20.92 → 20.23), so the cost is the pass over the bucket table itself, not the
  per-run call.
- **`unpack` costs 2.47 ms** and did not exist before: a full rewrite of `order` plus a scattered
  write of `moved`.

Net: −2.8 ms of coherence, +7.5 + 2.5 ms of new work and +8.2 ms of scatter regression. The two new
Runner passes did spread some work across the workers (`sort::Buckets` 2.22 ms and `sort::Invert`
0.69 ms appear as new rows), but not enough to pay for the serial cost.

## Memory

At 1M nodes: `packed` 8 MB, `bk`/`moved` 4 MB, the spare `slot` 4 MB — **+16 MB**. For comparison,
`docs/measurements/perf-pm-stencil.md:77`'s change cost +12 MB. This change cost more memory *and*
more time.

## Gates (all run on the code, at commit `d25253a1`)

| Gate | rc |
|---|---:|
| `scripts/orch/gr cargo fmt --all --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 (484 + 1612 + 267 + 208 + … , 0 failed) |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 (PASS) |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (8 of 8 seeds diverge) |
| `xtree.sh` on this branch, `diff` against the base run | 0 (identical at all 8 node counts) |
| `wasm-threads.sh` | 0 |
| `wasm-threads.mjs hash --wasm <threads.wasm> --serial <base-serial.wasm>` | 0, every row `equal`, 0 differing |
| the same with `--break` (negative control) | 1, 64 differing |

### Tests

New `collide/sort/tests.rs`:

- **`the_coherent_build_is_the_fresh_build_tick_after_tick`** — the required test. 20 rebuilds of one
  `Grid` over a moving layout (a seeded random walk of about half a cell per tick, five nodes jumping
  across the frame, a 100-node crowd inside one cell so one bucket passes the 32-entry insertion
  bound, a NaN and an infinity, and one `grow` to 300 more nodes halfway). After every rebuild,
  `order`, `start`, `slot` and `at` (`to_bits`) equal those of a fresh `Grid::new` + `build` over the
  same positions, for workers 1, 3 and 8.
- `the_walk_reorders_so_the_comparison_is_not_vacuous` — the order is a real permutation and the
  moves really change it, or the comparison above would hold for a sort that never read the previous
  order.
- `a_rebuild_from_the_last_order_keeps_the_contract` — after a rebuild that started from the previous
  order, `order` is a permutation, every bucket's run ascends by node index, and `slot` is `order`'s
  inverse.
- `every_division_of_the_coherent_build_is_the_one_thread_build` — one worker and seven agree over
  six ticks of the walk.
- `the_insertion_bound_sorts_a_reversed_run_ascending` — the insertion/`sort_unstable` split, on a
  reversed run (the failing direction for insertion's quadratic worst case), at lengths 0, 1, 2, 31,
  32, 33, 64.

The pre-existing `every_division_of_the_build_is_the_one_thread_build` (`collide/tests.rs:130`) and the
other four collide tests pass unchanged, 31 in the `particle_mesh` filter.

**Negative control.** Commenting out the per-bucket order restore (one line in
`sort::restore_order`) turns the suite red:

```
the_coherent_build_is_the_fresh_build_tick_after_tick ... FAILED
a_rebuild_from_the_last_order_keeps_the_contract ... FAILED
every_division_of_the_build_is_the_one_thread_build ... FAILED   (the pre-existing one)
```

so the new test is not vacuous, and neither is the one that was already there. Reverted immediately;
the suite was green again before any bench ran.

## Speed (the bench that decided it)

`tick --layout particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`, arms alternated base
(`bc34c94c`, frozen as `target/wf/pm-coherent-sort/base/graph-cli`) and this tree's frozen release
build, three rounds each, each after `quiet()` and under `flock ~/goinfre/orch/bench.lock`. Load
4.6–6.6 on 20 threads. Raw: `$GM_SCRATCH/bench/pm-coherent-sort/passes-{base,final}-w8-r{1,2,3}.out`.

Medians of the three runs:

| pass | base ms | coherent ms |
|---|---:|---:|
| collide `Gather` | 33.56 | 32.99 |
| motion `Velocity` | 10.96 | 11.28 |
| fft `Pass` | 8.58 | 8.94 |
| deposit `Deposit` | 7.12 | 9.32 |
| link `LinkPass` | 5.81 | 6.43 |
| link `LinkForces` | 5.34 | 5.71 |
| motion `Position` | 3.62 | 3.76 |
| charge `Interpolate` | 2.45 | 2.58 |
| deposit `Stencils` | 2.17 | 2.30 |
| collide `Sorted` | 2.13 | 2.27 |
| collide `sort::Buckets` (new pass) | — | 2.22 |
| motion `Shift` | 0.89 | 1.17 |
| frame `Blocks` | 0.87 | 0.90 |
| hash `Buckets` (removed) | 0.67 | — |
| collide `sort::Invert` (new pass) | — | 0.69 |
| **outside any pass (serial)** | **13.58** | **20.92** |
| **tick, mean of 7** | **98.01** | **111.29** |

Per-round serial and tick medians: base 13.74 / 13.29 / 13.58 and 101.07 / 97.47 / 98.31; coherent
33.47 / 20.71 / 20.92 and 237.90 / 111.29 / 108.49. (The first coherent round was run while the
`restore_order` length guard was still missing and the host was at load 6.6; the guard and the later,
quieter rounds are the fair read.)

**Keep rule: serial median drops by ≥ 2 ms and the tick median does not rise.** It rose by 7.3 ms
and the tick by 13.3 ms. **Not kept.**

## What this does not do

- `Caveat:` the host was loaded (2.9–6.6 on 20 threads) by landers and OpenCode jobs for every run.
  The arms alternated and each process waited for `quiet()`, so they saw comparable load; only medians
  are claimed, and the first coherent round's 237.90 ms tick is a load artefact, not a result.
- `Caveat:` the per-bucket sort's worst case is a bucket whose entries arrive reversed: insertion sort
  costs O(len²) up to 32 entries per bucket. The length guard skips runs of 0 or 1, which is most of
  them at 1M nodes (2²¹ buckets for 10⁶ slots), but the pass over the bucket table is paid whatever
  the run lengths are — and that pass measured 7.50 ms.
- It does not change any byte: the gates above are the evidence, and `xtree` is identical at all
  eight node counts.
- It does not make the browser number better. The serial row is the number that carries to the
  browser, and it grew.
- It does not touch `Rows::sort` (4.11 ms, a second counting sort over slots already in grid order, so
  its accesses were near-sequential already) or `gravity` (skipped at zero strength). Both were
  measured and neither is the target.
- **No browser measurement was run.** The wasm `tick` bench was not run on this tree, so nothing here
  is a claim about the browser beyond what the serial row implies.
- The next thing this measurement points at is not the sort's coherence but the sort's *shape*: the
  scatter's cost is the random `start` write, and `restore_order` and `unpack` are two extra full
  passes over the node set. Any next attempt should attack those, not the walk order.

## The revert

The code was reverted from the working tree after the bench. `d25253a1` is the code commit, so the
implementation and its tests can be archived from there. The only file that survives is this report.

Raw: `$GM_SCRATCH/bench/pm-coherent-sort/{xtree-base.out,xtree-branch.out,passes-*.out,passes.log}`;
frozen binaries in `target/wf/pm-coherent-sort/{base,final,guard,probe,probe2}/`.