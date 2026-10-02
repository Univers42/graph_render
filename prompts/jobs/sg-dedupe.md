# Job sg-dedupe (agent build, refactor: one copy of each primitive)

Why: a 2026-10-01 review found two copies of the same primitive in graph-core:
1. `crates/graph-core/src/layout/circular/hierarchy.rs:66-185` (`component_roots`, `sweep`,
   `multi_source_levels`, `drain`, `reseed`) duplicates `crates/graph-core/src/layout/hierarchical_3d/levels.rs:19-146`
   (same names; `levels` there is `depths` here).
2. `crates/graph-core/src/layout/circle_packing/fallback/seed.rs:36-160` re-implements a networkx
   Fruchterman-Reingold seed (`adjacency_matrix`, `initial_temperature`, `fruchterman_reingold`,
   `rescale_to`) next to `crates/graph-core/src/layout/force/spring/`, which ports the same networkx code.

Do:
1. Diff each pair line by line first and write the differences down. Two functions that differ in any
   arithmetic step (order of a sum, a clamp, a constant) are NOT duplicates: leave that pair alone and say why.
2. Item 1: keep one copy in a module named for the concern (BFS levels over a `SimpleGraph`), call it from
   both layouts. Item 2: only if the arithmetic is identical, call `force/spring` from the seed.
3. Behaviour must not change: this is a refactor. Before and after, record `hashgate --seeds 8` per-stage
   hashes for `layout.circular.hierarchy`, `layout.hierarchical3d` and `layout.packing.circle`; they must be
   identical. Paste both.

Paths: `crates/graph-core/src/layout/{circular,hierarchical_3d,circle_packing,force/spring}/**` and the
`mod.rs` that declares a new module. Nothing else. Do not touch the registry.

Done when: the merge floor is green; the per-stage hashes before and after are identical for the three
layouts (pasted); total line count of the touched files went down (paste `wc -l` before and after).
