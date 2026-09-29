# Phase 8 — grid routing: what was measured, and what the measurements changed

Scope: the `route` slice only — `post::grid_index` (the uniform grid) and `post::routed`
(obstacle-avoiding routing over it), registered as **`post.route.grid`**. The style
generators, FDEB and MINGLE are other slices and are not measured here.

Every number below is copied from a run's output in this tree, in `ge-rust`, on the gate's
hardware. None is hand-derived. The measurement tests are `#[ignore]`d and live in
`crates/graph-core/src/post/routed/measure.rs`; they are kept in the tree rather than
deleted after use, because a number nobody can re-run is a number nobody can check.

```sh
cargo test -p graph-core --release -- --ignored --nocapture routing_measurement
cargo test -p graph-core --release -- --ignored --nocapture resolution_measurement
cargo test -p graph-core --release -- --ignored --nocapture density_measurement
cargo test -p graph-core --release -- --ignored --nocapture gap_measurement
```

The layouts used are a jittered lattice filling `0..8` on both axes — **synthetic, and the
one weakness of every table here**. Real layouts (force, Sugiyama, tidy tree) are Phase 8
step 6's composability gate, not this slice's, so these numbers characterise the *algorithm's*
cost curve and its *failure thresholds*; they are not a claim about any shipped layout's
density. The edges are the hash gate's own synthetic model at `REFERENCE_DEGREE`, so the
edge counts are the ones the 4-way gate already uses.

## 1. Cost: linear in edges, quadratic in resolution

`routing_measurement` — default parameters (resolution 128, margin 2, clearance 0):

| n | m | cells | occupied | bytes | wall-clock | per edge |
|---|---|---|---|---|---|---|
| 200 | 299 | 16 104 | 200 | 16 904 | 457.5 ms | 1 530 µs |
| 500 | 768 | 16 764 | 500 | 18 764 | 1 278.6 ms | 1 665 µs |
| 1 000 | 1 541 | 17 424 | 1 000 | 21 424 | 2 795.0 ms | 1 814 µs |
| 2 000 | 3 075 | 17 424 | 2 000 | 25 424 | 5 890.0 ms | 1 915 µs |
| 5 000 | 7 721 | 17 424 | 5 000 | 37 424 | 15 566.2 ms | 2 016 µs |

**Per-edge cost is flat at 1.5–2.0 ms from 299 edges to 7 721 — a factor of 26 in `m` for a
factor of 1.3 in cost per edge.** That is the declared `O(m · cells · log cells)` with
`cells` pinned by the resolution rather than by `n`, and it is the number that sets
`scale_ceiling`. It is also the honest cost of the simple thing: **one Dijkstra per edge
over the whole grid**, where the reference solves one field per *distinct source node* and
reuses it across that node's edges (`routed.py::_plan`: 168 fields for 592 edges on 240
nodes, against 592 here). The saving available is roughly the average degree; the cost of
taking it is a field cache and the memory to hold it. Not taken in this slice, and named
here as the first thing to take if the ceiling is ever hit in anger.

At the default resolution the grid is ~17 400 cells whatever `n` is, so the per-edge cost is
set by the **resolution**, not the graph. That is the trade the phase's Ponytail names, and
`resolution_measurement` prices it — one fixed graph, every resolution:

| resolution | cells | occupied | bytes | wall-clock | fallbacks |
|---|---|---|---|---|---|
| 16 | 380 | 240 | 1 340 | 12.2 ms | 302 |
| 32 | 1 224 | 240 | 2 184 | 51.8 ms | 0 |
| 64 | 4 352 | 240 | 5 312 | 156.8 ms | 0 |
| 128 | 16 236 | 240 | 17 196 | 576.6 ms | 0 |
| 256 | 62 920 | 240 | 63 880 | 2 731.4 ms | 0 |

**×16 the resolution is ×224 the time** (12.2 ms → 2 731 ms), against ×165 the cells. The
excess over the cell ratio is the `log cells` and the worse cache behaviour, and it is the
"quadratically more expensive" the phase warns about, confirmed rather than asserted.
Resolution 32 already routes this graph with zero fallbacks at **1/11 the cost** of the
default; the default of 128 is the reference's `routed_resolution`, carried over unchanged,
and the table says it is 13× more expensive than this graph needs.

