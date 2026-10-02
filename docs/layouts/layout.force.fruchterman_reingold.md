# layout.force.fruchterman_reingold

Clean-room specification of the Fruchterman-Reingold layout as implemented in igraph 0.11.9.

Source: `vendor/source/igraph/src/layout/fruchterman_reingold.c` in the igraph 0.11.9 tree
(`/goinfre/dlesieur/refs/igraph-0.11.9/igraph-0.11.9/`).

- 2D dense variant: lines 29-160.
- 2D grid variant: lines 167-283.
- 2D entry point and argument checks: lines 357-424.
- 3D variant (no grid): lines 475-680.
- Random start: `layout/layout_random.c:106` (2D) and `:184` (3D).

Paper: T. M. J. Fruchterman and E. M. Reingold, "Graph Drawing by Force-directed Placement",
Software: Practice and Experience 21(11), 1129-1164, 1991. https://doi.org/10.1002/spe.4380211102
(cited at `fruchterman_reingold.c:~310`).

## What SciGraphs asks for

`SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:53-83` calls the Python binding with `dim=3`
and `niter=iterations`. `start_temp` is passed only when the caller gives a positive fraction `f`, as
`f * sqrt(n) / 10`. No seed, weights, bounds or grid option is passed. So the 3D variant runs, from a
random start, unweighted, with the binding's default temperature otherwise. The 2D form is specified
too, because the 3D form is the same algorithm plus one coordinate (and one defect, below).

## Parameters and defaults

| Parameter | Default (binding, `src/_igraph/graphobject.c:~8470`) | Meaning |
|---|---|---|
| `niter` | 500 | Number of iterations. Must be non-negative. |
| `start_temp` | `sqrt(n) / 10` | Initial temperature: the largest length of one vertex's move in one step. |
| `seed` | none (random start) | Optional start coordinates, an n x dim matrix. |
| `weights` | none (all 1) | Positive edge weights; multiply attraction. |
| bounds `minx..maxz` | none | Per-vertex coordinate clamps applied after each move. |
| `grid` | auto | Use the cell grid in 2D when n > 1000 (`fruchterman_reingold.c:409-415`). Not available in 3D. |

## Algorithm

Work in `d` dimensions (2 or 3), positions `p_v`, and a displacement accumulator `D_v` per vertex.

1. **Start.** If no seed is given, draw every coordinate uniformly from a box of side `sqrt(n)`
   centred on the origin: `[-sqrt(n)/2, +sqrt(n)/2]` on each axis. Bounds, if any, can widen that box.
2. **Connectivity test.** Test weak connectivity once. If the graph is connected the constant `C` is
   unused. If it is not, set `C = n * sqrt(n)`.
3. **Each iteration** (`niter` times), clear all `D_v`, then:
   1. *Repulsion, every unordered pair `(v, u)`.* With `delta = p_v - p_u` and `r2 = |delta|^2`, add
      `delta / r2` to `D_v` and subtract it from `D_u`. This is a force of magnitude `1/r`
      directed along the line joining the pair.
   2. *Disconnected graphs only.* The pair term is replaced by
      `delta * (C - r^3) / (r2 * C)` with `r = sqrt(r2)`, which equals `delta/r2 - delta*r/C`. The
      subtracted part is a weak linear attraction between every pair, equivalent to an implicit edge
      of weight `n^(-3/2)`. It keeps components from drifting apart.
   3. *Attraction, every edge `(v, u)` with weight `w`.* With `delta = p_v - p_u` and
      `len = |delta| * w`, subtract `delta * len` from `D_v` and add it to `D_u`. The pull grows as `w * r^2`. Balancing it against the `1/r` repulsion of an isolated edge gives an
      equilibrium length `r = w^(-1/3)`.
   4. *Move.* For each vertex add a tiny random perturbation (uniform in `[-1e-9, 1e-9]` per axis)
      to `D_v`. Let `L = |D_v|`. If `L > temp`, rescale `D_v` to length `temp`. If `L > 0`, add `D_v`
      to `p_v`. Then clamp each coordinate to its bounds where a bound exists.
   5. *Cool.* `temp -= start_temp / niter`.

