# Phase 9 progress — the two arms, the fixtures, the SCALE stage

**Status:** partial, and closer to end-to-end than the previous slice. Branch `p9` on
develop (p4 merged, so nothing is blocked on the ABI any more). The headline number now
exists and it is the phase's own: **crossover 10 000 native / 4 000 wasm32 / 2 000 for the
TypeScript oracle** against the 16.67 ms frame, in `BENCHMARKS.md`.

**Machine class:** one shared container host, with at least two other builds running for
most of the session. Seed 0, reference degree 8, medians over 3 runs. Not portable, and
**not stable under load**: the same native tick at N = 10 000 measured 4.598 ms and
22.064 ms in one session. Every report says so.

## What landed (TDD, RED before GREEN)

| Step | RED | GREEN |
|---|---|---|
| `harness/oracle-tick-bench.mjs` (the TypeScript arm) | 2 tests panicked on a missing module | `cargo test -p graph-cli bench::` 24 passed, including the harness's own `--self-check` and its negative control |
| `harness/wasm-tick-bench.mjs` (the wasm32 arm, via the JS SDK) | 2 tests panicked on a missing module | as above |
| `bench/campaign/{arms,report}.rs` — the three-arm crossover | 3 tests would not compile (`ArmReading`, `arms_markdown`, `read_arm_json` absent) | 24 passed; the sample's derived tick and settle pinned |
| `bench/scale.rs` — the fixture generator | 3 tests would not compile (`scale` module absent) | 24 passed; the 220-node fixture pinned equal to the synthetic model |
| `scale/{lod,simplify,adaptive,simple}.rs` in graph-core | 6 failures across the four modules (wrong tier ladder, no never-empty rule, path-order chain edges, a louvain partition, a stale-representative bug) | 21 passed in `cargo test -p graph-core scale::` |
| `capabilities.rs` — the three `scale` rows and `--ceilings-measured` | 3 failures (rows missing, ledger count 28, ceilings table mis-parsed) | 162 passed in the graph-cli bin |

**Negative controls that must fail, and do:** `a_broken_copy_of_the_oracle_tick_harness_fails_its_own_self_check`
and its wasm twin each copy the harness, change `SETTLE_TICKS`, and assert the copy's
`--self-check` exits non-zero — a self-check that cannot fail is not a check. The Phase 9
gate row `cargo test -p graph-core simplify_reversible` is
`simplify_reversible_restores_the_original_nodes_edges_and_representatives` plus
`simplify_reversible_holds_on_a_mixed_graph`.

## The three arms

| arm | harness | what it drives | crossover @ 16.67 ms |
|---|---|---|---:|
| native | `graph-cli bench --crossover` | `layout.force.barnes_hut` in this process | 10 000 |
| wasm32 | `harness/wasm-tick-bench.mjs` | the same layout through `crates/graph-sdk-js` over `graph_wasm.wasm` | 4 000 |
| TypeScript oracle | `harness/oracle-tick-bench.mjs` | `src/core/layout/forceLayout.ts` `tick()`, d3-force 3.0.0, no browser | 2 000 |

Both JS arms drive **the same graph**: the oracle arm uses `src/core/model/synthetic.ts`'s
`buildSyntheticModel(n)`, which graph-core's `synthetic.rs` is a call-for-call port of,
and both arms' `--self-check` pins the six-node model (6 nodes, 9 edges) that
`crates/graph-cli/src/bench/tests.rs` pins on the native side. The wasm arm writes the same
model as the provisional ingest document `graph-wasm`'s reader accepts.

**Every arm's tick is `run / 112` except the oracle's**, which times 112 real `tick()`
calls. The reason is stated in all three reports: the motor settles a layout in one call and
the ABI has no per-tick entry point, so a single tick is not observable from outside. That
is a derived number and it is labelled as one everywhere it appears.

## What the measurements found

- **The 33 B/node table in `prompt.md` §5.1 is wrong by 3.7×** — measured 123.6 B/node of
  columns plus 42.9 B/node of arena at N = 10 000. Reported as a correction with both
  numbers in `docs/measurements/phase09-ceilings.md`, and **not** applied to the ledger:
  per the phase's stop-and-ask, a declared ceiling that turns out to be dramatically wrong is
  a decision for a human.
