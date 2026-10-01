// The live loop against a fake port: the stop condition, frame dropping, the disabled state.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_KNOBS, type ForceKnobs, type LiveForce, NO_ADAPTER_REASON } from "../src/motor/live.ts";
import { ALPHA_MIN, TICKS_PER_FRAME, createForceHost } from "../src/motor/liveLoop.ts";
import type { ForceFrame, Result } from "../src/motor/protocol.ts";
import type { Session } from "../src/motor/session.ts";
import { serve } from "../src/motor/serve.ts";

const refuse = (): never => { throw new Error("a force request must not reach the session"); };
const NO_SESSION: Session = { open: refuse, load: refuse, layout: refuse, analysis: refuse, forces: () => null };
const KNOBS: ForceKnobs = { gravity: 0.5, charge: -100, linkStrengthScale: 1, linkDistance: 40 };

interface Clock { now: number; perStep: number }

interface Fake extends LiveForce {
  readonly calls: string[];
  alpha: number;
  decay: number;
}

function fake(decay: number, clock: Clock = { now: 0, perStep: 0 }): Fake {
  const port: Fake = {
    calls: [], alpha: 0, decay,
    pin: (id, x, y) => { port.calls.push(`pin ${id} ${x} ${y}`); },
    unpin: (id) => { port.calls.push(`unpin ${id}`); },
    setParams: () => { port.calls.push("params"); },
    step: (ticks) => {
      port.calls.push(`step ${ticks}`);
      clock.now += clock.perStep;
      port.alpha *= port.decay;
      return port.alpha;
    },
    positions: () => ({ xs: Float64Array.of(1, 2), ys: Float64Array.of(3, 4) }),
    reheat: (alpha) => { port.alpha = alpha; port.calls.push("reheat"); },
    shuffle: () => { port.calls.push("shuffle"); port.alpha = 1; return port.alpha; },
  };
  return port;
}

interface Rig {
  readonly emitted: Result[];
  readonly frames: () => number;
  readonly tick: () => void;
  readonly scheduled: () => number;
}

function lastFrame(emitted: readonly Result[]): ForceFrame {
  const found = emitted.filter((r) => r.type === "force-frame").at(-1);
  if (found?.type !== "force-frame") throw new Error("no frame was emitted");
  return found.frame;
}

const START = { type: "force.start", knobs: DEFAULT_KNOBS } as const;

function rig(port: Fake, clock: Clock = { now: 0, perStep: 0 }, budgetMs = 8) {
  const emitted: Result[] = [];
  let next: (() => void) | null = null;
  let scheduled = 0;
  const host = createForceHost(() => port, {
    schedule: (run) => { next = run; scheduled += 1; return () => { next = null; }; },
    now: () => clock.now,
    emit: (result) => { emitted.push(result); },
    budgetMs,
  });
  const out: Rig = {
    emitted,
    frames: () => emitted.filter((r) => r.type === "force-frame").length,
    tick: () => { const run = next; next = null; run?.(); },
    scheduled: () => scheduled,
  };
  return { host, out };
}

test("the loop steps until alpha is under alpha_min, then stops on its own", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle(START);
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
  assert.ok(port.calls.includes("unpin a"));
  for (let i = 0; i < 30; i += 1) out.tick();
  assert.equal(lastFrame(out.emitted).running, false);
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

test("a slow motor drops its next tick instead of running late, and never queues", () => {
  const clock = { now: 0, perStep: 20 };
  const port = fake(0.99, clock);
  const { host, out } = rig(port, clock, 8);
  host.handle(START);
  out.tick();
  assert.equal(port.calls.filter((c) => c.startsWith("step")).length, 1, "the tick overran the budget");
  out.tick();
  assert.equal(port.calls.filter((c) => c.startsWith("step")).length, 1, "so the next tick was dropped");
  assert.equal(out.frames(), 2, "and the frame was drawn anyway");
  assert.equal(out.scheduled(), 3, "exactly one frame is ever pending");
});

test("a fast motor steps once a frame, so the settle lasts as long as the animation", () => {
  const clock = { now: 0, perStep: 1 };
  const port = fake(0.999, clock);
  const { host, out } = rig(port, clock, 8);
  host.handle(START);
  for (let i = 0; i < 5; i += 1) out.tick();
  assert.equal(port.calls.filter((c) => c.startsWith("step")).length, 5, "one tick a frame, not a budgetful");
  assert.equal(port.calls.filter((c) => c === `step ${TICKS_PER_FRAME}`).length, 5);
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
  host.handle(START);
  out.tick();
  assert.deepEqual(Array.from(lastFrame(out.emitted).xs), [1, 2]);
});

test("a pause stops the frames and keeps the pins; resume carries on from that alpha", () => {
  const port = fake(0.99);
  const { host, out } = rig(port);
  host.handle(START);
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

test("force.start applies the panel's knobs, then restarts the settle from the seed", () => {
  const port = fake(0.5);
  const { host, out } = rig(port);
  host.handle(START);
  out.tick();
  port.calls.length = 0;
  host.handle(START);
  assert.deepEqual(port.calls.slice(0, 2), ["params", "shuffle"], "the knobs first, then a restart, not a resume");
  assert.ok(port.alpha > 0.9, "and it reheats to the top, so the bar fills again");
});

test("with no adapter every force request answers disabled, with the reason", async () => {
  const answer = await serve(NO_SESSION, { type: "force.drag", id: "a", x: 0, y: 0 });
  assert.deepEqual(answer.result, { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false });
  assert.equal(NO_ADAPTER_REASON, "live forces need the motor session (force-wasm)");
});
