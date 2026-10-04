// The live loop against a fake port: the stop condition, the frame period, the disabled state.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_KNOBS, type ForceKnobs, type LiveForce, NO_ADAPTER_REASON } from "../src/motor/live.ts";
import { ALPHA_MIN, TICKS_PER_FRAME } from "../src/motor/liveLoop.ts";
import type { Result } from "../src/motor/protocol.ts";
import type { Session } from "../src/motor/session.ts";
import { serve } from "../src/motor/serve.ts";
import { fake, lastFrame, mortal, rig } from "./force-rig.ts";

const refuse = (): never => { throw new Error("a force request must not reach the session"); };
const NO_SESSION: Session = { open: refuse, load: refuse, layout: refuse, analysis: refuse, forces: () => null };
const KNOBS: ForceKnobs = { ...DEFAULT_KNOBS, gravity: 0.5, charge: -100, linkStrengthScale: 1, linkDistance: 40, theta: 1.2 };
/** What the loop pushes when the session under it is released: no loop, and no session. */
const STOPPED: Result = { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false };

test("the loop steps until alpha is under alpha_min, then stops on its own", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.start" });
  for (let i = 0; i < 40; i += 1) out.tick();
  assert.equal(lastFrame(out.emitted).running, false);
  assert.ok(port.alpha < ALPHA_MIN);
  const before = out.frames();
  out.tick();
  assert.equal(out.frames(), before, "no frame after the loop stopped");
});

test("a held pin keeps the loop running past alpha_min; release lets it settle", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.drag", id: "a", x: 5, y: 6 });
  for (let i = 0; i < 30; i += 1) out.tick();
  assert.equal(lastFrame(out.emitted).running, true);
  assert.ok(port.calls.includes("pin a 5 6"));
  host.handle({ type: "force.release", id: "a" });
  assert.ok(!port.calls.includes("unpin a"), "a drop is not an unpin: the node keeps the position it was put at");
  for (let i = 0; i < 30; i += 1) out.tick();
  assert.equal(lastFrame(out.emitted).running, false, "and nothing holds the loop awake any more");
});

test("a flick — every move and the release in one batch — lands the node where it was dropped", () => {
  // What a hand does when it is quicker than a frame: twenty moves and the release arrive
  // together, so the frame the loop runs sees a release and nothing to place.
  const port = fake(0.9);
  const { host, out } = rig(port);
  host.handle({ type: "force.drag", id: "a", x: 0, y: 0 });
  host.handle({ type: "force.drag", id: "a", x: 30, y: 40 });
  host.handle({ type: "force.drag", id: "a", x: 60, y: 80 });
  host.handle({ type: "force.release", id: "a" });
  out.tick();
  assert.ok(port.calls.includes("pin a 60 80"), "the drop reached the motor, not only the moves before it");
  assert.ok(!port.calls.includes("unpin a"), "and the node is not let go of where it was put");
  port.calls.length = 0;
  for (let i = 0; i < 20; i += 1) out.tick();
  assert.deepEqual(port.calls.filter((c) => c.startsWith("pin")), Array.from({ length: 20 }, () => "pin a 60 80"),
    "so it cannot drift back to the equilibrium the drag broke");
});

test("drag events before a frame collapse to the last one", () => {
  const port = fake(0.9);
  const { host, out } = rig(port);
  host.handle({ type: "force.drag", id: "a", x: 1, y: 1 });
  host.handle({ type: "force.drag", id: "a", x: 2, y: 2 });
  host.handle({ type: "force.drag", id: "a", x: 3, y: 3 });
  out.tick();
  assert.deepEqual(port.calls.filter((c) => c.startsWith("pin")), ["pin a 3 3"]);
  assert.equal(out.scheduled(), 2, "three requests, one scheduled frame, then the next");
});

test("a slow motor ticks every frame, back to back, and never queues", () => {
  const clock = { now: 0, perStep: 20 };
  const port = fake(0.99, clock);
  const { host, out } = rig(port, clock, 16);
  host.handle({ type: "force.start" });
  for (let i = 0; i < 2; i += 1) out.tick();
  assert.equal(port.calls.filter((c) => c.startsWith("step")).length, 2, "no tick is dropped after a long one");
  assert.equal(out.frames(), 2, "one frame a tick, never a frame without one");
  assert.deepEqual(out.delays, [16, 0, 0], "a tick past the period is followed at once, and one frame is ever pending");
});

