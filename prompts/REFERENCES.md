# Reference implementations — where the math actually lives

Every algorithm this motor ports has a reference implementation. This file maps each one to it, and
says whether that reference is **on disk** (readable now) or **upstream** (must be fetched and pinned
before the port can start).

Read this before any porting phase. A port written from memory of a paper is not a port, it is a
guess with a citation.

## The correction this file exists to make

An earlier draft of the runbook claimed SciGraphs' Python is "a thin adapter — port the algorithms, not
the files." **That is only half true and the wrong half is dangerous.**

True: `_spring_layout_2d` (`networkx_layouts.py:20-28`) really is five lines around
`nx.spring_layout`, and all 8 Graphviz engines shell out to a native binary
(`scigraphs-utils` 0.2.0), and the igraph layouts convert via `_nx_to_igraph`
(`common.py:355-359`) and hand off.

False, and the reason this file exists: `networkx_layouts.py` is **346 lines**, and everything that is
not the five-line spring wrapper is *original engineering of exactly the kind that is expensive to
rediscover*. Spectral and Pivot MDS are implemented **natively there**, on scipy primitives only. So
is Sugiyama, so is circle packing. The math is sometimes borrowed; the **determinism, the fallback
cascades, the correctness guards and the measured performance facts are SciGraphs' own**, and they are
the crown jewels of this port.

## Tier 1 — on disk, port directly

| Algorithm | Reference | Notes |
|---|---|---|
| Grid, random, sphere, spiral, helix, cube | `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py` | trivial geometric; grid is the Phase-2 end-to-end canary |
| Force-directed (Fruchterman–Reingold / spring) | **`graph-engine/node_modules/d3-force/`** (on disk) + `forceLayout.ts:9-26` | `nx.spring_layout` is *also* FR, but d3-force is on disk **and is our differential oracle**, so it is the better reference. Do not fetch networkx for this. |
| Tidy tree (Reingold–Tilford) | `node_modules/d3-hierarchy/` (`tree.js`) | on disk via mermaid's transitive dep |
| Squarified treemap | `node_modules/d3-hierarchy/` (`treemap/squarify.js`) | on disk |
| Circular / radial hierarchy | `SciGraphs/.../mesh/layouts/hierarchical.py` (circular-hierarchy section) + `d3-hierarchy` cluster | concentric BFS rings |
| Sugiyama layered DAG | `SciGraphs/.../mesh/layouts/hierarchical.py:638-691` | **full pipeline, natively implemented**: greedy feedback-arc-set (Eades/Lin/Smyth 1993), longest-path layering, median+transpose crossing reduction, priority-method X-coords. Oracle: `node_modules/dagre-d3-es/`. |
| Circle packing (Collins–Stephenson) | `SciGraphs/.../mesh/layouts/circle_packing.py:281-393`, fallback `:407-542` | **natively implemented.** Emits per-node radii — the one layout SciGraphs' own contract could not hold (see below). |
| Bipartite | `SciGraphs/.../mesh/layouts/` (bipartite section) | falls back to greedy max-cut when not bipartite |

## Tier 2 — on disk, and more valuable than it looks

| Algorithm | Reference | Why it matters |
|---|---|---|
| **Spectral (graph Laplacian)** | `SciGraphs/.../mesh/layouts/networkx_layouts.py:81-175` | **Natively implemented on scipy only.** Carries a three-tier eigensolver cascade and the determinism work described below. This is the single highest-value file in the corpus. |
| **Pivot MDS** | `networkx_layouts.py:176-215` (`_pivot_mds_coordinates`) | **Natively implemented.** Pivot MDS, *not* classical MDS: hop distances to k farthest-point pivots, double-centered, leading eigenvectors of the k×k `Cᵀ C`. **O(k(n+m)) time, O(nk) memory** — against O(n³)/O(n²) for classical MDS over a full distance matrix. An earlier draft of the plan assumed classical MDS and wrongly declared an O(n²)-memory ceiling. There is none. |
| ForceAtlas2 | `SciGraphs/.../mesh/layouts/forceatlas.py` — but it *delegates* (networkx ≥3.4 native, else `fa2`/`fa2_modified`) | **upstream needed** — see below |
| Yifan Hu | `SciGraphs/.../mesh/layouts/yifan_hu.py:344-374` — delegates to bundled Graphviz `sfdp` | **upstream needed**; or treat as "multilevel FR" and build on our own force implementation |

### The determinism engineering to carry forward verbatim

These are decisions, not code style. Losing them loses reproducibility, and rediscovering them costs
days. All in `networkx_layouts.py` and `common.py`:

- **`_eig_start_vector(n)`** (`:57-66`) — a *fixed* start vector, orthogonal to the constant null
  vector. Its own comment states the reason: *"Grids and trees have lambda2 == lambda3, and inside a
  degenerate eigenspace the solver returns whatever rotation its start vector lands on, so an
  arbitrary one makes the layout differ between runs."* This is the answer to the spectral-determinism
  problem, already found.
- **`_fix_eigenvector_signs(vecs)`** (`:68-74`) — eigenvector sign is arbitrary; pin it by the
  largest-magnitude entry.
- **`_eig_converged(L, values, vectors)`** (`:76-79`) — verify `L v = λ v` by residual. Reason given:
  *"a solver that stops on its iteration cap still returns numbers."* Never trust a solver's exit.
