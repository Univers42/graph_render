# Phase 7 report — the ANALYSIS stage

Shape: `prompt.md` §12. **No command in this report was re-run against the phase's own
branch.** `prompts/RESUME.md` line 34 records p7 as *"merged via train (conflicts in
capability tests resolved: 26 rows, 8 `analysis.*` rows `Implemented`)"*. Every gate row
below is read from **one** develop-wide gate run,
`/goinfre/dlesieur/wt/p8/target/gate-develop/summary.txt`, on **tree `b980ae8`**
(2026-09-29). A row that ran on that tree is evidence for that tree, not for `p7` as it
stood. Anything not run is `NOT RUN`; UNKNOWN = FAIL.

**Provenance of the merge.** `git log develop` puts the phase on `develop` at **`ffb837a`**
(2026-09-29 03:39 +0200), a merge of `45a653f` (p3 + p5 + p6) with **`ab396f5`**, the `p7`
tip. The phase's own work is the 26-file diff `git show --stat ffb837a`: 2 516 insertions,
including `crates/graph-core/src/analysis/{mod,components,paths,centrality,communities,
depth}.rs`, `analysis/{centrality/tests,depth/tests}.rs`, `csr_petgraph.rs` +
`csr_petgraph/tests.rs`, the five `fixtures/analysis/*.json`,
`docs/decisions/petgraph-determinism-audit.md`, `docs/measurements/phase07-analysis.md` —
and, notably, `prompts/phase-07-analysis.md` itself (4 lines).

## 0. What this phase was

Mostly wiring, by the phase's own account: `petgraph` already implements Dijkstra,
Bellman-Ford, connected components and topological sort, so the phase implements
petgraph's graph traits over the project's existing CSR and reuses its algorithms rather
than rewriting textbook code. The part that is not wiring is the **determinism audit**,
which the phase text makes a precondition of `gated`, and the **defect it refuses to
inherit**: the reference offers Bellman-Ford in its pathfinding UI and the operator
silently dispatches Dijkstra instead, so negative-weight shortest paths do not work and
nothing says so.

## 1. Authorization compliance

**On the envelope (CREATE):**
`crates/graph-core/src/analysis/{mod,components,paths,centrality,communities,depth}.rs`;
`crates/graph-core/src/csr_petgraph.rs` (trait impls only); `fixtures/analysis/{weighted,
negative-weight,disconnected,star,two-cliques}.json`;
`docs/decisions/petgraph-determinism-audit.md` (required); `docs/measurements/
phase07-analysis.md`.

