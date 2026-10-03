# Job perf-bh-build (agent build): build the Barnes-Hut quadtrees in spatial order

Why: at 1M nodes on 8 workers, 499 of the 829 ms Barnes-Hut tick run on one thread, outside every
pass (`docs/measurements/perf-bh-1m.md`, "Where the tick goes"). Almost all of it is the two
quadtree builds (charge on `x, y`, collide on `px, py`).

- Each build inserts the points in index order into a pointer tree. Index order is random in space,
  so every level of every descent misses cache.
- The tree does not depend on the insertion order (`docs/decisions/bh-jiggle-key.md`). Inserting in
  Morton order makes each descent share its path with the previous one, and the arena comes out in
  near-preorder for the flatten.

This slice does that. It does not parallelise the insertion or the flatten: measure first, and leave
that to a later slice if the serial share is still large.

## Base

Start only when `origin/develop` contains both `origin/perf-bh-1m` and `origin/fix-qt-bail`
(`git merge-base --is-ancestor origin/<b> origin/develop` → 0 for each). Otherwise return
`status: blocked`.

## Facts (on develop 32d15ae3, plus the two branches above)

- **The build.** `Quadtree::build` (`crates/graph-core/src/layout/force/quadtree.rs:116-140`) runs:
  1. `reset`;
  2. `bounds_of` (the finite extent, `:46-53`);
  3. `cover` twice (`:157-195`): the root square, integer corners, a power-of-two side;
  4. `builder.add(i)` for `i` in `0..n` (`:135-138`);
  5. `flatten` (`quadtree/preorder.rs:41-60`).
- **Insertion.** `Builder::add` (`quadtree/build.rs:26-55`) descends with `Bounds::narrow`
  (`quadtree/bounds.rs:34-50`: midpoint `(x0 + x1) / 2`, slot `bottom << 1 | right`).
  - `insert_leaf` chains an exact coincidence, else splits until the two points separate.
  - It bails to a chain when the `x` span stops shrinking; `fix-qt-bail` re-hangs the leaf on a
    late bail.
  - A chain is most recent first.
- **The flatten.** `flatten` lays the pointer tree out in preorder, children in slot order 0..3. Each
  cell's bounds come from `Bounds::quadrant` (`bounds.rs:62-86`), which computes the same floats as
  `narrow`. The points no leaf holds (non-finite) are appended ascending.
- **The slot key.** `node_id(k)` (`quadtree.rs:81-88`, column `:64`, pushed at `preorder.rs:67`) is
  the arena slot, which is the serial push order.
  - Its only reader is the jiggle key in `approx` (`barnes_hut/charge.rs:229-230` on `perf-bh-1m`).
  - The doc comment at `charge.rs:62` describes it.
  - Tests that read it: `quadtree/tests.rs:131` and `:189-222`.
- **Callers of `build`.**
  - `barnes_hut/charge.rs:67` (`prepare_with`, which has a runner);
  - `barnes_hut/collide.rs:59` (`prepare`);
  - `forceatlas2/state/barnes_hut.rs:95`.

  ForceAtlas2 never reads `node_id`; its bytes must not move.
- **A kernel to copy.** `particle_mesh/collide/hash.rs:36-56` (`Buckets`) is a `StepRange` with
  `Out = u32`, one value per point. The `Runner` contract is in `crates/graph-core/src/exec/
  partition.rs:41-90`: a kernel writes only `out[range]`, and any division gives the same bytes.
- **No per-tick allocation.** `Quadtree::capacity` (`quadtree.rs:208-218`) and its test pin it. Any
  new buffer is a field that is cleared and refilled, and is counted in `capacity`.
- **Limits.**
  - At most 40 lines per function, 4 parameters, 300 lines per file (new code goes in
    `quadtree/morton.rs`), nesting depth 3.
  - No `unsafe`, no new dependency, no clock in committed graph-core code.
  - graph-core builds for `wasm32-unknown-unknown`.

## Do, in order

1. **The key change, its own commit** (`docs/decisions/bh-jiggle-key.md`).
   - Pin first. Write `~/goinfre/bench/bh-build/xtree.sh`, modelled on
     `~/goinfre/bench/pm-stencil/xtree.sh` but with `--layout layout.force.barnes_hut`:
     - check the exact `--layout` spelling with `graph-cli snapshot --help`;
     - sizes `333 1000 4096 12345 20000 50000 100000`, seed 0.

     Run it on the untouched worktree and save the output as `xtree-base.out`.
   - Change `approx`'s key to `k`.
     - Delete `node_id`: the column, its push, its `reset` and `capacity` lines, and the accessor.
     - Rewrite the doc comment at `charge.rs:62`.
     - Replace the test at `quadtree/tests.rs:189` with one that pins the new key: two builds of
       the same points in different index orders, with coincident points included, give equal
       cells. Change `:131` to compare cells instead of `node_id`.
   - Run `xtree.sh` again and save `xtree-key.out`. `diff xtree-base.out xtree-key.out` may differ:
     report how many sizes moved. Commit.
