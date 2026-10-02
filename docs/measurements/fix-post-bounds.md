# fix-post-bounds — FDEB `iterations` ceiling, and the ink walk's clip to the box

Source: `docs/measurements/fix-post-bundle.md` §"Decisions needed" items 7 and 8. Brief:
`prompts/jobs/fix-post-bounds.md` over `prompts/jobs/fix-common.md`. Paths under
`crates/graph-core/src/`. Branch `fix-post-bounds`, worktree `$GM_SCRATCH/wt/fix-post-bounds`,
2026-10-02.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| PB-7 | MAJOR | fixed | `post::fdeb::tests::run::an_iteration_count_past_the_references_own_ceiling_is_refused_rather_than_run` | `post/fdeb/limits.rs:22-32`, `post/fdeb.rs:240-262` |
| PB-8 | MAJOR | fixed | `post::ink::tests::an_edge_point_far_outside_a_one_by_one_box_finishes_and_marks_a_bounded_raster`, `a_segment_entirely_outside_the_box_marks_nothing_and_is_still_measured`, `a_segment_inside_the_box_marks_exactly_the_cells_the_unclipped_walk_marked`, `post::ink::clip::tests::*` | `post/ink/clip.rs:11-42`, `post/ink.rs:12-21`, `:120-159` |

## Notes per row

- **PB-7.** The reference panel's own bound, found in the pinned reference checkout:
  `edge_bundle_iterations` is `IntProperty(default=6, min=1, max=20)` at
  `SciGraphs/SciGraphs/properties/edge_style_properties.py:126-133` (`max=20` on line 131).
  The panel expresses every ceiling as a `max=` kwarg — `edge_segments` `max=32` at `:83`,
  `edge_fdeb_cycles` `max=10` at `:295` — so the two ceilings `MAX_SEGMENTS` and `MAX_CYCLES`
  already carried were read off it the same way, and the third was simply not read.
  `MAX_ITERATIONS: u32 = 20` now sits beside the other two in `post/fdeb/limits.rs` with the
  same `Ponytail:` shape, and `check` refuses past it with `StageError::Param { name:
  "iterations" }` at `post/fdeb.rs:254-259`.
  The three ceilings moved into a child module because `fdeb.rs` was already at the 300-line
  house cap; `pub use limits::{MAX_CYCLES, MAX_ITERATIONS, MAX_SEGMENTS}` keeps the published
  paths (`graph_core::post::fdeb::MAX_*`) unchanged, so this is not a surface change.
  The registered default is the `BUNDLED_DENSE` preset's `iterations: 8`
  (`post/fdeb.rs:161`), under the ceiling: the test asserts it, and the `post.bundle.fdeb`
  hashgate row is unchanged (item below).
  RED (the ceiling did not exist, so the test named the value 20/21 first):

  ```
  test post::fdeb::tests::run::an_iteration_count_past_the_references_own_ceiling_is_refused_rather_than_run ... FAILED
  thread '...' panicked at crates/graph-core/src/post/fdeb/tests/run.rs:190:6:
  refused: Bundled { geometry: Geometry { nodes: Point { x: [], y: [] }, edges: Polyline(Paths { offsets: [0], pts: [] }), notes: [], z: None }, pairs: 0, unbundled: 0 }
  test result: FAILED. 3 passed; 2 failed
  ```

  `expect_err` on a `u32::MAX`-able knob is the shape of the defect: `bundle` returned `Ok`
  with 21 iterations, which is `21` full passes over every subdivision point's surviving
  pair row.

- **PB-8.** `Raster::steps` is `ceil(max(|dx|, |dy|) / (0.5 / max scale))`, so the walk length
  was the *segment's*, not the box's: the only thing bounding it was how far the caller's
  coordinates reached. Clip is Liang–Barsky in its own module, `post/ink/clip.rs`, returning
  the two fractions `(t0, t1)` and the two clipped ends — pure numbers, no geometry type, no
  cell, no state — with three unit tests of its own (boundary-only segment kept whole,
  outside is `None` and a crossing is cut at both ends, and the `p == 0` parallel branch that
  would divide by zero). `Raster::mark` now walks the clipped part only and returns the same
  `length` as before: `length` is computed from the unclipped endpoints, before the clip.
  `Raster::hi` is the box's far corner, `lo + INK_RESOLUTION / scale`, which for a degenerate
  axis (scale `1`) is exactly as far as the axis's cells index.
  What the part outside the box used to mark: a stroke leaving the drawing marked the border
  cells its clamped cell index landed in — `cell` (unchanged, `post/ink.rs:196`) still clamps,
  and still must, because a curved path may reach left of the box. The module doc says so
  (`post/ink.rs:12-21`): the walk no longer marks on behalf of the part outside the box,
  because a cell count is a claim about the drawing and a point at 1e7 is not in the drawing.

  RED (two tests, both failing on the unclipped walk):

  ```
  test post::ink::tests::a_segment_entirely_outside_the_box_marks_nothing_and_is_still_measured ... FAILED
  thread '...' panicked at crates/graph-core/src/post/ink/tests.rs:39:5:
  the walk was bounded by the box alone
  test post::ink::tests::an_edge_point_far_outside_a_one_by_one_box_finishes_and_marks_a_bounded_raster has been running for over 60 seconds
  ```

  The second RED is the finding's own figure: a box of 1 with an edge point at `1e7` is
  `1e7 / (0.5 / 128) = 2.56e9` half-cell steps per segment, and it ran for over 60 seconds
  before the clip. After the clip the same fixture is two diagonals of a 1x1 box and finishes
  in the 0.00 s the whole `post::ink` suite now reports.

  The third test is the anti-regression half the brief asks for: a segment inside the box is
  walked both ways — through `mark` and through the old whole-segment walk, kept in the test
  file as `unclipped` — and the marked cell indices are compared index for index. Clipping a
  segment that never left the box is the identity, which is what that pins.

