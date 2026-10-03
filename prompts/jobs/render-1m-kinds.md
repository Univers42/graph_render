# Job render-1m-kinds (agent build: every node and edge kind on the GPU layer, at 1M)

Owner: `packages/graph-render` belongs to the peer session graph-render-08. This brief runs only with
that peer's agreement, or by the peer.

Goal: at 1M nodes and 1.5M edges the WebGL2 layer draws every kind the contract has, with width,
gradient and arrows, at ≥ 30 fps (`docs/decisions/obsidian-force.md` decision 4). Today it draws
curved and routed edges straight, 1 px wide, with no arrows.

Facts (verified on develop b2cbbcd9):
- `packages/graph-render/src/webgl2/layer.ts:8-13`, the Caveat: lines are one device pixel wide,
  routed and curved edges are drawn straight between their ends, and arrows are not drawn.
- `webgl2/draw.ts:80` draws edges as `gl.LINES` through `drawElements`. `:76` sets `u_gradient`
  when `style.edgeColour === "gradient"`, so gradient edges already work.
- `webgl2/draw.ts:94,102` draws nodes as instanced quads, with `u_box` for `Box`. Circles use the
  same pass. `:128-129` draws points as `gl.POINTS`.
- `webgl2/plan.ts:18` `BULK_THRESHOLD = 8192`: from that many elements, `auto` hands the frame to
  this layer.
- The contract's edge geometry (`docs/contract/binary-layout.md:23,115-120`):
  - `Line` (0): the endpoints only;
  - `Polyline` (1): `offsets: u32 × (m + 1)`, then `pts: f32 × 2·offsets[m]`;
  - `Curve` (2): one `degree` for the whole snapshot, then offsets and points in the same shape.

Do, in order:
1. Measure first: at 1M/1.5M, with Line edges, Point nodes and gradient on, report the fps of the
   current layer on the gm-chromium hardware arm. That is the floor every step below is compared
   to.
2. Wide lines: draw each segment as an instanced quad (two triangles) expanded in the vertex
   shader to a width in CSS pixels, scaled by the device pixel ratio. Keep the 1 px `gl.LINES`
   path when the width is ≤ 1 device pixel: it costs less. Gradient stays per vertex.
3. Polyline: one instance per segment. Build an index from the offsets, so no CPU loop runs per
   frame; it is rebuilt only when the geometry column's identity changes, the way `sync.ts`
   already does for positions.
4. Curve: one instance per edge. Tessellate in the vertex shader into a fixed segment count per
   degree, from the control points in a texture or a storage-shaped buffer. Write a `Caveat:` line
   on the fixed count: a very long curve on screen shows facets.
5. Arrows: one instanced triangle per directed edge at the target end. Orient it along the last
   segment's tangent and set it back by the target node's radius. Skip the pass when no edge is
   directed.
6. Measure again: every combination of node kind {Point, Circle, Box} × edge kind {Line, Polyline,
   Curve} × {flat, gradient} at 1M/1.5M, with arrows on and off. Report the fps per row: 3 rounds,
   medians, with the load. Check the pixels against Canvas2D at 5 000 elements (the existing
   `scripts/studio-backend.sh` parity method) for each new pass.
7. Update the Caveat in `layer.ts` to say what is still not drawn.

Out of bounds: the contract, the motor, `packages/graph-studio` beyond passing a width it already
has. No new dependency. Canvas2D stays the path below `BULK_THRESHOLD`. Studio house rules: no type
assertions, 40 lines a function, 300 a file.

Paths you may edit: `packages/graph-render/src/webgl2/`, `packages/graph-render/tests/`,
`docs/measurements/render-1m-kinds.md`.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/studio-backend.sh` exits 0, and `STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh`
  exits non-zero.
- `scripts/studio-smoke.sh` exits 0, and `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits
  non-zero.
- `docs/measurements/render-1m-kinds.md` holds:
  - the step-1 and step-6 tables, with load;
  - the parity result for every new pass;
  - every row below 30 fps, named rather than averaged away.
