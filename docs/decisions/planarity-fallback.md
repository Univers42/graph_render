# Circle packing (Collins–Stephenson) — exact path and its visible fallback

Status: **decided** (Phase 3, the Circle-packing spec; note code from user decision D-N).
Scope: `crates/graph-core/src/layout/circle_packing.rs` and
`crates/graph-core/src/layout/circle_packing/{geometry,triangles,radii,placement,fallback,fallback/seed}.rs`,
over the planar embedding `crates/graph-core/src/layout/planarity.rs` certifies, and note
code 3 (`docs/decisions/snapshot-notes.md`).

## Context

`layout.packing.circle` lays out a graph's own edges as tangent circles. SciGraphs'
`_circle_packing_layout` (`circle_packing.py:281-393`) does this by triangulating a planar
embedding and solving for radii whose corners close on a target angle sum — exact, when it
applies. Not every graph is planar, and even a planar graph's solver can fail to converge
or crowd into a numerically suspect packing; SciGraphs itself falls back to a force
relaxation for these cases (`:407-542`) and only logs that it did. Phase 3's own rule (D-N)
is that a degraded result must be visible in the snapshot, not only in a log.

## The user's decision, as given

The Phase 3 spec's Circle-packing section:

> Exact path: port networkx 3.6 `check_planarity` (LR planarity, algorithms/planarity.py)
> and `triangulate_embedding` (algorithms/planar_drawing.py) as needed, then SciGraphs
> circle_packing.py:281-393. **Never trust the planarity test's "planar":** verify the
> returned rotation system by Euler's formula (V − E + F = 1 + C, faces traced from the
> rotation system) and treat a failed certificate as non-planar. Non-planar (or any other
> SciGraphs fallback condition) → port the SciGraphs fallback (:407-542); where it calls
> nx.spring_layout, port networkx 3.6 `_fruchterman_reingold` with DETERMINISTIC initial
> positions (no RNG: a documented seeder, e.g. the golden-spiral at
> src/core/layout/forceLayout.ts:63) and record that deviation; replace cKDTree by a
> deterministic spatial pass with pairs sorted by (i, j). The fallback appends note code 3.
> Transcendentals via `libm` only (D1); no mul_add (D2).

And the registry ledger line:

> layout.packing.circle (Circle, hand + planarity certificate; degradation = the
> approximate fallback, flagged by note code 3).

The LR-planarity test and its Euler certification, and `triangulate_embedding`, are
[`crate::layout::planarity`], merged in from the `p3-planarity` branch; this module takes
its `Embedding`/`triangulate_embedding` output as given and is scoped to what comes after:
the flower, the radius solve, the placement walk, tangency refinement, and the fallback.

## Decision, as implemented

**Exact path** (`try_exact`, `n >= 3`):

