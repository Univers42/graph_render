# The six igraph layouts are written from prose specs and papers, never from igraph's C

Status: **decided** (p12-igraph, step 1). Scope: `layout.force.{fruchterman_reingold,
kamada_kawai, drl, lgl, davidson_harel, graphopt}`.

## Context

SciGraphs offers six 2D layouts through python-igraph
(`SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py`). igraph's C core is
GPL-2.0-or-later and this repository is not GPL, so a port that follows the C line by line
would be a derivative work.

## Decision

1. **Licence rule.** No line-by-line translation. A spec author may read the igraph C
   source and the cited paper and writes `docs/layouts/<id>.md`: steps, parameters and
   defaults, cooling and stopping rule, where randomness enters, complexity, citation
   (paper, and C file:line). The spec holds formulas and prose, no code. The implementer
   reads the spec and the paper only and never opens the igraph sources
   (`/goinfre/dlesieur/refs/igraph-0.11.9`). Fruchterman-Reingold and Kamada-Kawai may also
   be read from networkx 3.6 (BSD): `drawing/layout.py` lines 452 and 876.
2. **Blocked, not improvised.** A layout whose algorithm cannot be pinned down from the
   paper and its spec is `status: blocked`, and reported; the others proceed.
3. **The oracle may call igraph.** `harness/oracle-igraph.py` runs python-igraph in the
   `ge-python-oracle` image. It reads no source; it executes the released package.
4. **Determinism.** Our seeded generator only (Mulberry32, `initial_positions`), libm
   transcendentals, fixed reduction order, no `HashMap`. igraph's own RNG is never
   reproduced.

## Reference pin

igraph 0.11.9 sdist (the PyPI package `igraph`; `python-igraph` 0.11.9 is a 9.7 kB shim),
sha256 `c57ce44873abcfcfd1d61d7d261e416d352186958e7b5d299cf244efa6757816`, fetched by
`scripts/orch/fetch-refs.sh` to `/goinfre/dlesieur/refs/igraph-0.11.9`. The oracle image
builds it from that digest-checked tarball in a throwaway stage (`docker/python-oracle.Dockerfile`,
named context `ig`), asserts `igraph.__version__ == "0.11.9"`, and ships only the wheel.
Build tools (setuptools 80.9.0, wheel 0.45.1) and texttable 1.7.0 are hash-pinned.

## Oracle metric

igraph draws from its own RNG for most of these and every one is iterative, so coordinates
cannot gate (`prompts/RESUME.md` item 4, ForceAtlas2). Each layout's differential is
normalised stress against graph distance at the optimal uniform scale (weights D^-2,
pairs within a component), and the recorded worst is `max(ours / igraph)` over seeds.
igraph is handed our start positions as `seed` where the layout accepts one (all but LGL),
and `random` is seeded per case for the rest.

| Layout | Start passed | Why stress |
|---|---|---|
| Fruchterman-Reingold | yes | force balance, not stress: a quality floor only |
| Kamada-Kawai | yes | minimises exactly this energy: the sharpest of the six |
| DrL | yes | multilevel force-directed, quality floor |
| LGL | no | igraph's own start; quality floor |
| Davidson-Harel | yes | annealed, aesthetic energy: quality floor |
| Graphopt | yes | spring/charge model: quality floor |

Ceilings are measured (2026-09-29, 100 seeds) and rounded up to the next power of ten, as the
other differentials do: FR 1.30, KK 1.35, DrL 3.98, LGL 2.13 -> 10; Davidson-Harel 51.9,
Graphopt 15.4 -> 100. Davidson-Harel is handed SciGraphs' parameters (the ones our defaults
follow) rather than python-igraph's density-dependent ones. Graphopt's ratio is large because
igraph's stress reference is near-optimal on tiny graphs, not because either arm is chaotic.
A layout with no registered implementation writes no
`ours` column and has zero compared cases, which the verdict fails: NOT-RUN, never green.

## Consequences

- `graph-cli emit-igraph-fixtures` / `oracle-igraph` follow the `oracle-spectral` pattern;
  fixtures are emitted once and both arms read the same file.
- Ponytail: stress penalises a correct force-balance layout that igraph's stress happens to
  favour, and rewards an ugly stress-optimal one; a ratio near the ceiling needs a look at
  the picture before it is called a defect.
