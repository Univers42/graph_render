# `sg-bipartite3d` — conformance row `BIPARTITE_3D`

One row of `docs/measurements/scigraphs-conformance.md`, repaired. Before: tier `shape`,
cause `algorithm`, f32 **2/1020**, Procrustes median **0.405**. After: tier `tolerance`,
cause `arithmetic`, f32 **1020/1020**, Procrustes median **3.57e-16**.

## What the reference actually is

The repair this job was given said the picture was backwards, and it was. Repair 11 read:

> `BIPARTITE_3D` in particular: networkx draws two **columns** and graph-core's `partition`
> (`bipartite.rs:30`) places differently.

Both halves are the wrong way round. networkx's `bipartite_layout` is the **motor** the row
was mapped to (`layout.bipartite`); the two columns were never the disagreement, and no
change to `partition` could have fixed the row. The **reference** is SciGraphs'
`_bipartite_layout_3d`
(`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:213-242`), and it draws two
node sets on parallel **planes**, one ring each:

```text
set0 -> z = -scale*0.5,  set1 -> z = +scale*0.5
radius = scale*0.6                       the same radius on both planes
angle  = (i / max(1, count)) * 2 * pi    i is the node's SLOT IN THE SET, not its node id
```

So the repair was a layout that draws rings. `layout.bipartite_3d` is that layout, added
beside the networkx one; the two share SciGraphs' node sets and differ in every coordinate
after them, which is why `layout.bipartite` keeps its own oracle and its own id.

## The visiting order, which is where the risk was

The sets come from `_bipartite_parts` (`hierarchical.py:149-181`), or from
`_greedy_max_cut` (`:183-211`) when the graph does not two-colour. Written out in full,
because this is the part a port gets wrong silently:

- **Node order.** `for start in G.nodes()` (`:158`) — insertion order. SciGraphs builds the
  graph as `add_nodes_from(range(n))` (`common.py:239`) and the motor's node `i` *is* that
  index, so this is dense index order ascending. `adjacency::neighbours` reproduces it.
- **Neighbour order.** `G.neighbors(node)` (`:169`) — edge-first-insertion order, one
  entry per distinct neighbour, a self-loop listed once. `adjacency::neighbours` documents
  and does exactly this.
- **Which colour is set 0.** `color[start] = 0` (`:162`), `part = [[start], []]` (`:161`):
  the BFS root seeds colour 0 and therefore **set 0**. Not a majority vote, not the larger
  side.
- **BFS, not DFS.** `queue`/`head` (`:163-167`) is a FIFO, and each set is appended in
  **discovery** order (`:172`), not in sorted or dense order.
- **Component order.** Ascending `start`, so by first-uncoloured dense index; `set0` and
  `set1` accumulate across components by `extend` (`:179-180`).
- **The per-component flip.** `if abs(skew + len0 - len1) > abs(skew + len1 - len0)` (`:177`),
  where `skew = len(set0) - len(set1)` *before* this component is added. A **strict** `>`,
  so a tie keeps the start side. `skew` is recomputed per component, which is why it is not
  simply "put the smaller side second".
- **The greedy cut.** A node goes to side 1 exactly when `placed[0] > placed[1]` (`:194`) —
  a tie goes to side **0** — where `placed` counts only neighbours *already* placed. Then
  at most `_MAX_CUT_PASSES = 8` (`:5`) single-vertex passes, flipping iff
  `around[side[node]] > around[1 - side[node]]` (`:203`) and breaking when a pass moves
  nothing (`:206-207`).
- **Fallback is whole-graph.** `_bipartite_parts` returns `None` at the **first** odd cycle
  or self-loop (`:174-175`) and the entire graph — not the offending component — goes to
  the greedy cut. A port that coloured component by component and fell back only for the
  bad one draws a different picture on any graph that has both kinds of component.

**`partition.rs` already was all of that**, line for line: dense-ascending component order
(`:31`), BFS with the root on colour 0 (`:50-52`), discovery-order append (`:59`), the same
strict-`>` flip on the same `skew` (`:36-40`), `side = 1 iff placed[0] > placed[1]` (`:74`),
8 passes with the same strict flip and the same early break (`:76-88`). So the port was
**reused, not re-written**, behind one new named entry point,
`layout::bipartite::node_sets` — a second copy of `_bipartite_parts` could only ever drift
from the first. `layout.bipartite`'s own output is unchanged; it calls the same function.

