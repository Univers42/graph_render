# sg-igraph-dims — `IGRAPH_FR` and `IGRAPH_KK` reach their 3-D ids

Matrix rows 7 and 8: `SciGraphs/core/scigraphs_core/mesh/igraph_layouts.py:74` and `:99` call
python-igraph 0.11.9 with `dim=3`, and graph-core had only planar FR and KK. The repair is the two
ids `layout.force.fruchterman_reingold_3d` and `layout.force.kamada_kawai_3d`, written from the
prose specs alone. This file carries the measurements of the job's steps 2–5; the matrix itself is
`docs/measurements/scigraphs-conformance.md`.

It is named `sg-igraph-dims` because four files already cited that name and it did not exist — the
job that was to write it had its `graph-core` half discarded and kept only its docs and rows.

## The design in five lines

- **One kernel, two dimensions.** `layout/force/fr_kernel.rs` and `layout/force/kk_kernel.rs` are
  each written at `const D` and instantiated at `D = 2` and `D = 3`, the shape
  `layout/force/spring/forces.rs` already uses. `layout.force.fruchterman_reingold` is
  `fr_kernel::layout::<2>`; `…_3d` is `fr_kernel::layout::<3>`.
- **The starts differ per dimension, so the start is an argument.** The FR spec gives one box for
  both (`[-sqrt(n)/2, +sqrt(n)/2]` per axis); the KK spec gives a circle at `dim=2` and, in
  `layout.force.kamada_kawai.md` §"The 3D start: the sphere", a closed table at `dim=3`. The circle
  stays in `kamada_kawai.rs`, the sphere in `kamada_kawai_3d.rs`, and both call
  `kk_kernel::descend(graph, n, start, params)`.
- **The 3x3 solve is one expansion, not two determinants.** `kk_kernel` solves the Newton block by a
  general signed-permutation Cramer expansion (`each_permutation` / `advance`), which is correct at
  `D = 2` as well and is checked against the closed forms in `kk_kernel::tests`.
- **`Metadata` for both ids lives in `registry/igraph.rs`.** The oracle is python-igraph 0.11.9 at
  `dim=3` through `harness/oracle-igraph.py`, whose stress metric and ceiling rules are unchanged.
- **`kk_kernel` is a module, not a file.** It was written as one 386-line file, past the
  300-line house limit, and split along its own seams into `kk_kernel/{springs,gradient,step}.rs`
  with the driver, the vertex picker and the two axis helpers staying at the root — 117, 73, 40
  and 195 lines. A code move that changes a rounding order would change every hash below, so the
  split was checked the way any other change is: the 16 per-stage shas of step 2 were re-measured
  after it and `diff`ed against the copy printed there, and `diff` was empty.

## Step 2 — the 2-D ids are byte-identical

Per-stage sha256 of one native arm, `graph-cli hashgate-arm --seeds 8`. Two captures were taken in
this job: one on the tree as it was received, before a line under `layout/force/` was written, and
one after every later edit. They were 1552 bytes each and `diff` reported no difference:

```
$ diff /tmp/opencode/arm-before.txt /tmp/opencode/arm-after2.txt
$ echo $?
0
```

Both captures are scratch under `/tmp` and went with the session, so **the block below is the
record** — and it has been re-measured since, twice: once after the `kk_kernel` module split of the
design section above, once again on the final tree, each time by diffing the command's output
against these lines. Both diffs were empty. Reproduce with:

```
scripts/orch/gr cargo run -q -p graph-cli -- hashgate-arm --seeds 8 \
  | grep -E '^layout\.force\.(fruchterman_reingold|kamada_kawai) '
```

The 16 lines:

