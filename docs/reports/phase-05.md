# Phase 5 report — Sugiyama layered DAG

Shape: `prompt.md` §12. **No command in this report was re-run against the phase's own
branch.** `prompts/RESUME.md` line 31 records the phase as *"merged via train (inside
p6f). No mutants run, no `phase-05.md` report yet"*. Every gate row below is read from
**one** develop-wide gate run, `/goinfre/dlesieur/wt/p8/target/gate-develop/summary.txt`,
on **tree `b980ae8`** (2026-09-29), which contains this phase's code as merged. A row that
ran on that tree is evidence for that tree, not for `p5` as it stood. Anything not run is
`NOT RUN`, and a skipped check is not a pass.

**Provenance of the merge.** `git log develop` puts the phase's work on `develop` at
**`45a653f`** (2026-09-29 03:39 +0200), a merge of the `develop` line (`14c64ad`, Phase 3)
with **`67543b4`**, the tip of the "train" branch that carried p5, p6e and p6f together
(`RESUME.md` lines 31–32). The Sugiyama core itself is commit **`9023543`** (2026-09-28
02:35, second-parent side, parent `b7a068a` = the p3 substrate STATUS names), and its
fixtures, ADR and measurement file are its `CREATE` list verbatim. `45a653f`'s diff
(`git show --stat 45a653f`, 118 files) contains the whole of
`crates/graph-core/src/layout/sugiyama/{acyclic,layering,ordering,coords,routing}.rs`,
`fixtures/dag/*.json`, `docs/decisions/sugiyama-heuristics.md` and
`docs/measurements/phase05-crossings.md`.

## 0. What this phase was

Four algorithms in a pipeline, and the third one is a heuristic. That makes the phase
structurally different from Phase 3: its output is **not** uniquely determined by its
input, it **invents nodes** (dummy vertices for edges spanning more than one layer, which
are what turn an edge into a multi-point route), and it has a **real scale ceiling with a
defined degradation**. It is also the phase that discharges the Graphviz `dot` gap named in
`prompts/REFERENCES.md` — the port is native, cited to
`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:638-691`.

## 1. Authorization compliance

**On the envelope (CREATE):**
`crates/graph-core/src/layout/sugiyama/{mod,acyclic,layering,ordering,coords,routing}.rs`
(six files, as named); `fixtures/dag/{chain,diamond,cyclic,multi-span,wide-layer,
disconnected}.json` (six, as named); `docs/decisions/sugiyama-heuristics.md`;
`docs/measurements/phase05-crossings.md`.

**On the envelope (MODIFY):** `crates/graph-core/src/layout/mod.rs`,
`crates/graph-core/src/registry.rs`, `crates/graph-cli/src/capabilities.rs`,
`harness/oracle-layouts.mjs` (the dagre arm), `package.json` (dagre-d3-es promoted to a
direct devDependency — confirmed: `package.json` + a 470-line `package-lock.json` entry in
`9023543`).

**Deviations**, all visible in `git show --numstat 9023543` and `45a653f`:

| path | why |
|---|---|
| `crates/graph-core/src/layout/sugiyama/{tests.rs,routing/tests.rs}` | House limit (≤300 lines/file). `sugiyama/mod.rs` measured 291 and `ordering.rs` 298 in `9023543`; the invariants and the routing CSR-boundary tests did not fit under either. |
| `crates/graph-contract/src/{notes.rs,notes/tests.rs,notes/tests/json.rs}` | The phase's Ledger delta requires the degradation to be *in the snapshot*, not in a log, and `docs/decisions/snapshot-notes.md` reserves codes 4 (`dag.dummy_budget_exceeded`) and 5 (`dag.edge_reversed`) for exactly this phase. Turning a reserved code on is a contract minor bump and the enum has to change. `notes.rs`'s own doc says codes 4–6 are refused as `Reserved` by a 0.3 reader until a phase activates them; `NoteCode::from_code` now maps `4` and `5` and still refuses `6`. |
| `crates/graph-cli/src/snapshot_cmd/dag.rs` | The `dump_crossing_measurements` arm the measurement file's own Reproduction section names (`gr cargo test -p graph-core dump_crossing_measurements -- --ignored`, and the develop gate's `dag-dump` row) writes `target/dag-crossings.json`. That needed a `graph-cli` writer, and `graph-cli/src/{main.rs,capabilities.rs}` is all the envelope's MODIFY list names. |
| `crates/graph-cli/src/{hashgate.rs,hashgate/**,capabilities.rs,capabilities/**}`, `crates/graph-cli/tests/{cli.rs,cli_oracles.rs,snapshot.rs}` | The phase's Ledger delta is one row, `layout.dag.sugiyama`, and it needs a hash-gate stage and the row/problem counts asserted in the capability tests move with it. Follows from the delta, not chosen. |
| `crates/graph-core/src/registry.rs` and its new `registry/{hierarchy,spectral,force}.rs` submodules | The 300-line limit on `registry.rs`, which by this commit held the whole layout table; the split is the merge's, and the rows are unchanged in substance. |
| `harness/oracle-dag.mjs`, `package-lock.json` | The dagre arm ended up in its own file (`harness/oracle-layouts.mjs` is 202 lines after p3 and the two arms share no code); the lock is `npm ci`'s requirement for the pinned 7.0.14. |
| `scratch/{crossing-comparison.json,crossing-verdict.json,dag-crossings.json,measure-crossings.mjs}` | The measurement evidence named in `docs/measurements/phase05-crossings.md` "Raw evidence". Kept, not deleted. |
| `docs/reports/phase-05.md` | This report. |

