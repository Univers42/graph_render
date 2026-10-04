// One run per frame: a control that can be moved faster than a frame can be shown collapses
// into one run, and giving the frame up runs nothing.
import assert from "node:assert/strict";
import { test } from "node:test";

import type { FrameScheduler } from "../src/ui/frameThrottle.ts";
import { oneRunPerFrame } from "../src/ui/oneRun.ts";

/** A scheduler the test drives itself, as `frameThrottle.test.ts` does. */
function driven(): { readonly scheduler: FrameScheduler; readonly run: () => void; readonly asked: () => number } {
  const frames: (() => void)[] = [];
  return {
    scheduler: { next: (run) => { frames.push(run); return () => undefined; } },
    run: () => frames.shift()?.(),
    asked: () => frames.length,
  };
}

test("seven changes in one frame ask for one frame and one run", () => {
  const frame = driven();
  let runs = 0;
  const once = oneRunPerFrame(frame.scheduler, () => { runs += 1; });
  for (let move = 0; move < 7; move += 1) once.ask();
  assert.equal(frame.asked(), 1, "one frame is waiting, not seven");
  frame.run();
  assert.equal(runs, 1);
});

test("the next change after the frame asks for the next frame", () => {
  const frame = driven();
  let runs = 0;
  const once = oneRunPerFrame(frame.scheduler, () => { runs += 1; });
  once.ask();
  frame.run();
  once.ask();
  assert.equal(frame.asked(), 1);
  frame.run();
  assert.equal(runs, 2);
});

test("giving the frame up runs nothing, and the next ask is a new frame", () => {
  const frame = driven();
  let runs = 0;
  const once = oneRunPerFrame(frame.scheduler, () => { runs += 1; });
  once.ask();
  once.drop();
  once.ask();
  frame.run();
  assert.equal(runs, 1, "only the ask that came after the drop ran");
});

test("dropping with no frame waiting is nothing, not a throw", () => {
  const once = oneRunPerFrame(driven().scheduler, () => undefined);
  once.drop();
  once.drop();
});