**On the envelope (MODIFY):** `crates/graph-core/src/lib.rs`,
`crates/graph-core/src/registry.rs`, `crates/graph-core/Cargo.toml` (`petgraph = "=0.8.3"`,
`default-features = false`, `features = ["std"]` — pre-authorised and pinned exactly, more
than the house's 7-day release-age hold), `crates/graph-cli/src/{capabilities.rs,main.rs}`.

**Deviations** — from `git show --numstat ffb837a`:

| path | why |
|---|---|
| `crates/graph-core/src/csr_petgraph/tests.rs`, `analysis/centrality/tests.rs`, `analysis/depth/tests.rs` | House limit (≤300 lines/file). `csr_petgraph.rs` is 252 and `centrality.rs` 231; the tests did not fit. The house pattern (`index/tests.rs`) is the same. `docs/measurements/phase07-analysis.md` §6 records the first two as deviations and §7 the third (a review response). |
| `crates/graph-cli/src/capabilities/analysis.rs` (new, 153 lines) | The 300-line limit on `capabilities.rs`, which by this commit held the whole registry. The file's own doc carries the reason. |
| `crates/graph-cli/src/capabilities.rs`, `capabilities/{tests/mod,tests/registry}.rs`, `crates/graph-cli/tests/cli.rs` | **Recorded, with the phase's own reasoning, in `phase07-analysis.md` §6**: these hardcoded the pre-Phase-7 row/problem counts (9, 18) and an unconditional "every row is gated" invariant. Adding a non-`gated` row makes those literally false, and shipping a red `cargo test --workspace` is a worse stop condition than a mechanical edit to three existing assertions. |
| `prompts/phase-07-analysis.md` (4 lines) | **A prompt file, edited by the phase — an envelope breach by the strictest reading, and recorded as one in `phase07-analysis.md` §4.** The `only-petgraph-added` row's regex admitted `libm|indexmap|petgraph` and therefore failed on `graph-contract`, `graph-core`'s own workspace path dependency present since Phase 0. One token added. The row is not weakened: the negative control still bites (a non-allow-listed line leaves residual and the row fails). |
| `Cargo.lock` (35 lines) | `petgraph` is the only addition. |

Nothing under `src/`, `tests/` (the TypeScript oracle) or `verify/`, no layout, no edge
routing, nothing in osionos, and **no dependency beyond `petgraph`** — the allow-list was
opened exactly once, as the phase authorised.

## 2. Ledger diff

| id | before (`45a653f`) | after (`ffb837a`) | on develop now |
|---|---|---|---|
| `analysis.components` | — | **`implemented`** | `implemented` |
| `analysis.paths.dijkstra` | — | **`implemented`** | `implemented` |
| `analysis.paths.bellman_ford` | — | **`implemented`** | `implemented` |
| `analysis.centrality.degree` | — | **`implemented`** | `implemented` |
| `analysis.centrality.closeness` | — | **`implemented`** | `implemented` |
| `analysis.centrality.betweenness` | — | **`implemented`**, ceiling **20 000** | `implemented` |
| `analysis.centrality.eigenvector` | — | **`implemented`** | `implemented` |
| `analysis.communities.louvain` | — | **`implemented`** | `implemented` |
| `analysis.depth` | — | **not registered at all** | **still not registered** |

Row count **18 → 26** across `ffb837a`; 36 on develop today. `RESUME.md` line 34 records
the same 26 rows and 8 `analysis.*` rows `Implemented`.

**Every row is `implemented`, not `gated`, and the phase says why in the source.**
`crates/graph-cli/src/capabilities/analysis.rs`'s own doc: *"None of these are wired into
the 4-way hashgate or the TS oracle differential yet — `hashgate.rs` and `graph-wasm` are
outside this phase's authorization envelope… Every row below is therefore
`Status::Implemented`, honestly not `gated`: `problems()` only demands hash/oracle evidence
from a `gated` row."* The phase prompt's Ledger delta says `gated`; the phase declines,
because `prompt.md` §8 makes a `gated` claim without evidence a false claim. This is the
correct reading and is recorded here as a **deviation from the prompt's Ledger delta, with
the reason**, not as a pass.

**`analysis.depth` is a row that does not exist, and the reason is a merge dependency.**
`crates/graph-core/src/analysis/mod.rs`'s doc: *"`depth` is the odd one out: p3's
`layout/hierarchy.rs` is not on this branch's base, so depth reads the root/forest
convention through its own `depth::Roots` trait rather than re-deriving it — re-deriving
would be the second convention step 6 forbids. `impl depth::Roots for Hierarchy {}` is the
whole of the re-point at merge time… `analysis.depth` stays absent from the capability
ledger until then."* **The re-point has not happened on develop**: the row is still missing
from the 36. `docs/measurements/phase07-analysis.md` §4 says it "is now delivered" as code
(13 tests, later 18) but "stays out of the capability ledger until the merge supplies the
`Topology` entry point" — and p3 *is* on develop now, so the code, the trait impl and the
row are three steps and only the first was taken.

**The two required field declarations are present.** `analysis.centrality.betweenness`'s
`complexity` states the deviation from the phase's own complexity note verbatim: *"O(n * m
log n) (this weighted, Dijkstra-based Brandes; costlier than the O(n*m) unweighted/BFS form
the phase names, a recorded deviation for internal consistency with paths.rs's weighted
Dijkstra/Bellman-Ford)"*, and its `degradation` states that **no sampled variant ships
under this name**, so nothing degrades silently past the ceiling. `analysis.
centrality.degree`'s `oracle` reads *"topology's own degree column (Phase 1), reused
verbatim, not recomputed"* — step 4's "reuse, do not recompute" held.

## 3. Gate table

The phase prompt's Gate section, answered by the develop gate. Tree `b980ae8`.

