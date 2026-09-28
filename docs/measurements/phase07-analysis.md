# Phase 7 — ANALYSIS stage: what was measured, and what was deviated from spec

All commands below ran in the `p7` worktree, in `ge-rust`, against the tree this phase's
commit holds. Every number here is copied from a real run's output — none is
hand-derived or assumed (`bellman_ford_negative_weight...` and
`negative_cycle_detected...` are also hand-traced ahead of time in `paths.rs`'s doc
comments, and the real run confirmed the hand trace, rather than replacing it).

## 1. Modularity achieved (the quality claim, `prompts/phase-07-analysis.md` step 5)

```sh
cargo test -p graph-core --release -- --ignored --nocapture modularity_on_synthetic_models_measurement
```

| n (requested) | nodes | edges | communities found | Q |
|---|---|---|---|---|
| 200 | 200 | 299 | 49 | 0.5244196565342616 |
| 1 000 | 1 000 | 1 541 | 215 | 0.5502431589108413 |
| 5 000 | 5 000 | 7 721 | 1 086 | 0.550143429473432 |

And on the textbook two-triangles-plus-bridge fixture
(`fixtures/analysis/two-cliques.json`, `communities::tests::
modularity_of_the_clean_partition_is_positive_and_higher_than_one_lump`, now pinned
exactly by that test): **Q = 0.3571428571428571** for the correct 2-community split,
**Q = 0.0** for the one-community lump — exactly 0.0, as Newman's formula requires for
any single community, which is what caught the modularity bug below.

**A real bug this phase's own TDD process caught and fixed:** the first `modularity`
draft summed the null-model term `k_i k_j / 2m` only over pairs that are *edges*,
instead of over every pair in the same community whether or not an edge joins them. That
silently overstated `Q` — for the one-lump partition specifically, it read
**Q ≈ 0.58** instead of the mathematically required `0.0`, because most cross-clique
pairs in a lump are not edges and their (negative) null-model contribution was dropped
entirely. `communities.rs`'s doc comment on `modularity` explains the corrected formula
and why the earlier one was wrong; the `one_lump` assertion is what would catch a
regression.

## 2. Betweenness's ceiling — measured, not assumed

`analysis::centrality::betweenness` is exact (Brandes 2001), so it never gets the wrong
answer — its ceiling is wall-clock, not correctness or memory. Measured on synthetic
models (`build_synthetic_model`, same shape at every n: ~1.5 edges/node), release
profile, in `ge-rust`:

```sh
cargo test -p graph-core --release -- --ignored --nocapture betweenness_scaling_measurement
```

| n | edges | wall-clock |
|---|---|---|
| 200 | 299 | 335 µs |
| 500 | 768 | 1.34 ms |
| 1 000 | 1 541 | 2.99 ms |
| 2 000 | 3 075 | 12.2 ms |
| 4 000 | 6 193 | 40.7 ms |
| 8 000 | 12 383 | 159.6 ms |
| 16 000 | 24 793 | 474.8 ms |
| 32 000 | 49 613 | 3.10 s |

Growth is consistent with the declared `O(n·m log n)` (this implementation is the
*weighted*, Dijkstra-based Brandes — see deviation 4 below — not the `O(n·m)` unweighted
BFS form the phase text names): roughly ×4 per doubling of n through 16 000, steepening
toward ×6.5 at 32 000 (heap and cache effects at that size, on the gate's hardware).
`crates/graph-cli/src/capabilities/analysis.rs` sets `analysis.centrality.betweenness`'s
`scale_ceiling` to **20 000** — a usability judgement (interactive vs. not), pinned with
its own Ponytail, not a hard memory or numeric limit; nothing degrades silently past it,
because no sampled variant ships under this name (deviation 4).

The two temporary `#[ignore]`d measurement tests above were run for this document and
then removed from the shipped source — they were never part of the gate, and keeping ad
hoc benchmarking code in the permanent test suite past its one use added nothing.

## 3. Scope deviations (reported per `phase-07-analysis.md`'s own "Reuse before
implement" and stop-and-ask sections, and the task's explicit scope adjustments)

1. **`depth.rs` is not implemented; `analysis.depth` is not registered.** It needs
   Phase 3's `hierarchy.rs` root/forest logic, which is not on this branch's base (`p7`
   forked before `p3` landed). A missing reference is a stop, not an improvisation.
   `crates/graph-core/src/analysis/mod.rs`'s doc comment records this.

2. **Results are not yet wired into the 4-way hashgate, the snapshot, the JSON ledger's
   evidence, or the SDK.** `hashgate.rs`, `graph-wasm/src/lib.rs` and `harness/
   wasm-run.mjs` are outside this phase's authorization envelope (not in its MODIFY
   list), so hashing the analysis results as their own stage and exposing them to a
   frontend is deferred to the merge step. Every `analysis.*` capability row is
   therefore `Status::Implemented`, honestly not `gated` — `capabilities.rs`'s
   `problems()` only demands hash/oracle evidence from a `gated` row, so this adds rows
   without adding problems. `prompts/phase-07-analysis.md`'s own "Ledger delta" section
   says `gated`; this phase cannot honestly claim that without the wiring above, so it
   does not.

