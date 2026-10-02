# Layout review — hierarchy, tidy_tree, sugiyama, hierarchical_3d, radial, treemap, circle_packing

Scope: `crates/graph-core/src/layout/{hierarchy,tidy_tree,sugiyama,hierarchical_3d,radial,treemap,circle_packing}/**`
and their `.rs` parents, plus their rows in `crates/graph-core/src/registry*`. Reviewed as
`origin/develop` stood at the start of this job. Review only: no code was changed.

Every row below was verified by reading the cited code, or by running the cited command in this
job. Where the conformance matrix already names a divergence, the row cites that matrix row
instead of re-reporting it.

## Findings

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| L-01 | MAJOR | `layout/radial/twopi.rs:109`, `twopi/tree.rs:52-54,92` | `run` loops over components and calls `tree::grow` per component, but `search` and `count_leaves` each allocate three `vec![..; neighbours.rows()]` — **n**-length — inside that loop, so cost is O(components x n), not the declared O(n+m) | edgeless graph, n components = n nodes | measured, 32x nodes -> ~232x time (`500:0.49ms 1000:1.01ms 2000:3.29ms 4000:13.49ms 8000:47.76ms 16000:115.14ms`, scratch test since deleted; see "Commands"); declared complexity `registry/radial.rs:48` | size the per-component buffers to the component and carry one reusable scratch buffer through the loop, or hoist `search`/`count_leaves` into a single pass over all roots |
| L-02 | MAJOR | `layout/sugiyama/ordering.rs:107-113` | `sort_by_median`'s comparator recomputes `median_position` — which heap-allocates a `Vec` and sorts it (`ordering.rs:91-93`) — on **every comparison**; a width-W layer costs O(W log W) allocations per sweep, so crossing reduction is superlinear in layer width | any graph whose widest layer is W with dummies; declared complexity `registry/grid.rs:99` says "O(n+m) per phase" | `ordering.rs:107` `keyed.sort_by(\|..\| { let (m1, m2) = (median_position(..), median_position(..)); .. })` and `ordering.rs:92` `.collect()` | precompute each movable vertex's median once into a `Vec<(usize, u32, f64)>` and sort that by `(median, slot)`, making the comparator a pure field compare |
| L-03 | MAJOR | `layout/sugiyama/ordering.rs:123-143,146-173,233` | `pair_crossings` allocates two `Vec`s and runs two `sort_unstable`s per call, 4 calls per adjacent pair, inside `transpose`; and `transpose` is re-entered once per sweep with `rounds` up to 4 | any graph below the 150k-vertex transpose cutoff | `ordering.rs:124-133` (two `collect` + two `sort_unstable` per call); `ordering.rs:153-159` (4 calls per pair); `ordering.rs:233` `transpose(..., throttle.transpose_rounds)` inside the sweep loop | sort each vertex's neighbour positions once per pass into a reusable buffer, and swap in place instead of re-collecting per comparison |
| L-04 | MAJOR | `layout/circle_packing/fallback/seed.rs:38` | `adjacency_matrix` allocates `vec![0.0; n*n]` — 8n^2 bytes — with **no cap on n** anywhere in `fallback::pack` (`fallback.rs:28`); the relaxation then sweeps all n per node per round | sparse 100k-node graph: 80 GB allocation; also `n*n` is `usize`, so the same graph overflows `usize` on wasm32 above n=65536 and not natively — a native/wasm32 divergence | `seed.rs:38` `vec![0.0; n * n]`; `seed.rs:63` `for j in 0..n` inside `displacement`, called per node per iteration; declared `registry/grid.rs:71` "O(n^2) per relaxation round on the non-planar fallback", ceiling `PACKING_CEILING = 5_000` (`grid.rs:59`) | reject `params.scale <= 0` and gate the dense Fruchterman-Reingold seed on n (the golden-angle spiral alone above ~2e4), or use the CSR the caller already has instead of a dense matrix |
| L-05 | MAJOR | `layout/circle_packing/radii.rs:194-199`, `circle_packing.rs:236-240` | `worst_free_error` folds with `f64::max`, which returns the **non-NaN** operand, so a NaN angle-sum error is dropped and the solve reports `max_error = 0.0` — which then passes `is_embedded`'s `is_finite()` and `< TOLERANCE` gate. The doc at `circle_packing.rs:231-235` states this exact hazard was closed, but only on the `spread` fold | a solve whose `angle_sum` goes NaN (reachable via the `inf/inf` renormalisation at `radii.rs:212`) ships as a "certified" exact packing with no note code 3 | `radii.rs:198` `.fold(0.0, f64::max)`; `circle_packing.rs:239` `solved.max_error.is_finite() && solved.max_error < radii::TOLERANCE` | filter non-finite errors out and return `f64::INFINITY` if any survive, so `is_embedded`'s existing finiteness check refuses it |
| L-06 | MAJOR | `layout/circle_packing/placement.rs:180,143` | same NaN-swallowing fold in `worst = worst.max(error)`: a NaN error leaves `worst` at its previous value, so the `< 1e-9` convergence break fires and `refine_tangency` returns a NaN-drifted packing as converged | a tangency pass whose edge pull produces a NaN error | `placement.rs:180` `worst = worst.max(error)`; `placement.rs:143` `if worst < 1e-9 { break }` | as L-05: make a non-finite error poison `worst` to `INFINITY` |
| L-07 | MAJOR | `layout/circle_packing/circle_packing.rs:196-202` | `refine_tangency`'s convergence is discarded: exhausting `iterations` returns positions with no flag, so a tangency error still >= 1e-9 ships with `approximate = false` and no note code 3 from a supposedly certified packing | a planar graph whose tangency pass does not converge inside `params.iterations` | `circle_packing.rs:196-202` `let positions = placement::refine_tangency(..)` — return value is geometry only; `finish(.., approximate)` at `:204` never sees it | return the final `worst` from `refine_tangency` and OR it into `approximate` |
| L-08 | MAJOR | `layout/radial/twopi/adjacency.rs:37-61` | `Neighbours::of` rebuilds the out and inbound CSRs that `Topology` already holds, from two `Vec<(u32,u32)>` of m edges (16 B/edge of peak) plus two `sort_unstable` = O(m log m), where the same row mapping is reachable in O(m+n) from the existing per-row edge-index CSRs | every `layout.twopi` run at the declared ceiling (`RADIAL_CEILING = 1_000_000`) pays the duplicate build | `adjacency.rs:40-56` (`out`/`inbound` collected then `sort_unstable`ed); duplicated from `crates/graph-core/src/index.rs:152-153` | build `Neighbours` from the topology's existing out/inbound row mapping, keeping the graphviz-specific ordering contract |
| L-09 | MINOR | `layout/hierarchy.rs:24-25` vs `:56,190` | the module doc declares "one counting sort"; the code runs two **stable comparison** sorts (`notes.sort()`, `cuts.sort()`), so real cost is O(n + m log m), not O(n+m) | m = 9M edges at 1M nodes: ~200M comparisons and ~36 MB of sort scratch | `hierarchy.rs:56` `notes.sort()`; `hierarchy.rs:190` `cuts.sort()`; doc claim at `hierarchy.rs:24-25` | either use the counting sort the doc promises, or correct the doc to O(n + m log m) |
| L-10 | MINOR | `layout/hierarchy.rs:136` | `depth()` returns 1 (not 0) for every real root under a virtual root, with no `root_depth()` accessor to normalise, so a depth->ring mapping is off by one on every forest | a 14-node forest: all 14 nodes shift one ring | `hierarchy.rs:21-22` documents the shift; `hierarchy.rs:136` `depth()` has no offset accessor | expose `root_depth()` or document the offset at each depth-consuming call site |
| L-11 | MINOR | `layout/hierarchy/fixture.rs:49` | `text.parse().expect("a weight")` accepts `"NaN"`, `"inf"`, `"-inf"` with no finiteness check, so a non-finite weight enters every downstream sum and partition | add `"weight": "NaN"` to `fixtures/hierarchy/tree-degenerate.json`: NaN radii/rects in the treemap and packing consumers, undetected | `fixture.rs:49` `text.parse().expect("a weight")` | reject a non-finite parse at the fixture boundary |
| L-12 | MINOR | `layout/sugiyama/coords.rs:37` | above `PRIORITY_NODE_BUDGET` (200 000) `passes == 0`, so x is exactly the raw ordering slot index and the whole X phase is a no-op, **with no `NoteCode`** — the registry's `degradation` field documents the dummy budget and the transpose drop but not this | 1M-node graph: legal but maximally spread, and the consumer is told nothing | `coords.rs:37` `let passes = if total <= PRIORITY_NODE_BUDGET { 4 } else { 0 };` vs `registry/grid.rs:102-104` | emit a note when `passes == 0`, so the no-op is observable rather than silent |
| L-13 | MINOR | `layout/sugiyama/ordering.rs:50-51` | a vertex whose `layer_of >= num_layers` is silently dropped from `order`, and `Coords` then defaults it to x = 0.0 | `run` computing `num_layers` from `max()` of an empty `layer_of` while nodes exist | `ordering.rs:50-51` (no bounds check before `order[layer_of[..] as usize]`) | return a `StageError` instead of dropping the vertex |
| L-14 | MINOR | `layout/sugiyama/routing.rs:56` | `Route::Loop` returns with zero interior points, so a self-loop is emitted as a degenerate polyline whose first and last point are the same node; no loop arc is generated anywhere in the file | `n -> n` | `routing.rs:56` returns before any point is pushed | emit a real loop arc, or document the degenerate polyline as the intended convention |
| L-15 | MINOR | `layout/treemap/rows.rs:40` | `total_cmp` is not d3's `b.value - a.value` when an aggregate overflows to `+inf`: d3's comparator returns NaN, which `Array.prototype.sort` coerces to +0 ("keep order"), while `total_cmp` orders the two infinities | parent P with children a and b each summing to `+inf`, and c(5.0) | `rows.rs:40` `sorted.sort_by(\|..\| value[b].total_cmp(&value[a]))`; oracle `refs/npm/d3-hierarchy-3.1.2/package/src/treemap/squarify.js` sorts `b.value - a.value` | order by `total_cmp` only after replacing a non-finite value with the d3 effective key, or clamp the aggregate |
| L-16 | MINOR | `layout/tidy_tree/walk.rs:185` | `second_walk` iterates `Hierarchy::order()`, which is **breadth**-first, not `tree.js`'s `eachBefore` preorder; the output is identical only because `m[parent]` is provably final at visit time — an unasserted invariant with no test and no comment, and `skip(1)` silently assumes `order()[0]` is the layout root | any virtual-rooted forest (2+ roots) would misplace every node the day that invariant stopped holding | `walk.rs:185`; `hierarchy.rs:147,238-250` builds `order` breadth-first; `skip(1)` at `walk.rs:196` | iterate an explicit preorder, or assert the invariant at the function |
| L-17 | MINOR | `layout/tidy_tree/walk/contour.rs:55-56` | `.expect("the outside contour reaches as far")` panics out of `Walk::layout`, escaping `run` (`tidy_tree.rs:76`) as a panic rather than a `StageError` | any input violating the outside-contour invariant (asserted in code, never proved or tested) | `contour.rs:55` `c.vom = self.next_left(c.vom).expect(reach);` | return a `StageError` naming the violated invariant |
| L-18 | MINOR | `layout/hierarchical_3d/disk.rs:8-10` | the `count == 1` early return discards `radius` and returns the origin, but the general path for count=1 gives `ring_points(0,1,r,1) = (0.5*radius, 0)` — internally inconsistent, so every singleton level loses the widest-based scaling | a path graph whose level 0 is a single node with radius 1.7678 | `disk.rs:8-10` `if count == 1 { return vec![(0.0, 0.0)]; }` vs the `ring_points` path at `disk.rs:113-121` | either drop the early return or state the origin convention in the module doc |
| L-19 | MINOR | `layout/hierarchical_3d/levels.rs:19-47`, `hierarchical_3d.rs:83-87` | `run` re-derives levels from its own BFS (`component_roots` + `multi_source_levels`) and never reads the Sugiyama 2D result, so `depth_z` maps **this module's** undirected BFS depth, not `sugiyama::run`'s layer index | a digraph where BFS depth != longest-path layer: `0->1, 0->2, 1->3, 2->3` — sugiyama puts 3 on layer 2, this module puts it on BFS depth 2 as well, but a reversed chain `1->0, 2->1` gives reverse-topological depth | `hierarchical_3d.rs:83-87` (own BFS); `levels.rs:19-47` (undirected depth); `sugiyama/layering.rs:36` `assign_layers` does longest-path instead; the 3D row's own oracle is SciGraphs (`registry/three_d.rs:185`), which is itself BFS-based, so this is a **scope** question, not a conformance failure | confirm the 3D layout is an independent SciGraphs port (its oracle says it is) and fix the module docstring, which frames it as lifting the 2D layout |
| L-20 | MINOR | `layout/circle_packing/fallback.rs:78` | `params.scale == 0` is not rejected: `initial_radii` scales every radius by `sqrt(0) = 0`, `cutoff` becomes 0 so the overlap pass always `continue`s, temperature 0 caps every force to zero, and `rescale_to(..., 0)` collapses all positions to the origin | `CirclePackingParams { scale: 0.0, .. }` on any non-planar graph: n coincident zero-radius circles, no error | `fallback.rs:78` `*r *= factor` with `frame = 0.0`; `fallback.rs:32` `rescale_to(.., scale * 0.45)` | reject `scale <= 0` at `pack`'s entry |
| L-21 | MINOR | `layout/circle_packing/fallback/relax.rs:122-123` vs `:94` | the doc claims a self-loop contributes "`+` then `-`", but the branch tests only `u as usize == i`, so a self-loop `(i,i)` adds twice instead of cancelling; the gather is **not** bit-identical to the scatter it replaces | a self-loop reaching `relax` — currently unreachable from `pack` because `simple_pairs` filters them, so latent | `relax.rs:122-123` (doc) vs `relax.rs:94` (the `u as usize == i` test) | test both endpoints, or correct the doc |
| L-22 | MINOR | `layout/circle_packing/fallback.rs:37-46` | no non-finite check between the force loop and `Packed`; `cap_and_apply`'s guards are both `false` for a NaN magnitude, so a NaN force is applied verbatim and nothing downstream checks | any NaN/inf force poisons every coordinate with no note code 3 | `fallback.rs:37-46` builds `Packed` unchecked; `relax.rs:181-188` guards skip NaN | check the final positions for finiteness in `pack` |
| L-23 | MINOR | `layout/circle_packing/placement.rs:176,214` | `refine_tangency` reallocates `pulls` (24 B/edge) and `gradients` (16 B/node) **every** iteration: 500 iterations x ~1.5M edges is ~30 GB of allocator traffic at 1M nodes | 1M-node graph on the exact path | `placement.rs:176` `let (pulls, worst) = edge_pulls(..)` and `:214` the gradient gather, both inside `for _ in 0..iterations` | hoist both buffers out of the loop and `clear()` per round |
| L-24 | MINOR | `layout/circle_packing.rs:146` | `simple_pairs` builds a `BTreeSet<u64>` node per edge: O(m log m) at ~48 B/edge | 10M-edge graph spends ~500 MB and seconds before any geometry | `circle_packing.rs:146` | a sorted `Vec<u64>` plus binary search, or a sorted key vector |
| L-25 | MINOR | `registry/three_d.rs:17,213` | `BASIC_3D_CEILING`'s doc declares itself the ceiling of "the three graph-free 3D placements", but `three_d.rs:213` reuses it for `HIERARCHICAL_3D`, which reads the graph | a graph-reading layout contracted to a ceiling derived for graph-free ones | `three_d.rs:17` vs `three_d.rs:213` `scale_ceiling: BASIC_3D_CEILING` | give `HIERARCHICAL_3D` its own ceiling, or widen the constant's doc |
| L-26 | MINOR | `registry/hierarchy.rs:8,20` | one measured ceiling (`HIERARCHY_LAYOUT_CEILING = 4_600_000`, derived on a 933 B/node tidy-tree basis) backs four rows, while its doc names three layouts — the basis does not describe `CIRCULAR_HIERARCHY` | the disclosure sits only in the ponytail prose at `hierarchy.rs:64-65`, not in the ceiling's doc | `hierarchy.rs:20` shared by `:50`, `:77`, `:97`, `:120` | state the shared basis in the ceiling doc |
| L-27 | MINOR | `registry/radial.rs:23` and `registry/three_d.rs:54` | `RADIAL_CEILING` and `BASIC_3D_CEILING` are two independently-derived constants that both evaluate to `1_000_000`, each justified as "1000 000 is the largest size `bench` accepts" | the two derivations are textually redundant and can drift | `radial.rs:20-21,23`; `three_d.rs:20-21,54` | derive both from the one `bench` constant |
| L-28 | MINOR | `registry.rs:225-227` | `LAYOUTS` array order is declared load-bearing (`graph-wasm/src/exports/build.rs:23,32,166` maps layouts **by index**; `bench/campaign.rs:128`'s `DEFAULT_ARM` is `LAYOUTS[3]`) yet only the length (36) is type-checked | reordering two rows inside the array compiles silently and repoints the default crossover arm | `registry.rs:225-227` and the two call sites named there | add a `const _: () = assert!(...)` on the index of each index-mapped row, or a codegen check |

## Checked and found correct (no finding)

These were the review's most likely defects and are **not** defects; recorded so the next
reviewer does not re-derive them.

- **`tidy_tree`'s declared O(n) holds.** The Buchheim contour is a thread chain of node ids
  (`contour.rs:13-14,80,86`), so `next_left`/`next_right`/`next_ancestor` are O(1) over CSR rows —
  not a linear-scan `Vec`. There is no `Vec::insert(pos,..)`/`remove(pos)`. All four passes are
  iterative, so depth 1e6 does not overflow the stack. Verified against
  `refs/npm/d3-hierarchy-3.1.2/package/src/tree.js`: `firstWalk`, `executeShifts` (including the
  `shift += w.s + (change += w.c)` evaluation order), `apportion` (with `vop.a = v` and the
  `vim`-then-`vip` comma order), `moveSubtree` and `secondWalk` all match.
- **`sugiyama/acyclic.rs`'s FAS heap key is not inverted.** `heap_key` returns
  `(in_degree - out_degree, v)` into a `BinaryHeap<Reverse<..>>`, which is a min-heap and so pops
  the **largest** `out - in` — exactly the Eades-Lin-Smyth heuristic and its own doc at
  `acyclic.rs:3`. This matches the reference verbatim:
  `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:256,270,276` uses `heapq` (a
  min-heap) on `(in_degree[n] - out_degree[n], rank[n], n)`. Tie-break is the dense node index,
  which is the reference's `rank`. Self-loops are excluded from the degrees at `acyclic.rs:111`.
- **Self-loops, parallel edges and cycles are handled** in `sugiyama`: `acyclic.rs:40` never
  reverses a self-loop, `layering.rs:189` gives one `Route::Loop` with no dummies,
  `twopi/tree.rs:73` dedups parallel edges on first discovery, and `twopi/adjacency.rs:109` shows
  a self-loop-only graph terminates. `sugiyama/mod.rs:124`'s `left + right` divisor is guarded by
  the `== 0.0` branch and cannot produce NaN.
- **`treemap` is a faithful bit-level port** of `d3-hierarchy@3.1.2`: `rows.rs:66-90` is d3's
  inlined `layoutrow` with the O(1) running min/max row growth (no O(n^2) worst-ratio rescan), the
  rejected candidate is subtracted back exactly as `squarify.js` does (an f64 round trip that is
  visible in the emitted geometry), `treemap.rs:114-136` reproduces `dice`/`slice`, and
  `rows.rs:38-42` gives d3's leaf order (descending value, stable, over children already in
  ascending dense index). Degenerate counts are safe: 0 nodes -> `Boxes::new(0)`; 1 node -> unit
  box; all-zero weights -> `n * 1e-6 > 0`, no division by zero; a deep chain never recurses.
