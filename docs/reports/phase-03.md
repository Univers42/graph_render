# Phase 3 report — the four deterministic one-shot layouts

Shape: `prompt.md` §12. **No command in this report was re-run against the phase's own
branch**: the branch `p3` is not in this worktree and its own gate never finished
(`prompts/RESUME.md` line 30: *"merged via train. Its full gate (hashgate-1000, roundtrip,
oracle-layouts, mutants) never finished — it runs as part of the develop gate"*). Every
gate row below is therefore read from **one** develop-wide gate run,
`/goinfre/dlesieur/wt/p8/target/gate-develop/summary.txt`, on **tree `b980ae8`**
(2026-09-29), which contains this phase's code as merged. A row that ran on that tree is
evidence for that tree, not for `p3` as it stood. Rows that did not run at all are
`NOT RUN`, and per `AGENT_BRIEF.md` a skipped check is not a pass.

**Provenance of the merge.** `git log develop` puts the phase's work on `develop` at
**`14c64ad`** (2026-09-29 03:38 +0200), a merge of the pre-existing `develop` line
(`834f1a0`) with **`2538f0a`**, itself `Merge branches 'p3fix-a', 'p3fix-b', 'p3fix-c' and
'p3fix-d' into p3`. The 155 changed files are that merge's diff against `834f1a0`. The
`p3` branch commits themselves are on the second-parent side (`b7a068a` created
`layout/tidy_tree.rs`, `treemap.rs`, `circular.rs`, `circle_packing.rs`;
`a8ff18b` is `Merge branch 'p3' into p6f` and `9ed0bec` / `407237f` / `55b54f3` the
sub-branch merges). Ledger state is read from `gr cargo run -q -p graph-cli --
capabilities --json` on this tree, and from the same command's source at `834f1a0`
(before) and `14c64ad` (after).

## 0. What this phase was, and the two things it had to invent

Four layouts, all one-shot and deterministic, chosen because each one forces a different
piece of the contract to become real: the tidy tree exercises the **hierarchy CSR** built
in Phase 1 and unused until now, and emits **`Polyline`** edges; the squarified treemap
emits **`Box`**; the circular layout is BFS-depth ranking with a documented convention;
circle packing emits **`Circle`** — the geometry kind the reference's own format could not
hold. Two product decisions had to be made rather than discovered, and both are written
down as ADRs the phase was required to deliver: the **hierarchy repair** (what to do with
a cycle, a multi-parent node, a forest) and the **snapshot `notes` section** that makes
those repairs *visible in the output* rather than in a log.

## 1. Authorization compliance

**On the envelope (CREATE):** `crates/graph-core/src/layout/{tidy_tree,treemap,circular,
circle_packing}.rs` and `hierarchy.rs`; `harness/oracle-layouts.mjs`;
`fixtures/hierarchy/{tree-balanced,tree-degenerate,forest,cyclic}.json`;
`docs/decisions/planarity-fallback.md`.

**On the envelope (MODIFY):** `crates/graph-core/src/layout/mod.rs`,
`crates/graph-core/src/registry.rs`, `crates/graph-cli/src/{main.rs,capabilities.rs}`,
`package.json` (d3-hierarchy promoted to a direct devDependency — confirmed: the merge
touches `package.json` and adds `package-lock.json`).

**Deviations** — every path the merge touched outside those lists, with its cause. All are
visible in `git show --numstat 14c64ad`; none is under `src/`, `tests/` (the TypeScript
oracle) or `verify/`, and nothing is in osionos.

