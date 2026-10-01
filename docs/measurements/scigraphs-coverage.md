# SciGraphs layout coverage in the motor and the studio picker

Counted 2026-09-30 from the tree, read-only. The key is the SciGraphs side: one row per name
`apply_graph_layout` accepts, which is what the studio picker's SciGraphs-compatible set is. The
`dispatcher.py` line is the `elif algorithm ==` that selects the name; the eight Graphviz engines share
one branch (`dispatcher.py:140`) and are named in `yifan_hu.py:7-16`.

The motor side is `LAYOUTS` in `crates/graph-core/src/registry.rs:90` (27 entries) read through
`scripts/orch/gr cargo run -q -p graph-cli -- capabilities`; a row's registry id appears in that
command's output under `id`, and the ledger row it comes from is built at
`crates/graph-cli/src/capabilities/registry/layout_row.rs:28-56`. The studio picker reads the same registry, so a
layout reaches the picker the day it lands in `LAYOUTS` — there is no second list to update.

Regenerate the motor column with:

```sh
scripts/orch/gr cargo run -q -p graph-cli -- capabilities
git log --oneline origin/develop..origin/p12-igraph
```

| SciGraphs name | dispatcher | SciGraphs function | dim | motor registry id | status | reference the motor ports from | gating oracle |
|---|---|---|---|---|---|---|---|
| `RANDOM` | `dispatcher.py:51` | `_random_layout` `basic.py:5` | 3D | `layout.random` | on develop (2D port) | networkx 3.6 `random_layout` | `oracle-closed-form`, `capabilities/registry/unproven.rs:71`, `registry/closed_form.rs:23` |
| `GRID` | `dispatcher.py:53` | `_grid_layout` `basic.py:11` | 2D (z=0) | `layout.grid` | on develop | hand convention | `roundtrip` hand oracle, `registry/grid.rs:22` |
| `SPRING` | `dispatcher.py:55` | `_spring_layout_2d` `networkx_layouts.py:16` | 2D | `layout.force.spring` | on develop (p12-t2) | networkx 3.6 `spring_layout` at `dim=2` | `oracle-spring`, networkx arm, stress deficit, `unproven.rs:67` |
| `SPRING_3D` | `dispatcher.py:57` | `_spring_layout_3d` `networkx_layouts.py:26` | 3D | — | missing | networkx 3.6 `spring_layout` at `dim=3` | blocked on `contract-3d`; then the same `oracle-spring` at dim 3 |
| `CIRCLE_PACKING` | `dispatcher.py:59` | `_circle_packing_layout` `circle_packing.py:281` | 2D (Z=0) | `layout.packing.circle` | on develop | hand + planarity certificate | `roundtrip` hand oracle, `registry/grid.rs:61` |
| `FORCEATLAS2` | `dispatcher.py:62` | `_forceatlas2_layout` `forceatlas.py:150` | 3D by default (`dim=3`) | `layout.forceatlas2` | on develop (2D port) | networkx 3.6 `forceatlas2_layout` | `oracle-fa2`, `unproven.rs:59`, `registry/force.rs:190` |
| `IGRAPH_FR` | `dispatcher.py:65` | `_igraph_fruchterman_reingold` `igraph_layouts.py:53` | 3D (`'dim': 3`) | `layout.force.fruchterman_reingold` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `IGRAPH_KK` | `dispatcher.py:68` | `_igraph_kamada_kawai` `igraph_layouts.py:85` | 3D (`'dim': 3`) | `layout.force.kamada_kawai` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `IGRAPH_DRL` | `dispatcher.py:78` | `_igraph_drl` `igraph_layouts.py:281` | 3D (`'dim': 3`) | `layout.force.drl` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `IGRAPH_DRL_2D` | `dispatcher.py:83` | `_igraph_drl_2d` `igraph_layouts.py:359` | 2D (`'dim': 2`) | `layout.force.drl` (shared with the row above) | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `IGRAPH_LGL` | `dispatcher.py:88` | `_igraph_lgl` `igraph_layouts.py:423` | 2D | `layout.force.lgl` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `SPHERE` | `dispatcher.py:101` | `_sphere_layout` `basic.py:22` | 3D | — | missing | SciGraphs itself (Fibonacci sphere, `basic.py:22-34`) | hand + a SciGraphs-arm `oracle-basic-3d` |
| `SPECTRAL_3D` | `dispatcher.py:103` | `_spectral_layout_3d` `networkx_layouts.py:249` | 3D (`dims=3`) | `layout.spectral` | on develop (2D port) | SciGraphs `_spectral_component_coordinates` on scipy 1.16.2 | `oracle-spectral`, `registry/spectral.rs:25` |
| `SPIRAL_3D` | `dispatcher.py:105` | `_spiral_layout_3d` `basic.py:36` | 3D | `layout.spiral` | on develop (2D port) | networkx 3.6 `spiral_layout`, 2D | `oracle-closed-form`, `registry/closed_form.rs:56` |
| `HELIX` | `dispatcher.py:107` | `_helix_layout` `basic.py:65` | 3D | — | missing | SciGraphs itself (double helix, `basic.py:65-80`) | hand + a SciGraphs-arm `oracle-basic-3d` |
| `CUBE` | `dispatcher.py:109` | `_cube_layout` `basic.py:83` | 3D | — | missing | SciGraphs itself (cube corners + interior, `basic.py:83-`) | hand + a SciGraphs-arm `oracle-basic-3d` |
| `HIERARCHICAL_3D` | `dispatcher.py:111` | `_hierarchical_layout_3d` `hierarchical.py:113` | 3D | — | missing | SciGraphs itself (`_disk_positions` `hierarchical.py:90`, levels `hierarchical.py:54`) | hand + a SciGraphs-arm `oracle-hierarchical-3d` |
| `BIPARTITE_3D` | `dispatcher.py:113` | `_bipartite_layout_3d` `hierarchical.py:213` | 3D | `layout.bipartite` | on develop (2D port) | networkx 3.6 `bipartite_layout` given SciGraphs' node sets | `oracle-closed-form`, `registry/closed_form.rs:72` |
| `IGRAPH_DH` | `dispatcher.py:115` | `_igraph_davidson_harel` `igraph_layouts.py:461` | 2D (planar) | `layout.force.davidson_harel` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `IGRAPH_GRAPHOPT` | `dispatcher.py:130` | `_igraph_graphopt` `igraph_layouts.py:493` | 2D (planar) | `layout.force.graphopt` | in flight: p12-igraph | igraph | `oracle-igraph` (on the branch) |
| `MDS_3D` | `dispatcher.py:136` | `_mds_layout_3d` `networkx_layouts.py:271` | 3D | `layout.mds.pivot` | on develop (2D port) | SciGraphs `_pivot_mds_component_coordinates` on scipy 1.16.2 | `oracle-spectral`, `registry/spectral.rs:52` |
| `YIFAN_HU` | `dispatcher.py:138` | `_yifan_hu_layout` `yifan_hu.py:344` | 2D / 2Z / 3 by `props.sfdp_dim` | `layout.force.yifan_hu` | on develop (2D only) | SciGraphs' own multilevel scheme — explicitly **not** Graphviz `sfdp` | `stress` only, `unproven.rs:62`, `registry/force.rs:228` |
| `GRAPHVIZ_TWOPI` | `dispatcher.py:140` | `_graphviz_engine_layout` `yifan_hu.py:340` | 2D default | `layout.twopi` | in flight: p13-gv1 | Graphviz `twopi` 16.1.0 `lib/twopigen/circle.c` | `oracle-twopi`, `capabilities/registry/unproven.rs:86`, `registry/radial.rs:32` |
| `GRAPHVIZ_CIRCO` | `dispatcher.py:140` | same, `engine='circo'` | 2D default | — | planned: p13-gv1 | Graphviz `circo` | Graphviz's own output, docker-only oracle |
| `GRAPHVIZ_OSAGE` | `dispatcher.py:140` | same, `engine='osage'` | 2D default | `layout.packing.osage` | in flight: p13-gv1 | Graphviz `osage` 16.1.0 `lib/osage/osageinit.c` + `lib/pack/pack.c` | `oracle-graphviz --engine osage`, `capabilities/registry/unproven.rs:65`, `registry/graphviz_osage.rs`; **agrees exactly only below 11 nodes** — see `docs/measurements/p13-gv1-osage.md` |
| `GRAPHVIZ_PATCHWORK` | `dispatcher.py:140` | same, `engine='patchwork'` | 2D default | `layout.treemap.patchwork` | in flight: p13-gv1 | Graphviz `patchwork` 16.1.0 `lib/patchwork/tree_map.c` | `oracle-graphviz --engine patchwork`, `capabilities/registry/unproven.rs:106`, `registry/graphviz_patchwork.rs:40`; **worst gap 6.6e-2 pt = the oracle's own printed quantum** — see `docs/measurements/p13-gv1-patchwork.md` |
| `GRAPHVIZ_NEATO` | `dispatcher.py:140` | same, `engine='neato'` | 2D default; 3D eligible (`GRAPHVIZ_NATIVE_3D_ENGINES` `yifan_hu.py:18`) | — | planned: p13-gv2 | Graphviz `neato` | Graphviz's own output, docker-only oracle |
| `GRAPHVIZ_FDP` | `dispatcher.py:140` | same, `engine='fdp'` | 2D default | — | planned: p13-gv2 | Graphviz `fdp` | Graphviz's own output, docker-only oracle |
| `GRAPHVIZ_SFDP` | `dispatcher.py:140` | same, `engine='sfdp'` | 2D default; 3D eligible (`yifan_hu.py:18`) | `layout.force.sfdp` | in flight: p13-gv2 | Graphviz `sfdp` 16.1.0 `lib/sfdpgen/spring_electrical.c` | `oracle-graphviz --engine sfdp`, `capabilities/registry/unproven.rs:121`, `registry/graphviz_sfdp.rs:45`; **seed-sensitive** — the oracle disagrees *with itself* by 4.81e+2 pt between `-Gstart` 1 and 7, our worst gap is 3.88e+2 pt — see `docs/measurements/p13-gv2-sfdp.md` |
| `GRAPHVIZ_DOT` | `dispatcher.py:140` | same, `engine='dot'` | 2D default | — | planned: p13-gv2 | Graphviz `dot` | Graphviz's own output, docker-only oracle |
| `SUGIYAMA` | `dispatcher.py:142` | `_sugiyama_layout` `hierarchical.py:638` | 2D (z=0) | `layout.dag.sugiyama` | on develop | hand, checked on dagre-d3-es crossing counts | `roundtrip` + `harness/oracle-layouts.mjs --dag`, `registry/grid.rs:90` |
| `CIRCULAR_HIERARCHY` | `dispatcher.py:144` | `_circular_hierarchy_layout` `hierarchical.py:693` | 2D (z=0.0) | `layout.circular.hierarchy` | on develop (p12-t2) | SciGraphs itself, `hierarchical.py:693-732` | SciGraphs-arm `oracle-circular-hierarchy`, `unproven.rs:68` |

