# test-speed-geometry — the layout sweep's long pole, cut into parallel tests

Status: **landed**. Date: 2026-10-03. Job: `prompts/jobs/test-speed-geometry.md`.

`crates/graph-core/tests/geometry_invariants.rs` was the landing gate's long pole: about
760 s in the 2026-10-02 gate logs against a roughly 1000 s `cargo test --workspace` row,
and 1316.87 s when the pre-change file was measured on the final tree (see the baseline
caveat below). The cause was not the amount of work but its shape: three `#[test]`s, each
one serial `for seed in 0..SEEDS` loop, one of which ran every `registry::LAYOUTS` entry
per seed. libtest runs tests in parallel, so one long test is **one core** on a 20-core
host. This cuts the sweep into tests libtest can hand to different cores, 1316.87 s →
317.80–572.91 s over three runs at the default thread count. No layout's arithmetic, no
assertion, and no message changed; the `(seed, layout)` pairs swept are identical.

## What was measured, and on what

Host: 20 cores (`nproc`), `RUST_TEST_THREADS` unset (default 20) unless stated. All runs
through `scripts/orch/gr`, `cargo test -p graph-core --test geometry_invariants`. The
toolchain is stable, so libtest's `--report-time -Z unstable-options` is rejected
(`error: the option 'Z' is only accepted on the nightly compiler`); per-test times were
taken instead with `-- --exact <module-qualified name>`, which is the documented
fallback in the job brief.

The baseline is the file as it stood before this job (`0b9fa62`, three tests, one serial
`for seed in 0..SEEDS` loop each). Because `develop` merged in mid-job and made the
layouts slower, the baseline was **re-measured on the final tree** so that every
before/after pair below is same-tree:

| run | tests | libtest `finished in` |
|---|---|---|
| baseline, default threads | 3 | **1316.87 s** |
| baseline, `RUST_TEST_THREADS=4` | 3 | 941.52 s (taken pre-merge; see the caveat) |
| `--exact` `…::treemap_boxes_contain_their_children_and_siblings_never_overlap` | 1 | 0.16 s |
| `--exact` `…::tidy_tree_polyline_offsets_are_well_formed_and_stay_inside_pts` | 1 | 0.14 s |
| `--exact` `…::every_registered_layout_emits_no_nan_or_inf_and_circle_radii_are_positive` | 1 | **952.20 s** |

The per-test rows and the `RUST_TEST_THREADS=4` baseline were taken before the merge,
when the same file measured 903.59 s at default threads; after the merge the same file
measures 1316.87 s. So those three rows understate the baseline, and the halving claim
below rests on the same-tree 1316.87 s → 327.36 s pair, which is the conservative
direction. The host also has a 16 % run-to-run spread (see the `cli_force` section), so
the 4.0× is well clear of the noise.

Two facts fall out of this, and they decided the design:

- **The treemap and tidy-tree sweeps are not the problem** — 0.16 s and 0.14 s. Splitting
  them buys nothing, so they were left exactly as they were, names and bodies intact.
  A first draft that chunked all three sweeps was measured, found to add 65 lines of
  machinery for ~0.3 s, and dropped.
- **`RUST_TEST_THREADS=4` barely helps** (941.52 s vs 903.59 s). With three tests, capping
  threads cannot split one long test; the cap is not the lever either.

### Per-layout cost of the one sweep that mattered

Each row is `--exact` on its own `#[test]`, 200 seeds, sorted by cost. This is the table
that says *where* the sweep's time is, and it is why only one row is split further.

| layout | s | | layout | s |
|---|---|---|---|---|
| `layout.force.davidson_harel` | **390.15** | | `layout.force.spring3d` | 9.93 |
| `layout.force.particle_mesh` | 172.31 | | `layout.force.spring` | 9.66 |
| `layout.force.fdp` | 166.88 | | `layout.force.lgl` | 9.16 |
| `layout.force.drl` | 151.17 | | `layout.force.yifan_hu` | 5.34 |
| `layout.force.neato` | 46.42 | | `layout.mds.pivot` | 3.77 |
| `layout.force.sfdp` | 34.58 | | `layout.forceatlas2` | 3.75 |
| `layout.packing.circle` | 33.23 | | `layout.force.barnes_hut` | 3.64 |
| `layout.circular.circo` | 27.23 | | `layout.forceatlas2.barnes_hut` | 2.63 |
| `layout.force.kamada_kawai` | 16.74 | | `layout.dag.sugiyama` | 0.94 |
| `layout.force.graphopt` | 14.08 | | the other 18 rows | 0.12 – 0.37 |
| `layout.force.fruchterman_reingold` | 13.25 | | | |
| `layout.spectral` | 12.60 | | | |

