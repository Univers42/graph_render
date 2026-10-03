/**
 * The watchdog over a live settle the worker walked away from: the strip is up, the panel says
 * a session is live, and nothing will ever say otherwise.
 *
 * Two ways in, both named here: silence past the bound, and a worker `error` event. The unit
 * test drives them with a fake worker over an injected schedule, so neither waits on a clock.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type LiveBridge, createLiveBridge } from "../src/motor/bridge.ts";
import { SILENCE_MS, SILENCE_REASON, workerFailed } from "../src/motor/watchdog.ts";
import type { ForceFrame, ForceRequest, Result } from "../src/motor/protocol.ts";
import { HIDDEN } from "../src/ui/progress.ts";

interface Timer { readonly run: () => void; readonly due: number }

/**
 * A clock the test drives by hand, and which only runs a bound once its deadline has passed:
 * the watchdog re-arms on every message, so "the bound expired" only means something when the
 * test has said no message arrived in between. `waiting` is the one bound `setTimeout` holds.
 */
interface FakeClock {
  readonly waiting: Timer[];
  /** What the watchdog reads as "now", so a test can say how long ago a message was. */
  elapsed: number;
  /** Moves `ms` on and runs every bound that has come due, oldest first. */
  elapse(ms: number): number;
  readonly armed: () => number;
  /** When the one armed bound comes due. */
  readonly due: () => number | undefined;
}

function fakeClock(): FakeClock {
  const waiting: Timer[] = [];
  let now = 0;
  return {
    waiting,
    armed: () => waiting.length,
    due: () => waiting.at(-1)?.due,
    elapse: (ms) => {
      now += ms;
      const due = waiting.filter((timer) => timer.due <= now);
      for (const timer of due) waiting.splice(waiting.indexOf(timer), 1);
      for (const timer of due) timer.run();
      return due.length;
    },
    get elapsed() {
      return now;
    },
  };
}

function frame(alpha: number, running = true): ForceFrame {
  return { xs: Float32Array.of(1, 2), ys: Float32Array.of(3, 4), alpha, running };
}

/** A motor the test drives: it answers what it is sent, and can then go quiet or throw. */
interface FakeWorker {
  readonly bridge: LiveBridge;
  /** What reached the worker's port. */
  readonly sent: ForceRequest[];
  /** The one console line per dead session, in order. */
  readonly notes: string[];
  readonly clock: FakeClock;
  /** A live frame from the settle. */
  frame(alpha: number): void;
  /** The loop's own state, which is what a `force.request` answers with. */
  state(result: Result): void;
  /** The worker throws where the page can hear it. */
  fail(detail: string): void;
  /** The worker stops answering and stops failing: a worker that was terminated. */
  quiet(): void;
  /** Answers again, as a worker reopened by a new layout would. */
  wake(): void;
}

function fakeWorker(): FakeWorker {
  const sent: ForceRequest[] = [];
  const notes: string[] = [];
  const clock = fakeClock();
  let listener: (result: Result) => void = () => undefined;
  let failure: (detail: string) => void = () => undefined;
  let awake = true;
  const bridge = createLiveBridge({
    send: (request) => sent.push(request),
    onPush: (handler) => {
      listener = handler;
      return () => { listener = () => undefined; };
    },
    onFail: (handler) => {
      failure = handler;
      return () => { failure = () => undefined; };
    },
    paint: () => undefined,
    report: (reason) => notes.push(reason),
    schedule: (run, ms) => {
      const timer: Timer = { run, due: clock.elapsed + ms };
      clock.waiting.push(timer);
      return () => {
        const at = clock.waiting.indexOf(timer);
        if (at >= 0) clock.waiting.splice(at, 1);
      };
    },
  });
  const push = (result: Result): void => {
    if (awake) listener(result);
  };
  return {
    bridge, sent, notes, clock,
    frame: (alpha) => push({ type: "force-frame", frame: frame(alpha) }),
    state: push,
    fail: (detail) => { if (awake) failure(detail); },
    quiet: () => { awake = false; },
    wake: () => { awake = true; },
  };
}

