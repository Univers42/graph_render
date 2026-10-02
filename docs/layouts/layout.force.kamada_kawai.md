# layout.force.kamada_kawai

Clean-room specification of the Kamada-Kawai layout as implemented in igraph 0.11.9.

Source: `vendor/source/igraph/src/layout/kamada_kawai.c` in the igraph 0.11.9 tree
(`/goinfre/dlesieur/refs/igraph-0.11.9/igraph-0.11.9/`).

- 2D: `igraph_layout_kamada_kawai`, lines 103-350 (start layout 158-168, delta init 217, vertex pick 243, solve 271-296, bounds 298-309).
- 3D: `igraph_layout_kamada_kawai_3d`, from line 412 (start layout ~476-486, delta init 530, vertex pick 563, solve 623).
- Constant `KK_EPS = 1e-13` at line 28.

Paper: T. Kamada and S. Kawai, "An Algorithm for Drawing General Undirected Graphs",
Information Processing Letters 31(1), 7-15, 1989. https://doi.org/10.1016/0020-0190(89)90102-6
(cited in the doc comment of `kamada_kawai.c`, lines ~30-60).

## What SciGraphs asks for

`SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:85-111` calls the binding with `dim=3` and
forwards `maxiter`, `epsilon`, `kkconst` only when the caller gives a positive value. No seed, weights
or bounds. So the 3D variant runs from the deterministic sphere start with binding defaults.

## Parameters and defaults

| Parameter | Default (binding, `src/_igraph/graphobject.c:~8140`) | Meaning |
|---|---|---|
| `maxiter` | `50 * n` | Maximum number of single-vertex moves. Must be non-negative. |
| `epsilon` | 0 | Stop when the largest squared gradient norm is below it. 0 means never stop early (in practice only the equilibrium guard below acts). |
| `kkconst` | `n` | Spring strength constant `K`. Must be positive. |
| `seed` | none | Optional start coordinates. |
| `weights` | none (all 1) | Positive edge lengths for the shortest-path step. |
| bounds | none | Per-vertex clamps. |

## Algorithm

Model: every pair of vertices is joined by a spring. Energy `E = sum over pairs (1/2) k_ij (|p_i - p_j| - l_ij)^2`.
The method minimises `E` by moving one vertex at a time, choosing the vertex with the largest gradient.

1. **Start layout.**
   - With a seed: use it.
   - Without a seed but with any bound given: uniform random inside the bounded box (same generator as
     the FR start, box side `sqrt(n)`).
   - Otherwise: place the vertices deterministically on a circle (2D) or sphere (3D) of radius 1 using
     igraph's circle/sphere layouts, then scale by `0.36 * sqrt(n)`. The 0.36 is an empirical choice
     stated in the source (`kamada_kawai.c:161-167`): it makes the start close to the equilibrium of a
     cycle graph. The paper recommends 0.5.
   - If `n <= 1`, return here.

2. **Distances.** Compute all-pairs shortest-path distances `d_ij` on the graph treated as undirected,
   with edge weights as lengths (Dijkstra from every vertex). Let `d_max` be the largest finite one. Every
   infinite distance (different components) is replaced by `d_max`; that also makes every entry at most
   `d_max`. Disconnected components are thus treated as `d_max` apart.
3. **Spring constants.** Let `L0 = sqrt(n)` and `L = L0 / d_max`. Then
   `l_ij = L * d_ij` and `k_ij = K / d_ij^2`, for `i != j`.
4. **Initial gradient.** For each vertex `m` compute the gradient of `E` with respect to its position:
   `g_m = sum over i != m of k_mi * (delta - l_mi * delta / |delta|)` with `delta = p_m - p_i`.
   Store one component per axis (`D1, D2, D3`).
5. **Iterate**, at most `maxiter` times:
   1. Pick `m` maximising `|g_m|^2` (ties: the lowest index, since only strictly greater replaces). If
      that maximum is below `epsilon`, stop.
   2. Build the Hessian block of `E` for `m` alone, from the other vertices held fixed:
      diagonal terms `sum k_mi (1 - l_mi * (other two squared components) / r^3)` and off-diagonal
      terms `sum k_mi * l_mi * delta_a * delta_b / r^3`, where `r = |p_m - p_i|`.
   3. Solve the Newton step `H * step = g_m`. 2D solves the 2x2 system by the closed form; 3D solves the
      3x3 system by Cramer's rule with determinants.
      **Equilibrium guard:** if `|g_m|^2 < KK_EPS^2` the step is zero. This avoids a zero determinant
      on path graphs.
   4. Move `m` by minus the step in the sign convention of the source (the solve yields the correction to
      add), then clamp to bounds where given.
   5. Update the gradients incrementally: for every other vertex `i`, remove the old contribution of
      pair `(m, i)` and add the new one; recompute `g_m` from scratch. Cost `O(n)`.

