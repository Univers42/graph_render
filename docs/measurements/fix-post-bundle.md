# fix-post-bundle — review repairs: FDEB, MINGLE, edge styles, ink

Source: `docs/reviews/review-core-post.md`. Brief: `prompts/jobs/fix-post-bundle.md` over
`prompts/jobs/fix-common.md`. Paths below are under `crates/graph-core/src/`. Branch
`fix-post-bundle`, worktree `$GM_SCRATCH/wt/fix-post-bundle`, 2026-10-02.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| R3 | BLOCKER | fixed | `post::fdeb::tests::run::a_schedule_past_the_references_own_ceilings_is_refused_rather_than_overflowed` | `post/fdeb.rs:242`, `:250`, `:253-269` |
| R12 | MAJOR | fixed | `post::styles::tests::contract::a_self_loop_vertex_count_past_the_references_ceiling_is_refused` | `post/styles/params.rs:20`, `:105`; `post/styles/ledger.rs` COMPLEXITY |
| R13 | MAJOR | fixed | `post::styles::tests::contract::a_point_past_the_f32_range_is_refused_rather_than_written_as_infinity` | `post/styles.rs:244` |
| R14 | MAJOR | false | `post::styles::tests::shapes::a_chord_loops_only_at_exactly_zero_length_and_its_fan_gap_never_shrinks` | `post/styles/shapes.rs:20` |
| R15 | MAJOR | fixed | `post::mingle::tests::cost::the_declared_proximity_term_carries_the_row_sort_the_oracle_also_does` | `post/mingle.rs:102`, `:121` |
| R16 | MAJOR | fixed | `post::mingle::tests::cost::a_round_whose_first_pass_merges_nothing_stops_after_that_pass` | `post/mingle/run.rs:47` |
| R17 | MAJOR | fixed | `post::ink::tests::a_box_too_small_to_divide_is_one_cell_and_walks_in_bounded_steps` | `post/ink.rs:93`, `:122` |
| M1 | MINOR | fixed | `post::fdeb::tests::run::a_drawing_too_small_to_soften_still_bundles_to_finite_points` | `post/fdeb/points.rs:155` |
| M2 | MINOR | false | `post::fdeb::tests::terms::at_a_zero_threshold_a_pair_scoring_exactly_zero_is_kept_as_the_reference_keeps_it` | `post/fdeb/pairs.rs:13-16` |
| M3 | MINOR | doc-only | — | `post/fdeb.rs:114-116` |
| M4 | MINOR | fixed | the 14 `post::fdeb` tests, unchanged values | `post/fdeb.rs:217`, `post/fdeb/points.rs:38` |
| M5 | MINOR | doc-only | — | `post/fdeb.rs:14-22` |
| M6 | MINOR | doc-only | `post::fdeb::tests::terms::the_resample_lerp_is_the_references_algebra_and_not_its_bits` | `post/fdeb/points.rs:251-253` |
| M7 | MINOR | doc-only | — | `post/mingle/draw.rs:45-49` |
| M8 | MINOR | fixed | `post::ink` tests | `post/ink.rs:47`, `:162-165` |
| M9 | MINOR | fixed | the `post::mingle` tests, unchanged values | `post/mingle/pass.rs:11`, `:17`, `:35`, `:42` |
| M10 | MINOR | doc-only | — | `post/mod.rs:14` |
| M23 | MINOR | doc-only | — | `post/styles/params.rs:53` |
| M24 | MINOR | doc-only | `post::styles::tests::shapes::a_fanned_l_elbow_tilts_its_legs_where_a_fanned_z_keeps_its_right_angles` | `post/styles/shapes.rs:145`, `post/styles.rs:126` |
| M25 | MINOR | fixed | the 24 `post::styles` tests, unchanged values | `post/styles/shapes.rs:78`, `:153`, `perpendicular_shift` |
| M26 | MINOR | fixed | `post::styles::tests::contract::a_negative_parallel_offset_is_refused_rather_than_mirroring_the_fan` | `post/styles/params.rs:100` |

## Notes per row

- **R3.** The oracle is SciGraphs FDEB. Its own panel bounds both knobs: `edge_segments` max 32
  (`SciGraphs/properties/edge_style_properties.py:78-85`) and `edge_fdeb_cycles` max 10 (`:286-297`).
  `check()` refuses anything above with the existing `StageError::Param`. Each bound has a `Ponytail:`.
  The RED run panicked at `fdeb.rs:215`: `attempt to add with overflow`.
- **R12.** `self_loop_segments` is refused outside `3..=32`. The 32 is the reference's `edge_segments`
  ceiling (`edge_style_properties.py:78-85`), with a `Ponytail:` at `params.rs:20`. The ledger states
  `O(n + m x s), s <= 32`. RED: 100 000 000 was accepted.
- **R13.** The review's witness `(MAX, MAX) -> (0, -MAX)` stays finite: its y is 0.575·MAX, and the
  test checks it. The real witness is `(MAX, MAX) -> (-MAX, MAX)`, whose Bezier control reaches
  1.15·MAX. The cast gave `+inf`, not a saturated value (RED: `pts: [1.7014117e38, inf, ...]`).
  `build()` now refuses with `NonFinite { column: "edge.pts" }`, the contract's column name.
- **R14, false.** The reference's tolerances are absolute in its mesh units:
  - `np.allclose(atol=1e-6)` at `edge_styles.py:458`;
  - `length < 1e-10` at `:117` and `:203`.

  Centres here carry no unit, so an epsilon would loop every edge of a drawing laid out small enough.
  The test pins the exact-zero convention (a 5e-7 chord is drawn; a 1e-11 chord keeps the full fan
  gap) and stays. The convention is documented at `shapes.rs:20`.