- **`circle_packing`'s `angle_sums` is gather form (D10)** and is bit-identical to the scatter it
  replaces, pinned by `radii/tests/solver/gather.rs`. `solver_angle` (`radii.rs:50-54`) clamps the
  denominator as `circle_packing.py:160-161` does, which is the reference's own formula, not the
  `PI/3` guard used for placement.
- **`circle_packing`'s fallback has no RNG.** The reference's `seed.rand` is replaced by a fixed
  golden-angle spiral (`fallback/seed.rs:18,22`), a recorded Ponytail at `fallback/seed.rs:5-15`.
  `n = 0` and `n = 1` are safe, coincident pairs are handled by `separated`
  (`fallback.rs:91-98`, `dist` floored at `1e-6`), and the overlap force is bounded by the radii sum
  (`relax.rs:161-168`). The scatter into `forces[i]`/`forces[j]` (`relax.rs:169-172`) is a
  deliberate, documented D10 exception (`relax.rs:136-148`), single-threaded, so there is no race.
- **Determinism is clean across the seven modules.** No `HashMap`/`HashSet` is iterated for output
  order anywhere in scope; no unseeded RNG; no wall-clock; no `usize` on any wire path
  (`depth`/`parent`/`leaves` are `u32`, `Note.index` is `u32`, route point counts are `u32`).
  Every `sort_unstable` in scope is either over a total order with an explicit index tie-break or is
  immediately followed by `dedup`, which makes tie order unobservable. All transcendentals are
  `libm`; no `mul_add`, `powi`, relaxed-simd or FTZ/DAZ. `f32` narrowing happens once, at
  emission (`hierarchical_3d.rs:136-138`, `tidy_tree.rs:78-79`), never inside a reduction.