| path | why |
|---|---|
| `crates/graph-core/src/layout/planarity.rs` + `planarity/{adjacency,embed,lr,triangulate}.rs` and their `tests/` | Step 5 requires a planarity test, and the phase's own rule is that its "planar" must be *certified* (Euler's formula over the traced faces) rather than trusted. LR planarity plus a triangulation is the reference's own structure (`circle_packing.py:281-393` calls into it) and cannot fit inside `circle_packing.rs` under the 300-line house limit. Envelope growth, deliberate. |
| `crates/graph-core/src/layout/{tidy_tree,treemap,circular,circle_packing,hierarchy}/**` submodules and `tests/` | House limit (≤300 lines/file) applied to the CREATE files. `treemap/tests/shapes.rs` (420) and `treemap/tests/golden.rs` (354) are named in `RESUME.md` item 8 as still over 300 — recorded there, not hidden here. |
| `crates/graph-contract/src/{notes.rs,binary.rs,binary/decode.rs,canonical_json.rs,canonical_json/{read,schema}.rs,snapshot.rs,snapshot/error.rs,version.rs}` + tests | The `notes` ADR (`docs/decisions/snapshot-notes.md`, contract 0.2 → 0.3) is a **new snapshot section**; it cannot be delivered from `graph-core` alone. Required by step 1 ("make the choice visible in the output") and by the phase's own ledger delta. |
| `docs/decisions/{hierarchy-repair,snapshot-notes,circular-conventions}.md` | Step 1 requires the cycle/multi-parent/forest decision written down; step 4 requires the ring conventions recorded. The envelope names only `planarity-fallback.md`. |
| `crates/graph-cli/src/{snapshot_cmd.rs,snapshot_cmd/**,oracle_fixtures.rs,oracle_fixtures/**,runner.rs}` | The phase's own acceptance items are *differentials* (`emit-fixtures`, `roundtrip --seeds 1000`, a per-seed hand oracle for circular and packing), and the envelope's `graph-cli` MODIFY list is only `{main.rs,capabilities.rs}`. Without the CLI wiring the phase has no measurable claim at all. |
| `crates/graph-cli/src/{hashgate.rs,hashgate/**,capabilities.rs,capabilities/**}` and `crates/graph-cli/tests/{cli.rs,cli_oracles.rs,snapshot.rs}` | The four layouts each need a hash-gate stage and a ledger row; that is the phase's Ledger delta. The row/problem counts asserted in the capability tests move with them. |
| `crates/graph-core/tests/{geometry_invariants,memory}.rs` | Step 3's treemap containment/non-overlap invariant "over the whole seed sweep" is an integration test, and `phase04`'s ceiling arithmetic cites a per-node memory probe. |
| `crates/graph-core/src/{index.rs,index/view.rs,columns.rs,diff.rs,edgekind.rs,ids.rs,records.rs,stage/topology.rs,synthetic.rs,lib.rs}` | The `notes` section threads through the snapshot writer and the topology columns. Follows from the ADR, not chosen. |
| `crates/graph-wasm/src/lib.rs`, `harness/wasm-run.mjs` | The wasm arm must know the new stages or the 4-way gate refuses. Recorded in `phase-04.md` §6a as the same defect found independently there. |
| `docs/contract/{binary-layout.md,snapshot-schema.json}` | The binary face changed (0.3); these are the authoritative documents for it. |
| `Cargo.lock`, `crates/graph-cli/Cargo.toml`, `package-lock.json` | The lock files follow the new direct devDependency and the new `graph-cli` module; one line each in `Cargo.lock`. |
| `docs/reports/phase-03.md` | This report. |

## 2. Ledger diff

Read from `capabilities --json` on this tree, and from the same source at the two
commits that bracket the phase.

| id | before `834f1a0` | after `14c64ad` | on develop now (`ff19809`) |
|---|---|---|---|
| `layout.tree.tidy` | — | **`gated`**, `Point` + `Polyline`, `O(n)` | `gated` |
| `layout.treemap.squarified` | — | **`gated`**, `Box`, `O(n log n)` | `gated` |
| `layout.circular.radial` | — | **`gated`**, `Point`, `O(n)` | `gated` |
| `layout.packing.circle` | — | **`gated`**, `Circle` | `gated` (ceiling 5 000) |
| `layout.grid` | `gated` | `gated`, unchanged | `gated` |
| the 8 `topology.*` rows | `gated` | `gated`, unchanged | `gated` |

Row count: **9 → 13** at `14c64ad` (8 topology + `layout.grid` + 4 new); 36 on develop
today after p4/p6/p7/p9/p10 added theirs. `layout.none` is a *negative* id used only in
`registry::tests::a_registered_layout_emits_the_kinds_it_declares_at_its_default_parameters`
(`assert!(find("layout.none").is_none())`); it was never a row.

