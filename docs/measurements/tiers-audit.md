# Tiers audit — every registered layout against the Phase 11 compute tier

What the 15 layouts in `crates/graph-core/src/registry.rs:167` (`LAYOUTS`) would need to run
under a threaded `exec` runner, and what each would gain. Produced by reading the code: every cell
is a `file:line` you can open. **No bench was run for this audit**, so every speed-up and effort
cell is a judgement — with two exceptions, both measured since: row 11's, quoted from
`docs/measurements/phase11-threads.md` (Barnes-Hut, release, 16-core host, median of 5), and
row 5's, measured in `docs/measurements/tier-random.md` (this host, nproc 20, one run on the
plain bench path and a median of 9 on the campaign path).

Read this with the two corrections at the bottom in view. **The template bench command in
`prompts/jobs/tiers-audit.md:24` does not run as written, and `bench --tiers` cannot time any
layout but Barnes-Hut.** Every follow-on job hits both.

## The runner surface, in the four lines that matter

graph-core never spawns a thread (`exec/mod.rs:12-14`). It holds:

- **`Serial`** — `exec/partition.rs:97`, the tier-1a reference every other runner is compared against.
- **`Threads`** — `crates/graph-cli/src/exec_native.rs:34`, `impl Runner for Threads` at `:36`. `std::thread::scope` over `partition`'s ranges (`:52-67`), one buffer per worker, concatenated after the join (`:68`). It lives in the **CLI**, not graph-core, so a layout's threaded entry point takes `runner: &impl crate::exec::Runner` from its caller — the shape `BarnesHut::run_with` already has (`barnes_hut.rs:151`).
- **`StepRange`** — `exec/partition.rs:41`. `type Out: Copy + Default + Send` (`:43`), `fn len` (`:46`), `fn step_range(&self, range, out)` (`:59`). `Sync` is the supertrait, which is the D10 statement made into a type error: a kernel that had to mutate mid-step cannot be handed to another thread.

So "does this layout already use an exec runner" has exactly two possible yeses today, and one
of them is a trap:

| Layout | Through `exec::`? | Evidence |
|---|---|---|
| `layout.force.barnes_hut` | **yes, threaded** | 3 `StepRange` impls at `force/barnes_hut/step.rs:67`, `:101`, `:134`; dispatched at `charge.rs:42`, `link.rs:69`, `collide.rs:44`; `THREADED_PASSES` at `barnes_hut.rs:134` |
| `layout.force.yifan_hu` | **yes, but `Serial` only** | reuses the *same three kernels* through `barnes_hut/settle.rs:29-30`, which hard-codes `How { runner: &Serial, workers: 1, .. split: Split::None }` |
| the other 13 | **no** | `exec::` appears in none of them |

`layout.force.yifan_hu` is the whole finding of this audit in one row: the kernels are already
written, already gather-shaped, already byte-identical by construction. One literal at
`settle.rs:30` is the difference between a serial layout and a threaded one.

## The table, easiest first

Speed-up class is `none` / `≤2×` / `near-linear`, and the reason column says which. "None" is a
real answer, not a deferral — §"Sequential by nature" below says what it costs to overturn one.

