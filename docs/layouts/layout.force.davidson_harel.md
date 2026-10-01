# layout.force.davidson_harel

Simulated-annealing graph drawing after Davidson and Harel, as implemented in igraph 0.11.9.
Planar (2D) only. Edge directions are ignored.

Sources: `src/layout/davidson_harel.c` in the igraph 0.11.9 tree (function at line 143, helpers at lines 36 and 60).
Paper: Ron Davidson, David Harel, "Drawing Graphs Nicely Using Simulated Annealing", ACM Transactions on Graphics 15(4), 301-331, 1996, doi 10.1145/234535.234538.
Defaults passed by SciGraphs: `SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:461-491`.

## Parameters and defaults (SciGraphs)

| name | default | role |
|---|---|---|
| maxiter | 10 | annealing rounds |
| fineiter | 0 | fine-tuning rounds (SciGraphs never enables them) |
| cool_fact | 0.95 | multiplicative cooling per round, must lie in (0,1) |
| weight_node_dist | 1.0 | node-node repulsion |
| weight_border | 0.0 | border repulsion (off) |
| weight_edge_lengths | 1.0 | squared edge length penalty |
| weight_edge_crossings | 1.0 | crossing count penalty |
| weight_node_edge_dist | 1.0 | node-to-edge closeness penalty, active only in fine-tuning |

A `None` argument is replaced by these defaults. Negative maxiter or fineiter, or cool_fact outside (0,1), is an error (`davidson_harel.c:176-184`). No seed is given by SciGraphs, so the start is random. With fineiter = 0 the node-edge term never runs.

## Fixed constants

- Canvas: a square of side W = 10 sqrt(n), centred on the origin (line 153).
- Initial move radius r0 = W / 2 (line 158).
- Candidate moves per node per round: 30, at angles 2 pi k / 30 (lines 163, 238-241).
- Fine-tuning radius factor 0.01 (line 159).

## Algorithm

1. If n = 0 return an empty layout.
2. Place every node uniformly at random in the canvas, and record the bounding box of the placed points.
3. Run maxiter + fineiter rounds. Round index below maxiter is annealing, the rest are fine-tuning.
4. At the start of each round shuffle the node order. In a fine-tuning round set the move radius to 0.01 times the smaller of the bounding box width and height (this replaces the cooled radius).
5. For each node v in shuffled order, shuffle the 30 candidate directions, then try each in turn (so up to 30 trials per node per round, each judged against the current positions, and each accepted trial changes the node's position for the next trial):
   - Candidate position = current position + r * (cos, sin) of the direction, with r the current move radius.
   - Clamp to the canvas: a coordinate beyond the edge is set to the edge minus 1e-6 (lines 269-280). For the low side this places it just outside, at -W/2 - 1e-6; the clamp is not symmetric.
   - Compute the energy change dE between candidate and current position, from the terms below.
   - Accept if dE < 0. In an annealing round also accept with probability exp(-dE / r). Fine-tuning rounds accept only strict improvements.
   - On acceptance update the position and grow the bounding box (it never shrinks).
6. After each round multiply the move radius by cool_fact.

## Energy terms (change when v moves from p to q)

All weights multiply their term; a zero weight skips the term entirely.

- Node distance: sum over u != v of w_nd * (1/|q - u|^2 - 1/|p - u|^2). There is no guard against a zero distance, so coincident nodes give infinity.
- Border: for the four distances to the canvas sides d, term w_b * sum 1/d^2, with any negative d replaced by 2. Delta is new minus old.
- Edge length: sum over neighbours u of v (multi-edges counted repeatedly) of w_el * (|q - u|^2 - |p - u|^2).
- Edge crossings: for each neighbour u of v, and each edge (a,b) touching neither v nor u, count segment (q,u) crossing (a,b) minus segment (p,u) crossing (a,b), using a parametric intersection test that returns false for parallel segments. Delta is w_ec times that signed count. Cost is O(deg(v) m).
- Node-edge distance (fine-tuning only): for every edge not incident to v, w_ne * (1/d^2(q, edge) - 1/d^2(p, edge)) with d the point to segment distance; and for each edge (v,u) incident to v, every other node w != v,u contributes w_ne * (1/d^2(w, segment q-u) - 1/d^2(w, segment p-u)). A zero-length segment degrades to point distance. No guard for zero distance.

## Cooling and stopping

Geometric cooling of the move radius: r_k = r0 * cool_fact^k over annealing rounds. The same value is the temperature in the acceptance test. There is no convergence test: the algorithm stops after exactly maxiter + fineiter rounds. With the defaults the final annealing radius is r0 * 0.95^10, about 0.60 r0.

## Randomness

Uniform random numbers enter in the initial placement (2 per node), the node-order shuffle each round, the direction shuffle per node per round, and the annealing acceptance draw (one uniform per trial whose dE is not negative, annealing rounds only). The result is therefore not reproducible except through the RNG stream; a port must define its own seeded generator, and cannot match igraph coordinates bitwise.

## Complexity

Per annealing round O(n * 30 * (n + deg(v) m)), which the source states as O(n^2 + m^2) per iteration in its own accounting. Per fine-tuning round O(n * 30 * (m + deg(v) n)); the source states O(mn). Memory O(n + m).

## Notes for a port

- The bounding box is used only to size the fine-tuning radius. It starts from reversed extremes (line 161) and is updated with an else-if chain (lines 207-217, 429-437), so a single point that is both a new minimum and maximum updates only the minimum. It never shrinks after moves.
- SciGraphs rescales the returned coordinates afterwards (`_igraph_fit_positions`), so absolute canvas size is not observable there.
- Heuristic: simulated annealing over a non-convex energy; it finds a local optimum, and the quality depends on the weights, which igraph documents as graph dependent (density-based suggestions in the C doc comment lines 122-133 differ from the SciGraphs defaults).
