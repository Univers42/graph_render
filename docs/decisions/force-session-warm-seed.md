# The live force session starts on the layout's picture

**Status:** decided. **Date:** 2026-10-03.
**Code:** `crates/graph-wasm/src/session.rs` (`create_warm`), the export
`gm_force_session_create_warm`, the SDK's `forceSession(handle, params, engine, "layout")`, and the
studio's `startSession` and `renew` (`packages/graph-studio/src/motor/session.ts`).

## The defect

The studio settles every force layout live. The session it stepped was made once per graph and
seeded on the motor's golden spiral, so the first frame after any force run replaced that run's
picture with the session's own. Measured in the studio on 2026-10-03: ForceAtlas2 and DrL drew
the same bounds, `[-689.4, -677.9, 634.7, 606.9]`. Choosing a force layout changed nothing on
screen. A second cause was `bridge.start()`, which sent `force.start`. That request shuffles,
so every run restarted cold from the spiral.

## The decision

- **The motor.** `create_warm` seeds a session on the node centres of the graph's last layout
  run. The `f32` centres widen to `f64` exactly, so the session starts on the drawn coordinates
  bit for bit (`session/tests/warm.rs`). It refuses `NoGeometryYet` before any run and
  `TamperedGeometry` for a centre that is not finite.
- **The studio.** Every layout run drops the cached session (`renew`). The next session is made
  warm and reheated to `0`. The loop's first request is `force.settle`, which neither shuffles
  nor reheats: it reads the born alpha with `step(0)` and, under `ALPHA_MIN`, runs no tick at
  all. A tick is not free at alpha 0, because collide and center act at any alpha: one tick
  moved a 60-node DrL picture by 170 units (2026-10-03). The picture stays until a drag or a
  knob reheats it to 0.3. The knobs the last session held carry over to the new one.
  A re-layout tells the page that forces are still available (`disabled: null`), so the panel
  does not grey out.
- **What stays as it was.**
  - A graph past `LIVE_NODES` runs the O(n) scatter and starts its session cold on the spiral,
    hot, as before. A scatter is not a picture worth keeping.
  - "Animate" (`force.start`) still shuffles to a cold spiral.

## What it does not do

- It does not keep pins across a run. A pin holds a node of the last picture, and the new picture
  has moved it.
- It does not rescale the picture to the session's units. A drag reheats every force at the
  session's link distance and collide radius, so a picture drawn at another scale (DrL draws
  about ±3 for 60 nodes) grows while it settles; the camera follows it while it is fitted.
- It does not carry velocities. The new session starts at rest, so a drag on a fresh picture
  begins from zero momentum.
- The browser row `live-settle` (`deploy/nav/liverows.py`) now presses Animate before it samples.
  A small graph's drawing no longer moves on load, and that stillness is the fix, not a fault.