- **`twopi`'s root choice matches graphviz.** `center.rs:61` maximises distance-to-nearest-leaf
  (`findCenterNode`, `circle.c:96-114`), not max degree; the tie-break is a strict `>` over the
  component's ascending dense index, matching `circle.c:108`'s "first maximum stands". `is_leaf`
  and the BFS make `of` O(|component|), not O(n^2). Both visited sets are dense vectors, so
  membership is O(1) — no `Vec::contains`.
- **All seven in-scope layouts have a registry row** (`registry.rs:85,90,100,125,185,245`, plus
  `:95,200` for the circular family), and the `LAYOUTS` array's 36 entries match its declared
  length. Nothing in scope is unregistered or unreachable.

## Unverified

Claims I could not settle with the code and references on disk. Listed here, never as findings.

- **`sugiyama/coords.rs` implements no Brandes-Köpf.** Only the Sugiyama-Tagawa-Toda priority
  method is present — none of BK §3.1's four alignment types and no §4 averaging — so long edges
  are never straightened and a dummy chain stays kinked at every intermediate layer. Whether this
  is a defect depends on the row's contract, and the row's oracle is `dagre-d3-es 7.0.14` crossing
  counts (`registry/grid.rs:95-98`), which measures crossings, not straightness. I did not run the
  dagre differential to see whether a BK implementation would change a passing row. **Needs a
  decision, not a fix.**