test("a fast motor steps once a period, so the settle lasts as long as the animation", () => {
  const clock = { now: 0, perStep: 1 };
  const port = fake(0.999, clock);
  const { host, out } = rig(port, clock, 16);
  host.handle({ type: "force.start" });
  for (let i = 0; i < 5; i += 1) out.tick();
  assert.equal(port.calls.filter((c) => c.startsWith("step")).length, 5, "one tick a frame, not a budgetful");
  assert.equal(port.calls.filter((c) => c === `step ${TICKS_PER_FRAME}`).length, 5);
  assert.deepEqual(out.delays, [16, 15, 15, 15, 15, 15], "the wait is the period less the tick");
});

test("params reach the port and reheat; stop unpins and halts", () => {
  const port = fake(0.9);
  const { host, out } = rig(port);
  host.handle({ type: "force.drag", id: "a", x: 0, y: 0 });
  out.tick();
  host.handle({ type: "force.params", knobs: KNOBS });
  assert.ok(port.calls.includes("params"));
  const state = host.handle({ type: "force.stop" });
  assert.deepEqual(state, { type: "force-state", running: false, disabled: null, paused: false });
  assert.ok(port.calls.includes("unpin a"));
  const before = out.frames();
  out.tick();
  assert.equal(out.frames(), before);
});

test("frames carry copies: the port's buffers are never handed over", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.start" });
  out.tick();
  assert.deepEqual(Array.from(lastFrame(out.emitted).xs), [1, 2]);
});

/**
 * The columns cross the wire as f32, narrowed in the worker: the page narrowed anyway, so f64 spent
 * twice the bytes. Rounding to nearest is what the page's `Float32Array.set` did; the drawing is unchanged.
 */
test("the frame's columns are f32, narrowed in the worker to f32 precision and no further", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.start" });
  out.tick();
  const frame = lastFrame(out.emitted);
  assert.ok(frame.xs instanceof Float32Array && frame.ys instanceof Float32Array, "f32 on the wire");
  assert.deepEqual(Array.from(frame.xs), Array.from(new Float32Array(port.positions().xs)));
  assert.deepEqual(Array.from(frame.ys), Array.from(new Float32Array(port.positions().ys)));
});

test("a pause stops the frames and keeps the pins; resume carries on from that alpha", () => {
  const port = fake(0.99);
  const { host, out } = rig(port);
  host.handle({ type: "force.start" });
  out.tick();
  const at = lastFrame(out.emitted).alpha;
  host.handle({ type: "force.pause" });
  assert.deepEqual(host.handle({ type: "force.pause" }), { type: "force-state", running: false, disabled: null, paused: true });
  const before = out.frames();
  out.tick();
  assert.equal(out.frames(), before, "a paused loop schedules nothing");
  host.handle({ type: "force.resume" });
  out.tick();
  assert.ok(out.frames() > before, "the resume ran a frame");
  assert.ok(lastFrame(out.emitted).alpha < at + 0.2, "a resume continues, it does not reheat to the top");
});

test("a drag wakes the loop from a pause, and pins where the pointer left the node", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.drag", id: "a", x: 0, y: 0 });
  out.tick();
  host.handle({ type: "force.pause" });
  assert.deepEqual(host.handle({ type: "force.pause" }), { type: "force-state", running: false, disabled: null, paused: true });
  host.handle({ type: "force.drag", id: "a", x: 40, y: 50 });
  out.tick();
  assert.ok(port.calls.includes("pin a 40 50"), "the node is where the pointer left it");
  assert.deepEqual(host.handle({ type: "force.pause" }), { type: "force-state", running: false, disabled: null, paused: true });
});

test("a pause that never ran is a no-op, and the loop says so", () => {
  const { host } = rig(fake(0.5));
  assert.deepEqual(host.handle({ type: "force.resume" }), { type: "force-state", running: false, disabled: null, paused: false });
});