| # | layout id | kernel (`file:line`) | exec runner today | parallel shape | reduction that must stay fixed-order | speed-up | effort |
|---:|---|---|---|---|---|---|:--:|
| 1 | `layout.force.yifan_hu` | `force/yifan_hu.rs:81` `multilevel` → `force/barnes_hut/settle.rs:18` `settle`, `:27` tick loop, `:34` `sim.tick` | **yes, `Serial`** (`settle.rs:29-30`) | per-node gather, inherited from the Barnes-Hut tick (`barnes_hut/step.rs:11-17`); the levels (`yifan_hu.rs:62-79` coarsen, `:94-102` prolong) are greedy and sequential | none new — the three per-pass merges, already ascending-index (`charge.rs:49`, `link.rs:76`, `collide.rs:69`) | **near-linear** | **S** |
| 2 | `layout.grid` | `grid.rs:54` `Stage::run` → `:75` `positions`, hot loop `:84-85` | no | per-node gather, no edge work | none — the whole body is one `f32` product per coordinate (`grid.rs:84`) | `≤2×` | **S** |
| 3 | `layout.circular.ring` | `circular/ring.rs:21` `run`, hot loop `:27-28` | no | per-node gather (`(0..count).map`), then one shared serial pass | `coords.rs:20-21` centroid `x.iter().sum::<f64>()` / `y.iter().sum::<f64>()` — "summed in index order, so the result is fixed on every target" (`coords.rs:14`); `limit = limit.max(..)` fold `coords.rs:26` | `≤2×` | **S** |
| 4 | `layout.spiral` | `spiral.rs:26` `run` → `:33` `run_with`; default is the **archimedean** branch (`run` passes `equidistant = false`, `spiral.rs:27`), hot loop `:67-75` | no | archimedean branch: per-node gather. equidistant branch: **sequential by nature** — `theta += CHORD / radius` carries state at `:60` | `coords.rs:20-21`, `:26`, as ring | `≤2×`, archimedean only | **S** |
| 5 | `layout.random` | `random.rs:25` `run`, hot loop `:26-28` | no | **sequential by nature** — the `Mulberry32` stream is consumed in order, `x` then `y` per node, so node *i*'s pair is draws `2i`, `2i+1`; a chunk-seeding scheme is bit-identical but its serial prologue is 2n draws (the whole stream) against a 1/2 serial fraction, so Amdahl's ceiling is exactly 2.0× and every finite width is below it | none | **none**, *measured* — plain bench one run: 0.00 / 0.06 / 0.72 ms at n=220 / 10 000 / 100 000 (one `Instant` pair per cell, `bench.rs:205-207`; `--repeat` is ignored there); campaign medians of 9: 0.001 / 0.026 / 0.256 ms, and 0.256 / 0.257 / 0.261 at `--repeat 31` so the median is not a function of the repeat count. No tier arm exists, so this bounds the *work*, not a speed-up: at n=220 the whole body is 0.001 ms and cannot pay for 7 threads, and the one crossover in the tree (`phase11-threads.md`) has threads losing below n=10 000. Full record, with the 0.72-vs-0.256 gap between the plain and campaign paths called out, in `docs/measurements/tier-random.md` | **S** (the deliverable is a recorded "no tier", not code) |
| 6 | `layout.circular.radial` | `circular.rs:59` `run` → `:77` `positions`, hot loop `:82-88` | no | per-node gather reading `Hierarchy::depth`; upstream BFS `hierarchy.rs:238` `breadth_first`, queue walk `:243-249` | the `seen` slot counter `circular.rs:83-84` — a node takes the **first free slot**, so it depends on ascending dense index. `ring_counts` `:94-104` are integer counts, order-free | `≤2×` | M |
| 7 | `layout.bipartite` | `bipartite.rs:30` `run`, hot loop `:35-41` | no | placement loop is a **scatter**, not a gather — it walks each side's node list and writes `x[node]`/`y[node]` (`:37-40`), so a gather form needs the per-node inverse mapping first. Upstream `bipartite/partition.rs:23` is a BFS two-colouring (`colour_component` `:49`, walk `:53-65`), and its `greedy_max_cut` fallback `:70-95` is a Gauss-Seidel local search over 8 passes — **sequential by nature** | `coords.rs:20-21` centroid sums, as ring and spiral. (`bipartite` calls the same `rescale`; there is no per-layout reduction here) | `≤2×` (emit loop only) | M |
| 8 | `layout.treemap.squarified` | `treemap.rs:248` `run` → `:194` `compute` → `:148` `squarify_children` (`while i0 < n` `:153`); `treemap/rows.rs:66` `extend_row` (`while i1 < n` `:77`) | no | **sequential per parent** — `squarify_children` carries a cursor each child advances (`treemap.rs:150`). Different parents are independent, so the parallel axis is the **parent list**, not the node list | `node_values` sums children **last-child-first** (`rows.rs:28-30`, pinned bit-for-bit by the module doc); `extend_row`'s deliberate `sum_value += / -=` round trip (`rows.rs:79,84`) exists because `(1.0 + 1e-6) - 1e-6 != 1.0` (`rows.rs:57-65`); `min`/`max` folds `rows.rs:80-81` | `≤2×`, scaling with the tree's **width**, not its size | M |
| 9 | `layout.mds.pivot` | `pivot_mds.rs:236` `run` → `:220` `solve_component`; `pivot_distances` `:115` (inner `for j in 0..k` `:120`, each a full BFS `:81`), `gram` `:156` (O(nk²), inner `sum` `:160`), `project` `:200` (inner `sum` `:206`) | no | pivot selection is a **chain** — `chosen = argmax(&covered)` `:129` feeds the next BFS. `gram`/`project` are independent per `(p,q)` / `(d,i)` and would gather by row | `double_center` `:136`: column means `:141`, row means `:144`, grand mean `:146`, in-place loop `:147-152` ("sequential sums throughout (D3)"); `covered[i].min` `:126`; `argmax`'s strict `>` lowest-index tie-break `:101-109` | `≤2×` — `double_center` and pivot selection are serial chains; only the O(nk²) `gram` gathers | M |
| 10 | `layout.spectral` | `spectral_stage.rs:16` `spectral` → `spectral.rs:241` `run` → `:162` `solve_component` (dense `eigh` ≤256, else `lobpcg_smallest`); `matvec` `spectral/graph.rs:32-40`, `dense_matrix` `graph.rs:42-52`, `lobpcg.rs:192-195` | no | components independent (parallel **by component**); within a component LOBPCG is strictly sequential (`linalg/lobpcg.rs:83` `for iter in 0..=MAXITER`) | `peak` fold `spectral.rs:189`; `biggest` max `:214`; component sort `:208-213`; per-component rescale `:221-224`; `residual_converged` `linalg/mod.rs:43` (ascending matvec, `sum` `:62`, `worst.max` `:64`); `orthonormal` `linalg/mod.rs:75` (`sum` `:80`); `pin_signs` `linalg/mod.rs:101` (argmax, lowest-index tie-break); `column_norms` + `avg` `linalg/lobpcg.rs:118-119` | `≤2×` for one big component — the common case. near-linear only for many small ones | M |
| 11 | `layout.force.barnes_hut` | `barnes_hut.rs:115` `Stage::run` → `:148` `run_with` → `:164` `run_under`, `for _ in 0..TICKS` `:173` → `sim.rs:129` `tick` | **yes, threaded** | per-node gather (`step.rs:11-17`); the tick itself is **sequential by nature** — tick *t+1* reads what *t* wrote (`step.rs:5-9`) | the three merges, all ascending-index `vx[i] += deltas[i]` (`charge.rs:49`, `link.rs:76`, `collide.rs:69`); `center`'s `fold` `sim.rs:171-175`; `integrate` `:191-199` | **measured**: 1.34× / 2.14× / 2.97× at 2 / 4 / 7 workers, n=10 000; 4.29× at 7 workers, n=100 000 | **done** |
| 12 | `layout.tree.tidy` | `tidy_tree.rs:62` `run` → `tidy_tree/walk.rs:72` `Walk::layout`; hot loops `execute_shifts` `walk.rs:122-129`, `first_walk` `:132`, contour `walk/contour.rs:48-71`, `normalize` `walk.rs:195-212` | no | **tree walk, sequential by nature.** Postorder `firstWalk` then preorder `secondWalk`, each node reading its parent's *already-final* `m` (`walk.rs:186`), and `apportion` mutating the shared `z`/`m`/`c`/`s`/`anc`/`thread` state in place | the running `shift`/`change` sums `walk.rs:123-128`; the contour's `sim`/`sip`/`som`/`sop` accumulators `contour.rs:63-69`; `normalize`'s min/max/depth scan `walk.rs:198-208` (strict `<`/`>`, so first-visited wins on a tie) | **none** — O(n) total, and it is a depth-first walk over shared mutable state, not a scan | L |
| 13 | `layout.dag.sugiyama` | `sugiyama/mod.rs:62` `Stage::run` → `:70` `run` → `:29` `layered`; five phases, all sequential: FAS `acyclic.rs:28` (`greedy_fas_order` `:192`, hot `peel` `:165-187`, heap pops `:212`), layering `layering.rs:36` (`longest_path_layers` `:51`, `reduce_slack` `:70`, `(0..len).map(slide).sum()` `:72-73`), ordering `ordering.rs:45` (`order_layers` `:214`, hot `transpose` `:146-173`, `pair_crossings` `:123`), coords `coords.rs:29` (`priority_move` `:138`), routing `routing.rs:31`/`:42` | no | **sequential by nature, entirely.** FAS peels live degrees; `reduce_slack`'s `slide` reads neighbours' *current* layers; `transpose` mutates `position` that `median_position` reads | `weighted_median` `mod.rs:109` (sort + weighted average `:128`); `sort_by_median`'s comparator `ordering.rs:107-113`; `pair_crossings`' `count +=` `:140`; `bilayer_crossings`' `u64` accumulator `:189`; `reduce_slack`'s `moved` sum `:72`; `priority_move`'s chained `x[j] = x[j-1] + GAP` `:148`, `:163` | **none** | L |
| 13 | `layout.forceatlas2` | `forceatlas2.rs:35` `Stage::run` → `state.rs:88` (`for _ in 0..max_iter` `:89`) → `:96` `iterate`; hot loop `:130-144` `repulsion`, nested `for i` / `for j in (i+1)..n` `:133-134` | no | **per-edge scatter** — `ux[i] += ...; ux[j] -= ...` at `state.rs:138-141` writes into two accumulators. A D10 violation, and unlike circle packing's fallback there is **no Ponytail marker naming the exception** | position mean `state.rs:162-167`; `swing_and_traction` `:181-190` (`swing +=`, `traction +=`, ascending-index, carried across iterations — `state.rs:5-7`); `estimate_factor` `:193-220`; `apply_update`'s `moved` sum `:230`. Also `attraction` `:113-123` (per-edge scatter into both endpoints) and `gravity` `:157-179` | none as written; `≤2×` only after a rewrite to a full per-node gather (dropping the `i<j` halving doubles the flops to halve the writes) — and O(n²) × 100 iterations is already why `FA2_CEILING = 14_000` | L |
| 14 | `layout.packing.circle` | `circle_packing.rs:92` `run` → `:98` `run_with` → `:123` `pack`. Exact: `try_exact` `:181`, `radii.rs:122-129` Jacobi sweep, `angle_sums` `:181-192`, mean `:212`, `placement.rs:209-231` `gather_gradients`. Fallback: `fallback.rs:28` → `relax.rs:46` `relax`, O(n²) hot loop `:149` `overlap_and_repel`, inner `for j in (i+1)..n` `:151`, explicit in-place scatter `:169-172` | no | exact path: per-vertex gather. fallback: **per-edge scatter** — and `relax.rs:136-148` carries its own Ponytail stating this scatter is the deliberate D10 exception and the function is **not eligible for the compute tiers** | `angle_sums` `radii.rs:185-189`; the mean `:212`; `worst_free_error` fold `:194-199`; `bisect_share`'s 60-round sum `:85`; `gather_gradients`' per-node `gx/gy` sums `placement.rs:218-226` (proven bit-identical to the scatter it replaced); `apply_gradients`' `grad_norm` sum `:238`; `center` / `fit_to_scale` `geometry.rs:57` / `:75` | exact path `≤2×`; **fallback none** — and the fallback is the path a random or dense graph actually takes (`registry.rs:107-118`) | L (M for the exact path alone, which is the path nobody's real graph takes) |