**`scale_ceiling` is 5 000 nodes**, from the first table: 5 000 nodes and 7 721 edges is
15.6 s, and 8 000 nodes would be ~35 s on the same hardware. It is a **time** ceiling on the
gate's hardware at the default resolution — not a memory limit (37 KB of grid at n = 5 000)
and not a correctness limit. Past it routing computes the same routes, in the same order,
more slowly; there is no cutoff and no silent degradation.

## 2. Memory

The grid index holds one `u8` and one `u32` per cell and per node: **at most 5 bytes per
cell plus 4 per node**, which is the `bytes` column above — 37 424 B at n = 5 000 against
Phase 3's 919 B per node for the whole topology pipeline. Routing's own footprint is
negligible next to the layout that produced the positions, and the transient Dijkstra
structures (a visit bit-set and a heap over the cells) are not in that figure.

## 3. The measurement that changed the default: `clearance`

`density_measurement` sweeps the one parameter whose obvious value is wrong. `clearance` is
added around every node's footprint; at **1.0** — a whole cell — a node's own cell plus the
eight around it is a solid 3 × 3 block, so **every node is enclosed and no route can ever
leave one**:

| pitch (cells) | nodes | edges | clearance | occupied | % blocked | routed | fallbacks |
|---|---|---|---|---|---|---|---|
| 16 | 12 | 18 | 0 | 12 | 0.1% | 18 | 0 |
| 16 | 12 | 18 | 0.4 | 39 | 0.3% | 18 | 0 |
| 16 | 12 | 18 | 0.9 | 88 | 0.8% | 6 | 12 |
| 16 | 12 | 18 | **1.0** | 108 | 0.9% | **0** | **18** |
| 8 | 56 | 84 | 0 | 56 | 0.4% | 84 | 0 |
| 8 | 56 | 84 | 0.4 | 160 | 1.1% | 84 | 0 |
| 8 | 56 | 84 | 0.9 | 407 | 2.7% | 25 | 59 |
| 8 | 56 | 84 | **1.0** | 504 | 3.4% | **0** | **84** |
| 4 | 240 | 362 | 0 | 240 | 1.5% | 362 | 0 |
| 4 | 240 | 362 | 0.4 | 772 | 4.8% | 362 | 0 |
| 4 | 240 | 362 | 0.9 | 1 849 | 11.4% | 73 | 289 |
| 4 | 240 | 362 | **1.0** | 2 160 | 13.3% | **0** | **362** |
| 2 | 992 | 1 529 | 0 | 992 | 5.9% | 1 529 | 0 |
| 2 | 992 | 1 529 | 0.4 | 3 211 | 19.0% | 1 529 | 0 |
| 2 | 992 | 1 529 | 0.9 | 7 717 | 45.7% | 240 | 1 289 |
| 2 | 992 | 1 529 | **1.0** | 8 852 | 52.4% | **0** | **1 529** |

**At `clearance: 1.0` every edge of every layout falls back, at every density, including
0.9% of cells blocked.** Not "most" — all, because the sealing is local and total. The
capability is not degraded at that setting, it is *off*, and nothing but the flag count
would tell a caller.

So `GridParams::default().clearance` is **0.0**, and the type is `f64` rather than a cell
count so the useful sub-cell range is expressible: 0.4 of a cell widens every node and
still routes everything (the `0.4` rows), and the cliff is between 0.4 and 1.0. This is
pinned by `grid_index::tests::a_whole_cell_of_clearance_seals_every_node_which_is_why_the_default_is_zero`,
which fails if the default is set back to 1.

**A bug this measurement caught, which the unit tests did not.** The first run of this
sweep reported 0 routes at *every* setting including `clearance: 0`, on a graph 99% free —
impossible, and the unit tests were green. Two real defects, both in `routed.rs`:

1. **`solve` exempted only the target cell, not the source.** A node sits in its own cell,
   so the source is occupied like any other and every edge *out* of it was `f64::INFINITY`:
   Dijkstra never left. The unit tests missed it because their fixtures either had adjacent
   endpoints or a sealed graph, so the source never had to be walked out of.
