/**
 * The page's live bridge: the link the forces actions and the drag share, and the one store
 * the progress bar reads. Two things want that strip — a live settle and a batch layout run —
 * and a settle always wins while it is running.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type LiveBridge, createLiveBridge, settlesLive } from "../src/motor/bridge.ts";
import { DEFAULT_KNOBS } from "../src/motor/live.ts";
import type { ForceFrame, ForceRequest, Result } from "../src/motor/protocol.ts";
import { ALPHA_MIN } from "../src/motor/liveLoop.ts";
import { HIDDEN, type Bar } from "../src/ui/progress.ts";

function frame(alpha: number, running = true): ForceFrame {
  return { xs: Float64Array.of(1, 2), ys: Float64Array.of(3, 4), alpha, running };
}

function rig(): {
  readonly bridge: LiveBridge;
  readonly sent: ForceRequest[];
  readonly painted: ForceFrame[];
  push: (result: Result) => void;
  readonly seen: Bar[];
} {
  const sent: ForceRequest[] = [];
  const painted: ForceFrame[] = [];
  const seen: Bar[] = [];
  let listener: (result: Result) => void = () => undefined;
  const bridge = createLiveBridge({
    send: (request) => sent.push(request),
    onPush: (handler) => { listener = handler; return () => { listener = () => undefined; }; },
    paint: (f) => painted.push(f),
  });
  bridge.onBar((bar) => seen.push(bar));
  return {
    bridge, sent, painted, seen,
    push: (result) => listener(result),
  };
}

const RUNNING: Result = { type: "force-state", running: true, disabled: null, paused: false };

test("a settle shows the strip and drains it, and a stopped loop hides it", () => {
  const { bridge, push, seen } = rig();
  push(RUNNING);
  push({ type: "force-frame", frame: frame(1) });
  assert.equal(bridge.bar().visible, true);
  assert.equal(bridge.bar().fraction, 1);
  push({ type: "force-frame", frame: frame(ALPHA_MIN) });
  assert.equal(bridge.bar().fraction, 0, "the strip empties exactly when the loop's floor is reached");
  push({ type: "force-frame", frame: frame(ALPHA_MIN, false) });
  assert.deepEqual(bridge.bar(), HIDDEN);
  assert.deepEqual(
    seen.map((bar) => bar.visible),
    [false, true, true, false],
    "every change is announced once, the session's first answer included",
  );
});

test("a batch layout run shows the same strip, with no fraction of its own", () => {
  const { bridge } = rig();
  bridge.batch(1);
  assert.equal(bridge.bar().visible, true);
  assert.equal(bridge.bar().fraction, null, "one call reports no progress inside it");
  bridge.batch(2);
  assert.equal(bridge.bar().label, "2 running");
  bridge.batch(0);
  assert.deepEqual(bridge.bar(), HIDDEN);
});

test("a running settle wins the strip over a batch run, and takes it back when it stops", () => {
  const { bridge, push } = rig();
  bridge.batch(1);
  push(RUNNING);
  push({ type: "force-frame", frame: frame(0.5) });
  assert.equal(bridge.bar().label, "settling", "the live frames are what the drawing is showing");
  push({ type: "force-frame", frame: frame(ALPHA_MIN, false) });
  assert.equal(bridge.bar().label, "1 running", "and the batch takes the strip back");
});

test("a knob change reaches the worker, and the panel reads the value back", () => {
  const { bridge, sent } = rig();
  const knobs = { ...DEFAULT_KNOBS, charge: -700 };
  bridge.link.set(knobs);
  assert.deepEqual(sent, [{ type: "force.params", knobs }]);
  assert.deepEqual(bridge.link.knobs(), knobs);
});

test("animate on restarts from the seed, and off stops the loop", () => {
  const { bridge, sent } = rig();
  bridge.link.animate(true);
  const start = { type: "force.start", knobs: DEFAULT_KNOBS };
  assert.deepEqual(sent, [start], "the start carries the knobs the panel shows");
  assert.equal(bridge.link.animating(), true);
  bridge.link.animate(false);
  assert.deepEqual(sent, [start, start, { type: "force.stop" }]);
  assert.equal(bridge.link.animating(), false);
});

test("pause and resume are their own verbs, and neither is the animate toggle", () => {
  const { bridge, sent } = rig();
  bridge.link.pause();
  assert.deepEqual(sent, [{ type: "force.pause" }]);
  assert.equal(bridge.link.paused(), true);
  assert.equal(bridge.link.animating(), false, "a paused loop is not animating");
  bridge.link.resume();
  assert.deepEqual(sent, [{ type: "force.pause" }, { type: "force.resume" }]);
  assert.equal(bridge.link.paused(), false);
});

test("the panel is disabled until the worker has answered, and live once it says so", () => {
  const { bridge, push } = rig();
  assert.notEqual(bridge.link.disabled(), null, "nothing is claimed before the worker answers");
  push({ type: "force-state", running: false, disabled: null, paused: false });
  assert.equal(bridge.link.disabled(), null, "asked, and there is a session");
  push({ type: "force-state", running: false, disabled: "no motor here", paused: false });
  assert.equal(bridge.link.disabled(), "no motor here");
});

test("start shows the strip before the first frame, and destroy takes everything down", () => {
  const { bridge, sent } = rig();
  bridge.start();
  assert.equal(bridge.bar().visible, true);
  assert.equal(sent.at(-1)?.type, "force.start");
  bridge.destroy();
  assert.deepEqual(bridge.bar(), HIDDEN);
  assert.equal(sent.at(-1)?.type, "force.stop");
});

test("every frame is handed to the canvas, and destroy stops listening", () => {
  const { bridge, painted, push } = rig();
  push({ type: "force-frame", frame: frame(0.5) });
  assert.equal(painted.length, 1);
  bridge.destroy();
  push({ type: "force-frame", frame: frame(0.4) });
  assert.equal(painted.length, 1, "nothing reaches the view after the bridge is gone");
});

test("only the live session's own layout settles live; every other engine keeps its picture", () => {
  assert.equal(settlesLive("layout.force.barnes_hut"), true);
  const finished = ["layout.forceatlas2", "layout.force.yifan_hu", "layout.force.kamada_kawai", "layout.force.drl",
    "layout.grid", "layout.random", "layout.circular.radial", "layout.dag.sugiyama"];
  for (const id of finished) assert.equal(settlesLive(id), false, id);
});

test("a finished layout holds still: it stops the last settle and never restarts one", () => {
  const { bridge, push, sent } = rig();
  bridge.start();
  push(RUNNING);
  bridge.hold();
  assert.deepEqual(sent.map((request) => request.type), ["force.start", "force.stop"]);
  assert.deepEqual(bridge.bar(), HIDDEN, "no strip over a picture that is not moving");
  push({ type: "force-state", running: false, disabled: null, paused: false });
  assert.equal(bridge.link.disabled(), null, "the answer to the stop enables the forces panel");
});

test("an answer that changes only the session's state still tells the panel, once", () => {
  const { bridge, push, seen } = rig();
  bridge.hold();
  const before = seen.length;
  push({ type: "force-state", running: false, disabled: null, paused: false });
  assert.equal(seen.length, before + 1, "the strip stayed hidden, but the panel's reason changed");
  push({ type: "force-state", running: false, disabled: null, paused: false });
  assert.equal(seen.length, before + 1, "the same answer twice is not a change");
});

test("a drag pins the live session only over its own picture; over a finished layout it moves one node", () => {
  const { bridge, push } = rig();
  push({ type: "force-state", running: false, disabled: null, paused: false });
  bridge.hold();
  assert.notEqual(bridge.dragDisabled(), null, "a finished picture is not the session's to move");
  assert.equal(bridge.link.disabled(), null, "and the forces panel stays usable over it");
  bridge.start();
  assert.equal(bridge.dragDisabled(), null, "the live layout's settle is the session's picture");
  bridge.hold();
  bridge.link.animate(true);
  assert.equal(bridge.dragDisabled(), null, "so is an Animate");
  bridge.hold();
  bridge.link.set(DEFAULT_KNOBS);
  assert.equal(bridge.dragDisabled(), null, "and a knob change, which wakes the loop");
});