## Sequential by nature, and what it would cost

Three shapes are not "needs a rewrite", they are "the algorithm is the answer":

1. **An order-dependent stream** — `layout.random`'s `Mulberry32`. A chunk-seeding trick (advance
   the state serially once per chunk, then generate each chunk in parallel) is bit-identical, but
   the serial prologue is 2n draws — the whole stream — against a 1/2 serial fraction, so Amdahl's
   ceiling is exactly 2.0× and every finite width is below it. The one crossover measurement in the
   tree says threads *lose* below n=10 000 (`phase11-threads.md`), and this layout's whole body is
   0.001 ms at n=220 (`docs/measurements/tier-random.md`), so it cannot pay for seven threads at
   any width. Shipping a tier here would cost time and gain no bytes. **This one is now measured,
   not argued** — the deliverable was that recorded "no", and it is code-free by design.
2. **A Gauss-Seidel / local-search sweep** — `layout.dag.sugiyama`'s FAS, `reduce_slack` and
   `transpose`; `layout.treemap`'s per-parent cursor; `layout.forceatlas2`'s iteration. Each reads
   the state a neighbour just wrote. Converting any of them to Jacobi changes the output, and the
   output is what the hash gate pins.
2b. **A tree walk over shared mutable state** — `layout.tree.tidy`'s `firstWalk`/`secondWalk` read
   each parent's *final* `m` and `apportion` mutates the shared `z`/`m`/`c`/`s`/`anc`/`thread`
   arrays in place (`walk.rs:186`, `walk/contour.rs:63-69`). Subtrees are independent *in d3's
   algorithm*, which is why it reads as parallelisable; it is not, because the parent's shift is
   only known after the whole sibling set has been summed.