- **`hierarchical_3d`'s use of BFS depth instead of Sugiyama layering (L-19)** turns on whether the
  module is meant to lift the existing 2D result into 3D or is an independent port of SciGraphs'
  own `_hierarchical_layout_3d`. Its registered oracle is the latter and the conformance matrix
  row 17 reports 1020/1020 within 7.95e-08, so I could not establish a divergence. The module
  docstring's framing is the only thing I am confident is wrong.
- **`circle_packing/fallback.rs:40`'s conformance divergence is already recorded.** The
  conformance matrix row 5 (`CIRCLE_PACKING`) names it: bit-for-bit identical on the 20 gate
  models (5e-16) but 0.517 different on lesmis, because SciGraphs falls back to force-directed on
  non-planar input. Cited, not re-reported.
- **`twopi`'s convention gap is already recorded.** Conformance matrix row 27
  (`GRAPHVIZ_TWOPI`) reports a max gap of 303 in "units" and calls it a convention, with the
  published-`SCALE`-const remedy at `docs/measurements/scigraphs-conformance.md:236-242`. Cited, not
  re-reported. The same applies to row 31 (`SUGIYAMA`, "the layering differs, so the columns do not
  line up") and row 29 (`GRAPHVIZ_OSAGE`).
- **`twopi`'s disconnected-component overlap** (every component root at depth 0 and theta 0, so
  all roots coincide at the origin) is a **declared** Ponytail at `registry/radial.rs:52-63`. I
  did not re-report the overlap itself; L-01 is only its cost. I could not measure how graphviz's
  own `twopiinit.c:118-143` packs components apart.
