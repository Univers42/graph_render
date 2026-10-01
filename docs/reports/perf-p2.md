# Perf P2 report — the exact Barnes-Hut tick, one thread

Shape: `prompt.md` §12. Plan: `prompts/perf-plan.md` P2, with the targets restated in
`docs/measurements/perf-p1-baseline.md`. Branch `perf-p2`, worktree `$GM_SCRATCH/wt/perf-p2`,
base develop `94f7bc4`. The numbers are in `docs/measurements/perf-p2.md`; this report covers
what changed and what was checked.

## 0. What this phase was

Cut the work in d3's Barnes-Hut tick (charge, link, collide) on one thread without moving a
byte, in the lever order the baseline measured. The 1M tick went from 6338 ms to 1581 ms and
the 100k tick from 296 ms to 112 ms, with every golden and hash unchanged. One restated target
is missed: LL read misses at 1M are 81.3 M over 5 ticks against ≤ 40 M (392.4 M at P1).

## 1. Authorization compliance

| Plan item (P2 Changes, as restated) | File | Status |
|---|---|---|
| (1) queries in spatial order | `barnes_hut/step.rs` (`Pass`, `CollidePass`, the shared `merge`), `charge.rs`, `collide.rs`, `link.rs` | done |
| (2) stackless walk over a preorder arena | `quadtree.rs`, `quadtree/preorder.rs` (new), `charge.rs`, `collide.rs` | done |
| (3) leaf coordinates in the arena | – | **rejected** after measurement (deviation 2) |
| (4) `sqrt` hoist | `collide.rs`, `charge.rs` (`Body::open`) | done |
| (5) zero allocations per warm tick | `exec/partition.rs` (`ranges`), `sim.rs`, `step.rs`; `tests/tick_alloc.rs` asserts 0; rows `alloc-per-tick`, `negctl-tick-alloc` in `scripts/orch/rows/perf-p2.rows` | done |
| (6) bulk tree build | – | **skipped** (deviation 3) |
| `layout.force.particle_mesh` (new id) | – | moved to its own sub-slice `perf-p2-pm` (deviation 1) |
| plan items 4 and 5 (FA2-BH, Yifan Hu, Pivot MDS, Sugiyama, analysis scratch; ingest pre-sizing) | – | not started in this slice (deviation 4) |

Deviations, each named:

1. **The particle-mesh force is its own sub-slice.** It is a new layout id with its own
   metadata, oracle and quality gate, so it lands as `perf-p2-pm` → `perf-p2`, not mixed with
   byte-identical kernel work.
2. **Lever 3 rejected.** Storing each leaf's coordinates in the arena measured 7 % slower in an
   interleaved A/B at 100k once lever 1 had made the point gather sequential. Not committed.
3. **Lever 6 skipped.** The charge walk's jiggle hashes `tree.key(k)`, the pointer node's
   insertion-order id; a top-down build cannot reproduce it, so it would move bytes. By the
   plan's own rule that needs a new id, and the particle-mesh id supersedes the tree there.
4. **Plan items 4 and 5 are not in this slice.** The restated P2 targets are all about the
   Barnes-Hut tick; the O(n²) layouts and the ingest copies are separate slices, still open.

## 2. The ledger diff

None. No layout, analysis or post entry was added or changed.

## 3. The gate table

Run by `scripts/orch/gate.sh` over the 11 rows (the merge floor, wasm32, `hashgate-8` and the
P2 rows of `scripts/orch/rows/perf-p2.rows`), on `7a11a55` with `CARGO_BUILD_JOBS=3
RUST_TEST_THREADS=3`. A negative-control row's command ends `; test $? -eq 1`, so its
expected exit is 0. Log: `$GM_SCRATCH/logs/gate-perf-p2/summary.txt` (host-local).

| Row | Expect | Exit | Time | Verdict |
|---|---:|---:|---:|---|
| `fmt` | 0 | 0 | 1s | PASS |
| `clippy` | 0 | 0 | 2s | PASS |
| `test` | 0 | 0 | 998s | PASS |
| `wasm32-core` | 0 | 0 | 4s | PASS |
| `hashgate-8` | 0 | 0 | 5s | PASS |
| `negctl-degree` | 0 | 0 | 4s | PASS |
| `negctl-dim-z-mismatch` | 0 | 0 | 1s | PASS |
| `force-gate-4` | 0 | 0 | 0s | PASS |
| `negctl-force-gravity` | 0 | 0 | 1s | PASS |
| `alloc-per-tick` | 0 | 0 | 2s | PASS |
| `negctl-tick-alloc` | 0 | 0 | 0s | PASS |

## 4. The 4-way hash table

Every change is meant to be byte-identical, so the existing records are the check:
`hashgate-8` (native ×2, wasm32 ×2, per seed and per stage), its negative control, and the 65
frozen Barnes-Hut goldens (`session/tests/m1a.rs`) inside `cargo test --workspace`. All are in
§3. The 1000-seed hash gate runs once on develop after the merge (merge floor, `CLAUDE.md`).

## 5. Coverage

| Changed symbol | Exercised by |
|---|---|
| `Quadtree::flatten`, `open`, `Cell`, `order`, `key` | `quadtree/tests.rs`: `every_subtree_is_one_run_of_cells_and_one_run_of_points`, `a_cell_s_bounds_are_its_parent_s_quadrant`, `a_nan_point_is_ignored_and_never_reached_by_visit`, `exactly_coincident_points_chain_on_one_leaf_others_split_apart` |
| `charge::aggregate`, `Body`, `node_delta` | `charge/tests.rs`; `the_kernel_and_the_serial_loop_are_the_same_computation`; the 65 goldens |
| `collide::node_delta`, the hoisted `sqrt` | `tests/kernels.rs`; the goldens with collide on |
| `step::merge` in tree order | `the_whole_layout_is_worker_count_invariant_and_serial_one_is_the_stage`; the goldens; `hashgate-8` |
| `exec::partition::ranges` | `partition` is `ranges(..).collect()`, so the partition contract tests (`partition/tests/contract.rs`) cover it |
| zero allocations | `tests/tick_alloc.rs` and its negative control |

## 6. Caveat / Ponytail markers added

None. Every change is exact (byte-identical), and a marker on exact code is the misuse
`caveat.md` names. The measurement page carries its own Caveat line (cache model, shared host).

## 7. What could not be verified

| Item | State |
|---|---|
| `perf` hardware counters | SKIP, as in P1 (`perf_event_paranoid` 4, rootless Docker) |
| Hardware cache behaviour | callgrind's model only; the misses compare builds |
| LL read misses at 1M ≤ 40 M | **missed**: 81.3 M (P1 392.4 M). Of the 81.3 M, the two tree builds take 27.6 M, the rest of both `prepare`s (aggregate, flatten) 13.1 M, the two walks 18.5 M and the link pass 10.2 M, the rest of the tick 11.9 M (`docs/measurements/perf-p2.md`) |
| Peak heap | **a regression**: 102.0 → 122.6 MB at 100k, now during the session (`docs/measurements/perf-p2.md`, Heap). Recorded, not fixed |
| wasm32 speed | not measured in this slice; the hash gate proves its bytes, not its time. P3 measures the browser |

## 8. Stop-and-ask items

None. The rejected and skipped levers are measured decisions inside the envelope, and the
particle-mesh force follows the plan's new-id rule.
