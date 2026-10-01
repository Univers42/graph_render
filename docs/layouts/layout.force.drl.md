# layout.force.drl - DrL (Distributed Recursive Layout, OpenOrd lineage), 2D

Sources (igraph 0.11.9), all under `vendor/source/igraph/src/layout/drl/`: `drl_layout.cpp:240-485` (presets `:240-400`, entry point `igraph_layout_drl` at `:435`), `drl_graph.cpp` (schedule `ReCompute` `:571`, update loop `:816`, energies `:1007`, analytic step `:1064`, driver `draw_graph` `:1257`), `DensityGrid.cpp` (density field), constants `drl_layout.h:44-55`. The 3D variant (`*_3d.cpp`) has the same structure with a 100^3-cell grid; it is out of scope here except where noted. SciGraphs wrapper: `SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:114-216` (option carrier and presets), `:357-410` (2D call), `:280-355` (3D call).

Papers: Martin, Brown, Klavans, Boyack, "DrL: Distributed Recursive (Graph) Layout", SAND2008-2936, Sandia National Laboratories, 2008 (cited by igraph at `drl_layout.cpp:204` and `:421`); Martin, Brown, Klavans, Boyack, "OpenOrd: an open-source toolbox for large graph layout", Proc. SPIE 7868, 2011. Ancestor: VxOrd (Davidson, Wylie et al.). igraph keeps only the single-process, single-level part; the multi-level recursion and MPI are absent.

## Output
An n x 2 matrix in a layout plane of side 4000 centred on 0 (`VIEW_SIZE` 4000, `HALF_VIEW` 2000). Coordinates are stored as 32-bit floats. SciGraphs rescales the result afterwards.

## Inputs
Undirected simple weighted graph: each edge is stored symmetrically in per-node ordered maps, so parallel edges collapse to the last one written and a self-loop becomes a self-neighbour (`drl_graph.cpp:264-272`, edge map fill). Weights must be positive (`drl_layout.cpp:466`); default weight 1. Optional seed positions (`use_seed`) replace the all-zero start; seeded nodes are not held fixed (`read_real`, `:488-500`, fixed = false).

## Parameters
25 numbers: `edge_cut` plus, for each of six phases (init, liquid, expansion, cooldown, crunch, simmer), `iterations`, `temperature`, `attraction`, `damping_mult`. Damping multipliers must be non-negative. `edge_cut` is not validated in C.

Default preset (`drl_layout.cpp:246-277`, mirrored in SciGraphs `igraph_layouts.py:200`): edge_cut 0.8 (32/40).

| phase | iterations | temperature | attraction | damping_mult |
|---|---|---|---|---|
| init | 0 | 2000 | 10 | 1.0 |
| liquid | 200 | 2000 | 10 | 1.0 |
| expansion | 200 | 2000 | 2 | 1.0 |
| cooldown | 200 | 2000 | 1 | 0.1 |
| crunch | 50 | 250 | 1 | 0.25 |
| simmer | 100 | 250 | 0.5 | 0.0 |

Other presets (coarsen, coarsest, refine, final) differ in a few fields and are tabulated in `drl_layout.cpp:278-400`; coarsen and coarsest swap liquid/expansion attraction to 2/10 and coarsest has 200 crunch iterations; refine and final start at temperature 50 with zero-iteration liquid. SciGraphs passes the bare preset name when no phase field is set and otherwise builds the full 25-field record (damping clamped to [0, 1.5], `igraph_layouts.py:120,144`); the `iterations` argument of the wrapper is ignored.

Fixed constants: density grid 1000 x 1000 cells, cell = 4 layout units (`VIEW_TO_GRID` 0.25), kernel radius 10 cells, border margin 10 cells (`drl_layout.h`, `DensityGrid.cpp`).

## The model
Each node i has an energy
E_i = sum over neighbours j of w_ij * A * q(|x_i - x_j|^2) + D(x_i)
where A = 0.02 * a^4 with a the current attraction (`Compute_Node_Energy`, `:1007-1040`), and s = |x_i - x_j|^2 the squared distance. The attraction term is q(s) = s^4 in the liquid stage (stage 0), s^2 in the expansion stage (stage 1) and s from cooldown on, so attraction grows as distance^8, then distance^4, then distance^2. D is a repulsive density term, described below.

Density D(x): for the coarse field, each node contributes a separable tent kernel (1 - |dx|/10)(1 - |dy|/10) over a 21 x 21 block of cells; D is the square of the accumulated value at the node's cell (`DensityGrid.cpp:93-135`). A position within 10 cells of the border returns the constant 10000 so nodes cannot leave the plane. In the simmer stage the field switches to a fine mode: each cell keeps the list of nodes in it and D = sum over nodes in the 3 x 3 cells around of 1e-4 / (squared distance + 1e-50).