## The 3D start: the sphere (spec gap closed 2026-10-02)

Step 1's third case said "sphere … using igraph's sphere layouts" and left the placement itself
unwritten, which left the implementer with nothing to port. Closed here, as rule 1 of
`docs/decisions/layouts-igraph.md` requires: the spec author read the C and wrote the formula, and
the formula is all that follows.

`igraph_layout_kamada_kawai_3d` calls `igraph_layout_sphere` on the empty matrix and then scales it
by `0.36 * L0` (`kamada_kawai.c:476-485`), where `L0 = sqrt(n)`. `igraph_layout_sphere`
(`vendor/source/igraph/src/layout/circular.c:153-183`) walks `i = 0 .. n-1` in vertex-id order with
`phi = 0` carried across iterations and writes, per `i`:

| case | `z` | `r` | `phi` |
|---|---|---|---|
| `i == 0` | `-1` | `0` | unchanged |
| `i == n-1` | `+1` | `0` | unchanged |
| otherwise | `-1 + 2 i / (n - 1)` | `sqrt(1 - z*z)` | `phi += 3.6 / (sqrt(n) * r)` |

and then `x = r * cos(phi)`, `y = r * sin(phi)`. The first and last rows are special-cased in the
source to avoid a division by zero and to keep `1 - z*z` from going slightly negative, so a port
branches on them rather than guarding afterwards. Note `phi` advances only on interior rows, so
row 0 sits at `(0, 0, -1)` and the last row at `(0, 0, +1)` whatever `n` is.

The `3.6` is in radians per unit of `(sqrt(n) * r)`: it is the Saff–Kuijlaars spiral's constant, and
the paper the function cites is E. B. Saff and A. B. J. Kuijlaars, "Distributing many points on a
sphere", *Mathematical Intelligencer* 19(1), 5–11, 1997, <https://doi.org/10.1007/BF03024331>.

**Not specified here, and left to the port:** the 2D circle's own angles. `igraph_layout_circle`
(`circular.c:104-122`) advances `phi += 2 pi / (n - 1)`, so its first and last vertices land on the
same point. That is a 2D fact, the 2D id is pinned byte-for-byte, and nothing in the 3D path calls
it; the 3D port therefore does not need it and this spec does not bless it.

## Cooling and stopping

There is no temperature. Convergence is Newton descent on one vertex at a time, and the loop ends on
either the `epsilon` test (step 5.1) or after `maxiter` moves. With the default `epsilon = 0` the test
`max_delta < 0` is never true, so the run uses all `maxiter = 50 n` moves, although moves become zero
once every gradient is under `KK_EPS`.

## Where randomness enters

Only when a seed is absent and bounds are supplied (step 1, second case). The default path
(circle or sphere start) uses no random numbers, so the output is fully deterministic for a given input
and vertex order, as the SciGraphs docstring says. Ties in step 5.1 break by lowest index.

## Complexity

Set-up: `O(n * (m + n log n))` for all-pairs Dijkstra, plus `O(n^2)` for the matrices `d`, `k`, `l`
(memory `O(n^2)`, three matrices). Each iteration: `O(n)`. Total: `O(n^2 log n + maxiter * n)`, which is
`O(n^2 log n + 50 n^2)` at default `maxiter`. Not suitable for large graphs.

## Quirks worth deciding on

- **Zero distance.** If two vertices coincide, `|delta| = 0` and the terms `delta / |delta|` are NaN.
  The source does not guard this; the circle or sphere start avoids it for distinct indices.
- **Isolated graph.** If `d_max = 0` (no edges), `L = L0 / 0` is infinite. The source does not special-case
  it, so an edgeless graph with `n >= 2` gives non-finite output. Ponytail: inferred by reading the
  arithmetic at lines ~205-214, not by running igraph.
- **Sign of the solve.** 2D uses gradient `+g` with `delta_x = (B*D2 - C*D1)/det`; the 3D variant negates
  the gradient (`Ax = -D1`) and Cramer's rule. Both encode the same Newton step; a port should test both
  against oracle output rather than trust this prose on signs.
- **Scale.** Output lives in units where the mean spring length is about `sqrt(n) / d_max` per hop.
  SciGraphs normalises afterwards.
