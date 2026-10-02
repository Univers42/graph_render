# sg-fix-spring-temp — the spring3d opening temperature, and the circle-packing self-loop degree

Two defects a 2026-10-01 review of every layout against SciGraphs and its oracles found. Both
change coordinates on inputs the oracle accepts, so both were fixed TDD-style (RED first),
re-measured against the SciGraphs conformance gate, and re-pinned where the gate said so.

Everything here ran in this job, on this tree. The commands are the ones that produced the
numbers, in the order they were run.

| item | file | defect | reference |
|--:|---|---|---|
| 1 | `layout/force/spring/forces.rs` | opening temperature read all `D` columns | `networkx-3.6/networkx/drawing/layout.py:687` (dense), `:776` (sparse) |
| 2 | `layout/circle_packing/fallback.rs` | `initial_radii` dropped the self-loop degree | `SciGraphs/.../circle_packing.py:420`, `.../common.py:297` |

## 1. The opening temperature read a column the reference never reads

networkx's `spring_layout` sets its first step's bound as

```python
t = max(max(pos.T[0]) - min(pos.T[0]), max(pos.T[1]) - min(pos.T[1])) * 0.1
```

— `layout.py:687` in the dense branch, `layout.py:776` in the sparse one, **the same two
spans at every `dim`**. The port read the widest of all `D` columns, so at `dim = 3` a start
whose `z` span was the widest opened hotter than the reference ever opens a layout. The old
doc comment cited `layout.py:705-706`, which is not that line.

`opening` became a free function over `Field<D>` (it was a `Solver` method that read no
`solver` state, and a test inside `forces` cannot reach a private method without widening
visibility), and the doc now cites 687 and 776.

### The differential: networkx's own `t`, captured from the running interpreter

There is no `dim = 3` networkx oracle in this repo — `oracle_python/spring.rs` is `dim = 2`
only, and the registry's `oracle` field for `layout.force.spring3d`
(`registry/three_d.rs:241-268`) names the *same* `oracle-spring` harness re-run at `dim = 3`,
with the ledger recording it as `oracle-spring` for both ids
(`capabilities/registry/unproven.rs:175`). So the differential was run directly in
`ge-python-oracle` (networkx 3.6) on one fixture with a z-dominant start, with the reference's
`t` read out of the interpreter by a line tracer rather than restated:

```
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \
  python3 target/spring3d-opening-differential.py
```

| | |
|---|--:|
| `x_span` | 1.9196570835928177 |
| `y_span` | 0.9491860458049055 |
| `z_span` | 38.29972275699565 |
| `z_is_the_widest_span` | true |
| **reference `t`, captured at `layout.py:691`** | **0.19196570835928178** |
| reference `dt` | 0.0037640334972408192 |
| motor `t` **after** the fix | 0.19196570835928178 |
| motor `t` **before** the fix | 3.829972275699565 |
| before / after | 19.951335623607388 |

The reference's own number is **bit-identical** to the motor's after the fix
(`0.19196570835928178`) and **19.95x** cooler than the motor's before it. That is the defect,
measured against the reference rather than against a restatement of it.

The negative control is the same run's shape: `pos_columns_the_reference_solved` is 3, so the
traced `t` really is the three-dimensional one, and the fixture is the named failing input
(`z` the widest span). A control that only asserted "the formula is what I wrote" would have
passed with the bug in place.

## 2. A self-loop is degree twice over, and the port counted it zero times

The job's precondition was to check how `apply_graph_layout` builds `G`, because if it drops
self-loops item 2 is not a defect. **It does not drop them**, so it is one.

- `apply_graph_layout` (`dispatcher.py:37`) calls `_build_networkx_graph`.
- That builds `G = nx.Graph()` and `G.add_edges_from(edge_indices)` at **`common.py:297`** —
  no `u != v` filter. (The `simple` copy `_planar_triangulation` makes *for itself* does
  filter, `circle_packing.py:61`; that is a different graph.)