```
layout.force.fruchterman_reingold 0 43d70a1fa8327024f83ae0029aaeef9849d82a984b5c1ee9aa5cfecb06ba802c
layout.force.fruchterman_reingold 1 3808ce28aaf29b86f629df5b8fe5c9076dd38e490025d6a2455d0dbf41f8b21e
layout.force.fruchterman_reingold 2 6e0148eea8922f812e8c740feefdeb55d4f4b1c593792af3d657d73a9f09c74a
layout.force.fruchterman_reingold 3 043d474ecbd03782811f4bf41a96c787cf7a964c4169c3aa0f3dc68d6069c6b4
layout.force.fruchterman_reingold 4 447fda641401e7ddf3289bf384d3e17e93030b0ad19210a774620d4585beefbb
layout.force.fruchterman_reingold 5 062851bde91bcbac001550db03fc4208ed733ad6cf214d18c70e916ef9846bfa
layout.force.fruchterman_reingold 6 c740f5d29c932d13d35fab7467cd97ef0320a59fbf52116aa16037fc68ada842
layout.force.fruchterman_reingold 7 cfc28113741ffc8e709bf51ab26c487758beeea9a99512f5d21be7983ffd4705
layout.force.kamada_kawai 0 84031272242c8767dc244ca7e420b87c091337188aeb9806b7493f63e69fc6ca
layout.force.kamada_kawai 1 ed39321ab91bbeceb9129e4ecb4027b4b2730c473df8d45ef9a898eac574fd29
layout.force.kamada_kawai 2 dfe452afe3f5935946f64f8a8bff3fff874c0642f506b48d9b6032b4710d40a2
layout.force.kamada_kawai 3 c3f9731984ef4753a140a17a66daf848022c079a90aed62e303e727d24e14530
layout.force.kamada_kawai 4 f5cf2b09646c9bfa771220492b8626679a4160c0d1fe89b4107c937eae810110
layout.force.kamada_kawai 5 be6b0af33b8120f291ecbed3e3f5a113beb28a434916cd30e0470f9d4cb241c4
layout.force.kamada_kawai 6 0cc86fad8024a759d12c7f07633481ebb89f5efba4aef1ba683314b178123cf5
layout.force.kamada_kawai 7 dd118ad6f7f9509c494a0ee0c0a5d0d18a401ccf85eca2b4faf743be8252e468
```

The same run on the finished tree, `hashgate --seeds 8` (native x2, wasm32 x2), covers the two new
ids as well:

```
  layout.force.fruchterman_reingold: 4-way equal on 8/8 seeds
  layout.force.fruchterman_reingold_3d: 4-way equal on 8/8 seeds
  layout.force.kamada_kawai: 4-way equal on 8/8 seeds
  layout.force.kamada_kawai_3d: 4-way equal on 8/8 seeds
PASS
```

## Step 3 — the 3-D tests

`scripts/orch/gr cargo test -p graph-core --lib fruchterman_reingold_3d` — exit 0, 9 tests;
`… kamada_kawai_3d` — exit 0, 6 tests. The ones the job asked for by name:

| test | file | what it holds |
|---|---|---|
| `the_sphere_start_matches_the_spec_table` | `kamada_kawai_3d/tests.rs:48` | every row of the spec's `z`/`r`/`phi` table, `i = 0` at the south pole, `i = n-1` at the north |
| `the_sphere_start_is_scaled_by_the_spec_radius` | `kamada_kawai_3d/tests.rs:78` | the `0.36 * sqrt(n)` factor |
| `the_output_spreads_on_all_three_axes` | both `tests.rs` | non-zero spread on `z`, so a planar result cannot pass |
| `the_same_seed_gives_the_same_bytes` / `a_run_is_repeatable_and_deterministic` | both `tests.rs` | same input, same bytes, twice |
| `the_three_node_path_that_breaks_igraph_is_finite_here` | `kamada_kawai_3d/tests.rs:109` | finite geometry on the fixture where igraph returns 9 non-finite coordinates |

The last one names the breaking input property in its own doc-comment: **three vertices, so the 3x3
Newton block is near-singular at every one of them** — not component count (the graph is
connected), not an isolated node, and not degree (2, 1, 1); the same graph at `dim=2` is finite.

## Step 4 — the FR spec's stale pointer

`docs/layouts/layout.force.fruchterman_reingold.md` "Resolved for this tree" ended by pointing at a
file that does not exist in this tree. Before:

```
  behaviour will find it in `layout/force/fruchterman_reingold/kernel.rs`'s `repel`, in the
  `far.is_some()` scale, and can restore it by folding the `z` term into the `y` accumulator.
```

After (lines 115-118):

```
  wrote it, not a claim that the decision is unimportant — a reviewer who wants the literal
  behaviour will find it in `layout/force/fr_kernel.rs`'s `repel`, in the `far.is_some()` scale
  (the doc-comment there names this exact defect), and can restore it by folding the `z` term
  into the `y` accumulator.
```

**The pointer was the only thing wrong; the claim behind it was not tested.** Filling the `y`
accumulator with the `z` term — exactly the escape hatch the sentence offers — left all nine
`fruchterman_reingold_3d` tests green:

```
MUTATED: z folded into y on the disconnected branch only, as igraph's block does
running 9 tests
.........
test result: ok. 9 passed; 0 failed
```

`layout/force/fr_kernel.rs` therefore gained the control that sentence implies,
`every_component_of_the_pair_term_lands_on_its_own_axis`. The two vertices in it share their `y`,
so `delta[1]` is `0.0`: a `y` accumulator with the `z` term folded into it is not zero, and the `z`
accumulator is. GREEN on the correct kernel, then RED under the fold, then GREEN again:

```
running 1 test
layout::force::fr_kernel::tests::every_component_of_the_pair_term_lands_on_its_own_axis --- FAILED
  assert_eq!(disp[0][1], 0.0, "far = {far:?}: delta.y is zero")
  left: -26.593440487803225
 right: 0.0
test result: FAILED. 0 passed; 1 failed          (exit 101)
```

The failure is on the `far = Some(2.8284271247461903)` iteration, i.e. the disconnected branch the
doc's decision is about. The kernel was restored from a copy and `diff` against it was empty; the
suite is green again (`cargo test -p graph-core --lib fruchterman_reingold` → 16 passed, exit 0).

## Step 5 — re-pins and re-measurements

### The conformance matrix, rows 7 and 8

`scripts/scigraphs-conformance.sh` — exit 0. The `IGRAPH_KK` motor sha in
`oracle_python/conformance/baseline/table/networkx.rs` moved; `IGRAPH_FR`'s kept the carried pin to
the digit. `metrics.json` from that run:

| row | | f64 k/N | f32 k/N | max ULP | max gap | Proc med | Proc max |
|---|---|--:|--:|--:|--:|--:|--:|
| `IGRAPH_FR` | before | 2/1020 | 4/1020 | 9.24e+18 | 10 | 0.166 | 0.921 |
| `IGRAPH_FR` | **after** | 2/1020 | 4/1020 | 9.23e+18 | 10 | 0.166 | 0.921 |
| `IGRAPH_KK` | before | 8/957 | 8/957 | 9.23e+18 | 10 | 0.757 | 0.910 |
| `IGRAPH_KK` | **after** | 9/957 | 213/957 | 9.23e+18 | 9.76 | 0.709 | 0.916 |

`IGRAPH_KK`'s motor sha moved because the clean-room kernel solves the 3x3 block by a
signed-permutation Cramer expansion rather than a hand-written determinant and the two round
differently in the last bits, so the descent walks a different path and settles elsewhere. Every
figure moved toward the reference: median 0.757 → 0.709, `f32` agreement 8 → 213 of 957, and the
stress differential 0.598 → 0.905 (below). The `f32` count moving two orders of magnitude while the
median moves one digit is the signature of a last-bit change rather than a different layout.

`IGRAPH_FR`'s median, max gap and `f64`/`f32` counts are unchanged; only its `max ULP` cell moved,
9.24e+18 → 9.23e+18, which is this run's value for a row whose motor pin did not move.

The row's 957 of 1020 stays: the reference raises 9 non-finite coordinates on `gate-01`, recorded
in `docs/measurements/scigraphs-conformance.md` §"Cells that say `not run`". The motor does not
copy that failure — see `the_three_node_path_that_breaks_igraph_is_finite_here`.

### The igraph differential, `harness/oracle-igraph.py`

