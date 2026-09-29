# Phase 6 report — iterative and spectral layouts

Shape: `prompt.md` §12. **No command in this report was re-run against the phase's own
branches.** `prompts/RESUME.md` lines 31–32 record p5, p6e and p6f as *"merged via train
(inside p6f)"* — one merge, `45a653f`. Every gate row below is read from **one**
develop-wide gate run, `/goinfre/dlesieur/wt/p8/target/gate-develop/summary.txt`, on
**tree `b980ae8`** (2026-09-29). A row that ran on that tree is evidence for that tree, not
for `p6e`/`p6f` as they stood. Anything not run is `NOT RUN`; UNKNOWN = FAIL.

This report covers **two** branches and is explicit about which evidence belongs to which:
**p6e** = spectral, pivot MDS, `linalg` (eigensolver); **p6f** = Barnes–Hut, ForceAtlas2,
`rng`, the stress metric, the bench CLI, the d3 and FA2 oracles.

**Provenance of the merge.** `git log develop` puts both on `develop` at **`45a653f`**
(2026-09-29 03:39 +0200), a merge of `14c64ad` (Phase 3) with **`67543b4`**, the train
tip. The branch heads are **`d130fe2`** (p6e, 2026-09-29 02:32, 42 files) and **`e2e59a9`**
(p6f, second-parent side, 307 files — 58 020 insertions, most of them the
`scratch/synthetic/seed-*.json` measurement dumps).

## 0. What this phase was

The first chaotic determinism surface in the project, and the first with a genuinely
degenerate eigenspace. Two hard problems: a force simulation amplifies a 1-ULP difference
into a different picture, so the gate is *determinism* first and *quality* second; and when
λ₂ = λ₃ — which happens on grids and trees, i.e. common input — a solver returns whatever
rotation its start vector lands on. The reference solved both, in comments, and the phase
was told to port them as decisions.

## 1. Authorization compliance

**On the envelope (CREATE, p6e):** `crates/graph-core/src/linalg/{mod,lanczos,dense_sym}.rs`
— see the deviation table: `lobpcg.rs` replaces `lanczos.rs`;
`docs/decisions/eigensolver.md`; `docs/measurements/phase06-eigen.md`.

**On the envelope (CREATE, p6f):** `crates/graph-core/src/layout/force/{mod,barnes_hut,
quadtree,params}.rs`; `crates/graph-core/src/layout/forceatlas2.rs`; `rng.rs`;
`fixtures/force/{grid,tree,clustered,disconnected,single-node}.json`;
`docs/measurements/phase06-{force,stress}.md`.

**On the envelope (MODIFY):** `crates/graph-core/src/layout/mod.rs`,
`crates/graph-core/src/registry.rs`, `crates/graph-core/Cargo.toml` (**unchanged** — no
dependency was added; the allow-list stayed `libm` + `indexmap`),
`crates/graph-cli/src/{capabilities.rs,main.rs}`, `harness/oracle-layouts.mjs`.

**Deviations** — every path outside those lists, from `git show --numstat d130fe2 e2e59a9`
and `45a653f`:

| path | why |
|---|---|
| `crates/graph-core/src/linalg/lobpcg.rs` + `lobpcg/{ops,ritz,ritz/generalized}.rs` (+ tests) **instead of `lanczos.rs`** | **The named RESUME deviation.** The envelope names `lanczos.rs`; the ADR's own cascade (dense below 256, iterative above, each verified by residual) and the reference's own tier are *LOBPCG*, not Lanczos — `networkx_layouts.py`'s `_LOBPCG_MAXITER = 300` and the differential in `docs/measurements/phase06-eigen.md` are against scipy's `lobpcg`. Implementing Lanczos and then measuring against a LOBPCG reference would have compared two different algorithms. The file is named for the algorithm actually ported, and `docs/decisions/eigensolver.md` records the choice. |
| `crates/graph-core/src/layout/forceatlas2/{state,tests}.rs` | House limit on `forceatlas2.rs` (the port is 51 lines + 249). |
| `crates/graph-core/src/layout/force/**` submodules (`barnes_hut/{charge,collide,link,seed,sim}.rs`, `quadtree/{bounds,tests}.rs`, `force/tests.rs`) | House limit. `quadtree.rs` is 268 lines in `e2e59a9`. |
| `crates/graph-core/examples/force_dump.rs` | The measurement harness the three `docs/measurements/phase06-*.md` files name as their reproduction command. **RESUME item 5 records this example as a loose end still to clean up**: `registry/force.rs` still names the removed `examples/force_dump.rs`. Recorded here as a known inconsistency, not as a claim that it was checked. |
| `docker/python-oracle.Dockerfile`, `docker/python-oracle.requirements.txt` | **The named RESUME deviation: the python-oracle image.** `graph-core` cannot run scipy, and the envelope's file list has no place for a second image. `debian:trixie-slim` + pinned `numpy 2.3.3` / `scipy 1.16.2` (hash-pinned, `--require-hashes --only-binary --no-deps`) + `networkx 3.6` unpacked from a sha256-verified tarball via a named build context. Rule 0.3 forbids vendor language images and this is a minimal OS base plus exactly what is installed, pinned. It is a **differential oracle only** and is never a `graph-core` dependency. |
| `crates/graph-cli/src/oracle_spectral.rs`, `crates/graph-cli/src/{bench_cmd.rs,stress.rs,stress/**}` | **The named RESUME deviation: the bench CLI** (and, with it, the stress CLI). The phase prompt's own gate names `stress --oracle d3` and `bench --n 220,10000,100000`; neither command existed. `bench_cmd.rs` is the merged form of the p6e `bench_cmd.rs` (renamed) — the envelope lists no `graph-cli` file for it. |
| `crates/graph-cli/src/oracle_python.rs` + `oracle_python/{fa2,spectral,tests}.rs` | **The named RESUME deviation: `oracle_python.rs`.** By `45a653f` the two python-image differentials had converged on one generic three-step driver (emit fixtures → compare in the image → check and record) with a per-oracle `line` function. `p6e`'s standalone `oracle_spectral.rs` became `oracle_python/spectral.rs`. Recorded because it is a rename-plus-refactor of a file the p6e envelope created, and the p6f envelope did not list it. |
| `crates/graph-cli/src/{main.rs,capabilities.rs,capabilities/**,hashgate.rs,hashgate/**,evidence.rs,runner.rs}` and `crates/graph-cli/tests/{cli.rs,cli_force.rs,cli_oracles.rs,common/mod.rs,snapshot.rs}` | The phase's Ledger delta is four rows and a hash-gate stage each; the CLI rows, the knob list (which `RESUME.md` item 6's lesson says now lives once, in `crates/graph-cli/tests/common/mod.rs`) and the capability row/problem counts all move with it. |
| `harness/{oracle-spectral.py,oracle-fa2.py,stress-d3.mjs,oracle-dag.mjs}`, `harness/wasm-run.mjs` | The Python arms run in the oracle image and cannot be in `oracle-layouts.mjs`; `stress-d3.mjs` runs `d3-force@3.0.0` in `node:22-slim`. `wasm-run.mjs` must learn the new stages. |
| `crates/graph-wasm/src/lib.rs` | The wasm arm must hash the new layouts. |
| `crates/graph-core/src/registry/{spectral,force}.rs` | 300-line limit on `registry.rs`. |
| `scratch/**` (p6f: `dumps/*.txt|stderr`, `synthetic/seed-{0..49}.json`; p6e: none) | The measurement evidence `docs/measurements/phase06-stress.md` cites as its raw input. Kept. |
| `docs/reports/phase-06.md` | This report. |

Nothing under `src/`, `tests/` (the TypeScript oracle) or `verify/`, nothing in osionos,
no edge bundling/routing (Phase 8), and no new `graph-core` dependency.

## 2. Ledger diff

| id | before (`14c64ad`) | after (`45a653f`) | on develop now |
|---|---|---|---|
| `layout.spectral` | — | **`gated`**, `Point`, ceiling **700** | `gated` |
| `layout.mds.pivot` | — | **`gated`**, `Point`, ceiling **100 000** | `gated` |
| `layout.force.barnes_hut` | — | **`implemented`** (not `gated`), ceiling **100 000** | `implemented` |
| `layout.forceatlas2` | — | **`implemented`** (not `gated`), ceiling **14 000** | `implemented` |
| `layout.dag.sugiyama` (p5, same merge) | — | `gated` | `gated` |

Row count **13 → 18** across `45a653f`; 36 on develop today. `layout.none` was never a row.