| phase prompt row | develop row | exit | verdict |
|---|---|---:|---|
| `cargo fmt --check` | `fmt` | 0 | **PASS** |
| `cargo clippy --workspace -- -D warnings` | `clippy` | 0 | **PASS** |
| `cargo test --workspace` | `test` | 0 | **PASS** (106 s) |
| `cargo build -p graph-core --target wasm32-unknown-unknown` | `wasm32` | 0 | **PASS** |
| `hashgate --seeds 1000` | `hashgate-1000` | 0 | **PASS** (3052 s) |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | `negctl-reference-degree` | 1 | **PASS** (expected non-zero) |
| `cargo test -p graph-core bellman_ford_negative_weight` | `bellman-ford-negative-weight` | 0 | **PASS** — the defect the phase exists to fix |
| `cargo test -p graph-core negative_cycle_detected` | `negative-cycle-detected` | 0 | **PASS** |
| `cargo test -p graph-core analysis_determinism` | `analysis-determinism` | 0 | **PASS** |
| `cargo tree -p graph-core --depth 1 \| …` (allow-list) | `only-petgraph-added` | **1** | **FAIL — a prompt-row defect, §below** |
| `capabilities --check` | `capabilities-check` | **1** | **FAIL** (§below) |
| `node harness/sdk-smoke.mjs` | `sdk-smoke` | **2** | **FAIL — row defect, §below** |
| `docker build -t ge-check . && docker run --rm ge-check` | `ge-check` | 0 | **PASS** |
| — (not in the phase prompt) | `negctl-grid-spacing`, `negctl-node-count`, `negctl-sugiyama-layer-spacing`, `hashgate-negctl` | 1 | PASS, expected non-zero |
| — | `roundtrip-1000`, `emit-fixtures-1000`, `oracle-layouts`, `geometry-invariants`, `sugiyama-invariants`, `npm-ci`, `stress-d3`, `bench`, `wasm32`/`wasm-release`/`no-bindgen` | 0 | PASS |

**`only-petgraph-added` FAIL is a row defect, and the log proves it is not a regression.**
Its whole log is one line: `├── graph-contract v0.1.0 (/w/crates/graph-contract)`. The row's
regex in the develop rows file is `grep -vE "libm|indexmap|petgraph|graph-contract"` — which
admits the workspace path dependency — yet `graph-contract` still appears in the residual,
so the rows file as it stands cannot have produced this log; the gate ran the pre-fix
regex, and the fix has not been re-run. `docs/measurements/phase07-analysis.md` §4 records
the same failure on the **pristine base commit `900cf13`**, before this phase's changes,
which is the decisive evidence: it is a prompt-row bug, not an added dependency. The row
is FAIL with the diagnosis attached; the negative control (a non-allow-listed line leaves
residual) is intact.

**`capabilities --check` FAIL.** On the develop tree: `29 rows, 8 problems`, all eight
`topology.*` rows reading `gated, but no oracle-diff record: run the gate`. **None of the
eight `analysis.*` rows contributes a problem**, by construction — `problems()` only checks
evidence for a `gated` row, and all eight are `implemented`. Re-run in this worktree: exit
**1**, `36 rows, 34 problems`, every one a `gated` row with no record in a tree that has
not run the gate.

**`sdk-smoke` FAIL, and for this phase it is the p4 dependency, not a row defect alone.**
The log is `sdk-smoke: could not run: usage: sdk-smoke.mjs <wasm>`, exit 2 — the develop row
passes no argument. `docs/reports/STATUS.md` §3 p7 predicted this exact row would "pass once
p4 is in", and p4 *is* on develop (`507f1c2`) with `harness/sdk-smoke.mjs` in the tree — yet
the row still exits 2 on its usage line. So the **row is still broken after the p4 merge**,
and the phase prompt's own `sdk-smoke` acceptance item has produced no evidence. This is the
"p4–p7 SDK row" item `RESUME.md` line 44 lists as remaining work, and it is unresolved.

