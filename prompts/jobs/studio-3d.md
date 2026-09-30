# Job studio-3d (agent build, show 3D snapshots in the studio)

Why: the user wants the SciGraphs shapes in the studio exactly as SciGraphs shows them: SPHERE, HELIX, CUBE,
HIERARCHICAL_3D and the 3D force layouts (2026-09-30). Today, per `docs/decisions/contract-3d-verdict.md`, the
renderer and the studio refuse a 3D snapshot by name (`"dimension-3d"`). This job replaces that refusal with a view.

Precondition: `p13-3d` is on develop and at least one 3D layout id is registered (`p12-t3`). Otherwise stop.

Do:
1. `docs/decisions/studio-3d.md`: lift the refusal, and record why and which gate replaces it. Keep the renderer's
   only input as snapshot bytes (`docs/decisions/render-ports-not-imports.md`).
2. graph-render: decode the z column (`decode.ts`) and add a camera for dim=1: an orbit camera (drag to rotate,
   wheel to zoom, right-drag to pan), a perspective projection, depth-sorted painting in Canvas2D, and node
   radius scaled by depth. 2D snapshots take the existing path, byte for byte (the render tests stay unedited).
3. graph-studio: when the current layout is 3D, show a "3D" badge and a "reset camera" action, both registered in
   `src/actions/registry.ts` and reachable from the console. The layout picker already lists every registry id.
4. Live forces in 3D are out of scope; say so in the decision.
5. Tests: unit tests for the projection and the depth sort, and a render test on a 3D fixture. Add a browser
   gate, `scripts/studio-3d.sh` (modelled on `studio-nav.sh`): load `layout.sphere` and assert that a CDP drag
   rotates the view, i.e. the projected node positions change. Its negctl is `STUDIO_3D_BREAK=1`, which freezes
   the camera, and it must exit non-zero.

Checks: `scripts/studio.sh wasm`, `check`, `build`, `scripts/studio-3d.sh`, its negctl, and `scripts/studio-nav.sh`.
Paths: `packages/**`, `app/**`, `scripts/studio-3d.sh`, `docs/decisions/studio-3d.md`. Not `crates/`.