Each new row's declared `oracle` is the one the phase named, and the current
`capabilities --json` text still names it: `d3-hierarchy@3.1.2 tree()` for the tidy tree,
`treemap().tile(treemapSquarify)` for the treemap, a hand oracle for circular
(`docs/decisions/circular-conventions.md`), and hand + planarity certificate for packing.
`layout.packing.circle`'s `degradation` says the non-planar fallback still runs and
returns finite geometry, which is the phase's own required declaration.

## 3. Gate table

Every row the phase prompt's Gate section names, and the develop-gate row that answers it.
Tree `b980ae8`. `expect` is the phase prompt's own expected exit code.

| phase prompt row | develop row | exit | verdict |
|---|---|---:|---|
| `cargo fmt --check` | `fmt` | 0 | **PASS** |
| `cargo clippy --workspace -- -D warnings` | `clippy` | 0 | **PASS** |
| `cargo test --workspace` | `test` | 0 | **PASS** (106 s) |
| `cargo build -p graph-core --target wasm32-unknown-unknown` | `wasm32` | 0 | **PASS** |
| `hashgate --seeds 1000` | `hashgate-1000` | 0 | **PASS** (3052 s) |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | `negctl-reference-degree` | 1 | **PASS** (expected non-zero) |
| `roundtrip --seeds 1000` | `roundtrip-1000` | 0 | **PASS** (2266 s) |
| `emit-fixtures --seeds 1000` | `emit-fixtures-1000` | 0 | **PASS** |
| `node harness/oracle-layouts.mjs` | `oracle-layouts` | 0 | **PASS** |
| `cargo test -p graph-core geometry_invariants` | `geometry-invariants` | 0 | **PASS** (95 s) |
| `capabilities --check` | `capabilities-check` | **1** | **FAIL** (see below) |
| `docker build -t ge-check . && docker run --rm ge-check` | `ge-check` | 0 | **PASS** (10 s) |
| — (not in the phase prompt) | `negctl-grid-spacing`, `negctl-node-count` | 1 | PASS, expected non-zero |
| — (not in the phase prompt) | `npm-ci` | 0 | PASS |

**`capabilities --check` is a genuine FAIL on this phase's own criterion, and it is not
noise.** On the develop tree it reports `29 rows, 8 problems` and every one of the eight
is a `topology.*` row (Phase 1's, not this phase's) reading
`gated, but no oracle-diff record: run the gate` — the `ge-check`/node pipeline that
writes those records had not been run for them. This phase's four rows produced **zero**
problems in that run. Re-run on this worktree's tree it is worse and the reason is
honest: `gr cargo run -q -p graph-cli -- capabilities --check` exits **1** with
`36 rows, 34 problems`, every one a `gated` row with no record in *this* worktree
(`<gates>/hashgate.json`, `oracle-layouts.json`, … are per-tree). A ledger check run in a
worktree that has not run the gate cannot be green; it is recorded here as FAIL because
UNKNOWN = FAIL, with the scope of the failure named.

**NOT RUN, for this phase, on any tree:** `cargo mutants` (the orchestrator's row under
the host-wide lock, per `AGENT_BRIEF.md`), and this phase's *own* 1000-seed gate, which
never ran (`RESUME.md` line 30).

## 4. 4-way hash table

The develop gate's own stage list, from the `negctl-force-theta` log (the one row in the
gate that prints its header), on tree `b980ae8`:

```
stages=topology,layout.grid,layout.tree.tidy,layout.treemap.squarified,
layout.circular.radial,layout.packing.circle,layout.spectral,layout.mds.pivot,
layout.force.barnes_hut,layout.forceatlas2,layout.dag.sugiyama,
transport.wasm.columnar   seeds=8
```

This phase's four stages, at 8 seeds, from that log:

| stage | what hashes it | result | control that must fail |
|---|---|---|---|
| `layout.tree.tidy` | the tidy tree's own snapshot (Point nodes, Polyline edges, notes) | 4-way equal, 8/8 | `GM_MUTATE_GRID_SPACING=2` does **not** reach it; `GM_MUTATE_REFERENCE_DEGREE=9` does (topology changes) |
| `layout.treemap.squarified` | the treemap's own snapshot (`Box`) | 4-way equal, 8/8 | as above |
| `layout.circular.radial` | the circular layout's own snapshot | 4-way equal, 8/8 | as above |
| `layout.packing.circle` | the packing's own snapshot (`Circle` + note 3) | 4-way equal, 8/8 | as above |

`hashgate-1000` (exit 0) is the same 12-stage list at 1000 seeds, but the gate's own log
prints the per-stage counts only for the arm that ran at 8; the 1000-seed run's per-stage
line is not in the retained log, so the 1000-seed per-stage claim is **UNKNOWN** and is
not asserted here. What *is* asserted is the row's exit code.

**The honest gap, and it belongs to `phase-04.md` §7 as much as to this phase:** none of
these four stages has a negative control of its own. `GM_MUTATE_GRID_SPACING` reaches
`layout.grid` and the transport stage that restates it; it does not reach a layout whose
run ignores `GridParams`. The develop gate's four negctl rows are
`REFERENCE_DEGREE`, `GRID_SPACING`, `NODE_COUNT`, `SUGIYAMA_LAYER_SPACING` — **no row
perturbs a p3 layout**. Per the brief ("no row goes to `gated` without a control that must
fail") these four rows should not be `gated` on the evidence that exists, and
`verdict::red_control` is the rule that would refuse them. Recorded as an open item, not
papered over.

## 5. Coverage table

| changed symbol | the test that exercises it |
|---|---|
| `hierarchy` root detection, the parent cycle broken at its lowest dense index | `hierarchy::tests::{the_lowest_edge_index_keeps_the_parent_even_when_dense_order_disagrees, a_parent_cycle_is_broken_at_its_lowest_dense_index, self_loops_are_never_parents_and_never_noted, a_repeated_parallel_parent_edge_is_an_extra_parent}` |
| forest handling, the virtual root | `hierarchy::tests::{one_root_is_the_tree_root_and_two_hang_off_a_virtual_root, an_empty_topology_has_no_root}`, `hierarchy::tests::fixtures::{the_forest_hangs_every_root_off_one_virtual_root, the_degenerate_chain_is_rooted_at_its_last_listed_node, the_balanced_tree_needs_no_repair_whatever_the_spelling, cyclic_pins_every_repair_and_its_notes, reordering_an_acyclic_fixture_moves_no_parent_and_no_depth}` |
| the repairs are visible in the output, not a log | `hierarchy::tests::fixtures::cyclic_pins_every_repair_and_its_notes`; `layout::tidy_tree::tests::the_hierarchy_repairs_reach_geometry_notes_unchanged`; `layout::treemap::tests::every_fixture_is_structural_and_propagates_the_hierarchys_notes`; `layout::circular::tests::the_hierarchy_repairs_reach_geometry_notes_unchanged` |
| `tidy_tree` against `d3-hierarchy` | `tidy_tree::tests::{the_tree_balanced_fixture_matches_d3_hierarchy_bit_for_bit, the_tree_degenerate_fixture_matches_d3_hierarchy_bit_for_bit, a_caterpillar_matches_d3_hierarchy_bit_for_bit, a_deep_chain_matches_d3_hierarchy_bit_for_bit, a_ten_leaf_fan_matches_d3_hierarchy_bit_for_bit, an_unbalanced_tree_matches_d3_hierarchy_bit_for_bit, the_execute_shifts_witness_…, the_modifier_shift_witness_…, the_modifier_sign_flip_witness_…, the_left_extreme_tie_break_…, the_right_extreme_tie_break_…, the_second_finish_contour_branch_…}` (all `…_matches_d3_hierarchy_bit_for_bit`), plus the `tests/golden/` table `the_golden_shapes_are_not_all_the_same_layout` |
| the four `Polyline` CSR boundaries the prompt names (first edge, last edge, zero interior points, max span) | `tidy_tree::tests::{polyline_offsets_are_right_at_both_csr_boundaries_and_at_a_zero_length_row, path_offset_counts_points_and_refuses_past_the_u32_wire}`, and `crates/graph-core/tests/geometry_invariants.rs::tidy_tree_polyline_offsets_are_well_formed_and_stay_inside_pts` |
| `treemap` squarify against `d3-hierarchy` | `treemap::tests::{the_balanced_fixture_matches_d3_bit_for_bit, the_degenerate_fixture_matches_d3_bit_for_bit, a_deep_chain_matches_d3_bit_for_bit, a_ten_leaf_fan_matches_d3_bit_for_bit, an_unbalanced_tree_matches_d3_bit_for_bit, the_extend_row_value_round_trip_matches_d3_bit_for_bit}` |
| treemap containment / non-overlap, the invariant the prompt asks to be asserted over the whole sweep | `crates/graph-core/tests/geometry_invariants.rs::treemap_boxes_contain_their_children_and_siblings_never_overlap`, plus the sweep's own negative controls `treemap::tests::{the_invariant_sweep_rejects_a_child_that_escapes_its_parent, the_invariant_sweep_rejects_two_siblings_that_overlap, the_round_trip_control_input_does_not_reach_the_same_bits}` |
| non-positive / non-finite weight clamped to epsilon | `treemap::tests::{clamp_weight_only_touches_non_positive_or_non_finite, no_geometry_is_ever_nan_or_infinite, sorted_children_is_descending_value_stable_on_ties}` |
| `circular` ring order, radius progression, start angle | `circular::tests::{ring_one_slots_sit_at_the_closed_form_angles, three_children_share_ring_one_ascending_dense_index_from_the_positive_x_axis, the_virtual_root_never_reaches_the_output_and_real_roots_land_on_ring_one, a_single_node_sits_at_the_dead_centre, every_fixture_lays_out_finite_deterministic_and_ring_true}` |
| `circle_packing` exact Collins–Stephenson path | `circle_packing::tests::exact::*`, `…::small::{a_triangle_packs_into_three_mutually_tangent_circles, a_wheel_packs_exactly, a_square_with_one_diagonal_packs_exactly}`, `…::embed::{a_sparse_grid_with_no_triangles_of_its_own_still_packs_exactly, a_disconnected_planar_graph_is_joined_into_one_disk_and_packs_exactly}` |
| the fallback is **visible in the output** (the phase's explicit requirement) | `circle_packing::tests::fallback::{a_planar_graph_takes_the_exact_path_and_a_non_planar_one_the_fallback, a_k5_minor_hidden_inside_a_bigger_graph_still_falls_back, a_degenerate_all_boundary_flower_still_comes_back_flagged}` — each asserts note code 3, not a log line |
| the planarity test is certified, not trusted | `planarity::tests::faces::a_traced_embedding_of_a_real_disk_faces_itself_consistently`, `planarity::tests::negative::*`, `planarity::tests::positive::*`, `planarity::tests::properties::*` |
| the round-trip / hand oracles the phase's own gate needs | `crates/graph-cli/src/snapshot_cmd/hand_oracles/tests.rs` (+ `circular.rs`, `packing.rs`), `crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs`, `crates/graph-cli/src/oracle_fixtures/layouts/tests.rs`, `crates/graph-cli/tests/cli_oracles.rs` |
| no NaN/Inf and positive radii for **every** registered layout | `crates/graph-core/tests/geometry_invariants.rs::every_registered_layout_emits_no_nan_or_inf_and_circle_radii_are_positive` |

## 6. Ponytail markers added

The four markers the prompt names, one per layout, each with its failing input and
direction — the text is the ledger's own, read from `capabilities --json`:

- **Treemap.** Non-positive areas clamp to epsilon, so a zero-area subtree becomes a
  hairline rather than nothing. Direction: cosmetic under-representation, never a wrong
  containment. (Pinned by `treemap::tests::clamp_weight_only_touches_non_positive_or_non_finite`.)
- **Circle packing.** Exact only on planar input; the fallback is not guaranteed tangent
  or non-overlapping. Failing input: any K₅ or K₃,₃ minor. Direction: **overlap, the
  dangerous direction.** Escape hatch: read note code 3 (`packing.approximate`) in the
  snapshot — the same flag the phase required and the tests assert.
- **Circular.** The radius progression is a convention, not a derivation; dense rings
  crowd at high depth. Recorded in `docs/decisions/circular-conventions.md`.
- **Tidy tree.** **None owed** — the phase says say so plainly rather than invent a
  caveat, and the `…_matches_d3_hierarchy_bit_for_bit` tests are what that claim rests on.
  The one place a judgement is made is the hierarchy repair, and it carries its own
  markers (codes 1 and 2) rather than borrowing this row's.

Two markers the phase did not ask for and this report does not invent: the four
`scale_ceiling` figures are estimates derived from Phase 1's measured 442 B/node
(`docs/measurements/p1-topology-memory.md`) projected onto wasm32's 4 GiB, and each row
says so in its own text.

## 7. What could not be verified

- **This phase's own gate, on its own branch. UNKNOWN.** `p3` is not in this worktree and
  `RESUME.md` line 30 records that its full gate never finished. The develop gate on
  `b980ae8` is evidence for the merged tree; it is not the phase's own 1000-seed run and
  is not presented as one.
- **`cargo mutants` over this diff. UNKNOWN** — the orchestrator's row, under the
  host-wide lock, per `AGENT_BRIEF.md`.
- **Per-stage counts for this phase's four stages at 1000 seeds. UNKNOWN.** The
  1000-seed row's exit code is 0 and the stage list is the registry's, but the retained
  log does not print per-stage counts for that run, so "4-way equal at 1000 seeds *for
  these four stages*" is not asserted — only "the 12-stage run at 1000 seeds exited 0".
- **A negative control that reaches a p3 layout. UNKNOWN / absent.** §4's honest gap.
  Until one exists, the four `gated` statuses rest on unit tests and the differential, not
  on a control that must fail.
- **A green `capabilities --check`. Not observed on any tree.** §3.
- **Browser execution of the wasm arm.** Every JS check ran under `node:22-slim`;
  `WebAssembly.instantiateStreaming`'s browser `fetch` path is written and unexercised.
  UNKNOWN, unchanged from Phase 4.
- **wasm32 peak-memory measurement.** Every ceiling above is native and projected; the
  rows' own Ponytail says so.

## 8. Stop-and-ask items

- *"A layout differential fails only by a uniform scale or translation → probably d3's
  normalization convention; investigate, do **not** add a tolerance."* **Did not occur.**
  The oracle is `d3-hierarchy@3.1.2` at its own defaults (`size([1,1])`, the default
  separation) and the port matches it bit for bit after `Math.fround`, over fixtures and
  a synthetic sweep. No tolerance was added anywhere in this phase.
- *"You cannot implement a correct planarity test → stop."* **Not triggered; the test is
  certified.** LR planarity plus an Euler-formula check over the traced faces, with the
  negative suite (`planarity::tests::negative::*`) pinning inputs that must be reported
  non-planar. A failed certificate is treated as non-planar, which is the safe direction.
- *"A hierarchy fixture (cyclic, multi-parent, forest) has no obviously right answer →
  stop and ask."* **Decided, not guessed, and written down**: `docs/decisions/hierarchy-
  repair.md` (root detection, the parent cycle broken at its lowest dense index, the
  extra parent dropped, self-loops never parents, `hierarchy.virtual_root` for a forest)
  and `docs/decisions/snapshot-notes.md` (contract 0.2 → 0.3, codes 1/2/3, version
  dispatch by `minor >= 3` rather than EOF sniffing). `fixtures/hierarchy/cyclic.json`
  pins the behaviour, as the prompt requires.
- **New, and open:** the four rows are `gated` without a stage-specific negative control
  (§4). Either the gate grows four `GM_MUTATE_*` knobs, one per p3 layout, or
  `verdict::red_control` demotes all four to `implemented`. Recommended: add the knobs —
  the four runs are pure functions of their input and a per-layout constant is the
  cheapest control in the project.