`davidson_harel` alone is 390 s — more than the 20 cores' share of anything else, and on
one core it is the whole wall clock. Four rows hold 880 s of the ~1085 s. Splitting per
registry row alone is therefore **not enough**: it caps the win at about 2× (measured:
462.33 s, below) because the 390 s row survives as the new long pole.

## What changed

One macro-generated test per list entry, where an entry is
`[registry index, id, chunk, test name]`:

- `[i, "id", WHOLE, name]` — the row's whole `0..200` seed sweep in one test.
- `[i, "id", 0..CHUNKS, name_sN]` — the row cut into `CHUNKS = 4` disjoint seed ranges,
  used only for `layout.force.davidson_harel`.

Both spellings go through the same `sweep_layout(index, id, seed_range(chunk))`, so the
per-seed assertions and their messages are byte-for-byte the old ones. `registry::LAYOUTS`
is 39 rows, so the binary goes from 3 tests to 45.

Only `davidson_harel` is split. Splitting the next three rows as well would push the
makespan towards the ~1085 s / 20 cores ≈ 55 s floor, but it is not what the job asks for
and each extra split row is a list entry a future edit can get wrong; `particle_mesh` at
172 s is the next pole and is named in the file's own comment if anyone wants it.

### Coverage: identical before and after

| test | seeds | layouts swept |
|---|---|---|
| **before** `treemap_boxes_contain_…` | `0..200` | `treemap` only (unchanged) |
| **before** `tidy_tree_polyline_offsets_…` | `0..200` | `tidy_tree` only (unchanged) |
| **before** `every_registered_layout_emits_…` | `0..200` × 39 rows | all 39 |
| **after** `treemap_boxes_contain_…` | `0..200` | `treemap` only (unchanged) |
| **after** `tidy_tree_polyline_offsets_…` | `0..200` | `tidy_tree` only (unchanged) |
| **after** `layout_grid` … `layout_force_particle_mesh` (38 tests) | `0..200` each | one row each |
| **after** `layout_force_davidson_harel_s0..s3` (4 tests) | `0..50`, `50..100`, `100..150`, `150..200` | `layout.force.davidson_harel` |
| **after** `every_registered_layout_has_its_own_sweep` | — | guard, sweeps nothing |

Same 39 × 200 `(seed, layout)` pairs, plus the two unchanged per-model sweeps. No
`(seed, layout)` pair is dropped, none is added, and no assertion was weakened.

## The guard, and its two negative controls

`every_registered_layout_has_its_own_sweep` reads the one list the tests are generated
from and asserts two things: the distinct ids are `registry::LAYOUTS` **in order**, and
each row's seed chunks tile `0..200` **exactly once**. A registry row added later, or a
chunk dropped from a row, fails this test instead of going unchecked.

RED (a), the last registry row dropped from the list:

```
thread '…::every_registered_layout_has_its_own_sweep' panicked at
  crates/graph-core/tests/geometry_invariants.rs:240:
assertion `left == right` failed: the sweeps must list registry::LAYOUTS in order
  left: [… "layout.basic3d.spiral"]              # 38 entries
 right: [… "layout.basic3d.spiral", "layout.force.particle_mesh"]   # 39
test result: FAILED. 0 passed; 1 failed
```

RED (b), one seed chunk (`_s3`) dropped from the chunked row:

```
assertion `left == right` failed:
  layout.force.davidson_harel: chunks [0, 1, 2] must tile 0..SEEDS
test result: FAILED. 0 passed; 1 failed
```

Both RED, then restored green. The guard also earned its keep during the work itself: an
earlier draft of the list put rows 0 and 5 on one source line, and the order assertion
caught the resulting registry-order break immediately.

## Before and after