1. `planarity::planar_embedding(n, edges)` — `None` (self-loops and multi-edges already
   reduced away inside it, as SciGraphs' own `simple` reduction does) means not planar;
   falls back immediately.
2. `planarity::triangulate_embedding` on the certified embedding.
3. `triangles::flower` traces the triangulated embedding's faces, matches the one equal to
   `triangulate_embedding`'s own returned outer boundary (in either rotation, `_face_key`),
   and builds every vertex's triangle flower. `None` when a traced interior face is not a
   triangle, or there are none — the same defensive checks `_planar_triangulation` itself
   makes (`circle_packing.py:79-83`) before trusting the triangulation is a genuine disk.
4. `radii::packing_aims` — `2π` for every interior vertex; each boundary vertex's share of
   `(len(boundary) - 2)·π` in proportion to its triangle count, found by 60 rounds of
   bisection over `[0, π]` (discrete Gauss–Bonnet; 60 is SciGraphs' own fixed round count,
   `circle_packing.py:120-127`, not a tolerance this port chose).
5. `radii::solve_packing_radii` — Collins–Stephenson's Jacobi sweep (one pass per sweep
   from a fixed snapshot, matching SciGraphs' vectorised version's own semantics), up to
   `params.iterations.max(1) * 20` sweeps (SciGraphs' own `x20`, `circle_packing.py:325`),
   stopping early once every free vertex's angle-sum error is below `1e-9`
   (`radii::TOLERANCE`, SciGraphs' own `tolerance=1e-9` default).
6. `is_embedded` — the solve must have converged (`max_error < TOLERANCE`) and the radii's
   `max/min` spread must not exceed `1e3` (`MAX_RADIUS_SPREAD`, SciGraphs'
   `_MAX_RADIUS_SPREAD = 1e3`, `circle_packing.py:13`). If not, SciGraphs retries once with
   only the interior vertices free (the boundary pinned at whatever it last held,
   `circle_packing.py:325-347`) and accepts that result regardless, logging a warning
   ("Circles may overlap") rather than refusing it. This port does the same retry, but
   **always marks the retried result approximate** (note code 3) — see Alternatives.
7. `placement::lay_out_packing` walks the triangulation's dual graph from one seed
   triangle, placing each new circle tangent to two already-placed neighbours. `None`
   (falls back) if the walk does not reach every vertex — again a defensive check that
   should not fire once the triangulation is certified, mirrored here rather than trusted
   away.
8. `placement::refine_tangency` — gradient descent over the triangulation's own edges
   (not just the graph's, since the packing's rigidity comes from the chords the
   triangulation added) pulling every pair back to exact tangency, damped by the whole
   step's norm, for `params.iterations.max(1)` steps.
9. `geometry::finish` centres the result on its own mean and scales it so its extent (the
   furthest centre plus the largest radius) is `0.45 * params.scale`.

Every graph edge is also a triangulation edge (triangulation only adds edges, never
removes one), so step 8 makes every original edge tangent, not only the triangulation's
own chords.

**Fallback** (`fallback::pack`), taken whenever step 1, 3, 6 (after the retry) or 7 above
cannot certify an exact packing — every SciGraphs fallback condition, not only "not
planar":

1. `initial_radii` — degree-proportional, normalised so their squares sum to `0.35` of the
   scaled frame's area (`circle_packing.py:419-424`).
2. `fallback::seed::fruchterman_reingold` — networkx 3.6's dense `_fruchterman_reingold`
   (`drawing/layout.py:660-711`), `k = sqrt(1/n)`, run for `clamp(20_000/n, 10, 50)`
   iterations (SciGraphs' own iteration budget, `circle_packing.py:428-433`), seeded by
   `seed::seed_positions` — a golden-angle spiral (`GOLDEN_ANGLE = 2.399963229728653`, the
   same constant `src/core/layout/forceLayout.ts:63` uses for the interactive force
   layout) at radius `sqrt((i+1)/n)`, replacing `nx.spring_layout`'s `seed.rand(n, dim)`.
3. `fallback::seed::rescale_to` — `rescale_layout`'s own mean-centre-then-scale
   (`drawing/layout.py:1882-1922`).
4. `fallback::relax` — edge-pull toward tangency plus overlap-push, cooling like simulated
   annealing (`circle_packing.py:437-480`), then `fallback::settle` — a pure-overlap,
   no-cooling pass that stops as soon as a round moves nothing
   (`circle_packing.py:487-503`). Every "closest pairs" query SciGraphs would run through
   `cKDTree` is instead the deterministic ascending-`(i, j)` O(n²) scan
   (`overlap_and_repel`, `settle_round`) — the same branch SciGraphs itself falls back to
   without SciPy installed (`circle_packing.py:398-405`), always taken here rather than
   attempted conditionally.
5. `geometry::center` and `geometry::fit_to_scale` — the same centring/scaling finish as
   the exact path.

The fallback's own coincident-pair case (SciGraphs jitters it with `rng.rand() - 0.5`) is
replaced by `nudge`: a fixed, index-derived direction (`u * GOLDEN_ANGLE + v * π`), since
graph-core has no RNG.

**Note code 3.** `geometry::to_geometry` appends exactly one `Note { code:
PackingApproximate, index: SNAPSHOT_WIDE }` whenever the packing came from the fallback,
and none when it came from the exact path (including the `n < 3` special cases, which are
always exact). Every fallback trigger above reaches the same `Packed { approximate: true,
.. }` flag, so all four conditions are flagged identically — SciGraphs distinguishes them
only in its log message, never in its returned data.

Ponytail (module doc, `circle_packing.rs:31-38`): the packing is exact only for planar
input; the failing input is any graph with a K5 or K3,3 minor (or one whose certified
embedding cannot be triangulated into a genuine disk, defensive-only); the dangerous
direction is overlap, since the fallback does not guarantee tangency or non-overlap
either; the escape hatch is reading note code 3 off the snapshot — its absence is the only
trustworthy sign the packing is exact.

## Alternatives considered

- **Only flag the headline "not planar" case, matching SciGraphs' own log exactly.**
  SciGraphs' retry-after-non-convergence path (`circle_packing.py:325-347`) also logs
  "Circles may overlap" and returns data that is not certified — D-N's rule is that a
  degradation be visible in the *output*, and a consumer reading only the geometry cannot
  otherwise tell that packing apart from a converged one. Flagging every SciGraphs
  degradation trigger, not only the non-planar one, was chosen as the reading of "any
  other SciGraphs fallback condition" (spec, quoted above) that actually satisfies D-N;
  this is the one interpretive extension in this port beyond a literal line-for-line port,
  and is called out here for visibility.
- **Trust `planar_embedding`'s "planar" answer without re-tracing faces.** The spec
  explicitly forbids this ("Never trust the planarity test's `planar`"); `planarity.rs`'s
  own `euler_certificate` already does the re-verification the spec asks for, so this
  module inherits that guarantee rather than re-implementing it.
- **Seed the fallback with a fixed non-spiral point, e.g. all nodes at the origin, or a
  regular grid.** A shared start point stalls Fruchterman–Reingold's repulsion term
  (`dist` floored at `0.01`, never zero, so it would not crash — but every node moving in
  lockstep for many steps wastes the iteration budget). A grid is a valid deterministic
  choice but has no existing convention in this codebase; the golden-angle spiral does
  (`forceLayout.ts`), so reusing it needed no new constant to justify.
- **Use `cKDTree` via a Rust spatial-index crate.** Adding a dependency for an O(n log n)
  structure the reference itself only uses opportunistically (and falls back from without
  SciPy) is not justified; the deterministic O(n²) scan is exactly the branch SciGraphs
  already falls back to, so this is a straight port of that branch, not a new algorithm.
- **Jitter a coincident pair with a fixed epsilon offset along one axis instead of
  `nudge`'s angle.** Both are equally arbitrary and equally deterministic; `nudge` was
  chosen so that two different coincident pairs get different separating directions
  (index-dependent) rather than all splitting along the same axis, which would otherwise
  make every such pair overlap the same way if several occur in one packing.

## Why

- **Total order, no RNG, no wall clock.** Every step is a pure function of the topology's
  dense indices; the two substitutions the spec asks to record (seeding, close-pair scan)
  are both documented in place as Ponytails and here.
- **Every degradation is visible, not just the one the spec's own prose leads with.**
  A downstream reader checks one thing — note code 3's presence — to know whether the
  geometry it received is exact or approximate, regardless of which of the four
  SciGraphs-side conditions triggered it.
- **Exact path is provably exact by construction.** Because triangulation only adds edges,
  every original graph edge is also a triangulation edge, so `refine_tangency`'s tangency
  guarantee (subject to the stated tolerance) covers every graph edge, not just the
  triangulation's own chords.

## Pinned by

- `crates/graph-core/src/layout/circle_packing/tests/exact.rs`: every graph edge tangent
  within `1e-3` (`f32`) on the triangle, square+diagonal, bare cycle, wheel, grid, and
  isolated-node fixtures, computed and compared in `f64`; the worst observed tangency
  error across these fixtures is reported by
  `the_worst_observed_tangency_error_is_reported` (currently ≈ `2.3841858e-7`); no note on
  any of these; a self-loop/duplicate-edge reduction equivalence test.
- `crates/graph-core/src/layout/circle_packing/tests/fallback.rs`: K5 and K3,3 (Kuratowski's
  two minors) each produce exactly one note code 3 and finite, non-negative `Circle`
  geometry; a K5 minor hidden inside a larger graph still falls back.
- `crates/graph-core/src/layout/circle_packing/tests/determinism.rs`: two runs of the same
  topology are byte-equal on both paths; three seeded synthetic topologies
  (`DETERMINISM_SEEDS = [7, 23, 41]`, via graph-core's existing `seeded_model`) and every
  `fixtures/hierarchy/*.json` fixture (as a plain graph, ignoring hierarchy orientation)
  pack finitely with no `NaN`/`Inf`.
- `crates/graph-core/src/layout/circle_packing/tests/small.rs`: `n = 0, 1, 2` exact values;
  parameter validation (`scale` must be finite and above `0`).

## D10 exception

`prompt.md` §6's D10 requires per-step kernels to be gathers: element `i` reads only
start-of-step state and writes only `out[i]`, summing its own terms in a fixed order, with
no scatter into another element's accumulator. Three of this module's per-step kernels are
gathers, and two are not.

**Gather, as of this revision.** `placement::refine_tangency`'s gradient accumulation and
`fallback::relax::edge_pull`'s spring forces each build a per-node incidence list in the
edge list's own order and then write only their own node's slot; the `v` term is the
negation of the very same product the `u` term is. `fallback::seed::FrField::step` is
Jacobi for the same reason — every displacement is read from the round's starting
positions, and only then do the nodes move, as networkx 3.6's vectorised `pos += delta_pos`
is. Each of the three is pinned against the form it replaced, over seeded random inputs,
at `to_bits()` equality: `placement::tests::the_gather_is_bit_identical_to_the_scatter_over_seeded_random_cases`,
`fallback::relax::tests::the_edge_pull_gather_is_bit_identical_to_the_scatter_over_seeded_cases`
and `fallback::seed::tests::a_step_is_jacobi_so_no_node_sees_another_nodes_move`.

**The exception, and why.** `fallback::relax::overlap_and_repel` stays a pairwise scatter.
It is the `O(n²)` all-pairs scan — every close pair, visited once, both endpoints updated
in place. A gather would need each node's list of close pairs, which is the same `O(n²)`
enumeration with an `O(n²)`-sized incidence list built and carried to do it, for no
reduction in work. So the result is bit-exact only in this scalar ascending-`(i, j)`
order, and the function is **not** eligible for the Phase 11 compute tiers, whose entire
value is bit-identity with the scalar build. `settle_round` is in the same position and is
left as it is for the same reason.

**Direction and escape hatch.** A tier that vectorised or reordered these passes would
change the packing's last bits, never its correctness: the whole path is already flagged
approximate (note code 3) and its own Ponytail says neither tangency nor non-overlap is
guaranteed. Nothing downstream can mistake the result for a certified one.

**Ownership.** Phase 9 owns replacing this `O(n²)` fallback — it is the layout's
asymptotic weak point, not this form. **That replacement must be gather form (D10)**, and
must be bit-comparing against this scatter on the way in, exactly as the three above are,
so the change is a measured one rather than a silent reordering.
