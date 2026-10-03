# Job ux-params-dock (agent build: a Layout settings panel filled from the motor's own schema)

Your worktree is cut from `fix-sdk`, which carries `ux-params-abi` and is not on develop yet; do not merge develop. Read `docs/decisions/layout-params.md` first. The SDK there
gives `motor.layoutParams(id)` and `motor.run(id, { params })`.

Goal: every layout's parameters can be changed from the dock and from the console, with no
layout id or parameter name written in studio source. The default layout id in
`state/settings.ts` stays the only one.

Do, in order:
1. The worker protocol (`src/motor/protocol.ts`) carries the schema request and the run parameters.
   The worker is the only place that calls the SDK.
2. Add a "Layout settings" section to the dock. It shows one control per parameter of the current
   layout: a slider for a bounded float, a number field for an int, a toggle for a bool. Build the
   controls from `ui/controlOf.ts` and the existing controls.
3. A change re-runs the layout. Debounce it to one run per frame, and cancel any run in flight
   through the `view.cancel` path. The moving picture uses the existing transition.
4. Persist the values per layout id (`state/persist.ts`, `state/portable.ts`). A reset action brings
   back the motor's defaults.
5. Add console commands that go through `resolve` (`src/actions/registry.ts`):
   - `layoutset <param> <value>`
   - `layoutreset`

   Arguments are checked against the schema there, once.
6. Measure, on 2 000 and 20 000 nodes, from a slider move to the first new frame. Record it.

Paths: `packages/graph-studio/src/**` and its tests, `deploy/nav/` (a new gate is allowed), and
`docs/measurements/ux-params-dock.md`.

Out of bounds:
- `crates/`
- `packages/graph-render/`
- `motor/liveLoop.ts`

No new dependency. House limits.

Done when:
- `scripts/studio.sh check` exits 0, and `perf-p5.rows` is green.
- A gate row drives a parameter change in headless Chromium and checks that the drawing changed.
  Its negative control (`*_BREAK=1`) exits non-zero.
- A pw MCP pass on the built studio is done: change one parameter of a layered layout and one of a
  force layout, and save the screenshots under `docs/measurements/ux-params-dock/`. Read the store
  error, the console exceptions and the overlay text. Any one of them non-empty is a FAIL.