3. **A pivot chain** — `layout.mds.pivot` picks pivots one at a time (`:129`), and
   `layout.spectral` iterates LOBPCG one subspace at a time (`linalg/lobpcg.rs:83`). These have a
   genuine parallel axis (components / rows) *outside* the serial chain; threading it means
   choosing that axis deliberately, which is the M in rows 9 and 10.

Two layouts hold deliberate scatter exceptions and should say so in one line each, since D10 is
otherwise an unmarked breach: `layout.forceatlas2`'s `repulsion` (`state.rs:130-144`) has no
marker, while `circle_packing`'s does (`relax.rs:136-148`).

## Three corrections the follow-on jobs need

**1. The template bench command does not run.** `prompts/jobs/tiers-audit.md:24` gives
`bench --layout X --n 220,10000 --tiers --workers 1,4,8`. Three defects:

- `--tiers` is `Option<Vec<Asked>>` (`bench.rs:98`) and needs a value. `--tiers scalar,threads` is
  the form; `simd` and `gpu` are refused by name (`bench/tiers.rs:86-95`).
- `--workers 8` is **refused at runtime** by `refuse_unproved_widths` (`bench/tiers.rs:219-229`),
  because every width must be an arm `hashgate --tiers all` actually ran: `WORKER_COUNTS = [1, 2, 3, 4, 7]` (`hashgate/tier.rs:45`). Use `2,4,7`. Omit `1`: the sweep's scalar arm *is* the one-worker run (`bench/tiers.rs:46-50`).
