# fix-memory-3d — the 3D rows' own bytes-per-node figure

Source: `docs/measurements/fix-tree-registry.md` row **L-25** and its "L-25 rung" note.
`fix-tree-registry` widened the doc of `BASIC_3D_CEILING` instead of measuring it, because
`crates/graph-core/tests/memory.rs` had no 3D arm, so the memory half of every 3D row's
`scale_ceiling` argument was borrowed from the 2D rows. This job measures it.

`layout.force.spring3d` is out of scope: it takes `SPRING_CEILING`, not `BASIC_3D_CEILING`
(`registry/three_d/graph.rs:99`). **All six rows under `BASIC_3D_CEILING` are now measured** —
`SPHERE`, `HELIX`, `CUBE`, `SPIRAL_3D` (`three_d/spiral3d.rs:71`), `HIERARCHICAL_3D`
(`three_d/graph.rs:41`) and `BIPARTITE_3D` (`three_d/bipartite_3d.rs:51`).

## Rows

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| L-25 | MINOR | fixed | `three_d::hierarchical_3d_pipeline_memory_per_node` (measurement, `#[ignore]`d) | `crates/graph-core/src/registry/three_d.rs:51` |
| L-25 | MINOR | fixed | `three_d::basic_3d_closed_form_pipeline_memory_per_node` (measurement, `#[ignore]`d) | `crates/graph-core/src/registry/three_d.rs:51` |
| L-25 | MINOR | fixed | `three_d::bipartite_3d_pipeline_memory_per_node` (measurement, `#[ignore]`d) | `crates/graph-core/src/registry/three_d.rs:51` |
| L-25 | MINOR | doc-only | no test: a doc comment quoting the three blocks below | `crates/graph-core/src/registry/three_d.rs:38-105` |
| L-25 | MINOR | doc-only | no test: module-doc row count, five → seven, and the `SPIRAL_3D` / `BIPARTITE_3D` place in the row list | `crates/graph-core/src/registry/three_d.rs:1-30` |

No ceiling value changed — see "What the measurement says about the ceiling" below. A value
change would have been a stop: `capabilities --check` reports it.

## The arms

Three `#[ignore]`d tests in the new child module `crates/graph-core/tests/memory/three_d.rs`,
same counting global allocator, same `print_measurement`, same three node counts as the 2D
rows in the parent. `tests/memory.rs` needed an explicit
`#[path = "memory/three_d.rs"]`: a crate root resolves a submodule in its own directory, so a
bare `mod three_d;` in `tests/memory.rs` wants `tests/three_d.rs`. The sibling
`tests/edge_geometry_invariants.rs:30` walks around the same trap by wrapping its body in a
module named after the file; `#[path]` does it in one line and leaves the parent's 180 lines
unindented.

Command (the header of `memory.rs`, filtered to the three new arms here):

```sh
scripts/orch/gr cargo test --release -p graph-core --test memory -- \
  --ignored --nocapture --test-threads=1 three_d
```

## Measurement block, pasted

`test three_d::basic_3d_closed_form_pipeline_memory_per_node ... | layout.basic3d.sphere | 1000 | 1541 | 65808 | 887903 | 887.9 B | 1.41ms |`
`| layout.basic3d.sphere | 10000 | 15474 | 684672 | 8199698 | 820.0 B | 15.02ms |`
`| layout.basic3d.sphere | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 220.65ms |`
`| layout.basic3d.helix | 1000 | 1541 | 65808 | 887903 | 887.9 B | 1.10ms |`
`| layout.basic3d.helix | 10000 | 15474 | 684672 | 8199698 | 820.0 B | 12.61ms |`
`| layout.basic3d.helix | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 220.41ms |`
`| layout.basic3d.cube | 1000 | 1541 | 65808 | 887903 | 887.9 B | 1.11ms |`
`| layout.basic3d.cube | 10000 | 15474 | 684672 | 8199698 | 820.0 B | 12.58ms |`
`| layout.basic3d.cube | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 208.02ms |`
`| layout.basic3d.spiral | 1000 | 1541 | 65808 | 1315703 | 1315.7 B | 1.38ms |`
`| layout.basic3d.spiral | 10000 | 15474 | 684672 | 8199698 | 820.0 B | 13.65ms |`
`| layout.basic3d.spiral | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 196.82ms |`
`ok`
`test three_d::bipartite_3d_pipeline_memory_per_node ... | layout.bipartite_3d | 1000 | 1541 | 65808 | 887903 | 887.9 B | 1.21ms |`
`| layout.bipartite_3d | 10000 | 15474 | 684672 | 8199698 | 820.0 B | 13.74ms |`
`| layout.bipartite_3d | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 214.96ms |`
`ok`
`test three_d::hierarchical_3d_pipeline_memory_per_node ... | layout.hierarchical3d | 1000 | 1541 | 65808 | 887903 | 887.9 B | 1.04ms |`
`| layout.hierarchical3d | 10000 | 15474 | 684672 | 8290770 | 829.1 B | 7.99ms |`
`| layout.hierarchical3d | 100000 | 154978 | 7107252 | 91930690 | 919.3 B | 126.04ms |`
`ok`

