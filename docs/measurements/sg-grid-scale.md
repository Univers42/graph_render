# sg-grid-scale — `GRID` reaches `f32` 1020/1020

Row `GRID`, cause `convention`, SciGraphs `_grid_layout` against graph-core's registered
`layout.grid`. The repair is repair 1 of `scigraphs-conformance.md`.

## The gap

`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:11-20`:

```python
grid_size = int(np.ceil(np.sqrt(num_nodes)))
for i in range(num_nodes):
    x = (i % grid_size) * scale / grid_size
    y = (i // grid_size) * scale / grid_size
```

Two differences from `grid.rs`, and they are the same difference twice: the first cell is at
**the origin**, where the registered lattice is centred, and the pitch is **`scale / grid_size`**,
where the registered pitch is `GridParams::spacing`, default `1.0`. Same lattice, different
convention — the matrix already said so (`same shape … only scale and origin differ`, Procrustes
median 5.5e-32).

The order matters as much as the two numbers. `basic.py:16` is a Python `int * float` then
`/ int`, so both operations are binary64 and the snapshot narrows once. A `f32` pitch cannot
reproduce that: `3 · fl32(5/9) = 1.6666667461395264` against `fl32(15/9) = 1.6666666269302368`,
a whole ULP. `Grid::run_scaled` therefore multiplies and divides in `f64` and narrows once —
`a_f32_pitch_would_not_have_reached_these_bits` in `layout/grid/scaled.rs` is the negative
control that fails if anyone folds it back into the `f32` kernel.

**`GridParams` gained no field.** Eight call sites in five files build it by struct literal
without `..Default::default()`, and the registered default's bytes are a snapshot hash, so the
parameterised entry point is a separate `impl Grid` fn — the `sfdp::run_seeded` shape — and
`G_GRID_SCALE` closes.

## The change

| file | what |
|---|---|
| `crates/graph-core/src/layout/grid/scaled.rs` | new: `Grid::run_scaled`, the `f64` SciGraphs kernel, its tests |
| `crates/graph-core/src/layout/grid.rs` | `mod scaled;` and a module-doc pointer; no code change |
| `crates/graph-cli/.../conformance/motor.rs` | a `layout.grid` arm calling `Grid::run_scaled(t, SCALE, &Serial, 1)` |
| `crates/graph-cli/.../conformance/gaps.rs` | `G_GRID_SCALE` deleted; `G_GRID_ITER`'s `at` re-pointed at the moved `run_with` |
| `crates/graph-cli/.../conformance/rows.rs` | `GRID`'s `gaps` is now `[G_GRID_ITER]` |
| `crates/graph-cli/.../baseline/table/basic.rs` | the `GRID` row re-pinned from the proposal file |

## Before / after

| | f64 k/1020 | f32 k/1020 | max ULP | max gap | Proc med | tier | cause |
|---|--:|--:|--:|--:|--:|---|---|
| before | 342 | 342 | 9.22e+18 | 4 | 5.5e-32 | `bitwise` | `convention` |
| after | 842 | **1020** | 2.39e+08 | 2.12e-07 | 3.39e-32 | `tolerance` | `arithmetic` |

`f64` stops at 842/1020, not 1020/1020, and cannot go further: the reference's own value is a
binary64 fraction that the `f32` `Geometry` cannot hold, so the two arms differ by the one
narrowing. That is the `tolerance` tier and the `arithmetic` cause — the same position
`SPHERE`, `HELIX` and `HIERARCHICAL_3D` are in — not a remaining `convention` gap.
`G_GRID_ITER` stays: a closed placement still has no iteration to bound.

## Commands

`scripts/orch/gr cargo test -p graph-core --lib layout::grid` — RED, the pinned `n = 5` case on
the registered kernel:

```
test layout::grid::tests::the_scigraphs_placement_is_not_the_registered_lattice ... FAILED
  left: [(-1.6666666, -0.8333333), (0.0, -0.8333333), (1.6666666, -0.8333333), (-1.6666666, 0.8333333), (0.0, 0.8333333)]
 right: [(0.0, 0.0), (1.6666666, 0.0), (3.3333333, 0.0), (0.0, 1.6666666), (1.6666666, 1.6666666)]
test result: FAILED. 0 passed; 1 failed
```

`scripts/orch/gr cargo test -p graph-core --lib layout::grid` — GREEN, the same expectation against
`Grid::run_scaled`:

```
running 10 tests
test layout::grid::scaled::tests::a_f32_pitch_would_not_have_reached_these_bits ... ok
test layout::grid::scaled::tests::a_scale_that_is_not_finite_and_positive_is_refused ... ok
test layout::grid::scaled::tests::every_node_matches_the_reference_formula ... ok
test layout::grid::scaled::tests::the_gather_is_the_same_bytes_at_every_worker_count ... ok
test layout::grid::scaled::tests::the_placement_is_the_reference_one_at_scale_five ... ok
test layout::grid::tests::the_lattice_is_the_same_bytes_at_every_worker_count ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 1045 filtered out
```

`scripts/scigraphs-conformance.sh` — exit 0 with the re-pinned row, no other row moved:

```
  GRID: ok — 842 f64, 1020 f32 of 1020 coordinates, median 3.391e-32 <= 1.000e-31
```

`scripts/scigraphs-conformance.sh --break` — exit 1:

```
  SPRING_3D: FAIL — motor bytes are not the pinned ones
```

