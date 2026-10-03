# Job perf-bh-1m (agent build: Barnes-Hut at 1M on threads, the same bytes)

Goal: Obsidian's algorithm is d3-force with the Barnes-Hut many-body force
(`docs/decisions/obsidian-force.md`). Make a Barnes-Hut tick at 1M as fast as the CPU allows,
byte-identical to the serial tick, and measure where it lands against the live budget (native
≤ 25 ms, browser ≤ 100 ms).

Facts (verified on develop b2cbbcd9):
- A serial Barnes-Hut tick at 1M is 1 635 ms native, and charge plus collide take 95.5 % of its
  instructions (`docs/measurements/perf-p2-pm.md:7,30`).
- The threaded Barnes-Hut tick is already byte-identical to the serial one at workers
  {1, 2, 3, 4, 7}, natively and in wasm (`docs/measurements/perf-p3-session-threads.md`, the gate
  table: `session::tests::threaded` and `harness/wasm-threads.mjs session`).
- `graph-cli tick` hard-codes one worker for Barnes-Hut (`crates/graph-cli/src/bench/tick.rs:5,
  127-141`: `workers: 1`), so nobody has measured a threaded Barnes-Hut tick at 1M.
- The charge walk is a parallel gather (`barnes_hut/charge.rs:39-51`). Its prologue `prepare`
  (`charge.rs:60-68`) is serial:
  - `charge_tree.build`: d3's `add.js` insertion, one point at a time (`quadtree/build.rs`);
  - the preorder flatten (`quadtree/preorder.rs`);
  - `aggregate` (`charge.rs:88-112`): reverse preorder, each cell reading its children's bodies.
- In the arena a cell's subtree is the contiguous run `k..skip` and its points are
  `order[start..end]` (`preorder.rs:13-25`). Disjoint subtrees are disjoint ranges of `bodies`.
- The 65 golden digests and the m1a–m1e tests pin a session's bytes
  (`crates/graph-core/src/layout/force/session/tests/`).
- `scripts/orch/profile.sh [n] [ticks] [warm]` profiles `graph-cli tick` with callgrind. Its
  header says a 1M run takes tens of minutes.

Do, in order:
1. Let `graph-cli tick` pass `--workers` to Barnes-Hut. Delete the hard-coded 1 and update the
   header comment at `tick.rs:5-8`.
2. Measure Barnes-Hut at 1M, workers {1, 4, 8, 16}, `--warm 5 --ticks 5`, release build. Run 3
   rounds, alternating the worker counts within a round. Report medians only, with the load.
3. Profile one threaded 1M tick at workers 8: callgrind on `--n 1000000 --ticks 1 --warm 1`, or at
   200 000 if 1M overruns an hour. State which. Report each pass's share: tree build, flatten,
   aggregate, walk, link, collide, integrate.
4. Only if step 3 shows the serial prologue above 15 % of the threaded tick: make it parallel with
   identical bytes.
   - Build: give each point a key from the **same** `f64` midpoint descent `add.js` performs
     (`Bounds::quadrant`), 32 levels deep. Never derive keys by integer scaling of coordinates: it
     rounds differently from the midpoints. Sort by `(key, index)` on the runner. Then cut the
     arena from the runs of equal key prefixes.
     - Points whose keys tie at 32 levels go through the existing serial insertion for that cell
       only. Write a `Caveat:` line naming that case.
     - The result must be the arena the serial build makes, cell for cell and point for point.
   - Aggregate: cut the arena at a depth whose cells number at least 4 × workers. Aggregate each
     subtree in parallel; each writes only its own `bodies[k..skip]`. Then finish the cells above
     the cut serially, in reverse preorder as now. The per-cell arithmetic and its order must not
     change.
   - Keep the serial path as the reference. Add a test that builds both arenas for the 8 gate
     seeds and for `fixtures/adversarial-ids.json`, plus a case with coincident points, and
     asserts equal `cells`, `order` and `bodies`.
5. Measure collide's share at 1M from step 3, and the tick with `collideRadius` 0. Report it. Do
   not change any default: an `obsidian` preset is a separate decision (`obsidian-force.md`,
   Consequences).
6. Repeat step 2 after step 4, if step 4 ran, and add the browser number: the threads artifact
   (`scripts/orch/wasm-threads.sh`) under `harness/wasm-threads.mjs`, at 1M and workers 8.

Out of bounds: the frozen defaults, the opening criterion, any change of floating-point operation
order inside a cell, the wasm ABI, `packages/`. No `unsafe`, no new dependency, no `HashMap` (D4).
A concurrency change in `exec/` beyond calling the existing `Runner` is a stop: report it, because it
needs a devil verdict. House limits: 40 lines a function, 4 parameters, 300 lines a file,
nesting ≤ 3.

Paths you may edit: `crates/graph-cli/src/bench/tick.rs`, `crates/graph-core/src/layout/force/quadtree/`
(new child modules allowed), `crates/graph-core/src/layout/force/barnes_hut/charge.rs` and a new
child module beside it, tests beside them, `docs/measurements/perf-bh-1m.md`.

Done when:
- `scripts/orch/gr cargo fmt --all --check` exits 0.
- `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- `scripts/orch/gr cargo test --workspace --no-fail-fast` exits 0. The golden digests and m1a–m1e
  are unchanged.
- `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` exits 0.
- `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exits 0, and the same run with
  `-e GM_MUTATE_REFERENCE_DEGREE=9` exits non-zero.
- If step 4 ran: `gr node harness/wasm-threads.mjs session` reports 0 differing, and the negative
  control for the new arena test passes. The control is a build that swaps two sibling cells:
  the equality test must fail. Paste its output.
- `docs/measurements/perf-bh-1m.md` holds:
  - the step-2 and step-6 tables, with load;
  - the step-3 shares;
  - the step-5 collide numbers;
  - the gap to 25 ms native and 100 ms browser, stated plainly;
  - what the change does not do (the walk's own cost is untouched; the GPU tier is
    `docs/decisions/gpu-force-tier.md`).