Two of the four test fixtures exist only because of the last two bullets, and neither a
4-cycle nor a triangle can catch them:

- A 4-cycle: one component, colouring `{0,2}`/`{1,3}`, so `z` alternates in **set** order
  and not by dense index.
- A triangle: not bipartite, so the greedy cut runs — but it lands on `{0,2}`/`{1}`, the
  split a colouring would have chosen, so this fixture cannot tell the two rules apart.
- Three components (two edges plus a lone node): the skew rule and the component order.
- **An odd cycle beside an even one**: this is the fixture that separates the two rules.
  Both components would two-colour, but the whole graph is cut, so the 4-cycle is never
  coloured and its nodes land on the *same* plane as the triangle's.

## Before and after

| | tier | cause | f64 | f32 | max ULP | max gap | Procrustes med | Procrustes max |
|---|---|---|--:|--:|--:|--:|--:|--:|
| before | `shape` | `algorithm` | 2/1020 | 2/1020 | 9.22e+18 | 4 | 0.405 | 0.437 |
| after | `tolerance` | `arithmetic` | 480/1020 | **1020/1020** | 2.65e+08 | 1.18e-07 | 3.57e-16 | 5.26e-16 |

**`f32` reaches 1020/1020 and that is the row's exact target**, because the motor's
`Geometry` is `f32` (`layout/basic_3d.rs:50-61` narrows once) so the reachable exact target
*is* the `f32` column. `f64` stops at 480/1020 and the reason is numpy's own arithmetic, not
the port: numpy's `cos`/`sin` are its kernels, `libm`'s are a different correct
implementation of the same function, and they differ in the last ulp on some arguments —
the `sg-common` caveat. The row is `tolerance` / `arithmetic` for that reason and not
`bitwise`, and the Procrustes columns (3.6e-16) say the *shapes* are identical, which is
the claim that 0.405 was denying.

The graph-core tests pin both halves of that: the `f64` columns to within **4 ULP** of the
reference bit patterns (the one tolerance in the file, and the reason it is a ULP count
rather than a distance), and the `f32` snapshot **exactly**, over all four fixtures, because
narrowing 53 bits to 24 swallows the disagreement on every node measured.

Every bit pattern in the test table was generated once by running the reference itself in
`ge-python-oracle` on those exact graphs — `nx.Graph()`, `add_nodes_from(range(n))`,
`add_edges_from(pairs)` — and the Rust literal block was emitted by that script rather than
transcribed by hand. The first hand transcription got two sign bits wrong on the `y` of node
4 and node 6, which is what the RED run caught.

## Commands

```text
scripts/orch/gr cargo build --release -p graph-cli                       -> 0
scripts/scigraphs-conformance.sh            (untouched tree)            -> 0  PASS
scripts/orch/gr cargo test -p graph-core --lib bipartite_3d             -> RED:  2 passed, 3 failed
scripts/orch/gr cargo test -p graph-core --lib bipartite_3d             -> GREEN: 5 passed, 0 failed
scripts/scigraphs-conformance.sh            (after the port)           -> 1  FAIL, BIPARTITE_3D only
scripts/scigraphs-conformance.sh            (after the re-pin)         -> 0  PASS
scripts/scigraphs-conformance.sh --break                                -> 1  FAIL, names SPRING_3D
scripts/orch/gr cargo fmt --all -- --check                               -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings    -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                   -> 0  (every suite ok)
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8          -> 0  PASS
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 ... hashgate --seeds 8   -> 1  (gate), so the
                                                                    negctl-degree row's
                                                                    `test $? -eq 1` -> 0
scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check   -> 0
```

The RED line is worth its own note: the three failures were **wrong reference constants and
one wrong assertion of mine**, not three bugs in the port. The ULP helper also overflowed
(`i64::MIN - bits`) and was fixed with `wrapping_sub`. Two of the five tests passed on the
first run because the 4-cycle and the triangle are exactly the two fixtures that cannot
distinguish this layout from `layout.bipartite`.