- `_circle_packing_layout` hands that same `G` to the non-planar fallback at
  **`circle_packing.py:311`**, which reads `degrees = [G.degree(n) ...]` at
  **`circle_packing.py:420`**.
- networkx counts a self-loop twice: `DegreeView.__getitem__` is
  `len(nbrs) + (n in nbrs)` (`classes/reportviews.py:526`).

The port's `initial_radii` counted each incident edge once over a list `simple_pairs` had
already reduced, self-loops gone — so a node whose only edge was its own loop came out on the
`0.3` isolated floor where networkx gives it the full `1.0`. `loop_counts` now carries the
per-node count across the reduction into the starting degree.

**The fallback is not the only thing that sees it.** `_circle_packing_force_directed` also
builds its spring array from `G.edges()` (`circle_packing.py:432`) and seeds with
`nx.spring_layout(G, ...)` (`:428`), both on the loop-carrying `G`. This port still reduces
them away for the springs, which is the existing documented decision (a zero-length spring
fights the overlap pass) and is **out of this job's scope**; it is recorded in §5 as the
residual divergence, not fixed here.

## 3. What the conformance gate said, before and after

```
scripts/orch/gr cargo build --release -p graph-cli
scripts/scigraphs-conformance.sh          # BEFORE, both fixes reverted -> exit 0
scripts/scigraphs-conformance.sh          # AFTER,  both fixes in place  -> exit 1
```

**BEFORE exit=0. AFTER exit=1**, naming one row:

```
SPRING_3D: FAIL — motor bytes are not the pinned ones (sha 7cfcdd38ce96eb9113a08ffda8a43a55f79f5dbf5f0addf40049b735139f097f)
```

**`SPRING_3D` was the only row that moved.** No row that is neither spring nor circle packing
moved, so there is no regression to report. Only `SPRING_3D`'s first sha was re-pinned, in
`baseline/table/basic.rs`; `CIRCLE_PACKING`'s two shas are byte-identical in the proposed
baseline.

### The moved row's disparity did not move

| `SPRING_3D` | before | after |
|---|--:|--:|
| `bitwise f64` | 3/1020 | **3/1020** |
| `bitwise f32` | 4/1020 | **4/1020** |
| max ULP | 9.23e+18 | 9.23e+18 |
| max gap | 10 | **10** |
| Procrustes median | 0.1721478627737976 | **0.1721478627737976** |
| Procrustes max | 0.756094635718182 | **0.756094635718182** |

Per fixture, 6 of the 20 gate models shifted, all of them small and in **both** directions —
this is chaos, not a trend:

| fixture | max gap before → after | Procrustes before → after |
|---|---|---|
| `gate-14` | 6.289890733110708 → 6.2993104127524315 | 0.3286 → 0.3244 (worse) |
| `gate-15` | 7.651733812361295 → 7.778469976454312 | 0.4125 → 0.4212 (better) |
| `gate-16` | 6.278747720484159 → 6.218975705866239 | 0.2087 → 0.2137 (better) |
| `gate-17` | 7.527728023911499 → 7.527728023911499 | 0.1210 → 0.1325 (better) |
| `gate-18` | 9.936231136322021 → 9.822751522064209 | 0.2193 → 0.2171 (worse) |
| `gate-19` | 6.577035789346778 → 6.70718062386521 | 0.1350 → 0.1367 (worse) |

`gate-14`'s Procrustes got worse (0.3286 → 0.3244, −1.3%) and `gate-18`'s too (−1.0%); the
row median is unmoved to the last digit. **Per the job, a row that got worse is reported, not
re-pinned silently** — it is reported here, and the re-pin is of the whole row's motor bytes,
which is what moved.

`CIRCLE_PACKING` did not move at all, and the reason is a property of the fixture set, not of
the fix: **not one conformance fixture has a self-loop.** The gate models are built by
`synthetic_edges`, which returns early on `a == b` (`synthetic.rs:115-117`); `lesmis.json`,
`diamond.json` and `tree-balanced.json` have none; `bipartite` is `K(6,8)`. So item 2 is
correctness on the oracle's terms with **zero** effect on this gate, and its only evidence is
the unit test. That is stated rather than dressed up as a measured win.