Columns are `layout | n | m | snapshot bytes | peak | peak / node | wall`, as the parent's
`print_measurement` prints them.

## The closed forms share an allocation shape — 0 %, not 5 %

The four graph-free closed forms were swept as four rows rather than one so the claim could be
checked. At 10 000 and at 100 000 nodes `sphere`, `helix`, `cube` and `spiral` peak at the
**same byte, to the byte** (8 199 698 and 91 930 690), so the arm stands for all four and the
comment on it says so. At 1 000 nodes three of the four are 887 903 B and `layout.basic3d.spiral`
is 1 315 703 B — 48 % higher, the one row that moves, at the smallest size only. That is the
honest limit recorded in `three_d.rs:60`.

## What the measurement says about the ceiling

`BASIC_3D_CEILING` is `MAX_BENCH_NODES` = 1 000 000 (`registry/three_d.rs:119`), and the
measurement does not move it:

- measured 3D peak **919.3 B/node** at 100 000 nodes, against the 919 B/node `GRID_CEILING`
  derives 4 GiB from (`registry/grid.rs:16-20`) — the figure the 3D doc had borrowed. Within 1 %,
  and `4 GiB / 919 B = 4.67 M nodes` to the same two figures.
- 4.67 M against a ceiling of 1 M, so the ceiling is the bench cap, not a memory bound, and it
  is conservative in the direction the Ponytail already names (too low, never too high).

No value changed, so nothing to report under "decisions needed".

## Deviations / notes for the orchestrator

- `crates/graph-core/src/registry/three_d/graph.rs` is **not** in this job's path list, so the
  `HIERARCHICAL_3D` row's own `ponytail` still carries no `scale_ceiling` disclosure — the same
  gap `fix-tree-registry` recorded. The measurement is cited from the shared constant's doc,
  which is where the review proposed it. Recommend a one-line follow-up in `graph.rs` and
  `spiral3d.rs` pointing at `three_d.rs:51`.
- `three_d.rs:1-30` module doc said "the last five SciGraphs layouts" and "these five" while
  seven rows had joined. Corrected in this job's path list, together with the row list at
  `three_d.rs:14-25` (which omitted `BIPARTITE_3D` and its `bipartite_3d` child module) and
  "the three closed forms" → four. The "six rows" wording at `three_d.rs:38`, `:46`, `:75` and
  in `DEGRADATION` is **correct** and was left alone: six rows take `BASIC_3D_CEILING`,
  `SPRING_3D` alone takes `SPRING_CEILING` and carries its own `degradation` string.
- `layout.basic3d.spiral` at 1 000 nodes peaks 48 % above the other three closed forms. Not
  diagnosed here; the arm is a measurement script, and the figure is recorded as it stands.

## Commands, with their real exit codes

```
scripts/orch/gr cargo test --release -p graph-core --test memory --no-run  -> 101   RED: E0583, `mod three_d;` in a crate root wants tests/three_d.rs
scripts/orch/gr cargo test --release -p graph-core --test memory -- --ignored --nocapture --test-threads=1  -> 0   all 7 arms: 4 pre-existing + 3 new
scripts/orch/gr cargo test --release -p graph-core --test memory -- --ignored --nocapture --test-threads=1 three_d  -> 0   the 3 new arms
scripts/orch/gr cargo fmt --all  -> 0
scripts/orch/gr cargo fmt --all --check  -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings  -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast  -> 0   0 failed anywhere; `memory` reports 7 ignored (4 old + 3 new arms)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown  -> 0
scripts/scigraphs-conformance.sh  -> 0   PASS
scripts/orch/gr cargo run -q --release -p graph-cli -- capabilities --check  -> 1
```

`capabilities --check` exits **1** with 36 problems, and this is pre-existing, not this job's:
every one is a `gated, but no hashgate / oracle / roundtrip record: run the gate` line for a
timed gate this job is forbidden to run. `capabilities --check | grep -iE 'ceil|3d|basic3d'`
matches **nothing** (exit 1), so no `scale_ceiling` is reported — as expected, since no
ceiling value changed.

Not run, being timed gates the body reserves for the orchestrator: `hashgate --seeds 8` and its
`GM_MUTATE_REFERENCE_DEGREE=9` control, `mutants.sh`, `gate.sh`. No registered layout, post,
analysis or scale output moved — this job edits a doc comment and adds three `#[ignore]`d
measurements, and `scripts/scigraphs-conformance.sh` exits 0 with every row unmoved.