`scripts/orch/gr cargo run --release -p graph-cli -- hashgate --seeds 8` — exit 0;
`GM_MUTATE_REFERENCE_DEGREE=9` on the same command — exit 1:

```
FAIL: 8 of 8 seeds diverge
```

### The five review minors, 2026-10-03

`graph-render-3e`'s review approved landing with five minors; this is the run that closed them.
No behaviour changed except the one the review allowed: **`scaled_cells` now refuses a scale
whose widest coordinate does not fit `f32`.**

| minor | what closed it |
|---|---|
| `a_f32_pitch_would_not_have_reached_these_bits` never ran the kernel | it calls `layout(65, 5.0)` now and asserts the gather's own bits *before* the `f32`-pitch comparison, so a kernel folded onto an `f32` pitch fails it instead of two literals being compared. `every_node_matches_the_reference_formula` at n=601 stays the real guard |
| "refuses instead of writing a non-finite coordinate" was false | **measured, not assumed**: on this toolchain `5e38_f64 as f32` is `inf`, not `f32::MAX` — the cast does not saturate here — so a finite `1e39` really did put `inf` in a `Geometry`. `scaled_cells` refuses when `(cols - 1) * scale / cols > f32::MAX`, with `a_scale_that_does_not_fit_the_f32_range_is_refused` at `1e39` and `the_largest_scale_the_f32_range_holds_is_placed` on the boundary |
| `G_GRID_ITER`'s `at` named the wrong line | `grid.rs:101` → `grid.rs:100`, the `pub fn run_with(` line, checked on this tree |
| `scigraphs-conformance.md` was stale for `GRID` | the `arithmetic` and `convention` paragraphs, the `f32`-identical count and the "exactly four ids get an override" count |
| no fmt/clippy/test lines in this file | below |

`scripts/orch/gr cargo test -p graph-core --lib layout::grid` — 12 passed, 0 failed:

```
test layout::grid::scaled::tests::a_f32_pitch_would_not_have_reached_these_bits ... ok
test layout::grid::scaled::tests::a_scale_that_does_not_fit_the_f32_range_is_refused ... ok
test layout::grid::scaled::tests::a_scale_that_is_not_finite_and_positive_is_refused ... ok
test layout::grid::scaled::tests::the_largest_scale_the_f32_range_holds_is_placed ... ok
test layout::grid::scaled::tests::the_placement_is_the_reference_one_at_scale_five ... ok
test layout::grid::tests::the_lattice_is_the_same_bytes_at_every_worker_count ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 1234 filtered out
```

`scripts/orch/gr cargo fmt --all --check` — exit 0, no diff.
`scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` — exit 0, no warning.

`scripts/scigraphs-conformance.sh` — exit 0, `GRID` unmoved and no other row moved:

```
  GRID: ok — 842 f64, 1020 f32 of 1020 coordinates, median 3.391e-32 <= 1.000e-31
```

`scripts/scigraphs-conformance.sh --break` — exit 1, on the pinned row and no other:

```
  SPRING_3D: FAIL — motor bytes are not the pinned ones (sha 0a77181948f9232d3b077d82b8f573a150df0a81416660caa655e211396183c1)
scigraphs-conformance: --break caught: SPRING_3D
```

### The one red row in the workspace test run, and it is not this file's

`scripts/orch/gr cargo test --workspace --no-fail-fast` — exit 101, **1237 passed; 1 failed**:

```
---- layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them stdout ----
thread '...' panicked at crates/graph-core/src/layout/graphviz/dot/oracle_probe.rs:58:9:
/w/crates/graph-core/../../target/probe/rank1000.txt is missing; see this module's doc for the command that writes it
```

That module's own doc says a missing probe file should make the tests that read it **skip**
rather than fail — "a missing measurement file is not a wrong answer" — and `target/probe/`
does not exist in this worktree, nor does the `rank_oracle.py` its doc names, so the artifact
cannot be written here at all. It is a pre-existing red in `layout::graphviz::dot`, three
directories from anything this series touched. **Recorded, not fixed**: the fix is that module's,
and skipping on an absent file is a repair job, not one of these five minors.

### A stale `target/` turns this script red, and the way it did

An intermediate run of `scripts/scigraphs-conformance.sh` exited 1 on two rows,
`YIFAN_HU` and `GRAPHVIZ_SFDP`, both "reference bytes are not the pinned ones" with the same
sha. Those are the two rows `scigraphs-conformance.md` records as sharing **one** sfdp
reference, and the dir they read still held the bytes of an earlier run that was killed
mid-verdict. The Graphviz arm on its own reproduces the pinned sha `4c90e6c3…` three times
running, and the clean run of the whole script exits 0 with all 32 rows `ok`. The hazard is
the tree fingerprint, not the layouts: an edit between the emit and the verdict fails the run
with "fixtures, metrics and tree are not the same tree", which is what the killed first run
did.

### The pre-change baseline run

The first `scripts/scigraphs-conformance.sh` was launched in the background and the tree was
edited while it was still building, so its exit code (101) is the render step's compile of a
half-edited `motor.rs`, not a row verdict. Its `GRID` metrics are from the untouched motor and
match the committed matrix exactly — `bitwise_f32 342, bitwise_f64 342, max_gap 4.0,
max_ulp 9219368837186002034`, Procrustes median 8.86e-32 on `lesmis` — and are the "before" column
above. The untouched-tree exit 0 is the series' recorded baseline, not something this task ran
start to finish.