- **`which='SM'` is explicitly rejected** (`:82-88`) with a measured fact: the small Laplacian
  eigenvalues sit in a tight cluster, so ARPACK restarts to its default `10 * n` cap, *"which on a
  200k-node mesh means hours rather than a failure the caller can fall back from."*
- **Per-connected-component solving** (`_spectral_component_coordinates`, `:139-175`) — *"The
  Laplacian null space holds one vector per component, so a whole-graph solve hands back component
  indicators and every component collapses to a single point."* A correctness insight most
  implementations get wrong.
- **`_reset_layout_rng`** (`common.py:53-61`) — seeds the shared `RandomState` *and* stdlib `random`
  process-wide, *"because igraph draws from it and takes no seed."* A library that ignores your seed is
  a reproducibility hole; this is the plug. There is a whole `scigraphs_core.repro.determinism` module
  behind `get_layout_seed()` — read it.
- **Solver tier constants** (`:7-15`): `_DENSE_EIG_LIMIT = 256`, `_EIG_RESIDUAL_TOL = 1e-2`,
  `_LOBPCG_MAXITER = 300`, `_MDS_PIVOTS = 100`, `_COMPONENT_SPACING = 2.5`. Tuned values; keep them
  and record any change with a measurement.
- **`_connected_component_indices`** sorts each component's indices (`:44`) — deterministic ordering,
  not incidental.

### The contract defect to fix, not replicate

Circle Packing returns `(positions, radii)` but SciGraphs' layout contract is a bare
`(num_nodes, 3)` position array, validated as exactly that (`common.py:175-185`). So the radii are
smuggled out-of-band into a separate Blender mesh attribute (`circle_packing.py:281`,
`_store_radii_as_mesh_attribute` lines 15-36) that no other consumer reads. Our `Circle` geometry kind
exists precisely to hold this in-band. Same class of defect: `.sgraphs` is **name-keyed** while
`export_positions` (`export_utils.py:177-198`) is **index-keyed** with no names and no edges — two
artifacts from one system that cannot be joined.

## Tier 3 — POST stage, on disk

| Algorithm | Reference | Paper |
|---|---|---|
| FDEB edge bundling | `SciGraphs/engine/scigraphs_engine/bundling/fdeb.py` | Holten & van Wijk 2009 |
| MINGLE | `.../bundling/mingle.py` | Gansner et al. 2011 |
| Obstacle-avoiding grid routing | `.../bundling/routed.py` | Lambert et al. 2010 ("winding roads") |
| Edge style presets (orthogonal/bezier/quadratic) | `SciGraphs/core/scigraphs_core/mesh/edge_styles.py:11-40+` | — |
| SBEB | `.../bundling/sbeb.py` | Ersoy et al. 2011 — **deferred** (image/skeleton-based) |

## Tier 4 — ANALYSIS stage

| Capability | Reference | Notes |
|---|---|---|
| Dijkstra, Bellman-Ford, A*, components, toposort, MST | **`petgraph`** crate | Reuse, do not reimplement. Implement petgraph's traits (`IntoNeighbors`, `Visitable`) on our CSR so we keep one topology representation. Each algorithm determinism-audited before it counts as gated. |
| Communities | `SciGraphs/engine/scigraphs_engine/communities.py` | on disk. Note `pysurprise` (SurpriseMe) is a native binding — out of scope. |
| Centrality | `petgraph` where available, else standard definitions | betweenness needs Brandes' algorithm |

**Defect to fix, not replicate:** SciGraphs offers Bellman-Ford in its UI but the operator silently
dispatches Dijkstra (`docs/tutorials/panels/scigraphs/algorithms.qmd:74-75`), so negative-weight
shortest paths do not work. We implement it and **test the negative-weight case**.

## Upstream references that must be fetched and pinned

Not on disk. `SciGraphs/wheels/` is empty (`.gitkeep` only) and no Python packages are installed
anywhere on this machine — verified. So these cannot be read without fetching:

| Need | Source | Pin | Blocks |
|---|---|---|---|
| ForceAtlas2 exact formulation | networkx `networkx/drawing/layout.py` (`forceatlas2_layout`), or the Jacomy et al. 2014 paper | networkx **3.6** (`SciGraphs/constraints/base.txt`) | Phase 6 |
| igraph FR-3D / Kamada–Kawai / DrL / LGL / Davidson–Harel / Graphopt | igraph C source | python-igraph **0.11.9** | **deferred — not ported** |
| Graphviz dot/neato/fdp/sfdp/twopi/circo/osage/patchwork | Graphviz C source (~200k LOC) | — | **not ported.** SciGraphs itself does not reimplement it; it ships bindings to a binary. Our own set covers the *families*: dot≈Sugiyama, neato/fdp/sfdp≈force/YifanHu, twopi≈radial, circo≈circular, patchwork≈treemap. If exact Graphviz output is ever required the path is FFI, not a port. |

**Rule:** a phase that needs an upstream reference **stops and asks** for it to be fetched rather than
improvising the algorithm. Improvising is the failure this whole file prevents.

`nx.spring_layout` is deliberately absent from this table: FR is already on disk twice over
(`d3-force`, and our own `forceLayout.ts`), and d3-force is the differential oracle, so fetching
networkx to reference the same algorithm would add a dependency and a second source of truth for no gain.