2. **The petgraph trait impls were split across two types** — `GraphBase`/`Visitable` on
   `GridCsr`, `IntoEdges` on `&GridCsr`. `dijkstra` needs `IntoEdges + Visitable` on *one*
   type, so neither combination satisfied the bounds, and the code compiled anyway. Fixed by
   a single `Copy` newtype `GridGraph<'a>` carrying all the impls, which is Phase 7's
   `CsrDigraph` shape in `csr_petgraph.rs`. **This is the failure mode to watch for in the
   rest of the project: a split petgraph impl compiles and silently searches nothing.**

## 4. The narrow gap: measured, and it is not where the Ponytail expected

`gap_measurement` — a wall with a gap in the middle, the gap swept from 8 cells to none,
at resolution 32 (cell = 0.25):

| gap | cells wide | fallbacks | wall-clock |
|---|---|---|---|
| 8 cells | 2.00 | 0 | 0.479 ms |
| 4 cells | 1.00 | 0 | 0.373 ms |
| 2 cells | 0.50 | 0 | 0.246 ms |
| 1 cell | 0.25 | 0 | 0.236 ms |
| 0 cells | 0.00 | 0 | 0.214 ms |

**No fallbacks at any gap width, including a gap of zero cells.** The `margin: 2` means the
grid extends two cells past the drawing on every side, so a wall that reaches the drawing's
bounding box does not reach the grid's: a route always has somewhere to go. The phase's
Ponytail predicted "a detour, or a straight line through a node" for a gap narrower than a
cell, and what the measurement shows is that the **detour** is what happens, every time,
because the margin guarantees a way round. The straight-line-through-a-node case needs the
graph to be *sealed*, which is what `clearance` does (§3) and what a ring does
(`routed::tests::an_enclosed_node_falls_back_to_the_straight_segment_and_sets_the_flag`).

The Ponytail stands as written — the failing input is real and the flag reports it — but the
measured mechanism is **enclosure, not gap width**, and the ledger's `ponytail` text says
so rather than leaving the reader to infer it.

## 5. Determinism

`routed::tests::routing_twice_over_the_same_input_gives_the_same_bytes` runs the whole
routing twice over a five-node graph and compares the entire `Routed` value, then runs it
twice more over a *reused* grid buffer and requires all four to agree — so a buffer that
appended rather than refilled would fail. `a_symmetric_wall_…` pins the exact cell sequence
of a route whose two alternatives cost bit-identically
(`field[70] == field[94]`, both `1.2071067811865476e1`), which is the case the tie-break
exists for.

Every mutation below was applied to the tree and re-run; each is caught:

| # | Mutation | Caught by |
|---|---|---|
| M1 | tie-break `(cost, cell)` → `(cost, cell) <=` | **no — equivalent**, see below |
| M2 | tie-break drops the cell index (`cost < seen`) | `the_tie_break_is_by_cell_index_…` |
| M3 | plateau / loop guard removed | `a_field_that_never_descends_…` |
| M4 | descend on `dist[n]` alone (the reference's named bug) | `the_polyline_…`, `a_symmetric_wall_…` |
| M5 | source-cell exemption dropped from `solve` | 4 tests |
| M8 | boundary snap removed (`floor` only) | `a_boundary_point_snaps_up_…` |
| M9 | `BOUNDARY_TOL` 1e-9 → 2e-9 | `a_boundary_point_snaps_up_…` |
| M10 | `clearance` ignored | `a_whole_cell_of_clearance_seals_every_node_…` |
| M11 | `margin` ignored | 3 tests |
| M12 | cubic cell → per-axis cell | `an_unequal_layout_still_gets_square_cells_…` |
| M13 | diagonal cost `√2` → 1.0 | `equal_cost_routes_take_the_lower_cell_index_…` |
| M16 | x/y swapped in the flat index | `the_polyline_…`, `a_symmetric_wall_…` |
| — | default `clearance` 0.0 → 1.0 | `a_whole_cell_of_clearance_seals_every_node_…` |

**M1 is a provably equivalent mutant, not a gap.** `(cost, next) <= (seen, cell)` compares
tuples: it differs from `<` only when *both* components are equal, i.e. when `next == cell`
— the same cell compared with itself, which a scan over distinct row entries never does. It
is reported under "decisions needed" rather than papered over with a test that cannot fail.

M2 was the instructive one. Six tie-break mutations were initially **not** caught, because on
a real layout the stencil order and the cell-index order *agree* for every pair of
neighbours at equal distance: the stencil is sorted by `(dx, dy)` and the cell index by
`y` then `x`, and for same-distance neighbours the two orders coincide. Only a hand-built
distance field — one where two candidates are given bit-identical costs by fiat — separates
"lowest cell index" from "first in the row", so that is what
`the_tie_break_is_by_cell_index_and_not_by_the_order_the_row_is_walked` constructs.

## 6. Deviations from the reference and from the phase text

1. **The solver is petgraph's Dijkstra, not the reference's Jacobi Bellman-Ford.**
   `routed.py` relaxes by sweeps because "Dijkstra wants a priority queue a compute shader
   cannot have" — a GPU-shaped choice, and this is tier 1a. The phase text also forbids a
   third shortest-path implementation, and Phase 7's is petgraph's. **The trace is the
   reference's**, including the rule that killed a first draft here: the step minimises
   `dist[n] + w(n, x)`, not `dist[n]` alone (M4).
2. **The reference's soft per-cell costs are absent.** `density_cost` and `_reinforced` make
   routes *share* cells, which is edge bundling — `post::fdeb` / `post::mingle`, other
   slices. This grid marks cells as obstacles and nothing else, so every free cell costs 1.
3. **The reference's `_frame` (a PCA rotation into the graph's principal plane) is absent.**
   It exists because SciGraphs routes in 3D; the motor's node geometry is 2D.