### Why the SPRING_3D disparity is unmoved — and why that is the expected result

The motor's `dim = 3` start is drawn axis-by-axis from one Mulberry32 stream
(`spring.rs:start`), so its three spans are near-isotropic and the two rules usually agree.
Replaying that draw at the fixtures' own node counts:

| n | x | y | z | widest-of-3 / max(x,y) |
|--:|--:|--:|--:|--:|
| 77 | 0.998 | 0.984 | 0.983 | 1.0000 |
| 18–27 | 0.940 | 0.944 | **0.963** | **1.0201** |
| 33–48 | 0.998 | 0.955 | 0.963 | 1.0000 |

The two rules differ by **at most 2.01%** on any real fixture, and by 0% on most. That is
precisely why the row's disparity is unchanged while its bytes are not, and why re-pinning it
is safe. It is also why §1's fixture has to be *constructed* to be z-dominant: the 19.95x
divergence is real but only reachable on a start the gate does not contain.

## 4. The tests

Four new tests, each RED before the fix and GREEN after.

```
scripts/orch/gr cargo test -p graph-core --lib -- forces::tests circle_packing
```

- `a_three_column_start_opens_on_its_x_and_y_spans_and_not_on_its_z` — z span 100, x span 1.
  RED: `left: 4621819117588971520` (100 × 0.1), `right: 4591870180066957722` (0.1).
- `the_opening_temperature_is_the_larger_of_the_first_two_spans` — the answer follows whichever
  of x/y is wider, and moving the third column moves nothing. RED: `left: 4641240890982006784`.
- `a_two_column_start_still_opens_on_its_only_two_spans` — the `D = 2` control, which passed
  before and after, as it must: this is a `dim = 3` defect.
- `a_self_loop_counts_twice_in_the_starting_radii_as_it_does_in_networkx` — degrees
  `[0, 1, 1, 2]`, the loop at node 3 counting twice. RED: `left: 4597352824910773992`,
  `right: 4604170041854275266`.
- `loop_counts_are_per_node_and_skip_every_other_pair` (`circle_packing/tests/edges.rs`) —
  the production `loop_counts`, and that the reduced list and the count are independent.

**One existing test had to change**, and it is worth naming because it encoded the defect.
`a_self_loop_does_not_change_the_fallback_packing` asserted that a self-loop leaves the
fallback packing untouched. Its own comment had the facts right and drew the wrong conclusion
from them — it said "`G.degree` on a simple graph counts it as two of that node's incident
edges" and then required the loop to have no effect at all. It is now
`a_self_loop_reaches_the_fallback_as_a_degree_and_not_as_a_spring`, which asserts what the
reference does: the loop **moves** the packing, through the radius, and never becomes a spring
(finite centres, non-negative radii). Nothing was weakened to reach green — the assertion
moved in the opposite direction, from "no effect" to "an effect".

## 5. What this file does not claim

- **The circle-packing fallback's springs and seed still differ from SciGraphs'** on a graph
  with self-loops: SciGraphs feeds the loop-carrying `G` to `nx.spring_layout` (`:428`) and to
  its edge array (`:432`); this port reduces loops away for both. Only the **degree** is
  repaired here. Closing the rest is a separate, larger change.
- **No `dim = 3` networkx oracle harness exists.** §1's differential is a direct run in
  `ge-python-oracle`, not a gate row, and is not re-run by `quick.rows`.
- **The fix is disparity-neutral on this fixture set, not a measured improvement** (§3). The
  claim it supports is conformance with the reference's stated rule, which §1 measures at
  19.95x on a z-dominant start.
- **`metrics.json` is not byte-compared before/after.** It carries the tree fingerprint, and
  the tree changed by design. The row lines and the `motor/*.f64` shas are what were compared.