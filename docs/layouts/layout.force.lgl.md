# layout.force.lgl - Large Graph Layout (igraph flavour)

Sources (igraph 0.11.9): `vendor/source/igraph/src/layout/large_graph.c:90` (`igraph_layout_lgl`), grid helper `src/core/grid.c:48-262`, Python defaults `src/_igraph/graphobject.c:8634-8672`. SciGraphs defaults: `SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:423-460`.

Paper: Adai, Date, Wieland, Marcotte, "LGL: creating a map of protein function with an algorithm for visualizing very large biological networks", J. Mol. Biol. 340(1):179-190, 2004. igraph keeps the layered growth idea but replaces LGL's per-layer spring solver with a Fruchterman-Reingold style annealer whose repulsion is limited to a spatial grid (`large_graph.c:44-56`). Weights are not used; the paper's minimum spanning tree becomes a plain breadth-first tree (`large_graph.c:143-150`).

## Output
An n x 2 matrix. The plane is 2D only; SciGraphs then rescales the result (`_igraph_fit_positions`) and reports z = 0.

## Parameters and defaults
The C function has no defaults; the Python binding resolves them, and SciGraphs passes through only what is set.

| name | binding default | SciGraphs default | meaning |
|---|---|---|---|
| maxit (`maxiter`) | 150 | 150 | cooling iterations per layer |
| maxdelta | n (vertex count) | unset -> n | largest displacement per iteration at full temperature |
| area | n squared | unset -> n^2 | area of the placement disc |
| coolexp | 1.5 | 1.5 | cooling exponent |
| repulserad | area * n | unset -> area * n | radius term at which repulsion cancels |
| cellsize | area^(1/4) | unset -> area^(1/4) | side of a grid cell, also the repulsion cutoff |
| root | random | unset | first vertex; negative means random |

A non-positive value passed to SciGraphs for maxdelta, area, repulserad or cellsize is dropped, so the binding default applies. The C function rejects maxit < 0 and non-positive maxdelta, area, coolexp, repulserad, cellsize (`large_graph.c:118-140`). A zero-vertex graph returns an empty matrix before those checks (`:108-115`).

Derived constants: R = sqrt(area / pi) (radius of the placement disc), k = sqrt(area / n) (ideal edge length, called frk), H = sum over layers l = 1..L-2 of 1/l (harmonic number, `:201-203`), s = R / H (layer spacing constant), epsilon = 1e-5 (`10e-6`, `:214`).

## Algorithm
1. Choose the root: the given one, or a uniformly random vertex (`:160-165`).
2. Run a breadth-first search from the root over all edges ignoring direction. It yields the visit order, the layer boundaries (layer 0 is the root) and each vertex's BFS parent (`:172`). Vertices not reached keep a parent of -2 and produce only a warning that disconnected graphs are unsupported (`:177-180`); they stay wherever step 3 puts them and are never in the grid.
3. Scatter every vertex uniformly at random in the disc via the random layout scaled by R (`:189-190`), build a 2D grid of square cells of side `cellsize` covering [-R, R]^2, and insert the root at the origin (`:193-199`).
4. For each layer l = 1 .. L-1 in order:
   a. Placement. For each vertex v of layer l-1 (in BFS order) take the grid's centre of mass, normalised to unit length (m), and the unit vector from v's parent to v (p; zero if they coincide). The anchor is a = pos(v) + m + p. For each child c of v (children are contiguous in BFS order) choose a unit direction d and place c at a + (s / l) * d. In layer 1 the directions are equally spaced: angle 2*pi*(j-1)/(size of layer 1 - 1) by BFS slot index j; deeper layers draw d from a uniform square [-1,1]^2 and normalise it (`:222-270`). The root itself is pinned to (0,0). Children join the grid as they are placed.
   b. Edge activation. Every edge incident to a layer-l vertex whose other endpoint is already in the grid is appended to the active edge list. The list only grows, and an edge whose both endpoints lie inside layer l can be appended twice (once from each end, see Ponytail) (`:272-290`).
   c. Relaxation (annealing) on all vertices in layers 0..l, in visit order. Repeat while it < maxit and maxchange > epsilon:
      - temperature T = maxdelta * ((maxit - it) / maxit)^coolexp (`:299`);
      - zero the force accumulators;
      - attraction along each active edge: with distance d, a pull of magnitude d^2 / k toward the other endpoint;
      - repulsion for every pair of grid vertices in the same cell or the three "forward" neighbouring cells (right, up, up-right; each unordered pair visited once, `grid.c:213-260`) whose distance d < cellsize: push apart with magnitude k^2 * (1/d - d^2 / repulserad); a zero distance is replaced by epsilon;
      - each force vector is clipped to length T, then applied through the grid move; maxchange is the largest positive single coordinate displacement seen (`:355-372`). Because only positive components are compared, a step that moves only in negative directions counts as zero change (see Ponytail).
5. Return the matrix. Vertices are moved in place; there is no final rescaling in the C code.

## Cooling and stopping
Each layer restarts its own cooling curve at it = 0: T falls from maxdelta to 0 along a power law of exponent coolexp. The layer ends after maxit iterations or as soon as the largest positive coordinate change is <= 1e-5. The whole run ends after the last layer.

## Randomness
Three places: the root when none is given, the initial uniform scatter of all vertices (which fixes the position of any unreached vertex), and the random directions for layers >= 2. Layer 1 directions are deterministic. There is no other random step; the force loop is deterministic given the grid state. Determinism of the grid: cell membership lists are LIFO, so pair visit order depends on insertion and move history; this changes float summation order only.

## Complexity
Ideal O(D * maxit * (V + E)) with D the number of layers; worst case O(D * maxit * (V^2 + E)) when many vertices share one cell (`large_graph.c:83-87`). Each iteration touches all vertices of layers 0..l, so late layers cost the most. Memory O(V + E + cells).

## Ponytail
- Cell list order and float summation: results match only to rounding across implementations unless the same insertion order is reproduced.
- Repulsion is cut off at `cellsize`, so vertices farther apart than that do not repel; the result depends on the cell size chosen.
- maxchange only sees positive components, so convergence can trigger early when the net movement is negative in both axes.
- Duplicate active edges (both endpoints in the same layer) count twice in attraction. Multi-edges and self-loops are treated as ordinary edges.
- Disconnected input is unsupported: unreached vertices keep their random start and take part in no force computation.
- Root choice materially changes the picture, so seeded reproducibility needs the root fixed as well as the generator.