## Gaps

`G_NO_ITERATIONS` and `G_SNAPSHOT_SCALE` become `G_NO_ITERATIONS` and `G_BASIC3D_SCALE`.
Both name the same number at the same line (`layout/basic_3d.rs:43`), and the row now reads
its scale through `basic_3d::SCALE` exactly as `SPHERE`, `HELIX` and `CUBE` do, so the
basic_3d-worded gap is the accurate one. `G_NO_ITERATIONS` still holds: the colouring and
the placement are both closed forms with no budget to bound. No gap const was left orphaned
— every one of the three is still named by another row.

## The hash gate

`hashgate --seeds 8` exits 0 and **`layout.bipartite_3d` reports `4-way equal on 8/8 seeds`**,
which is prompt.md §6 stated as a result rather than as an intention: the drawing is
bit-identical native against wasm32, two runs each. That is the one check that could have
failed on a new layout and did not, and it is the reason the id is appended rather than
inserted — the gate walks `LAYOUTS` by position.

The run is the repo's own `hashgate-8` row (`scripts/orch/rows/quick.rows:5`) and the control
is its `negctl-degree` row (`:6`): the gate exits 1 under `GM_MUTATE_REFERENCE_DEGREE=9` and
the row's `test $? -eq 1` converts that to 0. The timed `--seeds 1000` row is left to the
orchestrator, as the job preamble reserves it.

## Left undone, on purpose

**The `f64` column is not bitwise and will not become so** without reimplementing numpy's
transcendental kernels, which prompt.md §6 forbids (`libm` only). The reachable exact target
is `f32`, and it is 1020/1020.

**No hash-gate knob for `layout.bipartite_3d`.** The three natively 3D layouts have
per-stage controls (`knobs::THREE_D_LAYOUT_STAGES`), and adding this stage to that table
means touching four more files (`hashgate/knobs.rs`, `hashgate/knob/three_d.rs`,
`hashgate/knob/arms.rs` and `hashgate/tests/knob/three_d.rs`) plus the `Knob::ALL` count.
Nothing requires it: no test asserts that every registered layout has a control, the count
in `Knob::ALL` is derived from the arm tables rather than from `LAYOUTS`, and this change does
not touch `GM_MUTATE_REFERENCE_DEGREE` or any other knob arm — which is why the control above
still exits 1. It is a real gap, not a closed one, and it belongs to a job about the hash gate
rather than to this row: this stage is *hashed*, it is not *controllable*.

## Capabilities routing

`capabilities --check` reports 36 problems in this tree, all of the form `gated, but no
<record>: run the gate` and none of them this row: the gate records are simply absent until
the orchestrator runs the gates. The new row publishes `status: implemented` and its
`oracle_diff` reads `scigraphs-conformance record is from another tree: re-run the gate`,
which is the proof that the record name resolves to the real record the conformance script
writes — `scripts/scigraphs-conformance.sh` step 6. It is deliberately **not**
`oracle-closed-form`, which is `layout.bipartite`'s record and covers networkx's columns.

## Files

New: `crates/graph-core/src/layout/basic_3d/bipartite_3d.rs`,
`crates/graph-core/src/layout/basic_3d/bipartite_3d/tests.rs`,
`crates/graph-core/src/registry/three_d/bipartite_3d.rs` (a child module, because
`registry/three_d.rs` was already at 296 of the 300-line cap).

Touched beyond the job's path list, all forced and all minimal:
`layout/bipartite.rs` (one named entry point, output unchanged), `registry.rs` (append +
`35`→`36`), `capabilities/registry/unproven.rs` and `capabilities/tests/registry.rs` (the
routing arm, or the row falls through to `roundtrip`/`Gated` and `capabilities --check`
fails on it), `hashgate/tests/report.rs`, `snapshot_cmd/tests.rs` and
`snapshot_cmd/roundtrip/tests.rs` (three pinned strings that enumerate every registered
layout), and `gaps.rs` (one gap's note, to name the fourth `basic_3d` consumer).