Step 3.4 uses the un-clamped length test on the perturbed vector; with the perturbation `L` is
almost never exactly 0.

## Grid variant (2D only, n > 1000 by default)

Vertices are binned on a square grid covering `[-sqrt(n)/2, sqrt(n)/2]^2` with cell side 2.
Repulsion is computed only for pairs in the same or adjacent cells, and only if `r2 < 4`
(`fruchterman_reingold.c:223`). Attraction, move and cooling are as above. There is no disconnected-graph
correction in this variant (no `C`). The grid is built once from the start positions and is not
rebuilt as vertices move (`:189-200`), so cell membership goes stale as the layout expands. Treat this
as a known approximation of the dense variant.

## Cooling and stopping

Cooling is linear: `temp_i = start_temp * (1 - i / niter)`. It reaches 0 after the last iteration.
There is no convergence test; the loop always runs exactly `niter` iterations. `niter = 0` returns the
start layout (random or seed).

## Where randomness enters

Three places, all through igraph's global generator:

- the initial placement (step 1);
- a coincident-pair fix: if two vertices have `r2 = 0`, redraw `delta` uniformly in
  `[-1e-9, 1e-9]` per axis until non-zero, then use that as the separation;
- the per-vertex tie-breaking noise of step 3.4, once per vertex per iteration.

Consequence: for a fixed generator state the layout is deterministic. The draw order matters (pair loop
order v ascending, u > v ascending, then vertex loop) if bit-level reproduction is wanted.
Ponytail: this doc gives that order from the source but does not claim the generator itself; the
generator (igraph's default Mersenne Twister) is out of scope here.

## Complexity

Dense: `O(n^2)` per iteration for repulsion, `O(m)` for attraction, `O(n)` for the move; total
`O(niter * (n^2 + m))`. Memory `O(n + m)`. Grid variant: roughly `O(niter * (n + m))` for
evenly spread vertices, degrading as cells fill. Connectivity test `O(n + m)` once.

## Defects and quirks worth deciding on

- **3D disconnected repulsion.** At `fruchterman_reingold.c:607` the z component of the pair term
  is added to `D_y` of vertex `v` instead of `D_z`. The other five lines of that block are correct.
  For a disconnected graph in 3D the result therefore differs from the "mathematically intended"
  layout. A faithful port must reproduce it; the decision belongs in `docs/decisions/`.
  **Resolved for this tree (2026-10-02, `sg-igraph-dims`): the port adds the z component to `D_z`,
  the intended axis, and does not reproduce the defect.** Three reasons, in the order they
  mattered. (i) The spec itself calls it a defect and calls the other five lines of the block
  correct, so the block is not a different algorithm — it is one mistyped subscript. (ii) The
  defect only reaches output on a **disconnected** 3D graph, and the disconnected pair term is
  already a documented approximation (an implicit edge of weight `n^(-3/2)`), so reproducing a typo
  inside an approximation buys no accuracy. (iii) It buys no conformance either: the conformance arm
  compares these rows after a Procrustes alignment, which is invariant to the perturbation's
  linear part, and the rows the differential scores are connected gate models where the branch
  never runs. Recording it here rather than in `docs/decisions/` is a scope limit of the job that
  wrote it, not a claim that the decision is unimportant — a reviewer who wants the literal
  behaviour should read `fruchterman_reingold_3d.rs`'s `FAR_PULL` and change one axis index.
- **Equilibrium length.** The doc-comment in the C file quotes an equilibrium length of `1/w^3`.
  Balancing the two forces above gives `r^3 = 1/w`. The comment and the code disagree; the code is
  authoritative.
- **Directed input.** Edge direction is ignored; the attraction is symmetric.
- **Multi-edges** attract once per edge; self-loops contribute zero (`delta = 0`).
- **Scale.** Output is in the arbitrary units of the `sqrt(n)` start box, not normalised; SciGraphs
  rescales afterwards (`_igraph_fit_positions`).
