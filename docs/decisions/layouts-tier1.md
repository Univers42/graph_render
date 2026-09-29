# Tier-1 layouts: closed-form networkx set

Status: accepted for the p12-t1 slice. Scope: the studio picker shows the layouts SciGraphs
offers in 2D; this records which the motor gained, which it did not, and why.

## Added

| id | reference | note |
|---|---|---|
| `layout.random` | networkx `random_layout` | Shape only: the motor's `Mulberry32` at a fixed seed, x then y per node. Not numpy's generator. |
| `layout.circular.ring` | networkx `circular_layout` | Index order on the unit circle. Distinct from `layout.circular.radial`, whose hashes are untouched. |
| `layout.spiral` | networkx `spiral_layout`, 2D | networkx defaults (resolution 0.35, `equidistant=False`); SciGraphs has no 2D spiral, so there is no setting to follow. `run_with` exposes the equidistant branch. |
| `layout.bipartite` | networkx `bipartite_layout` | Node sets from SciGraphs (`hierarchical.py` `_bipartite_parts`: per-component two-colouring, skew balancing, greedy max cut for non-bipartite input). A non-bipartite graph is drawn, never refused. |

## Not added

- `layout.circular.shell`: SciGraphs has no shell rule (`git grep -i shell -- SciGraphs`
  finds none in the layout code). Choosing shells would be an improvisation; a missing
  reference is a stop. Blocked until the shell partition is specified.
- `multipartite`, `arf`, `bfs`, `planar`: not started in this slice; each needs its own
  differential and, for `planar`, a planarity test (see `planarity-fallback.md`).
- `layout.force.yifan_hu`: added (`layout/force/yifan_hu.rs`). Greedy heavy-edge matching
  coarsens down to 16 nodes or a 10 percent stall; the coarsest level is solved by the
  Barnes-Hut simulation (112 ticks), each finer level restarts from its parent and is
  refined by the same simulation (48 ticks from alpha 0.3). Not Graphviz sfdp, and no
  differential exists; its ledger row is `implemented` and its Metadata and Ponytail say so.

## Differentials

Closed-form layouts compare to networkx (`harness/oracle-closed-form.py`, `oracle-closed-form`):
ring 1e-6 (networkx narrows the angle to `f32`), spiral and bipartite 1e-7; worst cases over 1000
seeds are in `docs/measurements/closed-form-oracle.md`. `random` has no numeric oracle by
construction.

Ponytail: the scale ceilings are measured (`docs/measurements/tier1-scale.md`): the closed forms
to 1 000 000 nodes, the largest size `bench` accepts, so a lower bound; Yifan Hu at 100 000, one
timing.