2. **Morton keys.** In the new `quadtree/morton.rs`, add a `StepRange` kernel with `Out = u32`. For
   each point:
   - if it is non-finite, the key is `u32::MAX`;
   - otherwise it is 16 `Bounds::narrow` steps from `root_bounds`, `key = key << 2 | slot`.

   Run it on the runner.
   - Guard: use the Morton order only when
     `max(|x0|, |x1|, |y0|, |y1|) < span × 2^36` on the root square, so every midpoint down to
     level 17 is exact.
   - Otherwise insert in index order, as today, and say so in a `Caveat:`.
3. **Sort.** Sort an insertion permutation by `(key, index)`. Use an LSD radix sort, 4 passes of
   8 bits:
   - it starts from `0..n` and is stable, so equal keys stay in index order;
   - its buffers are `keys`, `perm`, `perm_spare` and a `[u32; 256]` count, all fields reused
     across builds.

   Why ties in index order matter: two points in the same leaf are coincident, or inseparable
   below level 17, so their keys are equal and they are inserted in index order. That gives the
   same chain order as today. Points in different leaves may go in any order.
4. **Insert in that order.**
   - `build` keeps its signature for ForceAtlas2 and the tests: it is `build_with(&Serial, 1, …)`.
   - `build_with<R: Runner>(runner, workers, xs, ys)` is new, and charge's `prepare_with` calls it.
   - Collide's `prepare` has no runner: thread one through from its caller only if the change is
     under 15 lines. Otherwise leave collide on `Serial` and report it.
5. **Tests** (`quadtree/tests.rs`, or `quadtree/morton/tests.rs` if the file would pass 300 lines):
   - **`the_morton_build_is_the_index_order_build`.** Cases:
     - 2 000 seeded random points;
     - a crowd of 100 coincident points;
     - the inseparable pair `(1e6, 0.0)`, `(1e6, 1e-300)`;
     - a NaN and an infinity;
     - `fixtures/adversarial-ids.json`'s positions, if it has any;
     - one case that fails the guard.

     For each case, `cells` (bounds by `to_bits`, `skip`, `start`, `end`) and `order` equal those
     of an index-order build. Write the index-order build as a test-only helper; it is today's loop.
   - **`a_rebuild_allocates_nothing`** still passes, with the new fields counted in `capacity`.
   - **Negative control.** Make the radix sort unstable: reverse the order in which equal keys are
     scattered. The new test must fail on the crowd case. Run it, revert it, and report the
     failing test.
6. **Gates.** Report each exit code.
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0
   - `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8`
     → non-zero
   - `xtree.sh` on the final tree, then `diff xtree-key.out xtree-final.out` → 0
   - `~/goinfre/bench/bh-1m/session.sh`, copied with this worktree's path: `session` rc 0 with
     0 differing cells, and `--break` rc 1.
7. **Bench.** Run only when `free -g` shows at least 12 GB available and the 1-minute load is
   under 14; otherwise wait.
   - Arms:
     - base: develop at the branch point, built in a second worktree;
     - step 1's commit;
     - the final tree.
   - Run `tick --layout barnes-hut --n 1000000 --warm 5 --ticks 5` at `--workers 1` and `8`, and
     at `--n 100000 --workers 8`. Use the binary-per-arm pattern of `~/goinfre/bench/bh-1m/run.sh`.
   - Alternate the arms, 3 rounds, and print the load per run.
   - Run `--passes` once per arm at 1M, 8 workers.
   - **Keep rule:** keep steps 2–4 only if the 1M 8-worker tick median drops by at least 5 % and
     neither the 1-worker nor the 100k median rises. Otherwise revert steps 2–4, keep step 1 and
     the tests, and say "not kept".
8. **Report.** Write `docs/measurements/perf-bh-build.md`, shaped like `perf-bh-1m.md`, with:
   - step 1's cross-tree result;
   - the design and the guard;
   - the memory added at 1M;
   - the gate table;
   - the bench table (medians, loads);
   - the pass tables;
   - a `Caveat:` on host load, and one on the guard's fallback;
   - "what it does not do": no parallel insertion and no parallel flatten. Name the serial ms
     still left.

## Paths you may edit

- `crates/graph-core/src/layout/force/quadtree.rs` and everything under `quadtree/`;
- `barnes_hut/charge.rs`, `barnes_hut/charge/tests.rs` (the key), and `barnes_hut/collide.rs`
  (step 4 only);
- `docs/measurements/perf-bh-build.md`;
- `~/goinfre/bench/bh-build/`.

## Do not touch

- `forceatlas2/`, `particle_mesh/`, `exec/`, `graph-wasm`, `graph-cli`, `packages/` or `app/`;
- `docs/decisions/` (the ADR is written);
- any frozen default, `theta`, or the order of any floating-point sum.

## Done when

Every step-6 gate has its expected exit code. Step 1's cross-tree count is reported. The negative
control's failing test is named. The bench has three rounds per arm. The report says kept or not
kept against the step-7 rule.

## Return block

```
status: done | partial | blocked
key change: <n> of 7 sizes moved (xtree-base vs xtree-key)
kept: yes | no (1M 8w base → key → final ms; 1M 1w …; 100k 8w …; medians)
serial left: <ms> of <ms> at 1M 8w (--passes)
memory: +<MB> at 1M
negctl: <failing test>
changed: <files>
commands: <each gate> -> <rc>
deviations: <none | list>
decisions needed: <none | list>
```