- names = 32 (24 explicit `elif` comparisons plus 8 Graphviz engines)
- on develop = 12, in four flavours: 5 as SciGraphs names them (`GRID`, `CIRCLE_PACKING`, `SUGIYAMA`,
  `SPRING`, `CIRCULAR_HIERARCHY`), 6 as two-dimensional ports of a 3D name (`RANDOM`, `FORCEATLAS2`,
  `SPECTRAL_3D`, `SPIRAL_3D`, `BIPARTITE_3D`, `MDS_3D`), and 1 as a 2D-only cut of a name SciGraphs makes
  optional by dimension (`YIFAN_HU`, which is 2D/2Z/3 upstream)
- in flight: p12-igraph = 7 names over 6 ids (`DRL` and `DRL_2D` share `layout.force.drl`) ·
p13-gv1 = 3 names over 3 ids (`GRAPHVIZ_TWOPI` → `layout.twopi`, `GRAPHVIZ_OSAGE` →
  `layout.packing.osage`, `GRAPHVIZ_PATCHWORK` → `layout.treemap.patchwork`) ·
p13-gv2 = 1 name over 1 id (`GRAPHVIZ_SFDP` → `layout.force.sfdp`)
- planned: p12-t2 = 2 · planned: p13-gv1 = 1 (`GRAPHVIZ_CIRCO`) · planned: p13-gv2 = 3
- missing = 5, all of them 3D: `SPRING_3D`, `SPHERE`, `HELIX`, `CUBE`, `HIERARCHICAL_3D`
- motor ids with no SciGraphs name = 4 (`layout.tree.tidy`, `layout.treemap.squarified`, `layout.circular.ring`, `layout.force.barnes_hut`); out of scope for a table keyed on SciGraphs names