3. **Louvain: no RNG at all**, not even a seeded one. The phase text asks for "a seeded
   RNG threaded explicitly from `rng.rs`", but the task's own instruction preferred no
   RNG if avoidable; a fixed dense-index visit order plus a strict-gain tie-break gives
   the same reproducibility a seed would, with one fewer moving part. If a future
   revision needs one, `synthetic::Mulberry32` is the house RNG, to be threaded
   explicitly, never ambient. Also scoped to **local-moving phase only** (Blondel et
   al. 2008, phase 1), without the multi-level aggregation phase — a single level
   already optimises modularity by the same gain rule; getting the aggregated level's
   self-loop weight accounting exactly right (`2×self_weight`, verified against
   networkx's own semantics) was not something this phase could fully hand-verify under
   time pressure, so it is left as a named gap rather than shipped possibly-subtly-wrong.

4. **Betweenness is weighted (Dijkstra-based Brandes), not the unweighted/BFS form the
   phase's `O(nm)` complexity note names.** Chosen for internal consistency with
   `paths.rs`'s weighted Dijkstra and Bellman-Ford — every function in this module reads
   the same `edge.strength` weight model. No sampled betweenness ships under any name
   (stop-and-ask 3: a separately named capability, only with a consented sampling
   error on disk, which does not exist here).

## 4. petgraph: version, allow-list, and a gate defect found (not introduced)

`petgraph = "=0.8.3"` (released 2025-09-30, `default-features = false, features =
["std"]`), pinned exactly, more than the house's 7-day release-age hold past. The full
determinism audit of every algorithm this phase reuses is
`docs/decisions/petgraph-determinism-audit.md` (required by the phase text).

**The dependency allow-list gate command, run literally, fails on the pristine base
tree — before this phase's changes — for a reason unrelated to Phase 7:**

```sh
cargo tree -p graph-core --depth 1 | tail -n +2 | grep -vE "libm|indexmap|petgraph" | grep . && exit 1 || exit 0
```

fails (exit 1) because its output includes `graph-contract` (`crates/graph-core/
Cargo.toml`'s own workspace path dependency, present since before this phase and not
matched by the regex's allow-list). Confirmed on the pristine base commit
(`900cf13`, checked out to a throwaway worktree and torn down after) — this is not a
regression caused by adding petgraph, it is the same command failing the same way with
or without this phase's changes. Reported honestly rather than silently patched (the
gate script itself is outside this phase's file envelope) or its failure hidden.

## 5. `capabilities --check` — real exit code, and why it is not 0

Required gate row `capabilities --check` (expect 0) exits **1** in this worktree, with
**18 problems across 9 rows**, exactly the same 9 pre-Phase-7 rows
(`topology.index`...`topology.ids`, `layout.grid`) and the same reason class
(missing `oracle-diff`/`roundtrip` evidence records, which only the orchestrator's
`ge-check.sh`/node harness pipeline produces — explicitly not run in this speculative
worktree, per this phase's own instruction not to run it) that fail identically on the
pristine base tree (verified: 18 problems there too, before any `hashgate` record even
exists in that worktree). **None of the 17-9=8 new `analysis.*` rows contribute a single
problem** — they are `Status::Implemented` (deviation 2 above), and `problems()` only
checks evidence for a `gated` row, so this is by construction, not luck. The row count
in the printed line reads `17 rows, 18 problems` — a phase-specific worked example is in
`crates/graph-cli/tests/cli.rs`'s
`capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs`.

## 6. Envelope note: three test files touched outside the phase's literal MODIFY list

`crates/graph-cli/src/capabilities/tests/{mod.rs,registry.rs}` and
`crates/graph-cli/tests/cli.rs` each hardcoded the pre-Phase-7 row/problem counts (`9`,
`18`, and an unconditional "every row is gated" invariant). Adding any non-`gated` row
to the registry — the only honest option available given deviation 2 — makes those
counts and that invariant literally false, not merely stale; leaving them unedited would
mean shipping a red `cargo test --workspace`, which `prompt.md` and the task both treat
as a stop condition worse than a small, mechanical, fully-explained edit to three
existing assertions. `crates/graph-core/src/csr_petgraph/tests.rs` is a fourth new file,
not on the phase's literal CREATE list, but is the pre-existing house pattern (see
`crates/graph-core/src/index/tests.rs`) for keeping `csr_petgraph.rs` — itself an
authorized CREATE — under the 300-line house limit; nothing in it changed the tests
themselves, only their file.
