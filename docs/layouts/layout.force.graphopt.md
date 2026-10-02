# layout.force.graphopt

Physical-analogy graph layout (Coulomb repulsion plus Hooke springs), a port of Michael Schmuhl's graphopt, as implemented in igraph 0.11.9. Planar (2D) only.

Sources: `src/layout/graphopt.c` in the igraph 0.11.9 tree; main function at line 343, force helpers at lines 138, 197, 242.
Reference for the original: graphopt 0.4.1 by Michael Schmuhl (web archive of schmuhl.org/graphopt, sourceforge project graphopt), cited in the C doc comment. No journal paper exists.
Defaults passed by SciGraphs: `SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:493-518`.

## Parameters and defaults (SciGraphs, equal to the original graphopt)

| name | default | role |
|---|---|---|
| niter | 500 | number of steps |
| node_charge | 0.001 | charge q of every node; zero disables repulsion |
| node_mass | 30.0 | divisor from force to displacement |
| spring_length | 0.0 | rest length L |
| spring_constant | 1.0 | Hooke constant k |
| max_sa_movement | 5.0 | per-axis displacement cap |

A `None` argument is replaced by these defaults. Coulomb constant is fixed at 8987500000 (line 31).

## Algorithm

1. Start positions: igraph's random layout (no seed given by SciGraphs): per node in vertex order, x then y, each `unif(-1, 1)`, which the RNG evaluates as `u * (h - l) + l` = `u * 2.0 + (-1.0)` with `u` uniform on [0, 1) (`layout/layout_random.c:52-59`, `random/random.c:671-683`). Under python-igraph `u` is `random.random()`. The domain is [-1, 1], not FR's +-sqrt(n)/2. If a seed matrix of the wrong shape were supplied it is ignored with a warning.
2. Repeat niter times, each step synchronous (all forces computed from the positions at the start of the step, then all nodes move together):
   1. Zero the force accumulators.
   2. Repulsion, only if q is not zero: for every unordered node pair at distance d with 0 < d < 500, add force magnitude F = C * ((q * q) / (d * d)), evaluated in that order (the association decides the last bit), pushing them apart along the line joining them, equal and opposite on the two nodes. Pairs at distance zero or at least 500 are skipped.
   3. Springs: for every edge (multi-edges and self loops are visited like any edge; a self loop has distance zero and is skipped), with distance d: skip if d = 0. Let s = k * |d - L|. If d equals L exactly the force is zero. If d > L each endpoint is pulled toward the other with magnitude s / 2; if d < L each is pushed away with magnitude s / 2. So the effective per-node spring force is k |d - L| / 2 (half of Hooke, since both ends move).
   4. Move: displacement of node i along each axis is force_i / node_mass, clamped independently per axis to [-max_sa_movement, +max_sa_movement]. Time step is implicitly 1 and there is no velocity or momentum.
3. Return the positions after the last step.

Force components are obtained as (F * |dx|) / d and (F * |dy|) / d, then given their sign by comparing coordinates; this is equal, bit for bit, to (F * dx) / d. Per pair the first node (lower index) receives `+=` and the second `-=`; per edge the FROM endpoint receives `+=` and TO `-=`.

## Cooling and stopping

None. There is no temperature, no decay of max_sa_movement, and no convergence test: it stops after exactly niter steps. The C documentation states that no stable fixed point is guaranteed.

## Randomness

Only the initial placement. Given a start layout the iteration is deterministic. Force accumulation order (pairs by index, then edges by id) fixes floating-point summation order, so a port that keeps this order reproduces igraph up to the random start.

## Complexity

O(niter * (n^2 + m)); O(niter * m) when node_charge is zero. Memory O(n + m).

## Notes

- With the defaults q^2 C = 1e-6 * 8.9875e9, about 8987.5, so repulsion at distance d is about 8987.5 / d^2; divided by mass 30 the step is about 300 / d^2, capped at 5 per axis.
- With spring_length 0 every edge always pulls its endpoints together (d > 0 = L).
- The 500 cut-off is a hard-coded heuristic (comment at line 397-402 says the force is negligible beyond it, and that this may not always be desirable). It is wrong when the layout is much larger than 500 units: distant clusters then stop repelling. Escape hatch: none in the API.
- SciGraphs rescales the result afterwards (`_igraph_fit_positions`).
