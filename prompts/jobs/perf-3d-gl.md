# Job perf-3d-gl (agent build: 3D drawn by the GPU, orbit at display rate)

Why (user, 2026-10-03): "we want to be really fast in motorizing the 3D elements". Today 3D is
painted on Canvas2D, one shape per node, sorted furthest first on every frame.

Facts (confirm before editing):
- `packages/graph-render/src/three/paint3d.ts`: about 236 lines, header lines 1-25.
  `canvas2d/paint.ts:44` reaches it when the frame has a z column.
  - Batching was dropped on purpose: depth is per node, and Canvas2D has no z-buffer.
- `three/sort.ts` does the per-frame depth sort, `three/projection.ts` the camera, and
  `three/orbit.ts` the gesture.
- The WebGL2 layer (`webgl2/{layer,draw,gl,hook,plan,shaders,still,sync,glide,colour}.ts`) is 2D
  only. `webgl2/hook.ts` is where a 2D frame hands its bulk to the GPU. It falls back to Canvas2D
  for good once the layer fails.
- `scripts/studio-3d.sh` is the 3D browser gate.
- `scripts/studio-backend.sh` is the 2D WebGL2-vs-Canvas2D pixel parity gate. Use it as the
  pattern for the new gate.

Do, in order:
1. Measure first:
   - Layouts: one sphere-family and one spring3d-family layout, picked from `Motor.layouts()`.
   - Sizes: 2 000, 20 000 and 200 000 nodes.
   - Orbit frame time, p50 and p95, over a scripted drag through `deploy/perf/cdp.py`.
   - Record it in `docs/measurements/perf-3d-gl.md`.
2. Add a WebGL2 3D path:
   - one instanced draw for nodes, one for edges, and the depth test on;
   - no CPU sort;
   - the camera is one matrix uniform, ported from `three/projection.ts`;
   - a drag updates the uniform only, so nothing is uploaded per frame while the drawing is still.

   Shapes keep their kinds (`Box` w and h, `Circle` r). Edge interiors may be straight lines in
   the GL path. Write a `Ponytail:` line saying so, if you do that.
3. Canvas2D 3D stays as the fallback and as the parity reference.
4. Add a browser gate, `scripts/studio-3d-gl.sh`:
   - the GL picture against the Canvas2D picture, within a pixel tolerance you measure and state;
   - the fallback on a lost context;
   - a negative control, `STUDIO_3D_GL_BREAK=1`, that must exit non-zero.
5. Measure again at the same sizes. Keep the GL path as the default only where it is more than 3%
   faster at p95. Otherwise keep it opt-in and say so.

Paths:
- `packages/graph-render/src/three/**`
- new files under `packages/graph-render/src/webgl2/` (the name tells they are 3D)
- one dispatch line in `canvas2d/paint.ts`, or in `webgl2/hook.ts`
- `deploy/three/`, `deploy/perf/`
- `scripts/studio-3d-gl.sh`
- `docs/measurements/perf-3d-gl.md`

Out of bounds:
- `crates/`
- `packages/graph-studio/src/motor/**` (perf-pm-live, perf-p6-live-copy)
- `webgl2/{draw,glide,sync,still}.ts`, apart from a single import
- `canvas2d/loop.ts`

No new dependency; three.js is not allowed. House limits: 40 lines per function, 300 lines per
file.

Done when:
- `scripts/studio.sh check` exits 0, and `perf-p5.rows` is green.
- `scripts/studio-3d.sh` exits 0.
- `scripts/studio-3d-gl.sh` exits 0, and its negative control exits non-zero.
- A pw MCP pass on the built studio is done: switch to a 3D layout, orbit, and save the screenshots
  under `docs/measurements/perf-3d-gl/`. Read the store error, the console exceptions and the
  overlay text. Any one of them non-empty is a FAIL.
- The measurement doc has the before/after table.