4. **One Dijkstra per edge, not one field per source node** (§1).
5. **`petgraph` was added to `graph-core`'s `Cargo.toml` on this branch.** Phase 7 puts it
   on the allow-list with the pin `=0.8.3`; this branch (`p8-route`) forked from `p3`,
   before p7 landed, so the dependency had to be brought forward. It is the same crate, the
   same pin and the same features, so the merge is additive — but it is a real change to a
   file outside this slice's envelope, and at merge time p7's own `Cargo.toml` edit will
   conflict with it trivially.
6. **`post.route.grid` is registered as `implemented`, not `gated`.** Routing is wired into
   no hash-gate stage and no oracle differential — both live in `graph-cli`/`graph-wasm`,
   outside this slice's envelope — so there is no evidence to back a `gated` claim, and
   `problems()` only demands evidence from a `gated` row (`prompt.md` §8). Same position
   Phase 7 took for its `analysis.*` rows. `capabilities --check` confirms it: **14 rows,
   26 problems** — one more row than before this slice and the same problem count, which is
   what an honest `implemented` row looks like.
7. **`fixtures/post/obstacles.json` is not parsed by any test.** graph-core is pure and has
   no file I/O, so the fixture records the three scenarios and the tests construct the same
   geometry in code. This is the same gap Phase 7's review recorded for
   `fixtures/analysis/*.json` and it is **not fixed here**; the fixture's own `cases` block
   names the test that covers each scenario so the pairing is checkable by hand.

## 7. Decisions needed

- **`fixtures/post/obstacles.json` is unparsed** (deviation 7). Recommended: leave it as the
  scenario record for this slice, and have the phase's integration step add a graph-cli test
  that loads every `fixtures/post/*.json` and compares its node/edge ids against a restated
  Rust model, the way `oracle_fixtures` already does for the oracle's inputs. Doing it in
  graph-core is not possible without giving the pure crate file I/O.
- **`scale_ceiling: 5 000` is measured on a synthetic lattice** (§1). It is a defensible
  time ceiling, but a real force layout at 5 000 nodes has a different node *density*, and
  §3 shows density changes the fallback count sharply between `clearance` 0.4 and 1.0.
  Recommended: re-measure against the Phase 8 composability layouts once step 6 lands, and
  revisit the number then rather than treating 5 000 as settled.
- **The one-field-per-source-node optimisation is not taken** (§1, deviation 4). It is worth
  roughly the average degree — about 1.5× on these models, more on hub-heavy graphs.
  Recommended: leave it out until a caller hits the ceiling; it trades a simple,
  exactly-reproducible function for a cache whose reuse pattern is part of the output.