- **R15.** The oracle `mingle.py` sorts every row too. The declared complexity now reads `m^2 log m`
  (`mingle.rs:121`). The RED test reads the declared string.
- **R16.** RED: 8 scored passes where the reference runs 1. A pass that merges nothing leaves the state
  untouched, so stopping does not change the output (the hashgate row `post.bundle.mingle` is still
  4-way equal).
- **R17.** RED: 4294967295 steps for one box width. A non-finite scale now draws one cell.
  `steps()` bounds the walk.
- **M2, false.** The reference prunes with `cm >= thresh` (`fdeb.py:286`), so a pair scoring 0 is kept
  at threshold 0. The test pins that the drawing is identical at thresholds 0 and `f32::MIN_POSITIVE`.
- **M6.** The claim holds: the test shows a 1-ulp difference between `a + (b - a)·f` and
  `a·(1 - f) + b·f`. META's oracle row already disclaims bit agreement, so this row is a doc fix.
- **M24.** A fanned `L` moves its one bend, so its legs tilt; a fanned `Z` keeps its right angles. This
  is documented on `Style::Orthogonal` and in `orthogonal_row`, and the test pins both.
- **M25.** Two changes:
  - `Pt::quarter_turn` replaces `perp()` and the inline turn in `perpendicular_shift`;
  - `Pt::mid` replaces the inline midpoint.

  The only arithmetic difference is the sign of a zero where `dx = 0`. Every `post::styles` pin is
  unchanged, and the four `post.style.*` hashgate rows are 4-way equal.

## Unverified items in these paths

| id | verdict | evidence |
|---|---|---|
| U2 | false (out of path) | Node columns are refused non-finite at the snapshot boundary (D9). `ink()` is a measurement, not a column. |
| U3 | out of scope | The `offsets`/`pts` invariant is the contract's (`docs/contract/`), not this job's. |
| U4 | fixed-by R16 | A pass that merges nothing now ends the round. |
| U5 | false | `Geometry::with_edges` carries z (`layout/mod.rs:105-112`), pinned by `layout::tests::re_edging_carries_the_z_column_and_the_notes_untouched`. Bundling in xy matches `Paths.pts`, which is xy-only (`graph-contract geometry.rs`). |
| U14 | false | The reviewer's own verdict is "no finding": the loop row matches the reference (`edge_styles.py:302-307`). |
| U15 | false | Same n-gon, vertex 0 rotated 90°. The reference is declared non-oracle for styles (`styles.rs:5-7`). |
| U16 | false | A curve emits control points, which is what `EdgeGeometry::Curve{degree}` means (`graph-contract geometry.rs:146-151`). |
| U19 | false | Sound by inspection: for `p` in `1..=k-2`, `q = k - 1 - p` is also in `1..=k-2`, and every row has the same `per_edge`. |
| U20 | resolved | M6's test constructs the differing input. |
| U21 | partly fixed-by M4/M8 | `centres` is read once per pass. The `1e-12` guard and 2D `hypotf` duplication across fdeb and mingle is left; see decisions. |
| U22 | false | A documented divergence (`fdeb.rs:111-112`). |

## Commands (worktree root, `CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4`)

```
scripts/orch/gr cargo fmt --all --check                                    exit 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings      exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.52s
timeout 2400 scripts/orch/gr cargo test --workspace --no-fail-fast         exit 0
    1652 passed, 0 failed, 13 ignored over 22 binaries (graph-core lib: 1056 passed, 6 ignored)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown  exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.45s
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8            exit 0
  4-way equal on 8/8 seeds
PASS
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8   exit 1
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
scripts/orch/gr cargo build --release -p graph-cli                         exit 0
    Finished `release` profile [optimized] target(s) in 19.19s
timeout 2400 scripts/scigraphs-conformance.sh                              exit 0
  CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15
PASS
```

Conformance: all 32 rows ok (23 reference + 9 Graphviz), none moved. `GRAPHVIZ_FDP` carries its
standing "reference not pinned" note. This job touched no layout.

The two `self-check FAILED` lines in the workspace log come from the negative controls
`bench::tests::harness::a_broken_copy_of_*`, which pass.

## Decisions needed

1. **Module roots.** `post/styles.rs` and `post/mingle.rs` are the module roots of `post/styles/**` and
   `post/mingle/**`, and this job edited both. No other job's brief names them (`git grep` over
   `prompts/jobs/`).
2. **R12 and M26 refuse values the API used to accept:** `self_loop_segments > 32` and
   `parallel_offset < 0`. No caller in the repository sets either (`git grep`).
3. **R14:** keep exact zero, or adopt a unit-relative epsilon. The reference's are absolute.
4. **M2:** keep zero-scoring pairs at threshold 0, as the reference does. The pair count differs from the
   reference on zero-length edges, where its 0/0 is NaN; the drawing does not.
5. **M24:** keep the tilted fanned `L`, or refuse the fan for `L`.
6. **M8:** `post::ink::centres` stays public because it is published surface. Deleting it is a breaking
   change.
7. **FDEB `iterations`** (`fdeb.rs:142`) is still unbounded. The review did not name it, and the
   reference panel's bound for `bundle_iterations` was not checked here.
8. **ink's walk** is bounded per box width, but an interior point far outside the node box is not.
   Estimated, not run: a box of 1 with a point at 1e7 is about 2.56e9 steps. A fix belongs with whoever owns the ink input
   contract.
9. **U21:** the `1e-12` length guard and the 2D `hypotf` are duplicated across fdeb and mingle. Not
   extracted here: it would be a new shared primitive in `post/mod.rs` for two callers.
