# Job yifan-hu-octree (agent build; an octree for Barnes-Hut, then `layout.force.yifan_hu.3d`)

Why: p12-t4b shipped the 3D arms of five force kernels but not `layout.force.yifan_hu.3d`: its
Barnes-Hut sits on `crates/graph-core/src/layout/force/quadtree.rs`, a 4-way tree with a 2-bit slot
index, and making it 8-way touches the 65 force-session digests that the 2D gate depends on.
Orchestrator decision 2026-10-02: its own job, octree first; the 3D arm is a thin layer on top.

Do:
1. First gate row, before any edit: record the 65 session digests and every 2D yifan_hu hash on this
   tree (name the command). They must be byte-identical at the end. The simplest way to keep them is a
   new octree beside the quadtree, not a generalised tree; if you share code, show the digests unchanged.
2. The octree: same build order, same theta test, same fixed-order reduction as the quadtree (D1-D10 in
   `prompt.md` §6: `libm`, no FMA, no `HashMap`, no clock). Unit tests next to the quadtree's, including
   a brute-force-vs-tree force comparison at theta 0.
3. `layout.force.yifan_hu.3d` in `crates/graph-core/src/registry/arms_3d.rs`, in the shape of the other
   3D arms (read `force/fruchterman_reingold` 3D for the pattern), with every `Metadata` field and a
   `Ponytail:` line. Its capabilities row follows the other 3D arms.
4. Quality: the 3D layout's stress on the shared fixtures, next to the 2D arm's, in
   `docs/measurements/yifan-hu-3d.md`, with the command lines.

Out of bounds: the 2D quadtree's behaviour, `graph-contract` (a geometry change is a stop), the other
layouts. Adding a dependency is a stop.

Done when: quick.rows green (wasm32-core, hashgate-8 and negctl included), the step-1 digests unchanged
(paste before and after), the octree tests green, and the measurement file committed.