**NOT RUN, for this phase, on any tree:** `cargo mutants` (orchestrator's row).
**`RESUME.md` item 6 notes p7's own two measurement tests were temporary and removed** —
`modularity_on_synthetic_models_measurement` and `betweenness_scaling_measurement` were
`#[ignore]`d, run for the measurement file, and deleted. Their numbers are therefore
**quoted, not re-runnable** (§7).

## 4. 4-way hash table

**This phase has no hash-gate stage, and that is the honest state, not an omission.**

| stage | what hashes it | result | control |
|---|---|---|---|
| — | nothing: `analysis.*` is wired into no stage | **NOT RUN for this phase** | — |

The develop gate's stage list is the same twelve as in `phase-03.md` §4 and
`phase-06.md` §4 — `topology, layout.grid, layout.tree.tidy, layout.treemap.squarified,
layout.circular.radial, layout.packing.circle, layout.spectral, layout.mds.pivot,
layout.force.barnes_hut, layout.forceatlas2, layout.dag.sugiyama,
transport.wasm.columnar` — and **none of them is an `analysis.*` stage**. So:

- The determinism claim for this phase rests on `analysis-determinism` (exit 0,
  `analysis::mod::analysis_determinism`: same input, same output, bit for bit) plus a
  `repeated_runs_agree_bit_for_bit` test in **each** of the five analysis modules
  (`components.rs`, `paths.rs`, `centrality/tests.rs`, `communities.rs`,
  `depth/tests.rs`).
- That is **on-target** determinism. The cross-target (native vs wasm32) property this
  project actually gates on is **untested for every analysis function**, because no
  analysis output is hashed. This is UNKNOWN and is stated as such.

The phase prompt's own gate row for this is `hashgate --seeds 1000` and
`negctl-reference-degree`, both of which are green on the develop tree — but they say
nothing about analysis, and this report does not present them as if they did.

## 5. Coverage table

| changed symbol | the test that exercises it |
|---|---|
| `csr_petgraph.rs`: the trait impls over the project's CSR, no data copied into a petgraph container | `csr_petgraph::tests::*`; the "if you find yourself building a `petgraph::Graph`, stop" rule is structural — `CsrDigraph` is a newtype over the existing CSR |
| neighbour iteration preserves the project's CSR order (Phase 1 matched the oracle's Map order deliberately) | `csr_petgraph::tests::*`; the whole `analysis_determinism` row |
| `components`: ids in ascending dense-index-of-first-member order, not discovery order | `components::tests::{weak_components_ignore_direction_and_number_by_first_member, an_isolated_node_is_its_own_component_of_both_kinds, strong_components_split_a_directed_edge_the_reverse_cannot_cross, repeated_runs_agree_bit_for_bit}` |
| `paths`: Dijkstra with equal distances broken by dense index (D5) | `paths::tests::{negative_weight_defect, non_negative_weights, an_unreachable_node_is_infinite_in_both_algorithms, repeated_runs_agree_bit_for_bit}` |
| `paths`: **the defect the phase exists to fix** | `paths::tests::bellman_ford_negative_weight_gives_a_different_correct_answer_from_dijkstra` (the develop row `bellman-ford-negative-weight`, **exit 0**) and `negative_cycle_detected_names_the_cycle_not_a_bogus_distance` (`negative-cycle-detected`, **exit 0**) |
| `centrality`: degree reuses Phase 1's column | `centrality::tests::degree_is_the_topology_column_not_a_recomputation` |
| closeness, and its negative-weight precondition | `centrality::tests::{the_center_of_a_star_is_closer_to_everyone_than_any_leaf, closeness_panics_in_debug_on_a_negative_weight_graph, closeness_of_diverges_between_dijkstras_wrong_distance_and_bellman_fords_correct_one}` — the last feeds Dijkstra's wrong distance and Bellman-Ford's correct one to the guard-free core and shows they differ |
| Brandes betweenness, and the O(nm)-at-scale ceiling | `centrality::tests::{on_a_star_only_the_center_has_positive_betweenness, betweenness_panics_in_debug_on_a_negative_weight_graph, repeated_runs_agree_bit_for_bit}`; the ceiling 20 000 comes from the (now removed) measurement test, `phase07-analysis.md` §2 |
| eigenvector centrality: fixed start vector, sign pinning, non-convergence reported | `centrality::tests::{eigenvector_converges_and_favours_the_higher_degree_node, eigenvector_on_a_bipartite_star_reports_non_convergence_not_a_plausible_lie}` |
| Louvain: fixed visit order, strict-gain tie-break, no ambient RNG | `communities::tests::{two_cliques_stay_separate_communities_across_the_bridge, modularity_of_the_clean_partition_is_positive_and_higher_than_one_lump, an_edgeless_graph_gives_every_node_its_own_community_and_zero_modularity, move_node_via_bundled_state_merges_two_connected_singletons, repeated_runs_agree_bit_for_bit}` |
| the modularity formula, and the bug the phase's own TDD caught | `communities::tests::modularity_of_the_clean_partition_is_positive_and_higher_than_one_lump` — the one-community lump reads **exactly 0.0**, as Newman's formula requires; the first draft read ≈0.58 because it summed the null-model term only over edges (`phase07-analysis.md` §1) |
| `depth`: BFS depth through a four-method `Roots` trait, one convention | `depth::tests::{a_single_root_is_depth_zero_and_the_tree_counts_out_from_there, two_roots_hang_off_the_virtual_root_so_both_sit_at_depth_one, a_declared_root_is_depth_zero_whatever_the_forests_own_roots_are, a_node_no_declared_root_reaches_is_unreached_not_depth_zero, a_node_two_roots_reach_takes_the_shorter_of_the_two_depths, a_declared_root_that_descends_from_another_declared_root_stays_at_depth_zero, a_cycle_in_the_children_is_walked_once_and_the_first_shortest_depth_wins, the_levels_do_not_depend_on_the_order_the_children_are_listed_in, repeated_runs_agree_bit_for_bit}` + the five added later (`a_child_index_past_the_last_node_panics`, `two_roots_with_no_virtual_root_are_refused_in_debug`, `a_depth_lookup_past_the_last_node_panics`, `a_lone_root_sits_at_depth_zero_even_when_the_source_names_a_virtual_root`, `declaring_no_root_reaches_nothing_at_all`) |
| the five added `depth` tests actually bite | `phase07-analysis.md` §4a: each was shown to bite by perturbing the implementation once and restoring the file byte for byte. Dropping `&& roots.len() >= 2` → 1 failure (`a_lone_root_sits_at_depth_zero_…`; **no pre-existing test noticed that clause**); deleting the `assert!(child < n, …)` → 1 failure, and the walk then panicked with the standard library's index-out-of-bounds message, so the guard's own message is load-bearing |
| the house limit on `move_node`'s six parameters | `move_node_via_bundled_state_merges_two_connected_singletons` (RED first — a compile error, `LouvainState` not yet existing) |
| no `HashMap` order reaches any output | `docs/decisions/petgraph-determinism-audit.md`, one entry per reused algorithm, and `analysis_determinism` + the five `repeated_runs_agree_bit_for_bit` tests as the runtime check |

