/**
 * The frame throttle: a live settle publishes the bar about sixty times a second and the
 * panel may answer once a frame, so the throttle is measured here against a scheduler the
 * test opens and closes by hand — node has no frames of its own, and neither does a worker.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Bar, HIDDEN, batchBar } from "../src/ui/progress.ts";
import {
  FRAME_MS,
  type BarStore,
  type FrameScheduler,
  frameScheduler,
  throttledBar,
} from "../src/ui/frameThrottle.ts";

/** Frames held open until the test runs them, so no row here waits on a clock. */
interface Held {
  readonly scheduler: FrameScheduler;
  readonly opened: () => number;
  readonly dropped: () => number;
  readonly frame: () => void;
}

function heldFrames(): Held {
  let waiting: (() => void)[] = [];
  let opened = 0;
  let dropped = 0;
  const scheduler: FrameScheduler = {
    next: (run) => {
      opened += 1;
      waiting = [...waiting, run];
      return () => {
        dropped += 1;
        waiting = waiting.filter((other) => other !== run);
      };
    },
  };
  return {
    scheduler,
    opened: () => opened,
    dropped: () => dropped,
    frame: () => {
      const due = waiting;
      waiting = [];
      for (const run of due) run();
    },
  };
}

/** The store the bridge publishes into: a value, and whoever is listening. */
interface Pushable {
  readonly store: BarStore;
  readonly push: (bar: Bar) => void;
  readonly listening: () => number;
}

function pushable(shown: Bar = HIDDEN): Pushable {
  let current = shown;
  let watching: ((bar: Bar) => void)[] = [];
  return {
    store: {
      bar: () => current,
      onBar: (handler) => {
        watching = [...watching, handler];
        return () => {
          watching = watching.filter((other) => other !== handler);
        };
      },
    },
    push: (bar) => {
      current = bar;
      for (const handler of [...watching]) handler(bar);
    },
    listening: () => watching.length,
  };
}

const BUSY = [HIDDEN, batchBar(1), batchBar(2), batchBar(3), batchBar(4)];

test("many publishes inside one frame are one redraw, and it waits for the frame", () => {
  const frames = heldFrames();
  const { store, push } = pushable();
  let redraws = 0;
  throttledBar(store, frames.scheduler, () => {
    redraws += 1;
  });
  for (const bar of BUSY) push(bar);
  assert.equal(frames.opened(), 1, "one frame was asked for, not five");
  assert.equal(redraws, 0, "and nothing is drawn before it runs");
  frames.frame();
  assert.equal(redraws, 1);
});

test("a publish after the frame ran is a second frame and a second redraw", () => {
  const frames = heldFrames();
  const { store, push } = pushable();
  let redraws = 0;
  throttledBar(store, frames.scheduler, () => {
    redraws += 1;
  });
  push(batchBar(1));
  frames.frame();
  push(batchBar(2));
  assert.equal(frames.opened(), 2, "the throttle let go when the frame ran");
  assert.equal(redraws, 1, "and the second publish is not drawn yet either");
  frames.frame();
  assert.equal(redraws, 2);
});

test("the cancel drops the frame waiting and lets the store go", () => {
  const frames = heldFrames();
  const { store, push, listening } = pushable();
  let redraws = 0;
  const stop = throttledBar(store, frames.scheduler, () => {
    redraws += 1;
  });
  push(batchBar(1));
  assert.equal(listening(), 1);
  stop();
  assert.equal(listening(), 0, "the store has nobody left to tell");
  assert.equal(frames.dropped(), 1, "and the frame waiting is not going to run");
  frames.frame();
  assert.equal(redraws, 0);
  push(batchBar(2));
  assert.equal(frames.opened(), 1, "a publish after the cancel opens no frame at all");
});

/** Resolves when the work settles, and gives up rather than hanging the row. */
function within(ms: number, work: Promise<void>): Promise<void> {
  return new Promise((resolve, reject) => {
    const bail = setTimeout(() => reject(new Error("the fallback frame never came")), ms);
    void work.then(
      () => {
        clearTimeout(bail);
        resolve();
      },
      (error: unknown) => {
        clearTimeout(bail);
        reject(error instanceof Error ? error : new Error(String(error)));
      },
    );
  });
}

test("a host with no frames redraws from a timer of its own, once", async () => {
  assert.equal(FRAME_MS, 16, "a frame is 60 Hz, and the fallback says so");
  const scheduler = frameScheduler({});
  let redraws = 0;
  const ran = new Promise<void>((resolve) => {
    scheduler.next(() => {
      redraws += 1;
      resolve();
    });
  });
  await within(5000, ran);
  assert.equal(redraws, 1);
});

test("the fallback's cancel takes its timer back before it fires", async () => {
  const scheduler = frameScheduler({});
  let redraws = 0;
  const stop = scheduler.next(() => {
    redraws += 1;
  });
  stop();
  await within(FRAME_MS * 4, Promise.resolve());
  assert.equal(redraws, 0, "a frame that was dropped never ran");
});

test("a host that has frames is asked for them, and a drop takes one back", () => {
  const asked: number[] = [];
  const dropped: number[] = [];
  const scheduler = frameScheduler({
    requestAnimationFrame: () => {
      asked.push(asked.length + 1);
      return asked.length;
    },
    cancelAnimationFrame: (token) => dropped.push(token),
  });
  const stop = scheduler.next(() => undefined);
  assert.deepEqual(asked, [1], "the host's own frame request is the one used");
  stop();
  assert.deepEqual(dropped, [1], "and its own cancel takes the frame back");
});