- **`--layout` is ignored.** `bench.rs:146-148` returns `tiers::entry(plan)` before any layout
  resolution, and `bench/tiers/sweep.rs:113-119` always calls
  `BarnesHut::run_under(topology, &ForceParams::default(), ..)`. So `bench --tiers` times exactly
  one layout no matter what `--layout` says. **There is currently no command that times any other
  layout under a tier.** Extending `sweep.rs` to route by `plan.layouts` is the first acceptance
  criterion of every job below, not a detail.

**2. `--tiers all` recomputes one layout.** `hashgate.rs:169` is
`let bytes = if id == BarnesHut::ID { ..run_under(..&Threads..) } else { bytes }` — the other
fourteen stages reuse the scalar run's bytes (`:181-183`). Until a layout's id is in that match,
"4-way equal" for it is **vacuous**: every arm hashes the same bytes by construction. So each job
must add its id there, and its negative control must prove the threaded arm was really recomputed.
**`layout.random` is the deliberate exception**: it has no threaded arm at all, so its base 4
arms are the whole claim, and `GM_MUTATE_NODE_COUNT` is the control that says the stage is
hashed. See `docs/measurements/tier-random.md`.

**3. There is no arm named "Threads", and the one knob is Barnes-Hut's.** `--tiers all`
(`hashgate/tier.rs:82-91`) = the 4 base arms + `native scalar` + one `native threads N` per
worker count = **10 arms**, compared line-for-line against arm 0 (`compare.rs:54-56`) and printed
as `{ways}-way equal` (`hashgate.rs:227-231`). `GM_MUTATE_SPLIT_SUM` (`knob.rs:100`) carries a
`barnes_hut::Split` on `Setting` and reaches a stage only through
`BarnesHut::run_under(..., setting.split_sum)` (`hashgate.rs:175`) — it is **inert for every other
layout**. Two traps with it: under `--tiers base` it is silently ignored (`refuse_a_vacuous_control`
runs at `hashgate.rs:64-68`, but `setting.split_sum` is only read at `:175`), and
`Split::Collide | Split::All` sets `min_seeds() == 5` (`barnes_hut.rs:85-93`), so `--seeds 2` exits
2 rather than going red.