test("force.start throws the nodes back to random positions before it settles again", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle({ type: "force.start" });
  out.tick();
  port.calls.length = 0;
  host.handle({ type: "force.start" });
  assert.equal(port.calls[0], "shuffle", "the restart is a shuffle, not a resume");
  assert.ok(port.alpha > 0.9, "and it reheats to the top, so the bar fills again");
});

test("force.settle runs no tick on a session born cold, and settles a scatter born hot", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  const answer = host.handle({ type: "force.settle" });
  out.tick();
  assert.deepEqual(port.calls, ["step 0"], "collide and center act at any alpha: a warm picture is not ticked");
  assert.equal(out.scheduled(), 0);
  assert.deepEqual(answer, { type: "force-state", running: false, disabled: null, paused: false });
  const hot = fake(0.5);
  hot.alpha = 1;
  const scatter = rig(hot);
  scatter.host.handle({ type: "force.settle" });
  for (let i = 0; i < 3; i += 1) scatter.out.tick();
  assert.ok(!hot.calls.includes("shuffle") && !hot.calls.includes("reheat"), "a settle neither shuffles nor reheats");
  assert.equal(lastFrame(scatter.out.emitted).running, true, "a scatter born hot settles on screen");
});

test("a renew stops the loop but leaves the forces enabled, and the next settle runs a new session", () => {
  const port = fake(0.9);
  let live: LiveForce = port;
  const { host, out } = rig(port, { now: 0, perStep: 0 }, 8, () => live);
  host.handle({ type: "force.drag", id: "a", x: 1, y: 2 });
  out.tick();
  const next = fake(0.5);
  live = next;
  host.renew();
  assert.deepEqual(out.emitted.at(-1), { type: "force-state", running: false, disabled: null, paused: false });
  assert.ok(port.calls.includes("unpin a"), "the pins of the last picture go with it");
  host.handle({ type: "force.settle" });
  assert.deepEqual(next.calls, ["step 0"], "the new session is the one asked");
});

test("a frame already scheduled does not step a session that was released under it", () => {
  // A graph replaced mid-settle: the old session is released and the studio answers no port.
  const port = mortal(0.9);
  let live: LiveForce | null = port;
  const { host, out } = rig(port, { now: 0, perStep: 0 }, 8, () => live);
  host.handle({ type: "force.start" });
  port.dead = true;
  live = null;
  assert.doesNotThrow(() => out.tick(), "a released session is never stepped");
  assert.equal(port.calls.filter((call) => call.startsWith("step")).length, 0);
  assert.equal(out.frames(), 0, "nothing is drawn from a session that is gone");
  assert.deepEqual(out.emitted.at(-1), STOPPED, "but the loop says the settle ended, so the page rests");
  assert.equal(out.scheduled(), 1, "and no frame is scheduled in its place");
});

test("the release notice stops the loop at once, and the next request starts a fresh one", () => {
  const port = mortal(0.9);
  let live: LiveForce | null = port;
  const { host, out } = rig(port, { now: 0, perStep: 0 }, 8, () => live);
  host.handle({ type: "force.drag", id: "a", x: 1, y: 2 });
  out.tick();
  port.calls.length = 0;
  const next = mortal(0.9);
  // What createSession does on replace(): the port is marked dead, then the host is told.
  port.dead = true;
  live = next;
  assert.doesNotThrow(() => host.forget(), "a held pin is not unpinned on a dead session");
  assert.deepEqual(port.calls, [], "the old port is not touched again");
  const before = out.frames();
  assert.doesNotThrow(() => out.tick());
  assert.equal(out.frames(), before, "the frame the old loop had scheduled is gone");
  assert.deepEqual(out.emitted.at(-1), STOPPED, "and the release notice says the settle ended");
  host.handle({ type: "force.start" });
  assert.ok(next.calls.includes("shuffle"), "the host still works over the new session");
});

test("with no adapter every force request answers disabled, with the reason", async () => {
  const answer = await serve(NO_SESSION, { type: "force.drag", id: "a", x: 0, y: 0 });
  assert.deepEqual(answer.result, { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false });
  assert.equal(NO_ADAPTER_REASON, "live forces need the motor session (force-wasm)");
});