**The two `implemented` rows are the phase's own honest position, and it is the one the
phase prompt asks for.** Its Ledger delta says *"To `gated`: `layout.force.barnes_hut`,
`layout.spectral`, `layout.mds.pivot`. FA2 and Yifan Hu only if their references were
obtained — otherwise they stay `absent` with the reason recorded, which is a valid and
honest phase outcome."*

- **Barnes–Hut is `implemented`, not `gated`.** Its `oracle_diff` field says
  `not backed: no stress record: run the gate` and its `oracle` text says outright:
  *"Identity is NOT claimed and cannot be: link and collide are Jacobi gathers where d3
  scatters in visit order, and a force simulation amplifies a 1-ULP difference into a
  different picture, so the gate is the stress metric"*. A force layout cannot be gated on
  a byte differential; `prompt.md` §8 requires evidence for a `gated` row, and the honest
  status is the one the code supports.
- **FA2 is `implemented` too**, and its `oracle` names networkx 3.6's
  `forceatlas2_layout` as a real port target. `docs/measurements/phase06-stress.md` §1
  says the row is held to that reference in the oracle image, "a different record
  (`<gates>/oracle-fa2.json`) and a different criterion, because FA2 is a *port* of that
  function and can be compared against it directly where Barnes–Hut deliberately cannot".
  The differential exists; the row has not been flipped to `gated`, and the reason is
  §7.
- **Yifan Hu is absent entirely** — no row at all. `docs/measurements/phase06-force.md`
  gives the reason verbatim: Graphviz `sfdp` is not on disk, the corpus's own `YIFAN_HU` is
  a different algorithm, and "a missing reference is a stop, not an improvisation". That
  is the phase prompt's stop-and-ask answered the way rule 0.6 requires, and it is a valid
  outcome by the prompt's own words.