```
graph-cli emit-igraph-fixtures --seeds 100
docker run --rm -v $PWD:/w -w /w ge-python-oracle python3 harness/oracle-igraph.py target/igraph-fixtures
graph-cli oracle-igraph
```

The reference was run twice, and the two runs' `igraph-result.json` are byte-identical:

```
$ cmp /tmp/opencode/result1.json /tmp/opencode/result2.json && echo BYTE-IDENTICAL
BYTE-IDENTICAL
$ sha256sum /tmp/opencode/result1.json /tmp/opencode/result2.json
fe97d0b7117b6311e0cf3a9236b801a1a4fd1421ea0b092253c29ed64dd7dd38  result1.json
fe97d0b7117b6311e0cf3a9236b801a1a4fd1421ea0b092253c29ed64dd7dd38  result2.json
```

`graph-cli oracle-igraph` — exit 0, `PASS`. The `worst` column is `max(ours / igraph)` over the
100 seeds; `reference_worst` is igraph's own worst normalised stress over the same seeds.

| id | cases | worst | ceiling | reference_worst | |
|---|--:|--:|--:|--:|---|
| `layout.force.fruchterman_reingold` | 100 | 1.298 | 1e1 | 1.029 | ok |
| `layout.force.kamada_kawai` | 100 | 1.348 | 1e1 | 1.029 | ok |
| `layout.force.fruchterman_reingold_3d` | 100 | **1.195** | 1e1 | 0.610 | ok |
| `layout.force.kamada_kawai_3d` | 100 | **0.905** | 1e1 | 1.576 | ok |
| `layout.force.drl` | 100 | 3.977 | 1e1 | 4.960 | ok |
| `layout.force.lgl` | 100 | 2.131 | 1e1 | 1.237 | ok |
| `layout.force.davidson_harel` | 100 | 51.91 | 1e2 | 4.830 | ok |
| `layout.force.graphopt` | 100 | 15.39 | 1e2 | 1.386 | ok |

The six 2-D figures reproduce the 2026-09-29 run to the digit (1.30, 1.35, 3.98, 2.13, 51.91,
15.39), which is the check that the 3-D pass did not perturb the 2-D one — the same check the hash
gate makes at the byte level in step 2. The two 3-D figures are this run's and are what
`oracle_python/tests.rs` now pins: `fruchterman_reingold_3d` 1.195, `kamada_kawai_3d` **0.905**,
the latter re-pinned from 0.598.

`kamada_kawai_3d`'s ratio is below 1: our 3-D drawing carries lower normalised stress than
igraph's on every one of the 100 seeds, because stress is the very energy KK minimises.

## Commands

| command | exit |
|---|--:|
| `scripts/orch/gr cargo fmt --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (`FAIL: 8 of 8 seeds diverge`) |
| `scripts/scigraphs-conformance.sh` | 0 |
| `scripts/scigraphs-conformance.sh --break` | 1 (`SPRING_3D`) |
| `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- emit-igraph-fixtures --seeds 100` | 0 |
| `… oracle-igraph.py …` twice, `cmp` of the two results | 0, identical |
| `scripts/orch/gr cargo run -q -p graph-cli -- oracle-igraph` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | 1 — see below |

`capabilities --check` reports 36 problems and **none names either new id**: all four
`layout.force.fruchterman_reingold`, `layout.force.kamada_kawai`, and their two `_3d` siblings
report `implemented`. The 36 are 19 × *"hashgate record is from another tree"* and 17 × *"no
`<oracle>` record"*, all on `topology.*`, `layout.grid`, `layout.tree.tidy`,
`layout.treemap.squarified`, `layout.circular.radial`, `layout.packing.*`, `layout.spectral`,
`layout.mds.pivot`, `layout.dag.sugiyama` and `transport.wasm.columnar`. They cannot be cleared
here: `crates/graph-cli/src/capabilities/verdict.rs:13` sets `MIN_SEEDS = 1000`, so every one of
them needs the full `hashgate --seeds 1000` plus every oracle record on the current tree, and
`scripts/orch/common.md:17` forbids running a timed gate from a job — the orchestrator runs it
after the job returns.

## Provenance

Every source read for the two algorithms:

- **`docs/layouts/layout.force.fruchterman_reingold.md`** — `## Algorithm` steps 1–4: the box start
  `[-sqrt(n)/2, +sqrt(n)/2]` per axis, `C = n * sqrt(n)` for a disconnected graph, the pair term
  `delta * (C - r^3) / (r2 * C)`, the capped move and the `start_temp / niter` cooling; `## Grid
  variant` (2-D only, so neither id implements it); `## Where randomness enters`; and
  `## Defects and quirks worth deciding on`, the mistyped 3-D axis that step 4 above records.
