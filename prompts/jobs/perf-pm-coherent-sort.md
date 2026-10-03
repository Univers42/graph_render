# Job perf-pm-coherent-sort (agent build): the collide grid's counting sort in last tick's order, same bytes

Why: at 1M nodes on 8 workers, 14–16 ms of the 107 ms particle-mesh tick runs outside every pass,
on one thread (`docs/measurements/perf-pm-stencil.md:62`). No worker count shortens it, and in the
browser it costs the same.

- The largest serial piece is likely the collide grid's counting sort. It walks the nodes in index
  order, but their buckets are in spatial order, so every count and every scatter is a random
  access into an 8 MB `start` table.
- Nodes move little from one tick to the next. Walked in last tick's sorted order, the same sort
  touches memory almost sequentially.

Measure first. Change only if the measurement says the sort is worth it.

Facts (verified on develop 50d0015d):

- **`Grid`** (`crates/graph-core/src/layout/force/particle_mesh/collide.rs:38`, 249 lines) holds:
  - `order` (slot → node);
  - `start` (`buckets + 2` entries; buckets = `(2n).next_power_of_two()`, so 2^21 at 1M);
  - `slot` (node → slot, `order[slot[i]] == i`);
  - `at` (positions in slot order);
  - `hash`, `blocks`.

  `Grid::new` (`:66`) sets `order` and `slot` to the identity.
- **`Grid::build`** (`:87-123`) runs these steps:
  1. `frame::bounds` (Runner);
  2. `Buckets` (Runner, `:101`) writes each node's bucket into `slot`, in node order;
  3. `start.fill(0)`;
  4. a count over `slot`;
  5. a prefix sum;
  6. a scatter in node order (`:111-117`), writing `order[k] = i` and `slot[i] = k`;
  7. `Sorted` (Runner, `:122`) gathers `at`.

  Steps 3 to 6 are on one thread.

  The contract (`:84-86`): inside a bucket, slots ascend by **node index**. The scatter in node
  order gives that for free.
- **Callers.** `collide::apply` (`:228-245`) builds the grid each tick on `(px, py)`. Then:
  - `Gather` reads `grid.order`, `grid.slot`, `grid.at` and `start` via `reads`;
  - the next tick's deposit walks `grid.order` (`mesh.rs:109`);
  - `motion::integrate` reads `grid.slot` (`particle_mesh.rs:133`, `charge.rs:49`).

  Any change must leave `order`, `start`, `slot` and `at` bit-identical to today's.
- **The test that already pins it.** `collide/tests.rs:130`
  `every_division_of_the_build_is_the_one_thread_build`:
  - a rebuild after a move, starting from the first build's columns, must equal a fresh build;
  - `assert_same_build` is at `:113`.

  A coherent sort makes that test meaningful: the rebuild then really starts from the old order.
- **`--passes`.** `graph-cli tick --passes` (`crates/graph-cli/src/bench/tick/passes.rs:1-9`,
  `:57`) times every Runner pass, and the row "outside any pass (serial)" is the rest.
- **Rows with sorts.** `Rows::sort` (`particle_mesh/deposit.rs:70`) is a second counting sort, over
  slots already in grid order, so its accesses are near-sequential already. `gravity::apply`
  (`particle_mesh.rs:128`) is serial.
- **Limits.**
  - ≤ 40 lines per function, ≤ 4 parameters, ≤ 300 lines per file (split into
    `collide/sort.rs`), nesting ≤ 3.
  - No `unsafe`, no new dependency.
  - graph-core builds for `wasm32-unknown-unknown`.
  - No clock in committed graph-core code.

Do, in order:

1. **Pin.** In `~/goinfre/bench/pm-coherent-sort/`, copy `~/goinfre/bench/pm-stencil/xtree.sh`, run
   it on the untouched worktree and keep `xtree-base.out`.
2. **Measure the serial row.** Patch `Grid::build`, `Rows::sort` and `gravity::apply` with
   `std::time::Instant` timers. The patch is throwaway and never committed. Accumulate the
   per-segment totals in a `static` `AtomicU64` per segment, and print them per tick at the end of
   `graph-cli tick`.
   - Segments: `fill` + count + prefix, the scatter, `Rows::sort`, gravity.
   - Run `GR_MEM=12g scripts/orch/gr cargo run --release -q -p graph-cli -- tick --layout
     particle-mesh --n 1000000 --ticks 7 --workers 8 --passes`, 3 runs, and print the load per run.
   - Then `git checkout -- <the patched files>` and show that `git diff --stat` lists none of them.
   - **Stop rule:** if count + scatter is under 3 ms per tick (median), do not do steps 3–5. Write
     the report with the breakdown and return `status: done`, `kept: not attempted`.
