# fix-post-routed: review repairs for routed edges and the grid index

Job brief: `prompts/jobs/fix-post-routed.md`, under `prompts/jobs/fix-common.md`. The findings are
in `docs/reviews/review-core-post.md`. Paths are relative to `crates/graph-core/src/`. Date:
2026-10-02, branch `fix-post-routed`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| R1 | BLOCKER | fixed | `routed::tests::stencil::a_border_cells_steps_cost_their_stencil_lengths`, `..::a_route_along_the_bottom_border_stays_on_it` | `post/routed/csr.rs:99-111`, `:145-149`; `post/routed/petgraph_impls.rs:131-140` |
| R2 | BLOCKER | fixed | `grid_index::tests::limits::a_resolution_whose_cell_count_overflows_u32_is_refused`, `..::a_margin_past_half_of_u32_is_refused_rather_than_overflowed`, `..::the_ceiling_is_u32_max_over_8_cells` | `post/grid_index/build.rs:31-39` (ceiling, `Ponytail:`), `:46-63` |
| R8 | MAJOR | deferred (decision needed) | none: the impassable rule is pinned by `routed::tests::routes` | divergence documented at `post/routed.rs:55-63`; the row text is `capabilities/post.rs:53-60` |
| R9 | MAJOR | fixed (cap removed, not scaled) | `routed::tests::limits::a_route_longer_than_any_fixed_step_cap_is_still_routed` | `post/routed.rs:49-53` |
| R10 | MAJOR | fixed | `grid_index::tests::limits::a_zero_extent_axis_is_measured_over_the_references_floor_span`; `grid_index::tests::cells::a_single_point_is_gridded_over_the_references_floor_span` (re-pinned, see below) | `post/grid_index/build.rs:20-29`, `:51` |
| R11 | MAJOR | fixed | `routed::tests::adapter::edge_references_list_every_edge_once_in_cell_order` | `post/routed/petgraph_impls.rs:147-171`, `:205-218` |
| M11 | MINOR | fixed | `routed::measure::the_lattice_jitter_wraps_rather_than_overflowing_in_a_debug_build` | `post/routed/measure.rs:43-44` |
| M12 | MINOR | fixed (no test: not observable, as the review says) | none | `post/routed/measure.rs:33` |
| M13 | MINOR | fixed | `routed::measure::gap_measurement` (ignored; release) asserts that the hole equals the gap | `post/routed/measure.rs:189-250` |
| M14 | MINOR | fixed | `routed::measure::resolution_measurement` (ignored; release) prints `240 nodes, 362 edges` | `post/routed/measure.rs:115` |
| M15 | MINOR | fixed | `routed::tests::limits::route_over_refuses_a_non_finite_node_rather_than_emitting_it`, `..::route_over_refuses_an_endpoint_past_the_nodes_rather_than_panicking`, `..::route_over_refuses_a_grid_built_over_other_nodes` | `post/routed.rs:185-186`, `:198-222` |
| M16 | MINOR | fixed | `routed::tests::adapter::an_empty_grids_csr_answers_a_cell_query_without_dividing_by_zero` | `post/routed/csr.rs:125-130` |
| M17 | MINOR | fixed | `routed::tests::adapter::a_cell_out_of_range_has_no_neighbours_and_no_edges` | `post/routed/csr.rs:134-140` |
| M18 | MINOR | fixed-by R1 (the overflow is bounded by R2's ceiling) | `routed::tests::adapter::an_edge_id_names_its_source_and_its_stencil_slot` | `post/routed/petgraph_impls.rs:129-135` |
| M19 | MINOR | doc-only (the doc is fixed and the row is now borrowed, from R1/R11's rewrite) | none | `post/routed/petgraph_impls.rs:104-113`; `post/routed/csr.rs:134` |
| M20 | MINOR | doc-only | none | `post/routed/csr.rs:45-56` |
| M21 | MINOR | fixed | `grid_index::tests::limits::a_negative_size_is_refused_rather_than_marking_nothing`, `..::a_negative_or_non_finite_clearance_is_refused` | `post/grid_index/build.rs:66-84`, `:115-143` |
| M22 | MINOR | doc-only; the row text is deferred to the capabilities owner | none | `post/grid_index.rs:23-26` |
| U6 | unverified | fixed-by R2 (ids fit u32 once cells ≤ `u32::MAX / 8`) | `grid_index::tests::limits::the_ceiling_is_u32_max_over_8_cells` | `post/grid_index/build.rs:39` |
| U7 | unverified | false | `routed::tests::ties::the_tie_break_is_by_cell_index_and_not_by_the_order_the_row_is_walked`, `..::a_symmetric_wall_makes_two_exactly_equal_detours_and_the_lower_cell_route_wins` (4/4 ties tests pass before and after) | `post/routed/tests/ties.rs:53`, `:135` |
| U8 | unverified | promoted, deferred (decision needed) | `routed::measure::routing_measurement` (ignored; release): 5 000 nodes take 16 971.653 ms | `capabilities/post.rs:36-44` |
| U17 | unverified | false | none: no gate pins a route output | see U17 below |
| U18 | unverified | doc-only (decision needed) | none | `post/grid_index.rs:23-27` |

## Evidence

**R1, M16, M17, M18, R11, RED**
- Command: `cargo test -p graph-core --lib post::routed::tests`, before the fix.
- Result: 13 passed, 6 failed:
  - `stencil::a_border_cells_steps_cost_their_stencil_lengths`: cell 0 → 3, `left: (-1, -1) right: (0, 1)`.
  - `stencil::a_route_along_the_bottom_border_stays_on_it`: `left: [4, 11, 10, 1, 0] right: [4, 3, 2, 1, 0]`. The border route used to leave the border.
  - `adapter::an_empty_grids_csr_...`: panicked at `csr.rs:109:10`, divide by zero.
  - `adapter::a_cell_out_of_range_...`: panicked at `csr.rs:58:69`, index out of range.
  - `adapter::edge_references_...`: `left: 0 right: 40`.
  - `adapter::an_edge_id_...`: id 0, `left: (-1, -1) right: (0, 1)`.

**GREEN:** 6 passed. All 19 routed tests pass.

**The routed golden before and after R1:** `routed::tests::ties`, 4 passed before and 4 passed after. The equal-detour golden `[74, 63, 64, 53, 42, 31, 32, 33, 46, 58, 70, 82]` is unchanged, because it runs over interior cells. R1 moves only routes that step from a border cell. The RED/GREEN pair above (`[4, 11, 10, 1, 0]` → `[4, 3, 2, 1, 0]`) is such a moved route.

**R2, R10, M21, RED:** `cargo test -p graph-core --lib post::grid_index::tests::limits`, 0 passed, 5 failed:
- `grid_index.rs:231:30` and `build.rs:36:26`: attempt to multiply with overflow.
- R10: `left: (132, 4) right: (132, 5)`.
- M21 (two tests): `expect_err` on `Ok`.
- `the_ceiling_is_u32_max_over_8_cells` did not compile, because `axes` returned no `Result`.

**GREEN:** 24 grid_index tests pass. Re-pin, caused by R10: the old `a_degenerate_layout_still_produces_one_cell_per_axis` pinned a single point at `(1, 1)`. The reference floors the span at 1e-9 (`routed.py:55`) and so divides it into `resolution` cells. The test now pins `(8, 8)` and 64 cells, and is renamed.

**R9, M15, RED:** `routed::tests::limits`, 0 passed, 4 failed:
- The serpentine: `limits.rs:42:5`, "a route exists through the gaps", which was a straight fallback through 127 walls.
- `routed.rs:220:43`: index out of bounds, `the len is 2 but the index is 2`.
- The NaN node: `Ok`, a NaN carried into the route.
- The mismatched grid: `Ok`.

**GREEN:** all post tests, `93 passed; 0 failed; 4 ignored`.

**M11, RED:** `measure.rs:42:19`, attempt to multiply with overflow (a debug build). **GREEN:** 1 passed.

**M13:** there is no separate RED run. The old sweep printed no hole column. The review derived holes of 3, 3 and 4 cells for gaps of 0, 1 and 2. The new sweep measures the hole in column 26 and asserts that it equals the gap.

### Release measurements

Command: `cargo test --release -p graph-core --lib post::routed::measure -- --ignored --nocapture`. Result: 3 passed in 30.70 s, run while three other jobs' cargo builds and tests were running.

```
gap sweep, resolution 32 over a span of 8 (cell = 0.25)
   8 cells      8 cells   0 fallbacks
   4 cells      4 cells   0
   2 cells      2 cells   0
   1 cells      1 cells   0
   0 cells      0 cells   0
post.route.grid — 240 nodes, 362 edges
 resolution 16: 302 fallbacks; 32, 64, 128, 256: 0
      n       m     cells   wall-clock
   2000    3075     17424    5834.137ms
   5000    7721     17424   16971.653ms
```

### U17 and U8

**U17.** No gate pins a route output, so no layout can pin the old R1 or R10 behaviour:
- `hashgate` checks that four arms are equal. It holds no golden.
- `scigraphs-conformance` covers only layouts, and has no post rows.
- `fixtures/post/obstacles.json` is referenced by no crate and no harness (`git grep obstacles.json`).

**U8.**
- `capabilities/post.rs:40-41` says 5 000 nodes and 7 721 edges is "the largest input measured inside a 10-second budget".
- `docs/measurements/phase08-routing.md:36` records 15 566.2 ms for that input. This run measured 16 971.653 ms.
- The largest input measured under 10 s is 2 000 nodes, at 5 834 ms here and 5 890 ms in phase 8.

## Decisions needed

Each item below waits on a path this job does not own, or on a behaviour choice.

- **R8.** Choose one of two fixes:
  - Match the reference: a finite `density_cost`, with `avoid` defaulting to 0. This moves routes, and `routed::tests::routes` pins the impassable rule.
  - Keep the obstacles and restate the oracle text of row `post.route.grid` (`capabilities/post.rs:53-60`) to name the divergence.
- **M22.** The same row text should say the grid is sized on footprints, not on coordinates.
- **U8.** `ROUTE_CEILING = 5_000` contradicts its own 10 s claim. Either lower it to 2 000, or restate the budget (`capabilities/post.rs:36-44`).
- **U18.** Decide whether to raise a `resolution` below 8 to 8, as the reference does. It is documented, not changed.
- **R9.** This deviates from the review's proposal, which was to scale the cap with `min(4 · max(nx, ny), 4096)`. That cap fails the 33 000-cell serpentine, which the reference's own cap would fail sooner. The trace descends strictly, so no cap is needed.
- **R2.** The new refusal is a parameter refusal on `resolution` × `margin`, independent of the node count. `ROUTE_DEGRADES` ("never refuses", `capabilities/post.rs:46-50`) could say so.

### Resolved after the job (branch `post-rowtext`, 2026-10-02)

| id | decision | where |
|---|---|---|
| R8 | Keep the obstacles; the row's oracle text names the divergence | `capabilities/post.rs`, row `post.route.grid` |
| M22 | The row's oracle text says the grid is sized on footprints | same |
| U18 | Keep `resolution` < 8 as given; the row's oracle text says so | same |
| U8 | Ceiling stays 5 000; the stated budget becomes 20 s (15 566.2 ms in phase 8, 16 971.653 ms here), matching `phase08-routing.md:67` | `capabilities/post.rs:34-46` |
| R2 | `ROUTE_DEGRADES` names the one refusal: a parameter refusal on `resolution` × `margin`, independent of the node count | `capabilities/post.rs` |

## Commands

All commands ran in `/home/dlesieur/goinfre/wt/fix-post-routed`.

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s`` |
| `timeout 2400 scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 20 test binaries: 1 657 passed, 0 failed, 12 ignored |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.87s`` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` / `PASS` (`post.route.grid: 4-way equal on 8/8 seeds`) |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 | `FAIL: 8 of 8 seeds diverge` |
| `scripts/orch/gr cargo build --release -p graph-cli` | 0 | ``Finished `release` profile [optimized] target(s) in 14.73s`` |
| `scripts/scigraphs-conformance.sh` | 0 | `CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15` / `PASS`; every row ok, none moved (the script has no post rows) |

The first conformance run exited 1 before its judge ran. The cause was
`target/scigraphs-conformance-judge.log: Permission denied`: Docker had created this worktree's
`target/` as root. After `scripts/orch/gr chown <uid>:<gid> /w/target`, which changes only the
top directory, the rerun above exited 0. Only the environment changed, not the code.