## Pinned ink figures

`git grep -n 'cells' -- docs/measurements/phase08-ink.md crates/` for ink: the pinned figures
are prose in `docs/measurements/phase08-ink.md` (`:13` 13 104, `:22-24` 8871 / 8395 / 2261 /
211.029 / 212.470 / 371.313, `:26` 6764 / 4648 / 8232, `:34-41` the sweep's MINGLE ink
reductions) and one assertion, `post/ink/tests.rs:29` (`cells == 1` on a degenerate box,
unchanged and still passing). No `post::*` test asserts an ink cell count in absolute terms;
`post/tests.rs:263` and `ink_cmd.rs:73` both compare *relative* (`after.cells < before.cells`).
No number in `phase08-ink.md` moved, and none was re-pinned — see the commands below: the
`hairball` fixture reproduces 8871 / 8395 / 2261 and the grid row 6764 / 4648 / 8232 as
printed.

The counts do not move, and the reason is that both walks step at half a cell of the
coarser axis — `steps` divides by `0.5 / max(scale)` in both cases — so the clipped walk
lands in the same cells as the unclipped one along the part of the segment that stayed in
the box. A cell can differ only where the walk passes within rounding of a cell corner, and
the pinned fixtures are nowhere near that: the figures come back identical (8871 / 8395 /
2261 circular, 6764 / 4648 / 8232 grid, and every length to the printed digit). What the
clip removes is exactly the part of the walk that had no cell of its own: a point outside the
box has no cell index of its own, and the unclipped walk spent half a cell of travel per unit
marking the clamped border cell it kept landing in.

## Commands (worktree root, `CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4`)

```
scripts/orch/gr cargo fmt --all --check                                    exit 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings      exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.83s
timeout 2400 scripts/orch/gr -e CARGO_BUILD_JOBS=4 -e RUST_TEST_THREADS=4 cargo test --workspace --no-fail-fast   exit 0
    1678 passed, 0 failed, 12 ignored over 21 binaries (graph-core lib: 1079 passed, 6 ignored)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown  exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.63s
scripts/orch/gr cargo build --release -p graph-cli                         exit 0
    Finished `release` profile [optimized] target(s) in 12.56s
scripts/orch/gr cargo run -q --release -p graph-cli -- ink --fixture hairball   exit 0
    straight  layout=circular.radial nodes=42 edges=178 cells=8871 length=211.029 resolution=128
    post.bundle.fdeb  cells=8395 length=212.470 ink_reduction=5.37% pairs=994 unbundled=61 millis=0.5
    post.bundle.mingle  cells=2261 length=371.313 ink_reduction=74.51% pairs=133 unbundled=31 millis=5.6
scripts/orch/gr cargo run -q --release -p graph-cli -- ink --fixture hairball --layout grid   exit 1
    straight  layout=grid nodes=42 edges=178 cells=6764 length=527.531 resolution=128
    post.bundle.fdeb  cells=8232 length=544.172 ink_reduction=-21.70% pairs=224 unbundled=57 millis=0.7
    post.bundle.mingle  cells=4648 length=732.217 ink_reduction=31.28% pairs=131 unbundled=21 millis=11.1
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8            exit 0
  post.bundle.fdeb: 4-way equal on 8/8 seeds
  4-way equal on 8/8 seeds
  PASS
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8   exit 1
  4-way equal on 0/8 seeds
  FAIL: 8 of 8 seeds diverge
timeout 2400 scripts/scigraphs-conformance.sh                              exit 0
  CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15
  PASS
```

Conformance: 32 rows, 31 `ok`, none moved. `IGRAPH_KK` is 23/24 because the reference's own
dispatcher raises on a fixture of its own (`ValueError: IGRAPH_KK produced 9 non-finite
coordinate(s)`), which is the reference's row, not a row of ours; the script still exits 0.
This job touched no layout.

The `ink --layout grid` run exits 1 and did so before this job too: FDEB *raises* the cell
count on a lattice layout (`-21.70%`), which `docs/measurements/phase08-ink.md:26-30` records
as expected and explains. The cells it prints (6764 / 4648 / 8232) are the pinned ones.

The two `self-check FAILED` lines in the workspace log come from the negative controls
`bench::tests::harness::a_broken_copy_of_*`, which pass.

The `post::ink` suite's own timing is the PB-8 measurement: 7 tests (4 `post::ink`, 3
`post::ink::clip`) finish in the 0.00 s the log reports, where the single 1e7 fixture took
over 60 seconds before the clip.

## Decisions taken

- **`MAX_ITERATIONS` = the panel's `max=20`, not the panel's `default=6`.** The registered
  default here is the `BUNDLED_DENSE` preset's 8, which the panel's own slider would also
  permit, and the brief requires the registered default not to move. A ceiling of 6 would
  have refused the crate's own default.
- **The clip is exact, not a heuristic, so it carries no `Ponytail:` line.** Liang–Barsky is
  the exact intersection of a segment and a box; there is no failing input to name. The
  residual approximation is in `Raster::hi` (a division standing for the span `high - low`),
  and that is where the `Ponytail (far corner)` line sits.
- **The three ceilings moved to `post/fdeb/limits.rs`.** `fdeb.rs` was at exactly 300 lines,
  the house cap, and a third ceiling plus a doc mention does not fit. `pub use` keeps the
  published `MAX_*` paths byte-identical, so nothing downstream needs to move.

## Decisions needed

None. Item 8 of `fix-post-bundle.md` said "a fix belongs with whoever owns the ink input
contract"; this job's brief claims it, and the fix needs no change to that contract: the
clip is inside the raster, which is what owns it.
