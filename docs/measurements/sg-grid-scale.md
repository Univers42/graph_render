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

### The pre-change baseline run

The first `scripts/scigraphs-conformance.sh` was launched in the background and the tree was
edited while it was still building, so its exit code (101) is the render step's compile of a
half-edited `motor.rs`, not a row verdict. Its `GRID` metrics are from the untouched motor and
match the committed matrix exactly — `bitwise_f32 342, bitwise_f64 342, max_gap 4.0,
max_ulp 9219368837186002034`, Procrustes median 8.86e-32 on `lesmis` — and are the "before" column
above. The untouched-tree exit 0 is the series' recorded baseline, not something this task ran
start to finish.