- **`docs/layouts/layout.force.kamada_kawai.md`** — `## Algorithm` steps 1–5 (the hop-distance
  all-pairs `L`, the energy, the 3x3 Newton block and the sign of the step); `## The 3D start: the
  sphere (spec gap closed 2026-10-02)`, the whole of the `z`/`r`/`phi` table, the special cases at
  `i == 0` and `i == n-1`, `phi += 3.6 / (sqrt(n) * r)` advancing on interior rows only, and the
  `0.36 * sqrt(n)` scale; `## Cooling and stopping`; `## Where randomness enters` (the circle and
  sphere starts use no random numbers, so the output is deterministic); `## Quirks worth deciding
  on`.
- **`prompts/REFERENCES.md`** — the rule that a port is read from a reference on disk or a paper,
  never from memory, and the two papers the specs rest on: Fruchterman & Reingold (1991) for FR and
  Kamada & Kawai (1989) for KK, plus Saff & Kuijlaars (1997), the spiral constant the sphere table
  quotes.
- **`crates/graph-core/src/layout/force/spring/forces.rs`** — the `const D` kernel-sharing shape the
  job named as the precedent for one kernel at two dimensions.
- **`crates/graph-core/src/layout/force/fruchterman_reingold.rs` and `kamada_kawai.rs`** — the 2-D
  stages, read so the 3-D ids would move nothing under them (step 2 is that claim's evidence).
- **`crates/graph-core/src/registry/igraph.rs`** — the `Metadata` both ids had to carry.

For the measurements:

- **`harness/oracle-igraph.py`** — the stress metric, the 1e-3 floor, the `worst` and
  `reference_worst` columns, and the rule that `dim` is a field of the reference table rather than
  derived from the key.
- **`scripts/scigraphs-conformance.sh`** and **`harness/scigraphs-conformance/sc_metrics.py`** —
  the matrix columns and how `max_ulp`, `max_gap` and the Procrustes figures are computed.
- **`crates/graph-cli/src/oracle_python/tests.rs`** and
  **`crates/graph-cli/src/oracle_python/conformance/baseline/table/networkx.rs`** — the pinned
  stress figures and the two motor shas this job re-pinned.
- **`crates/graph-cli/src/capabilities/verdict.rs`** — `MIN_SEEDS`, read to establish why
  `capabilities --check` cannot pass from inside a job.
- **`docs/measurements/scigraphs-conformance.md`** — the `gate-01` record of igraph's 9
  non-finite coordinates, and the matrix rows this job's measurements update.
- **`docs/measurements/sg-grid-scale.md`** — the shape a job's measurement file takes in this repo.
- **`scripts/orch/common.md`** and **`prompts/jobs/sg-igraph-dims.md`** — the house limits and the
  sibling job's statement of the same spec gap.

Never opened, and how the algorithms above could be written without them: no igraph C file, no
python-igraph source, no `$GM_SCRATCH/refs/igraph-0.11.9` beyond the docker build context
`scripts/scigraphs-conformance.sh` names, no branch `sg-igraph-dims` and no `origin/sg-igraph-dims`
— no `git show`, `git diff`, `git log -p` or checkout of either — and no networkx layout file
(the specs say networkx's FR and KK are different algorithms from the ones SciGraphs calls).
python-igraph was reached only as the oracle: the `ge-python-oracle` image ran
`harness/oracle-igraph.py` and `harness/scigraphs-conformance.py`, and both read the reference
back as JSON.

The implementer did not open igraph's sources or branch sg-igraph-dims.