## 6. Ponytail markers added

Four, as the phase requires, read from the row text `capabilities --json` returns:

- **Louvain is a heuristic and order-dependent** (`analysis.communities.louvain`). Failing
  input: a graph with near-tied modularity gains. Direction: a valid but non-optimal
  partition — cosmetic. Escape hatch: the seed. **The phase's deviation sharpens this**:
  it ships *no RNG at all* — a fixed dense-index visit order plus a strict-gain tie-break
  gives the same reproducibility a seed would, with one fewer moving part — and it is
  scoped to the **local-moving phase only**, without multi-level aggregation, which is
  named in the row's own `oracle` text as a recorded scope deviation.
- **Betweenness's O(n·m log n) ceiling of 20 000** (`analysis.centrality.betweenness`).
  Measured 475 ms at 16 000 nodes / 24 793 edges and 3.1 s at 32 000 / 49 613. The row's
  `degradation` says nothing degrades silently past it, **because no sampled variant ships
  under this name** — the phase's own stop-and-ask 3 answered correctly. There is
  therefore **no sampled-betweenness Ponytail to write**, and the phase prompt's
  conditional marker ("if shipped") is honestly unmet.
- **Eigenvector centrality's power iteration** (`analysis.centrality.eigenvector`).
  May not converge on disconnected or bipartite-ish graphs; the row is pinned by
  `eigenvector_on_a_bipartite_star_reports_non_convergence_not_a_plausible_lie` — the
  function reports non-convergence rather than returning a plausible lie.
- **No marker** on Dijkstra, Bellman-Ford or components — they are exact, and none of the
  three rows carries a heuristic caveat. Confirmed by reading the rows: all three say "no
  TS oracle exists" / "petgraph, reused" and say nothing about quality.

## 7. What could not be verified

- **Cross-target determinism of every analysis function. UNKNOWN.** No analysis output is
  hashed by any stage (§4). The whole claim this project gates on is untested for this
  phase's eight functions; what is tested is same-target, same-process repeatability.
- **`analysis.depth` has no ledger row, and the re-point has not been done.** p3's
  `hierarchy.rs` *is* on develop, so the stated blocker is gone and the remaining work is
  `impl Roots for Hierarchy {}` plus the row. This is stale, not pending.
- **The analysis results are not attribute columns on the topology.** Step 7 asks for
  `community: Vec<u32>`, `betweenness: Vec<f32>` … "hashed as part of the analysis stage,
  and exposed through the SDK and the JSON snapshot". **None of the three exists**:
  `grep` for `community` / `betweenness` in `crates/graph-core/src/columns.rs` and
  `records.rs` returns nothing, no stage hashes them, and the SDK does not publish them.
  The step is unmet, and the phase's own reason — `hashgate.rs`, `graph-wasm` and the
  snapshot writer are outside the envelope — is the correct reason, but the step is still
  unmet. This is the largest open item in this report.