const RUNNING: Result = { type: "force-state", running: true, disabled: null, paused: false };

test("a worker that goes quiet past the bound hides the strip and names the cause", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  worker.frame(0.5);
  assert.equal(worker.bridge.bar().visible, true, "the strip is up while the worker talks");
  const armed = worker.clock.due();
  assert.equal(armed, worker.clock.elapsed + SILENCE_MS, "the bound is armed from the last message");
  worker.quiet();
  assert.equal(worker.clock.elapse(SILENCE_MS - 1), 0, "nothing happens a millisecond early");
  assert.equal(worker.bridge.bar().visible, true, "the strip is still up just before the bound");
  assert.equal(worker.clock.elapse(1), 1, "the bound runs out and the watchdog fires");
  assert.deepEqual(worker.bridge.bar(), HIDDEN, "a strip nobody is filling goes away");
  assert.equal(worker.bridge.link.disabled(), SILENCE_REASON, "the panel stops claiming a session");
  assert.deepEqual(worker.notes, [SILENCE_REASON], "and the console is told once, naming the cause");
});

test("every message re-arms the bound, so a slow but talking settle is never dead", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  for (const alpha of [1, 0.5, 0.2, 0.1]) {
    worker.frame(alpha);
    assert.equal(worker.clock.armed(), 1, `one bound armed after alpha ${alpha}`);
    // Four frames, each just inside the bound: the whole settle outlives SILENCE_MS by a lot.
    assert.equal(worker.clock.elapse(SILENCE_MS - 1), 0, `nothing fired at alpha ${alpha}`);
  }
  assert.equal(worker.bridge.bar().visible, true, "a settle that kept talking survives its bound");
  assert.deepEqual(worker.notes, [], "nothing was declared dead");
});

test("a worker error takes the same path at once, without waiting out the bound", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  worker.frame(0.5);
  worker.fail("unreachable");
  assert.equal(worker.clock.armed(), 0, "the armed bound was cancelled, not left to fire");
  assert.equal(worker.clock.elapse(SILENCE_MS * 10), 0, "and nothing fires after it");
  assert.deepEqual(worker.bridge.bar(), HIDDEN);
  assert.deepEqual(worker.notes, [workerFailed("unreachable")], "one line, and it names the event");
});

test("a worker error with no message of its own still names the failure", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  worker.fail("");
  assert.deepEqual(worker.notes, [workerFailed("")]);
  assert.deepEqual(worker.bridge.bar(), HIDDEN);
});

test("the batch strip is not watched: a worker busy in a layout is not a dead worker", () => {
  const worker = fakeWorker();
  worker.bridge.batch(1);
  assert.equal(worker.bridge.bar().visible, true, "the strip is up for a batch run");
  assert.equal(worker.clock.armed(), 0, "and no bound is armed: a layout run pushes no messages");
  assert.equal(worker.clock.elapse(SILENCE_MS * 10), 0, "so a long layout is never called dead");
  assert.equal(worker.bridge.bar().visible, true, "nothing took the strip away");
});

test("a dead session is not sticky: a message that arrives later brings it back", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  worker.quiet();
  worker.clock.elapse(SILENCE_MS);
  assert.deepEqual(worker.bridge.bar(), HIDDEN, "the settle is over: the worker was gone");
  // The worker answers again — a re-layout opened a new session, or the fault was transient.
  worker.wake();
  worker.state(RUNNING);
  assert.equal(worker.bridge.link.disabled(), null, "the panel is live again");
  worker.bridge.start();
  worker.frame(0.4);
  assert.equal(worker.bridge.bar().visible, true, "and the strip comes back with it");
  assert.equal(worker.clock.armed(), 1, "watching the new settle again");
});

test("destroy stops the watchdog, so an unmounted studio is never written to", () => {
  const worker = fakeWorker();
  worker.bridge.start();
  worker.bridge.destroy();
  assert.equal(worker.clock.armed(), 0, "the bound was cancelled with the bridge");
  assert.equal(worker.clock.elapse(SILENCE_MS * 10), 0);
  assert.deepEqual(worker.notes, [], "and nothing was reported afterwards");
});