- **`ordering.rs:123-143`'s self-loop asymmetry** in `pair_crossings(v,w)` vs `pair_crossings(w,v)`
  when a node is in its own `up`/`down` list, which would bias `transpose` and make
  `total_crossings` count a self-loop as a crossing. Conditional on `layering.route`, which I did
  not review; `layering.rs:189` suggests a self-loop never reaches the ordering adjacency, which
  would make it unreachable.
- **`hierarchy.rs:83` (`self.parent.len() as u32`) and `hierarchy.rs:202` (`stamp = start + 1`)** are
  unchecked truncation/overflow, guarded today only by `of`'s `checked_add` at `:50`. Unreachable at
  any size this crate can allocate, and `n = u32::MAX` is not a reachable failing input.
- **`disk.rs:29`'s `f64::max` NaN semantics** could in principle differ between native and wasm32 if
  LLVM lowers one to a libm `fmax` call. Unreachable today because `count as f64 / PI` is never NaN
  for a `usize` count. Same caveat, same conclusion, for `geometry.rs:76-90` and
  `circle_packing.rs:249-250`.
- **`circle_packing`'s declared O(n^2) fallback cost is a contract, not a defect.**
  `registry/grid.rs:49-58` derives `PACKING_CEILING = 5_000` from a measured quadratic, and I
  confirmed it: `| layout.packing.circle | 300 | 3188.7 B | 295.95ms |`, `| 1000 | 8823.2 B |
  1.55s |`, `| 3000 | 24876.2 B | 15.56s |` (`crates/graph-core/tests/memory.rs`, run in this job).
  L-04 is only the parts of that quadratic the ceiling does **not** cover: the uncapped `n*n`
  allocation and its `usize` overflow above n=65536.