3. **The coherent sort** (new `collide/sort.rs`; `Grid::build` calls it):
   - **Buckets in slot order.** A Runner pass computes `bk[k] = bucket(xy[order[k]])`, the previous
     order's node at each slot. `bk` is a `Vec<u32>`, sized in `Grid::new`.
   - **Count.** `start[bk[k] + 2] += 1` for each `k`, then the same prefix sum.
   - **Scatter in `k` order.**
     - Read the cursor `c = start[bk[k] + 1]` and increment it.
     - Write `packed[c] = (order[k] as u64) << 32 | k as u64`. `packed` is a `Vec<u64>`, sized in
       `Grid::new`.
   - **Restore the per-bucket node order.** Sorting a bucket's `packed` entries by value sorts them
     by node index. Use an insertion sort for a bucket of at most 32 entries, and
     `sort_unstable` above that: the keys are unique, so stability is moot.
     - Caveat: a bucket whose entries arrive reversed costs O(len²) up to 32 entries per bucket.
     - Hoist the check: skip a bucket when `packed[s..e]` is already ascending.
   - **Unpack.**
     - For each `c`: `order[c] = hi(packed[c])` and `moved[lo(packed[c])] = c`. `moved` is
       indexed by old slot and may reuse `bk`'s buffer.
     - Then a Runner pass sets `slot[i] = moved[old_slot[i]]`. That is a gather by node; write it
       into a spare `slot` buffer and swap.
   - **Report the added memory** at 1M (`packed` 8 MB, `bk`/`moved` 4 MB, the spare slot 4 MB),
     against `docs/measurements/perf-pm-stencil.md`'s +12 MB line.
   - **Why it is the same bytes:**
     - each bucket gets the same set of nodes;
     - the per-bucket sort puts them in ascending node index, which is today's order;
     - `start` is the same counts, and `slot` is `order`'s inverse.

     The first build after `Grid::new` starts from the identity, which is exactly today's
     algorithm.
4. **Tests** (`collide/tests.rs`, or a new `collide/sort/tests.rs`):
   - **The existing tests pass unchanged**, `every_division_of_the_build_is_the_one_thread_build`
     included.
   - **New: `the_coherent_build_is_the_fresh_build_tick_after_tick`.**
     - Setup: 20 rebuilds of one `Grid` over positions that move by a seeded random walk.
       - A step of about half a cell for most nodes.
       - A few nodes jump across the frame.
       - A crowd of 100 nodes in one cell, so that one bucket goes over 32 entries.
       - A NaN and an infinity.
     - Check: after each rebuild, `order`, `start`, `slot` and `at` (`to_bits`) equal those of a
       `Grid::new` + `build` over the same positions, for workers 1, 3 and 8.
   - **Negative control.** Skip the per-bucket sort; the new test must fail. Run it, revert it and
     report the failing test's name.
5. **Gates.** Report the exit code of each.
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0
   - `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8`
     → 1
   - `xtree.sh` on the branch, then `diff xtree-base.out xtree-branch.out` → 0
   - this tree's threads wasm (`scripts/orch/wasm-threads.sh`), then
     `harness/wasm-threads.mjs hash --serial ~/goinfre/bench/pm-stencil/serial.wasm` → rc 0, every
     row `equal`; with `--break` → rc 1.
6. **Bench.** Run only when `free -g` shows ≥ 12 GB available and the 1-minute load is < 14; wait
   otherwise.
   - Use step 2's command, without the patch.
   - Arms: base (develop at the branch point, in a second worktree) and branch, alternated, 3 rounds
     each. Print the load per run.
   - **Keep rule:** keep the code only if the serial row's median drops by ≥ 2 ms and the tick
     median does not rise. Otherwise revert the code, keep the new test and the report, and say
     "not kept".
7. **Report.** Write `docs/measurements/perf-pm-coherent-sort.md`, shaped like
   `perf-pm-stencil.md`, with:
   - step 2's breakdown;
   - the design;
   - the memory;
   - the gate table;
   - the per-pass table (base vs branch, 8 workers, medians);
   - a `Caveat:` on host load and on the per-bucket sort's worst case;
   - "what it does not do".

Paths you may edit:

- `crates/graph-core/src/layout/force/particle_mesh/collide.rs`, `collide/tests.rs` and new files
  under `collide/`;
- `docs/measurements/perf-pm-coherent-sort.md`;
- `~/goinfre/bench/pm-coherent-sort/`.

Do not touch:

- `collide/gather.rs`: perf-pm-collide-soa owns it;
- `exec/`, `barnes_hut/`, `graph-wasm`, `graph-cli`, `packages/` or `app/`.

Step 2's patch never leaves the worktree uncommitted at the end.

Done when:

- either step 2's stop rule held and the report says so with the breakdown;
- or every step-5 gate has its expected exit code, the negative control's failing test is named, the
  bench has three rounds per arm, and the report says kept or not kept against the step-6 rule.

Return block:

```
status: done | partial | blocked
breakdown: fill+count+prefix <ms>, scatter <ms>, Rows::sort <ms>, gravity <ms> (median per tick, 8w, 1M)
kept: yes | no | not attempted (serial base → branch ms, tick base → branch ms, medians)
memory: +<MB> at 1M
negctl: <failing test>
changed: <files>
commands: <each gate> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