## Groups, and the briefs

Three S groups, one brief each. The two that ship a tier are blocked on correction 1 (the sweep
cannot route) and correction 2 (the hashgate recomputes one layout). `tier-no-tier` is blocked on
neither: it delivers no arm, so it measured on the plain and campaign bench paths instead.

| brief | layout(s) | runner | existing knob to reuse | needs a new knob |
|---|---|---|---|---|
| `prompts/jobs/tier-settle.md` | `layout.force.yifan_hu` | `Threads`, into a new `run_with` mirroring `barnes_hut.rs:148` | **`GM_MUTATE_SPLIT_SUM`** — becomes live for this layout by changing one literal at `settle.rs:30` | no |
| `prompts/jobs/tier-closed-form.md` | `layout.grid`, `layout.circular.ring`, `layout.spiral` | `Threads`, one `StepRange` per layout, one shared serial merge loop at `coords.rs:15` | `GM_MUTATE_GRID_SPACING` (grid only, stage-level) | yes — one `rescale`-merge knob serves all three |
| `prompts/jobs/tier-no-tier.md` | `layout.random` | **none** | `GM_MUTATE_NODE_COUNT` | no | **done** — `docs/measurements/tier-random.md`; no code, no arm |

## Ponytail (the effort and the speed-up columns)

Both columns are **judgements from reading the code**, not measurements, and no bench was run for
this audit. Failing input: a layout whose hot loop is not the one this audit read (a refactor
moves a `file:line`), or a host whose memory bandwidth or core count differs from the 16-core
release box `phase11-threads.md` used. Direction: over-reporting effort, which costs an afternoon;
under-reporting it, which costs a tier that is slower than serial and looks like a regression.
Escape hatch: `exec::select` / `resolve` (`exec/select.rs:139`, `:157`) bypass every threshold, so
a wrong `≤2×` here can never select a tier that does not pay for itself — and, more importantly,
can never change a byte, because every tier `select` can return is hash-equal to scalar per stage.

One fact that is *not* a judgement and should be stated plainly: `Caps`, `Thresholds`, `select` and
`resolve` have **no production call site** anywhere in the workspace (only
`exec/select/tests.rs` and `hashgate/tier/tests.rs`). The runtime tier today comes from the
`--tiers` flag and the caller's own `runner` argument. "Which runner to use" is therefore a
per-layout `run_with` parameter, not a threshold the motor consults.