- **House-limit compliance** (`CLAUDE.md`: <=40 lines per function, <=300 per file) is clean in
  scope: the largest in-scope files are `sugiyama/layering.rs` (299), `sugiyama/ordering.rs` (298),
  `sugiyama/acyclic.rs` (294) and `registry/three_d.rs` (296). No style nits are reported, per the
  job's rule that a nit needs a house rule behind it.

## Counts

| module | files reviewed | BLOCKER | MAJOR | MINOR | notes |
|---|---|---|---|---|---|
| `layout/hierarchy` (+ `fixture.rs`, `tests.rs`) | 3 | 0 | 0 | 3 | L-09, L-10, L-11; depth offset and fixture weight parsing |
| `layout/tidy_tree` (+ `walk.rs`, `walk/contour.rs`) | 3 | 0 | 0 | 2 | L-16, L-17; declared O(n) verified against `tree.js` |
| `layout/sugiyama` (mod, acyclic, layering, ordering, coords, routing) | 6 | 0 | 3 | 3 | L-02, L-03, L-12, L-13, L-14; FAS heap key refuted |
| `layout/hierarchical_3d` (+ `levels.rs`, `disk.rs`) | 3 | 0 | 0 | 2 | L-18, L-19; conformance row 17 already green |
| `layout/radial`/`twopi` (+ tree, angles, center, adjacency) | 6 | 0 | 2 | 0 | L-01, L-08; L-01 measured, L-08 read against `circle.c` |
| `layout/treemap` (+ `rows.rs`) | 2 | 0 | 0 | 1 | L-15; otherwise a bit-level d3 port |
| `layout/circle_packing` (+ placement, radii, geometry, fallback, seed, relax, triangles) | 8 | 0 | 4 | 5 | L-04..L-07, L-20..L-24 |
| `registry*` rows for the above | 4 | 0 | 0 | 3 | L-25, L-26, L-27 (+ L-28 in `registry.rs`) |
| **total** | **35** | **0** | **9** | **18** | 5 refuted claims recorded under "Checked and found correct" |

Every MAJOR row carries a command or an explicit reference `file:line`: L-01 and L-04 carry
measurements; L-02, L-03, L-05, L-06, L-07 quote the offending line and the declared contract it
contradicts; L-08 quotes both the duplicate and the original it duplicates.
