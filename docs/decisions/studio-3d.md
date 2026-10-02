# studio-3d — the studio draws the 3D layouts

Status: **accepted** (user, 2026-09-30 and 2026-10-01)
Supersedes: condition 5 of `docs/decisions/contract-3d-verdict.md`, which required the
renderer and the studio to refuse a 3D snapshot by name. The refusal was the right call on
2026-09-30, when the painter could not draw a z column: a 3D snapshot read as 2D puts the
columns at the wrong offsets, and a flat lie is worse than a refusal. That reason has
expired. What replaces it is named below.
Rules on: `docs/decisions/contract-3d.md` (the byte layout, unchanged here),
`docs/decisions/contract-3d-verdict.md` (the version rule, unchanged here),
`docs/decisions/render-ports-not-imports.md` (the renderer's only input is snapshot bytes).

## Context

The user asked for the SciGraphs shapes in the studio as SciGraphs shows them (2026-09-30):
`SPHERE`, `HELIX`, `CUBE`, `HIERARCHICAL_3D` and the 3D force layouts. `p12-t3` registered
the five layouts, and `p13-3d` made the contract carry the third column: `dim` in header byte
14, a `z` column spliced in after `y`, a 0.3 label for every 2D snapshot and a 0.4 one only
where `dim = 1`. Every producer writes that way, so no 2D byte moved.

What was left was the consumer side. `packages/graph-render/src/snapshot/decode.ts` refused
`dim = 1` at byte 14 as `dimension-3d`, before either geometry tag, and the studio's
`errors.ts` told the user the studio drew 2D only. The layout picker already listed all five
3D layout ids, so a user could pick one and be refused by name.

The only input to this work is the decision: the gate below is what replaces the refusal, and
the refusal is not deleted until the gate is green.

## Decision

**1. The renderer reads the z column and projects it. `dim = 1` is drawn, not refused.**

`Snapshot` gains `dim: number` and `z: Float32Array | null`; the header check becomes
`takeDim`, which returns 0 or 1 and refuses anything higher. A `dim = 0` snapshot reads
exactly as before — no column moves, no offset shifts — so **every 2D render is byte-for-byte
what it was**, and the 2D paint path is not edited at all.

The 2D camera (`camera.ts`) is untouched and still owns every 2D frame. A 3D frame gets an
**orbit camera** (`three/orbit.ts`): a target, a yaw, a pitch and a distance, projected in
perspective. The basis is spelled out in that file rather than taken from a matrix library,
because one property has to be visible in the source and tested: **at yaw 0 and pitch 0 a
node with `z = 0` projects exactly as the 2D camera draws it.** A 3D layout that is flat in z
therefore opens on the 2D view, and the first drag is what makes it 3D
(`tests/three-orbit.test.ts`, "a yaw of zero and a flat z column project as the 2D camera
does"). The screen axes are the 2D painter's own, y downwards, with a third axis added.

**2. Depth is carried by order, and the order is a counting sort, not a comparison sort.**

Canvas2D has no z-buffer, so the painter's algorithm is the whole mechanism: the furthest node
is drawn first and the nearest over it (`three/sort.ts`). The order comes from a counting sort
on a quantised depth rather than `Array.prototype.sort` on `Float64Array`. That is a
determinism choice, not a speed one (D2/D3: fixed-order reductions, ties broken by a dense
index). A comparison sort over floats has to be argued into being deterministic; integer
buckets are exact, and a node at one depth always paints in dense index order however the
depths were arranged. The unit test for it is a repeat-run row: the same depths give the same
order four times over.

**3. Batching gives way on a 3D frame, deliberately.** The 2D painter fills one path per
palette entry (`canvas2d/nodes.ts:59`), which is worth hundreds of draws on a big graph. A
batch has one draw order for the whole bucket and depth is per node, so the two cannot both
hold. A 3D frame fills one disc per node. The escape hatch is the drawing's own size: these
are closed forms over a handful of shapes, not a 20 000-node force run.

**4. Gestures, and what each one is for.** Drag turns, wheel dollies, right-drag slides the
target. A 3D drawing has no axis to pan along — moving it in the view plane is what the
target already does — so a background drag is the camera, and the right button is the one that
pans. The orbit has no drag slop, because the gesture is the camera and a slop would make the
first few pixels of every drag a dead zone the 2D pan does not have. **On a 2D frame every
one of these is unchanged**: `dragKindOf` reads `frame.z` and a 2D frame takes the 2D branch,
including the right button staying the host's menu.

**5. A press on a 3D frame never grabs a node.** A left press that does not move is a click on
a node and selects it; a left press that moves is the camera. Live forces in 3D are out of
scope and are not half-built here: `docs/decisions/live-force-session.md` covers the live
session, and it is a 2D document. A 3D layout is a finished placement, and dragging a node
out of a closed form has nothing to write back to.

**6. A node behind the eye is dropped, not clamped.** Past the eye plane a perspective divide
throws the node across the viewport; this one leaves the drawing instead, and the drag back
brings it. That is the honest limit of a painter's algorithm on a 2D context, and it is
recorded as such rather than papered over with a clamp.

**7. The studio says what is on screen, and the console can put the camera back.** A `3D`
badge in the HUD, and a `headon` action. Both read the **run's `dim`**, which comes off the
decoded snapshot — the z column's own presence, the same thing the painter branched on. A
layout id could promise 3D and deliver a flat graph; `dim` cannot. `headon` is refused by name
on a 2D drawing, so a user who types it there is told why.

**8. The gate that replaces the refusal is `scripts/studio-3d.sh`.** The refusal existed
because nothing could see a 3D drawing. The gate loads `layout.basic3d.sphere` by typing its
id into the console, and then drives real CDP mouse and wheel input against the served
build, reading the view's own projected positions back. Rows: the layout draws a 3D frame
whose nodes are not coplanar; a 200 px drag turns the camera and moves the nodes; the same
drag back turns it the other way; a vertical drag tips it; a wheel notch pulls the camera in;
a right-drag slides the drawing without turning anything; `headon` puts a turned camera back;
and the HUD carries the badge.

Its negative control is `STUDIO_3D_BREAK=1`, which **pins the camera** from the probe: a
`requestAnimationFrame` loop puts the held orbit back every frame, so every drag is accepted
by the view and thrown away. That is the fault the whole gate is written against, and a gate
that cannot see a frozen camera would pass it. The two rows that do not touch the camera —
"the drawing is 3D" and "the badge is there" — still pass under it, which is what shows the
freeze is the fault and not the gate's own setup.

**Live forces in 3D are out of scope.** A 3D layout is a finished placement: there is no
motor to steer, and a drag that pulled one of its nodes would have nothing to write back to.
`docs/decisions/live-force-session.md` is a 2D document and this does not extend it. The
consequence in the chrome is that a 3D frame does not animate: `showFrame` gives it a fresh
orbit and the z column has no blend, so a layout change is a cut rather than a move.

## What replaces the refusal, precisely

| Was | Now |
|---|---|
| `decode.ts` refused `dim = 1` as `dimension-3d` | `takeDim` returns 1; the z column is read as `node.z` |
| `dim` of 2 or more refused as `dimension-3d` | refused as **`reserved-dim`** — a dim no reader here implements, not a 3D drawing |
| studio's `errors.ts`: "This studio draws 2D only" | names the reserved dim, and says nothing else was drawn |
| `export.snapshot` of a 3D run refused | writes the 3D bytes, `dim` byte and all |
| no camera for a z column | `three/orbit.ts`, with the 2D-identity negative control |
| nothing could see a 3D drawing | `scripts/studio-3d.sh`, with a frozen-camera negctl |

The refusal code changed name rather than disappearing, because there is still a dim this
reader cannot express. A `dim` past 1 is not a 3D snapshot: reading it as one would put every
column at an offset nothing in the contract says it is at. The negative control for that is in
`tests/dimension-3d.test.ts` — the same flow over a `dim` of 1 must draw, so a reader that
refused every non-zero dim would fail there rather than pass quietly.

## Consequences

- **2D output is unchanged, and that is the claim to hold.** No 2D snapshot byte moved (`p13-3d`
  owns that), and no 2D paint pass was edited — `nodes.ts`, `edges.ts`, `glow.ts` and
  `camera.ts` are untouched, and `paintFrame`'s 2D body is byte-identical. The negative
  control is `tests/three-paint.test.ts`'s 2D row, which asserts a 2D frame is still drawn by
  the batched 2D path — one fill per palette entry, which the 3D painter cannot produce.
- **The decode→paint seam is tested from real bytes, not from a hand-built frame.** The other
  3D paint tests build a `Frame` directly, which is right for asking the painter things and
  wrong for asking about the seam: a reader that took the z column for the radius column would
  hand that painter a frame whose radii are depths, and every hand-built test would still pass.
  So one test goes the whole way — `spaceBytes` in `tests/support.ts` writes the columns in
  wire order, and the test decodes, frames, projects and paints them. Its radius assertions are
  the column-order assertions. This was checked by mutation: reading the radius column where
  the z is turns that one test red and leaves the other nine green, which is the gap closing.
- The renderer's only input is still snapshot bytes (`render-ports-not-imports.md`). Nothing
  here imports the oracle, and the 3D camera is ported maths owned by the renderer.
- `RunSummary` gains `dim`. It is a `0`/`1` read off the snapshot, not a promise from the id.
- The console word is `headon`, not one with a 3 in it: every console word in the studio is one
  lowercase word (`tests/parity.test.ts:40`), so the word names what it does. The action is
  declared in `actions/view.ts` and reaches the registry through `studioActions()`, which is
  what every other action does — there is no second list to add it to.
- `View` gains `orbit()`, `setOrbit()`, `projected()` and `resetOrbit()`. `projected()` is
  what the gate reads: a host asking where every node is drawn, without asking the painter.
- Edge paths stay 2D in the contract, so a 3D edge is the straight line between its ends'
  projected points. Routing a 3D edge is a second breaking change and is not this one's.
- Labels, glow, spheres, arrows and the edge gradient are not drawn on a 3D frame. The 3D
  painter is its own module and says why for each; adding them is ordinary work, and none of
  it is on the critical path for seeing a shape.
- The 3D layouts are all in the catalog already, so the picker needed no change. That is why
  this job was possible at all: a user could already pick `layout.basic3d.sphere` and be
  refused. The gate loads it by typing that id into the console, which is the same path a user
  takes and the reason the row can be trusted to be about the app rather than about a fixture.

## What this job did not do

- **Live forces in 3D.** Out of scope, and stated as such in §5 above rather than half-built.
- **A z-buffer.** Canvas2D has none, so depth is the painter's order and a node behind the
  eye leaves the drawing (§6). A WebGL context would remove that limit and is the honest
  reason to reach for one later; it is also a second backend and a second set of determinism
  questions, which is why it is not this change.
- **3D edge routing.** The contract's edge paths are 2D, so a 3D edge is the straight line
  between its ends' projected points. Changing that is a second breaking contract change.
- **The 1.0 format declaration** and the reserved-dim error's own name in the Rust reader.
  Both belong to other decisions; this one changes only the TypeScript side.

## Alternatives considered

- **Keep the refusal, add a separate 3D viewer.** Rejected: two viewers means two cameras, two
  painters and two sets of gestures for one contract, and the studio's job is to show what the
  motor laid out. The 2D path stays exactly as it is, which is the property that made this
  safe, so there is nothing to gain from a second surface.
- **Orthographic projection, no divide.** Rejected: a 3D drawing in parallel projection reads
  as a 2D drawing you can rotate, which is the failure mode the refusal was guarding against.
  The perspective divide is what makes depth legible, and the eye-plane drop is its price.
- **A WebGL context for the 3D frame.** Rejected here: a second backend is a second set of
  determinism questions (D1-D10 are written for the motor) and a second set of parity rows,
  for a drawing set that is a handful of closed-form shapes. A z-buffer would remove the
  painter's-algorithm caveat in §6, and that is the honest reason to reach for it later.
- **Sort with `Array.prototype.sort` and argue for stability.** Rejected: V8's sort is stable
  today and the tie-break would still have to be written out to survive a change. Integer
  buckets make it a property of the code instead of a property of the engine.