- **`layout.spectral` does not carry a per-platform-reproducibility exception.** The prompt
  says such an exception "belongs in `degradation`, visible, never implied" and only if the
  iterative tier cannot be made bit-identical. The measurements say the opposite: the
  4-way hash gate lists `layout.spectral` among the twelve stages that are 4-way equal, and
  `docs/measurements/phase06-eigen.md` records `is_deterministic_run_twice_across_both_
  tiers` passing on the dense **and** LOBPCG tiers. The `degradation` field instead states
  the real failure: past the ceiling a path-like component may not converge in 1500
  iterations, fails the residual gate, and is **skipped with its nodes left at the origin
  and the run refused with `StageError::Param` if nothing solved — never a random layout**.

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
| `grep -rn mul_add … && exit 1 \|\| exit 0` | `no-mul-add` | **1** | **FAIL — false positive, §below** |
| `cargo test -p graph-core eigen_determinism` | `eigen-determinism` | 0 | **PASS** |
| `stress --oracle d3` | `stress-d3` | 0 | **PASS** |
| `bench --n 220,10000,100000` | `bench` | 0 | **PASS** (465 s) |
| `capabilities --check` | `capabilities-check` | **1** | **FAIL** (§below) |
| `node harness/sdk-smoke.mjs` | `sdk-smoke` | **2** | **FAIL** — row defect (§below) |
| `docker build -t ge-check . && docker run --rm ge-check` | `ge-check` | 0 | **PASS** |
| — (p6e's own rows) | `emit-spectral-1000` | 0 | PASS |
| — | `oracle-spectral-py` (ge-python-oracle) | 0 | PASS (41 s) |
| — | `oracle-spectral` | 0 | PASS |
| — | `bench-spectral`, `bench-mds` | 0 | PASS |
| — (p6f's own rows) | `negctl-force-theta` | **0** | **FAIL — expected non-zero, §below** |
| — | `negctl-fa2-scaling-ratio` | 1 | PASS (expected non-zero) |
| — | `emit-fa2-1000` | 0 | PASS |
| — | `oracle-fa2-py` (ge-python-oracle) | 0 | PASS (285 s) |
| — | `oracle-fa2` | **1** | **FAIL** (§below) |
| — (not in the phase prompt) | `negctl-grid-spacing`, `negctl-node-count`, `negctl-sugiyama-layer-spacing`, `hashgate-negctl` | 1 | PASS, expected non-zero |
| — | `roundtrip-1000`, `emit-fixtures-1000`, `oracle-layouts`, `dag-dump`, `oracle-layouts-dag`, `geometry-invariants`, `sugiyama-invariants`, `npm-ci`, `wasm32` variants | 0 | PASS |

**`no-mul-add` FAIL is a false positive, and the evidence says so.** Its log lists five
hits, every one of them a **comment** stating that the crate does *not* use `mul_add`:
`circle_packing.rs:42`, `post/styles.rs:70`, `post/mingle.rs:51`, `post/mingle/level.rs:152`,
`post/fdeb.rs:34`. No call site. `RESUME.md` item 3 records the same finding and that the
row now greps `mul_add[[:space:]]*[(`. The failure is in the row's pattern, not in the
code — and the pattern **has still not been re-run**, so the row is FAIL here, with the
diagnosis attached.

**`negctl-force-theta` FAIL is real, and it is this phase's most important open item.**
With `GM_MUTATE_FORCE_THETA=0.5`, the gate printed all twelve stages
`4-way equal on 8/8 seeds` and `PASS`, exit 0 — the row expected non-zero. **The θ knob
does not reach any stage.** Barnes–Hut's own stage is `layout.force.barnes_hut` and it was
unperturbed, so the project's only force-layout negative control is inert. This is exactly
the gap `phase-03.md` §4 and `phase-05.md` §4 record from the other direction: Sugiyama has
a working per-capability control (`negctl-sugiyama-layer-spacing`), the p3 layouts and
Barnes–Hut do not. The FA2 control (`negctl-fa2-scaling-ratio`) **does** bite.

**`oracle-fa2` FAIL is the known open item, `RESUME.md` item 4, and it is not the port's
fault.** Its log reads verbatim: `layout.forceatlas2: 1000 cases, worst 1.654e-1, ceiling
1e-3: FAIL`. The placeholder `CEILING = 1e-3` in
`crates/graph-cli/src/oracle_python/fa2.rs:23` (still there on this tree) will go red. The
measurement `RESUME.md` records over the same 1000 seeds: n < 100 median 2.1e-8 (the f32
floor), n 300–400 median 2.0e-3, worst 0.165 at seed 340 (n = 342), 327 seeds above 1e-3 —
**and networkx against itself, with the start perturbed by 1 ulp, diverges just as far on
the same seeds** (seed 340: 0.151; seed 846: 0.200 against our 0.133). It is chaos, not a
port bug. The row therefore cannot gate as written, and the fix named in `RESUME.md` (a
short-iteration comparison, or a "not worse than networkx by more than X" bound on `stress`)
**has not been applied**: `layout.forceatlas2` is still `implemented`, and the ceiling is
still the placeholder.

**`capabilities --check` FAIL** — `29 rows, 8 problems`, all eight `topology.*` rows
(Phase 1's) reading `gated, but no oracle-diff record: run the gate`. None of the six rows
this phase adds contributes a problem, because two are `implemented` (never evidence-checked)
and two have their records from the same tree. Re-run in this worktree: exit **1**,
`36 rows, 34 problems`, every one a `gated` row with no record in a tree that has not run
the gate.

**`sdk-smoke` FAIL** — the log is `sdk-smoke: could not run: usage: sdk-smoke.mjs <wasm>`,
exit 2: the develop row invokes the harness with no argument. Same defect as `zero-copy`
(`wasm-run.mjs: usage: … | probe | --assert-zero-copy`). A row defect, not evidence about
force layouts — but the phase prompt's own `sdk-smoke` item is therefore FAIL.

**NOT RUN, for this phase, on any tree:** `cargo mutants` (orchestrator's row,
`AGENT_BRIEF.md`). And **the cross-target wasm32-vs-TypeScript speed comparison at
N = 220**, which the phase prompt calls for explicitly and which
`docs/measurements/phase06-force.md` already answers honestly for itself: *"Not measured,
and therefore not claimed… The wasm32 arm is exercised by the 4-way hash gate, which proves
the two targets produce bit-identical output — it does not, and cannot, compare their
speed."*

## 4. 4-way hash table

| stage | what hashes it | result at 8 seeds | control |
|---|---|---|---|
| `layout.spectral` | the spectral layout's own snapshot (`Point`) | 4-way equal, 8/8 (digest `e7436052…` in the `negctl-force-theta` header run, which is the unperturbed gate) | **none of its own** — `GM_MUTATE_*` reaches no spectral parameter |
| `layout.mds.pivot` | the pivot MDS snapshot | 4-way equal, 8/8 | none of its own |
| `layout.force.barnes_hut` | the Barnes–Hut snapshot | 4-way equal, 8/8 | `GM_MUTATE_FORCE_THETA=0.5` → **still 4-way equal, PASS, exit 0. The control does not bite (§3).** |
| `layout.forceatlas2` | the FA2 snapshot | 4-way equal, 8/8 | `GM_MUTATE_FA2_SCALING_RATIO=3` → exit 1, PASS as expected non-zero. **The one force-layout control that works.** |

**This is the phase's central result and it is a real one.** The prompt's first stop-and-ask
is *"4-way hash equality fails and D1–D9 are all satisfied → stop and report. This is the
finding that matters most in the project."* **It did not trigger.** Six chaotic layouts —
four from this phase, plus Phase 3's — hash bit-identically across native run 1, native run
2, wasm32 run 1 and wasm32 run 2 at 8 seeds, and the same twelve-stage run exits 0 at 1000
seeds (3052 s). No tolerance was added anywhere.

**Two honest limits on that.** (a) As in `phase-03.md` §4 and `phase-05.md` §4, the
1000-seed run's per-stage counts are not in the retained log, so "4-way equal at 1000
seeds *per stage*" is UNKNOWN; the row's exit code is what is asserted. (b) The
`negctl-force-theta` row's own log is also the best evidence that Barnes–Hut's determinism
is *not* currently under test: a knob that does not perturb the stage leaves the stage's
green unchallenged.

## 5. Coverage table

| changed symbol | the test that exercises it |
|---|---|
| `rng.rs`: explicit, threaded, no globals; counter-based jiggle | `force::tests::the_same_topology_and_seed_settle_to_the_same_geometry_run_to_run`; the jiggle's order-independence (`jiggle(seed,tick,pass,(i,j)) == jiggle(seed,tick,pass,(j,i))`) is asserted by test, per `docs/measurements/phase06-stress.md` deviation 2 |
| `quadtree.rs`: reused buffer, no per-tick allocation, deterministic build order | `force::quadtree::tests::{building_the_same_points_twice_gives_the_same_structure_run_to_run, an_empty_or_all_nan_point_set_builds_an_empty_tree, exactly_coincident_points_chain_on_one_leaf_others_split_apart, cover_grows_a_square_that_contains_every_point, a_nan_point_is_ignored_and_never_reached_by_visit, bounds_of, postorder_lists_every_child_before_its_parent}` |
| `barnes_hut`: the frozen force set, `TICKS = 112` | `force::barnes_hut::tests::{a_small_graph_settles_to_finite_positions, a_single_node_does_not_divide_by_zero, the_same_topology_settles_to_the_same_geometry_run_to_run}`; the tick count's own derivation is `ticks_is_the_least_k_below_alpha_min` (`docs/measurements/phase06-force.md`) |
| the Jacobi/gather deviation from d3's Gauss–Seidel (D10) | `force::barnes_hut::tests::a_step_is_jacobi_so_no_node_sees_another_nodes_move`; the stability fixture the phase was told must not be normalised away: `the_jacobi_link_stability_fixture_stays_finite_and_bounded_devil_c9` — measured, it settles, no oscillation |
| `forceatlas2`: networkx 3.6's function ported whole | `forceatlas2::tests::{a_three_node_path_lands_on_three_exactly_spaced_points, a_single_node_does_not_divide_by_zero, an_empty_graph_produces_empty_geometry}`; the differential is the `emit-fa2-1000` → `oracle-fa2-py` → `oracle-fa2` chain |
| the eigensolver cascade, dense tier | `linalg::dense_sym::tests::{cross_checks_tred2_tql2_on_a_path_including_eigenvectors, cross_checks_tred2_tql2_eigenvalues_on_a_degenerate_cycle_and_grid}`, `matches_the_hand_solved_two_by_two_generalized_eigenvalues` |
| LOBPCG tier, and its reported cap | `linalg::lobpcg::tests::{is_deterministic_run_twice, finds_the_smallest_nontrivial_path_eigenvalues_above_the_dense_limit, finds_the_smallest_nontrivial_cycle_eigenvalues_above_the_dense_limit, a_30_iteration_cap_hit_is_reported_not_hidden, a_non_positive_definite_gram_b_is_refused_not_miscomputed, measurement_lobpcg_iterations_and_timing}` |
| the spectra themselves, both tiers, against closed forms | `linalg::lobpcg::tests::path_cycle_star_complete_and_grid_spectra_match_closed_form`; `spectral::tests::complete_spectra_match_the_closed_form_dense_and_lobpcg`, `…::cycle_…`, `…::grid_…` |
| **degenerate eigenspaces pinned** (λ₂ = λ₃ on grids and trees) | `eigen_determinism_on_degenerate_grid_pins_basis_and_signs`, `eigen_determinism_run_twice_across_both_tiers`, and the row the phase prompt names: `cargo test -p graph-core eigen_determinism` → develop row `eigen-determinism`, **exit 0** |
| residual verification — never trust a solver's exit status | `residual_converged` / `converged` in `spectral.rs` and `pivot_mds.rs`; `nothing_solved_only_when_something_was_attempted_and_none_solved` |
| `spectral.rs` per component, single-node/two-node degenerate | `spectral::tests::end_to_end::{c_300_lays_out_as_a_circle_equal_radii_uniform_angles, path_lays_out_monotonically_along_the_first_axis, disconnected_graph_reports_only_attempted_components_and_places_every_node, is_deterministic_run_twice_across_both_tiers}` |
| `pivot_mds.rs`: farthest-point pivots, unreachable pivots zeroed | `pivot_mds::tests::{common_shapes_solve_with_the_expected_pivot_count}` + the `oracle-spectral` differential |
| the stress metric as a gate | `graph-cli stress --oracle d3` → `docs/measurements/phase06-stress.md`'s wiring section: 40 seeds, 39 correlated, median margin **+0.03515**, worst **−0.01633** against a floor of −0.05, 0 of 39 below it |
| the ceilings come from a command, not a document | `graph-cli bench --n 220,10000,100000` → `FORCE_CEILING = 100 000` (2 376 ms at 10 000, 57.8 s at 100 000) and `FA2_CEILING = 14 000` (13 861 ms at 10 000; `refused` at 100 000) |
| D2: no `mul_add` anywhere in `graph-core` | **No test.** The `no-mul-add` gate row is a grep, and it is currently red on comments (§3). |

## 6. Ponytail markers added

Five, on the five things the phase prompt names, each with its failing input and direction,
quoted from the row text `capabilities --json` returns and from
`docs/decisions/{eigensolver,phase06-*}.md`:

- **Force layout is chaotic** (`layout.force.barnes_hut`). The same graph with one node
  added is a different picture, not a perturbed one. Direction: cosmetic-but-surprising;
  escape hatch: a fixed seed.
- **Spectral in a degenerate eigenspace** (`layout.spectral`). Pinned by a fixed start
  vector orthogonal to the constant null vector, and by sign pinning at the largest-
  magnitude entry — ported from `networkx_layouts.py:57-66` and `:68-74`. Failing input:
  grids and trees, λ₂ = λ₃. A *different* fixed vector gives a different but equally valid
  layout.
- **Iterative eigensolver non-convergence** (`layout.spectral`). Failing input: a path-like
  component past `SPECTRAL_CEILING = 700` (spectral gap O(1/n²)); measured, `n = 800` is
  refused and `n = 4096` does not converge even at a 5 000-iteration cap. Direction:
  **a skipped component sits at the origin — visibly wrong, the dangerous direction.**
  Escape hatch: the residual is reported, and the run refuses rather than returning a
  random layout.
- **Pivot MDS** (`layout.mds.pivot`). `k = min(100, n)` pivots approximate the full distance
  matrix, so accuracy degrades as k/n shrinks; unreachable pivots (other components) are
  **zeroed**, which distorts cross-component geometry rather than failing.
- **Barnes–Hut θ** (`layout.force.barnes_hut`). θ trades accuracy for speed; a large θ
  visibly clumps distant nodes. **And the honest requirement the phase prompt put in this
  report by name:** d3's `forceManyBody` already uses a quadtree, so this is a
  **constant-factor** win (SoA memory, no GC, no JS object overhead, more ticks per frame),
  not an asymptotic one, and the justification is a measured number at N = 220 / 10k / 100k
  — which `docs/measurements/phase06-force.md` gives, in `bench`'s own table.

`layout.forceatlas2` carries its own: the dense `O(n²)` all-pairs repulsion is a property
of **the algorithm being ported** (networkx has no spatial approximation), so past
`FA2_CEILING = 14 000` the layout returns finite geometry and simply takes tens of seconds
— the caller applies a timeout, and there is no refusal and no trap.

## 7. What could not be verified

- **`GM_MUTATE_FORCE_THETA` is inert. Verified as inert** — this is the one item in this
  report that is a *finding* rather than an absence. Barnes–Hut's negative control must be
  repaired before its `gated`-adjacent claims can be defended. Recommended: either the
  knob reaches the Barnes–Hut stage's run, or the control is deleted rather than left
  green-but-useless — a row that passes when it should fail is worse than a missing row.
- **The FA2 coordinate differential. FAIL, and the fix is not applied.** §3. The
  placeholder `CEILING = 1e-3` (`oracle_python/fa2.rs:23`) is on this tree. Per
  `RESUME.md` item 4, `layout.forceatlas2` (and Barnes–Hut, on `stress`) must stay
  `Implemented` until the metric is replaced — which is where they are, so **no wrong claim
  is being made**; but the row is red and the task is open.
- **`cargo mutants` over either diff. NOT RUN** — the orchestrator's row.
- **These branches' own gates. UNKNOWN.** Neither `p6e` nor `p6f` is in this worktree.
- **Per-stage counts at 1000 seeds. UNKNOWN** (§4).
- **wasm32 vs TypeScript speed at N = 220. NOT MEASURED**, and correctly not claimed by the
  phase's own measurement file (§3). The prompt asks for it and the answer on record is
  "not measured"; `RESUME.md` item 7 puts the crossover in Phase 9, whose
  `docs/measurements/phase09-crossover.md` exists but was not read for this report.
- **The dense-tier timings in `docs/measurements/phase06-eigen.md` are debug-build numbers
  on a different host** (that file's own header says so, and cites `/home/user/gr`). The
  ceiling table in the same file is release-build on this host. They are not comparable and
  the file says so; both are quoted here as the file states them, not re-measured.
- **`registry/force.rs` still names the removed `examples/force_dump.rs` and a
  non-existent oracle-layouts d3-force arm** (`RESUME.md` item 5). Recorded here because
  the phase's ledger text is what a reader follows; not checked in this session.
- **The `docs/measurements/phase06-{force,stress,eigen}.md` files still show the old
  `--nodes` bench form and `/home/user/...` paths** (`RESUME.md` item 5). The bench CLI's
  unified form is `--layout X --n 220,10000,100000`; the `--nodes` form in those files is
  stale, and `phase06-eigen.md`'s "gate model" command line is wrong as written. Quoted
  numbers in this report are attributed to the file, not to a re-run.

## 8. Stop-and-ask items

- *"4-way hash equality fails and D1–D9 are all satisfied → stop and report."* **Did not
  trigger** (§4). Twelve stages 4-way equal at 8 seeds; the 12-stage run exits 0 at 1000
  seeds. No tolerance was introduced anywhere in the phase.
- *"FA2 or Yifan Hu reference unavailable → stop (rule 0.6). Do not improvise a force
  formulation."* **Both answered, differently and correctly.** FA2: SciGraphs delegates
  to networkx/`fa2`, and networkx 3.6's `forceatlas2_layout` **is** on disk at
  `/goinfre/dlesieur/refs/networkx-3.6`; the phase ports that function whole, at its own
  defaults, and says so in the row's `oracle`. Yifan Hu: Graphviz `sfdp` is not on disk and
  the corpus's own `YIFAN_HU` is a *different algorithm*, so the phase did **not**
  improvise a multilevel coarsening over its own force code under a borrowed name. It stays
  absent, with the reason in `docs/measurements/phase06-force.md`. The prompt calls that "a
  valid and honest phase outcome" and so does this report.
- *"The eigensolver cannot be made bit-identical → stop, present the measurement, and get
  the per-platform exception approved explicitly. Do not take that decision alone."* **The
  measurement says it is bit-identical, so no exception is needed and none was taken.**
  `docs/decisions/eigensolver.md` was written before any `linalg` code (C4/rule 0.6) and
  records the cascade, the reasons, and the measured residuals; `layout.spectral` therefore
  carries **no** per-platform caveat in `degradation`, which is the correct outcome and not
  a silent omission.
- *"WASM is slower than the TypeScript at N = 220 → report it, do not hide it."* **Not
  measured, and not hidden**: the phase's own measurement file declines to claim it (§3).
  Reported as an open item rather than as a pass.
- **New, and open:** the inert `GM_MUTATE_FORCE_THETA` control (§3, §7). Recommended: fix
  the knob before the next develop gate; a control that cannot fail is the one class of
  green this project treats as worse than a red.