Nothing under `src/`, `tests/` (the TypeScript oracle) or `verify/`, nothing in osionos,
no force/iterative layout, and no `graph-core` dependency added — the allow-list was
closed and this phase did not open it.

## 2. Ledger diff

| id | before | after | on develop now |
|---|---|---|---|
| `layout.dag.sugiyama` | — | **`gated`**, `Point` + `Polyline`, ceiling **200 000** | `gated` |

Row count: **13 → 14** across `45a653f` (which also brings p6e's `layout.spectral` and
`layout.mds.pivot` and p6f's `layout.force.barnes_hut` / `layout.forceatlas2`, so the
same commit adds five rows in total); 36 on develop today. `layout.none` was never a row
— it is only the negative id in
`registry::tests::a_registered_layout_emits_the_kinds_it_declares_at_its_default_parameters`.

Every field the phase's Ledger delta requires is present in the row's own text, read from
`capabilities --json`:

- `geometry`: `Point` + `Polyline` — **as required**.
- `complexity`: *"O(n+m) per phase; crossing reduction is a heuristic (median + transpose
  local search), not a minimiser"* — the required "say so, do not imply optimality".
- `scale_ceiling`: `200000`, the dummy budget.
- `degradation`: *"past the dummy budget (200000) long arcs are left straight and unrouted
  and each is reported as note 4 dag.dummy_budget_exceeded; above 150000 layered vertices
  the transpose rounds drop to 0, so crossings rise while the drawing stays valid"* — both
  halves of the required degradation contract, in the output rather than in a log.

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
| `roundtrip --seeds 1000` | `roundtrip-1000` | 0 | **PASS** (2266 s) |
| `cargo test -p graph-core sugiyama_invariants` | `sugiyama-invariants` | 0 | **PASS** |
| `node harness/oracle-layouts.mjs --dag` | `oracle-layouts-dag` | 0 | **PASS** |
| `capabilities --check` | `capabilities-check` | **1** | **FAIL** (§below) |
| `node harness/sdk-smoke.mjs` | `sdk-smoke` | **2** | **FAIL** (row defect, §below) |
| `docker build -t ge-check . && docker run --rm ge-check` | `ge-check` | 0 | **PASS** |
| — (needed by the `--dag` arm; the measurement file's step 1) | `dag-dump` | 0 | PASS |
| — (not in the phase prompt) | `negctl-sugiyama-layer-spacing` | 1 | PASS, expected non-zero |
| — (not in the phase prompt) | `negctl-grid-spacing`, `negctl-node-count`, `hashgate-negctl` | 1 | PASS, expected non-zero |
| — (not in the phase prompt) | `geometry-invariants`, `oracle-layouts`, `emit-fixtures-1000`, `npm-ci` | 0 | PASS |

**`capabilities --check` FAIL.** On the develop tree: `29 rows, 8 problems`, all eight
`topology.*` rows (Phase 1's) reading `gated, but no oracle-diff record: run the gate`.
**`layout.dag.sugiyama` contributes none of them** — its `oracle` names the dagre
crossing differential and its `oracle_diff` field names the `roundtrip` record, and the
`roundtrip-1000` row is green on the same tree. Re-run in this worktree the same command
exits **1** with `36 rows, 34 problems`, every one a `gated` row with no record in a tree
that has not run the gate. Recorded as FAIL with the scope named, per UNKNOWN = FAIL.

**`sdk-smoke` FAIL, and it is a row defect rather than a phase failure.** Its log reads,
verbatim: `sdk-smoke: could not run: usage: sdk-smoke.mjs <wasm>` — exit 2. The develop
rows file invokes it as `node-slim.sh node harness/sdk-smoke.mjs` with **no argument**, so
the harness refuses to start. The row has never passed on the develop rows file; this is
the same defect as `zero-copy`, whose log reads
`wasm-run: usage: wasm-run.mjs <wasm> hash <seeds> <stage>... | probe | --assert-zero-copy`.
Both are open repair items, not evidence about Sugiyama.

**NOT RUN, for this phase, on any tree:** `cargo mutants`. `RESUME.md` line 31 says so
plainly for this phase, and `AGENT_BRIEF.md` forbids an agent running it. **This is the
one phase of the five whose mutation result is not merely deferred but explicitly recorded
as never run.**

## 4. 4-way hash table

Stage list on tree `b980ae8` (from the `negctl-force-theta` log, the row that prints its
header): `topology, layout.grid, layout.tree.tidy, layout.treemap.squarified,
layout.circular.radial, layout.packing.circle, layout.spectral, layout.mds.pivot,
layout.force.barnes_hut, layout.forceatlas2, layout.dag.sugiyama,
transport.wasm.columnar`.

| stage | what hashes it | result at 8 seeds | control that must fail |
|---|---|---|---|
| `layout.dag.sugiyama` | the pipeline's own snapshot: Point nodes, Polyline edges through each dummy chain, notes 4/5 where they apply | **4-way equal, 8/8** — native run 1, native run 2, wasm32 run 1, wasm32 run 2 all on digest `e7436052c8d17113aab985feeaa92518198269a95787ada8060c2a9d16ddc4a0` | `GM_MUTATE_SUGIYAMA_LAYER_SPACING=2` → exit 1, the row PASSes as expected non-zero |

**This is the only layout in the project with a knob of its own that goes red on its own
stage**, and that is worth stating plainly: the develop gate's four negative controls are
`REFERENCE_DEGREE`, `GRID_SPACING`, `NODE_COUNT`, `SUGIYAMA_LAYER_SPACING`, and only the
last is a per-capability control. The three Phase-3 layouts and the two p6 layouts have
none — see `phase-03.md` §4 and `phase-06.md` §4 for the same finding on the other rows.

`hashgate-1000` exits 0 over the same 12 stages; as in `phase-03.md` §4 the retained log
does not print per-stage counts for the 1000-seed run, so the per-stage 1000-seed claim is
**UNKNOWN** and is not asserted. What is asserted is the row's exit code and the 8-seed
four-way digest above.

## 5. Coverage table

| changed symbol | the test that exercises it |
|---|---|
| cycle breaking: greedy FAS, a total order over dense indices (D5) | `sugiyama::tests::{greedy_fas_order, sugiyama_invariants, run_is_deterministic}`; the invariants assert "acyclic after FAS" over the fixture set and the synthetic sweep |
| reversed edges recorded, then un-reversed in the routes (an edge left reversed is a wrong diagram that looks right) | `sugiyama::routing::tests::a_reversed_max_span_edge_still_runs_source_to_target`; `sugiyama::tests::run_builds_point_nodes_and_polyline_edges_with_the_expected_notes` asserts the `dag.edge_reversed` (code 5) notes reach the snapshot |
| longest-path layering, dummy vertices, `DUMMY_BUDGET` surfaced in the output | `sugiyama::tests::{longest_path_layers, reduce_slack, materialize, budget_plan, sugiyama_invariants}`; the degradation is asserted as note 4, and `sugiyama::routing::tests.rs:162` names the note-4 edge as the one exempt from the spacing invariant |
| crossing reduction: median + transpose, deterministic tie-breaks | `sugiyama::tests::{order_layers, sort_by_median, median_position, init_order, transpose, pass, heap_key, total_crossings, bilayer_crossings, pair_crossings, run_is_deterministic}`; the measurement file's own numbers (§8) are the quality claim |
| X assignment (priority method) and `LAYER_SPACING` | `sugiyama::tests::{priority_of, priority_move, slide}`, and `the_stage_is_registered_under_its_id_and_scales_y_by_its_spacing` |
| routing: dummy chain → `Polyline` in layer order; a one-layer edge gets a straight line (empty interior) | `sugiyama::routing::tests::{a_direct_edge_first_and_last_has_a_zero_width_offset_pair, a_max_span_edge_owns_one_point_per_dummy_source_to_target, a_reversed_max_span_edge_still_runs_source_to_target}` — first edge, last edge, zero interior points, maximum span: the four boundaries the phase prompt names |
| disconnected-component placement reuses Phase 3's `hierarchy` spacing, not a second convention | `sugiyama::tests::sugiyama_invariants` over `fixtures/dag/disconnected.json`, and the `unique_neighbours` / `reindex` passes it shares with the layer builder |
| the six named fixtures, and the synthetic sweep the differential runs on | `sugiyama::tests::{load_dag, synthetic_dag, sugiyama_invariants}`; `harness/oracle-layouts.mjs --dag` and `graph-cli test dump_crossing_measurements -- --ignored` read the same fixtures |
| the registry row's own contract | `the_stage_is_registered_under_its_id_and_scales_y_by_its_spacing`, plus graph-core's `registry::tests::every_layout_is_a_layout_stage_with_its_metadata_filled` |

## 6. Ponytail markers added

Three, on the three heuristic stages, each exactly as the phase names them —
`docs/decisions/sugiyama-heuristics.md` collects them, and each is in the module's own doc:

- **Crossing reduction is a heuristic, not a minimiser** (`ordering.rs`). Failing input: a
  graph whose optimal ordering the median/transpose local search cannot reach. Direction:
  more crossings than optimal — cosmetic, never incorrect.
- **The dummy budget degrades to unrouted arcs** (`layering.rs`). Failing input: a graph
  with very long spans. Direction: routes become straight lines that may pass through
  nodes — **visually wrong, the dangerous direction**. Escape hatch: note 4
  `dag.dummy_budget_exceeded` in the snapshot.
- **FAS is greedy, not minimum** (`acyclic.rs`). Direction: more edges reversed than
  necessary — cosmetic, because each reversal notes `dag.edge_reversed` and the route is
  drawn head to tail rather than dropped.
- **No marker** on `layering`'s longest-path/slack-reduction or `coords`' priority method,
  which are exact. The bilayer cross counter is exact too and carries none.

## 7. What could not be verified

- **`cargo mutants` over this diff. NOT RUN, ever.** `RESUME.md` line 31 states no mutants
  were run for p5, and an agent must not run them (`AGENT_BRIEF.md`). This is the weakest
  mutation evidence of the five phases reported here and is recorded as such rather than
  folded into a general "not run".
- **This phase's own gate, on its own branch. UNKNOWN.** `p5` is not in this worktree; the
  develop gate on `b980ae8` is evidence for the merged tree.
- **Per-stage counts at 1000 seeds. UNKNOWN** (§4).
- **`sdk-smoke.mjs`, on any tree. NOT RUN** in substance: the row exits 2 on its usage
  line, so the phase prompt's own `sdk-smoke` acceptance item has produced no evidence at
  all. It is not this phase's fault — the row is missing its `<wasm>` argument — but the
  item is FAIL, not PASS.
- **`capabilities --check` green. Not observed on any tree** (§3).
- **A tuning-free result.** The phase prompt says a tuned-to-pass heuristic is not
  verified. `docs/measurements/phase05-crossings.md` states the margin was frozen before
  measuring, and the file's own text says so; but that file was written 2026-09-28 and I
  did not re-run its two reproduction commands in this session (the phase's branch is not
  present and the commands are not in the develop rows file beyond `dag-dump` and
  `oracle-layouts-dag`). **The 5242-vs-7657 figures are quoted from the measurement file,
  not re-measured here**, and per rule 0.5 are labelled as such wherever they appear.
- **The real crossing count against dagre on a *larger* DAG than 236 graphs.** The
  measurement's own synthetic sweep is 230 seeds of 6–26 nodes. The margin is frozen and
  met; nothing here says the heuristic holds at 10⁴ nodes, and the transpose throttle at
  150 000 layered nodes is a declared constant that has never been exercised.

## 8. Stop-and-ask items

- *"Our crossing count is materially worse than `dagre-d3-es` on a fixture → stop and
  report the number. Do not tune constants until the gate passes."* **Did not happen.**
  `docs/measurements/phase05-crossings.md` records the frozen margin first, then the
  numbers: on the six named fixtures, ours equals or beats dagre everywhere (`wide-layer`
  = K₄,₄ at 36 on both sides, which is the forced optimal value and is what caught the
  measurement's own first, wrong technique — reconstructing crossings from dagre's spline
  control points, which reported 4). Over 236 graphs, sum(ours) 5242 against
  sum(dagre) 7657, against a 1.10× limit of 8422.7; strictly better on 133, equal on 89,
  worse on 14 (worst +13 on `synthetic-227`). Neither margin clause is crossed and no
  constant in `ordering.rs` was tuned. **Caveat: quoted, not re-measured here (§7).**
- *"A tie-break has no obviously deterministic resolution → stop."* **Did not happen.**
  `docs/decisions/sugiyama-heuristics.md` states dense-index tie-breaks are used at every
  point a tie can occur — FAS candidate order, layer compression, ordering initialisation,
  median ties, priority-move ties — and `run_is_deterministic` plus the whole invariants
  suite are the check.
- *"Disconnected-component placement conflicts with Phase 3's convention → stop."* **Did
  not happen:** the phase reuses `hierarchy.rs`'s root/forest handling rather than
  introducing a second convention, which is what the prompt requires.
- **Recorded, not resolved:** a broken measurement technique was replaced **before** its
  output was trusted (the geometry-reconstruction crossings count). The measurement file
  says so itself and calls it a deviation. It is a good precedent and is recorded here
  because the alternative — quietly re-running until the number looked right — is the
  failure this project cannot absorb.