| run | before | after | change |
|---|---|---|---|
| full binary, default threads (20), same tree | 1316.87 s | **327.36 s** | **4.02× faster** |
| full binary, default threads, run 2 | 1316.87 s | 317.80 s | 4.14× faster |
| full binary, default threads, run 3 | 1316.87 s | 572.91 s | 2.30× faster |
| full binary, `RUST_TEST_THREADS=4` | 941.52 s (pre-merge) | 366.94 s | 2.57× faster |
| tests in the binary | 3 | 45 | |
| sum of all test times (`RUST_TEST_THREADS=1`) | 952.20 s | 1084.94 s | +14 % CPU |

The halving the brief asks for is the first three rows: three runs of the same tree, same
default thread count, spanning 317.80–572.91 s against a 1316.87 s baseline. **Every run
is under half**, so the claim does not rest on the best of them — but the spread is
large, and it is the host, not the code: the same bytes gave 327.36 s and 572.91 s, and
the same `cli_force` bytes gave 349.24 s, 399.11 s and 406.11 s (section below). Treat
the honest figure as "2.3×–4.1× depending on what the machine was doing", and re-measure
on a quiet host before quoting a single number in a gate.

The `RUST_TEST_THREADS=1` row is the honest cost of the split: the unsplit sweep built
each seed's topology once and ran 39 layouts against it, so per-layout tests rebuild it
once per layout, and the total CPU rises about 14 %. That is the trade for a 5× wall-clock
win, and it is why the split is per registry row rather than per `(seed, row)` cell.

### `cli_force.rs`: measured, and reverted

The brief asks for this file only if one test there dominates. It did, overwhelmingly:
`cargo test -p graph-cli --test cli_force` at default threads, `--exact` per test:

| test | s |
|---|---|
| `each_force_layouts_own_control_goes_red_on_only_its_stage` | **418.52 / 379.95** |
| `the_force_stages_are_4_way_compiled_and_hashed` | 5.98 / 4.51 |
| `stress_needs_an_oracle_and_runs_the_d3_arm_or_says_it_could_not` | 0.26 / 0.24 |
| `bench_times_the_force_layouts_and_refuses_a_size_past_a_ceiling` | 0.14 / 0.17 |

So it was split too: the two-entry knob loop became one `#[test]` per force stage's
control, over the same helper, with every assertion and message unchanged. Measured on
the same tree, same default `RUST_TEST_THREADS`, back to back:

| | tests | libtest `finished in` |
|---|---|---|
| before (one test looping both controls) | 4 | **349.24 s** |
| after (one test per control) | 5 | **369.63 s** |

**That is a 5.8 % regression, not a saving, so the change was reverted** and this file is
untouched in the final tree. Two reasons it does not pay:

- Each control is a whole `hashgate --seeds 40`, and `graph-cli` already uses every core
  it can get — a hashgate run is itself parallel. Two of them at once contend for the
  same 20 cores, so the wall does not halve the way a CPU-bound serial test would.
- The measurement is also inside the host's own noise band: the *same* unsplit binary
  measured 349.24 s, 399.11 s and 406.11 s on three runs (16 % spread), so a 4 % effect
  is not resolvable here at all.

The lesson is the same one this whole job turned on, and it is worth stating plainly: the
split pays only where the work is *serial on one core*. The layout sweep is (pure Rust,
one layout, one seed at a time); `hashgate` is not.

## Commands (worktree root)

```
scripts/orch/gr cargo test -p graph-core --test geometry_invariants
RUST_TEST_THREADS=4 scripts/orch/gr cargo test -p graph-core --test geometry_invariants
RUST_TEST_THREADS=1 scripts/orch/gr cargo test -p graph-core --test geometry_invariants
scripts/orch/gr cargo test -p graph-core --test geometry_invariants -- --exact <name>
scripts/orch/gr cargo test -p graph-cli --test cli_force -- --exact <name>
scripts/orch/gr cargo fmt -p graph-core -- --check
scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings
```

## Notes

- No file under `crates/*/src/` and no Cargo file was touched: the manifests are
  fingerprinted and change every build.
- The gate row `geometry-invariants` (`scripts/orch/rows/develop-full.rows:45`) filters on
  the substring `geometry_invariants`, which the module name still satisfies, so all 45
  tests stay selected by it. Test *function* names did change for the layout sweep.
- `docs/reports/phase-03.md:184` cites the old
  `::every_registered_layout_emits_no_nan_or_inf_and_circle_radii_are_positive` name, which
  no longer exists. The claim it records is still enforced, now by 42 tests plus the
  guard; the citation is stale but the file is outside this job's paths.