- **`cargo mutants` over the diff. NOT RUN** — the orchestrator's row.
- **The modularity and betweenness-ceiling numbers. QUOTED, not re-measured.** The two
  measurement tests were `#[ignore]`d, run once and deleted
  (`phase07-analysis.md` §2, §7 finding 6). Q = 0.5244 / 0.5502 / 0.5501 at n = 200 /
  1 000 / 5 000 and the 335 µs → 3.10 s betweenness table cannot be regenerated from the
  tree as it stands. A number nobody can re-run is a number nobody can check — the same
  objection `phase08-routing.md` §0 raises, and here it is worse because the code is gone
  rather than `#[ignore]`d.
- **`fixtures/analysis/*.json` are not parsed by any test.** Recorded, deliberately not
  fixed, in `phase07-analysis.md` §7 finding 3: `graph-core` has no JSON parser and its
  allow-list is closed, and moving the test to `graph-cli` is outside the envelope. They
  are hand-cross-checked reference fixtures. **The same gap is repeated in
  `fixtures/post/obstacles.json` in Phase 8** (`phase08-routing.md` deviation 7) and is
  not fixed there either.
- **A preserved RED transcript for the phase's own original TDD.** One commit for the whole
  phase; no failing-test output captured anywhere. The reviewer's own fixes each keep
  their observed RED, but the original modularity bug's RED is gone (`phase07-analysis.md`
  §7 finding 6). Recorded as UNKNOWN.
- **A green `sdk-smoke` row. NOT RUN in substance, and this is the p4–p7 dependency
  `RESUME.md` line 44 lists** (§3). The row is still broken *after* p4 landed.
- **This branch's own gate. UNKNOWN** — `p7` is not in this worktree.

## 8. Stop-and-ask items

- *"A petgraph algorithm's output depends on `HashMap` order and cannot be wrapped
  deterministically → stop. Do not ship it; write it ourselves or defer."* **Did not
  trigger.** Every reused algorithm is audited in
  `docs/decisions/petgraph-determinism-audit.md` (required by the phase), one entry each,
  and where order could leak the answer is collect-and-sort-by-dense-index. The review
  found the audit incomplete by exactly one entry (`centrality::eigenvector`, MINOR) and
  it was added — a documentation-completeness fix, with the finding that no functional
  defect existed.
- *"Adding a dependency beyond `petgraph` → stop."* **Did not happen.**
  `crates/graph-core/Cargo.toml` gained `petgraph = "=0.8.3"` and nothing else; the
  allow-list row that checks this is red for a *prompt-regex* reason (§3), not a
  dependency reason, and the log's single line is `graph-contract`, the workspace crate
  present since Phase 0.
- *"Betweenness is needed at a scale where exact is intractable → stop and ask whether
  sampled-under-a-separate-name is acceptable."* **Answered by not shipping it.** Exact
  Brandes only, a declared 20 000 ceiling, and the row says in its own text that **no
  sampled variant ships under this name** — the Bellman-Ford defect in a new costume,
  refused. The `scale_ceiling` is a usability judgement with its own Ponytail, not a
  hidden truncation.
- **New, and open:** step 7 is unmet (§7). Recommended: add an `analysis` hash-gate stage
  over a column set and expose the columns in the snapshot and SDK. Until then no
  `analysis.*` row can honestly be `gated`, and the eight rows' `implemented` status is
  correct rather than merely cautious.

---

## Addendum (2026-09-29)

The re-point described as pending in §7 ("`analysis.depth` has no ledger row, and the re-point has not been done") has landed. Commits 3486ca5 and 746f3a8 added `impl Roots for Hierarchy {}` in `crates/graph-core/src/analysis/depth.rs` and the corresponding ledger row for `analysis.depth.bfs`. The `analysis.depth` module now reads p3's repaired `Hierarchy` directly as a `Roots` — there is one convention across the codebase, not two. The stale "pending" language in this report, in `crates/graph-core/src/analysis/mod.rs`, `crates/graph-core/src/analysis/depth.rs`, `crates/graph-core/src/analysis/depth/tests.rs`, and `docs/contract/wasm-abi.md` has been corrected in those files.