## Algorithm
1. Setup. Convert edge_cut to the cutting schedule: end length L_end = 40000 * (1 - edge_cut), floored at 1; start length 4 * L_end; per-iteration rate r = (start - end) / 400 (`drl_graph.cpp:376-410`). All nodes start at the origin unless seeded. The stage counter starts at 0 with iteration counter = init_iterations (note: it is a starting counter, so a positive value shortens the liquid stage rather than adding a stage; the init temperature, attraction and damping apply to the very first sweep only).
2. Repeat sweeps until the schedule ends. One sweep (`update_nodes`, `:816-890`) visits every node once in index order (single process), and for each node:
   a. remove the node from the density field;
   b. compute the analytic candidate: the weighted centroid c of its neighbours; the candidate is damping * x + (1 - damping) * c where damping = 1 - damping_mult (a node with no neighbours stays put) (`Solve_Analytic`, `:1064-1105`);
   c. edge cutting (skipped when the fine field is on or when edge_cut yields L_end >= 39500): if the node has at least min_edges neighbours, find the neighbour maximising sqrt(degree) * squared distance from c; if that value exceeds the current cut length, delete that one edge from this node's neighbour map only (so the edge becomes one-directional). At most one edge per node per sweep;
   d. compute the energy at the candidate and at a second candidate obtained by jittering it by (0.5 - u) * 0.01 * T per axis with u uniform in [0,1); keep the lower energy, ties going to the jitter;
   e. put the chosen position into the density field (except before the first population of the field).
3. After each sweep, advance the schedule (`ReCompute`, `:571-815`). Stage 0 (liquid) runs liquid.iterations sweeps with liquid temperature, attraction and damping. Stage 1 (expansion) starts from expansion values and each sweep lowers attraction by 0.05 while above 1, min_edges by 0.05 while above 12, cut length by r, and damping_mult by 0.005 while above 0.1. Stage 2 (cooldown) starts from cooldown values and each sweep lowers temperature by 10 while above 50, the cut length by 2r while above L_end, and min_edges by 0.2 while above 1. Stage 3 (crunch) keeps its parameters constant, cut length pinned to L_end, min_edges 1. Stage 5 (simmer) sets min_edges to 99 (disables cutting), turns on the fine field and lowers temperature by 2 per sweep while above 50. Stage 6 ends the run. Between stages, temperature, attraction and damping are reloaded from the next phase and min_edges is reset (12 entering cooldown, 1 entering crunch, 99 entering simmer).
4. Copy positions out.

## Stopping rule
Fixed schedule; there is no convergence test. The number of sweeps is roughly liquid + expansion + cooldown + crunch + simmer iterations plus a small constant number of stage-transition sweeps (each transition runs one extra sweep with the outgoing parameters).

## Randomness
Only the jitter in step 2d, two uniform draws per node per sweep from the host random source. With no seed layout every node starts at the same point, so the first sweeps are symmetric apart from jitter. The seed parameter of the original tool is ignored (`init_parms`, `:376-378`). Iteration order over nodes is the dense index order, and neighbour maps iterate in key order, so the only non-determinism is the generator.

## Complexity
Per sweep O(n * (deg + K^2)) with K = 21 for the kernel update (fixed constant) plus the jitter evaluation; the density grid costs O(cells) = 10^6 floats memory regardless of n, matching the observation in SciGraphs that memory is flat. The fine stage adds neighbour-bin scans proportional to local density. Total O(S * n * (avg degree + 441)) with S the total sweep count (about 800 for the default preset). igraph documents the time complexity as unknown (`drl_layout.cpp:432`).

## Ponytail
- Single-precision arithmetic: positions, energies and density are 32-bit floats; a reimplementation in double will drift from the reference beyond rounding, and the chaotic stage-0 dynamics amplify it.
- Sequential, index-ordered updates: each node sees positions already updated in the same sweep, so the result depends on node numbering.
- The density field is only approximately consistent: nodes are subtracted and re-added using their previous stored position, and the border rule (10000) is a heuristic wall; nodes whose position falls outside the grid after a jump raise a runtime error ("Exceeded density grid").
- Edge cutting is heuristic and permanent, one-directional and can disconnect the effective graph; higher edge_cut cuts more edges.
- Seeds are not pinned, and without a seed every node starts at the origin, a degenerate start that only the jitter breaks.
- The stage-transition sweep count and the stage-0 init-iterations quirk are read from code, not documented by igraph; the figures here were derived by reading, not by running.