## Why every `missing` row is 3D

The motor is 2D only. `NodeGeometry::Point` carries `x, y` and nothing else
(`crates/graph-contract/src/geometry.rs:106-108`), and the snapshot header writes a hard `0` into the
reserved z channel (`crates/graph-contract/src/snapshot.rs:106-107`, written at `:136`). So a SciGraphs
name that is 3D cannot be answered today at all, and the only reason seven 3D names read `on develop`
is that the motor ported the *2D question* each one answers and said so in the module doc —
`crates/graph-core/src/layout/spectral.rs:29-31` is the pattern (`DIMS: usize = 2`, with the note that
every `dims=3` in the reference ports as 2).

Two rows are the exception that proves the rule. `SPRING_3D`'s 2D half *is* `SPRING`, which is p12-t2's
`layout.force.spring` — the two rows are one algorithm at two dimensions, and the port says so at
`crates/graph-core/src/layout/force/spring.rs:6-11`; `SPRING_3D` itself stays `missing` until
`contract-3d` exists, at which point the same differential runs at `dim=3`. `HIERARCHICAL_3D` cannot be
collapsed: `hierarchical.py:113-147` puts the BFS level on the z axis (`z = level / max_level * 2 *
scale - scale`) and only the in-plane disk radius varies with population, so dropping z stacks every
level on the same disk. The motor's `layout.circular.radial` is a *different* function
(`circular.rs:4` names SciGraphs' `hierarchical.py:693`, which is `CIRCULAR_HIERARCHY`, not
`HIERARCHICAL_3D`) and its own oracle declines the SciGraphs comparison outright
(`crates/graph-core/src/registry/hierarchy.rs:109-135`, the `CIRCULAR` row).

Ponytail: the statuses are one read of the tree on 2026-09-30, not a permanent fact — `in flight` is
decided by what `git log --oneline origin/develop..origin/p12-igraph` says today, and that branch
lands six ids in one commit (`8238039`) against a `LAYOUTS` array whose length is a compile-time
literal (`registry.rs:78`), so its rows are unreadable from this worktree until it merges. The
`on develop (2D port)` label is the one most likely to mislead: it means the id answers the 2D
question, not that the 3D layout exists. The `dim` column is SciGraphs' own, read from each
function's return shape, and is `2D (z=0)` where the function returns three columns with a constant
z — dropping that column is a convention, not a port.