- **WASM loses to native at every N measured** (2.1× at 220, 7.8× at 10 000). Reported as a
  row in every table, not buried.
- **Rust is not dramatically faster than the oracle at N = 220** (0.145 ms vs 0.510 ms) —
  3.5× on a graph that is 3.5× inside the budget either way. The win appears with scale,
  which is what the crossover is for.
- The force layout's declared ceiling of 200 000 is a *usability* ceiling, not a
  frame-budget ceiling; the two are different numbers and the ledger's one field conflates
  them.

## What remains

1. **The native campaign at N = 100 000 and N = 10⁶.** `--n` now reaches 10⁶ (past the
   model's 100 000-node cap a fixture is whole prefixed components of it, so a 10⁶ graph is
   *easier* than one preferential-attachment graph of that size — stated wherever it is
   used), but the run was killed at 100 000 after ~20 minutes on this host: the 100 000-node
   canonical-JSON snapshot alone dominates. The phase gate row
   `bench --n 220,10000,100000,1000000 --repeat 5` therefore **does not pass in this
   session** and `docs/measurements/phase09-bench.md` covers 220 … 10 000 only.
2. **`fixtures/scale/{n10k,n100k,n1m}.json` are not committed** — 2.7 MB, 27 MB and 270 MB.
   The generator is committed and `fixtures/scale/README.md` quotes it; `n220.json` is
   committed as the one sample small enough to read. The phase's own §1 calls a 100 MB
   literal absurd, so this is the intended shape, but it is a decision worth a human's eye.
3. **The per-kernel Amdahl split and the `f32x4` autovectorisation check (§6b) are not
   measured.** They need per-kernel timers inside `barnes_hut/sim.rs` (Phase 6 code, outside
   this envelope) and a wasm opcode inspection. `BENCHMARKS.md`'s Phase 11 section says so
   rather than guessing; Phase 11 must not assume a split it has not been given.
4. **The three `scale` rows are `implemented`, not `gated`.** Nothing hashes them: the hash
   gate's stage list (`crates/graph-cli/src/hashgate/stages.rs`) and `graph-wasm`'s exports
   are outside this phase's envelope. Promoting them, and exposing LOD hints and
   simplification journals through the snapshot, the ABI and the JS SDK, is merge-step work.
5. **`scale.lod` does not use Phase 8's `grid_index`** — that module is not on this branch
   (p8 is a separate worktree). The alternative, a second spatial structure inside `lod`, is
   what the phase prompt forbids, so `lod` builds none and tests every node against the
   viewport rectangle in `O(n)`. Recorded as a deviation, to be revisited when p8 merges.
6. **`capabilities --check` is red in this worktree** for a pre-existing reason: there are no
   `target/gates/*.json` records, so every `gated` row reports "no hashgate record". The
   orchestrator's gate run produces them. `--ceilings-measured` on its own passes and prints
   `ceilings measured: 4 of 31 rows; 27 still reasoned`.

## Deviations from the phase's envelope (all deliberate, all recorded)

| what | why |
|---|---|
| `crates/graph-cli/src/bench/{campaign/{arms,report}.rs,scale.rs}` instead of one `bench.rs` | the 300-line house cap; `campaign.rs` was already 338 lines before this slice and is now three files |
| `harness/wasm-tick-bench.mjs` (not named in the envelope) | the wasm32 arm has to run in a JS engine; graph-cli has no wasm runtime and adding one is a manifest change, which the envelope does not allow. The task named `crates/graph-sdk-js / harness` as the route, and this is it |
| `node_modules/{d3-force,d3-quadtree,d3-dispatch,d3-timer}` (gitignored, untracked) | this worktree has no `node_modules`, so both JS arms and the two pre-existing `oracle_layouts_*` tests cannot resolve d3. Copied from the read-only pinned tree at the exact versions `package-lock.json` resolves; `d3-dispatch` and `d3-timer` are not in `/goinfre/dlesieur/refs/npm`, which holds only `d3-force` and `d3-quadtree` |
| `fixtures/scale/README.md` | the generator needs its constants and its exact command written down next to the one committed sample |
