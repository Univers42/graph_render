/**
 * Row `host-api-unit`, the previews behind `resolve` (`docs/contract/host-api.md`, verdict 6): the
 * cap, the generation, the uncached rejection and eviction at entry 257.
 *
 * Each check is a function run twice: once on the real previews, where it must hold, and once on
 * a subject broken in exactly the way the check exists to catch, where it must not. A check that
 * holds on both proves nothing, so the second run is its negative control.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { NodePreview, Resolve } from "../src/host/contract.ts";
import { PREVIEW_LIMITS, type PreviewLimits } from "../src/host/preview.ts";
import { PREVIEW_CAPACITY, type PreviewDeps, type Previews, createPreviews } from "../src/host/previews.ts";

/** The debounce and the timeout, run when the test says so and not when a clock does. */
interface Clock {
  readonly schedule: NonNullable<PreviewDeps["schedule"]>;
  /** Runs every pending timer whose delay is `ms`; returns how many ran. */
  readonly run: (ms: number) => number;
}

function clock(): Clock {
  const pending = new Set<{ readonly run: () => void; readonly ms: number }>();
  return {
    schedule: (run, ms) => {
      const timer = { run, ms };
      pending.add(timer);
      return () => void pending.delete(timer);
    },
    run: (ms) => {
      const due = [...pending].filter((timer) => timer.ms === ms);
      for (const timer of due) {
        pending.delete(timer);
        timer.run();
      }
      return due.length;
    },
  };
}

/** A host that counts its calls and answers each one when the test settles it. */
interface Host {
  readonly resolve: Resolve;
  readonly asked: string[];
  readonly signals: AbortSignal[];
  readonly answer: (at: number, value: NodePreview) => void;
  readonly refuse: (at: number) => void;
}

function host(): Host {
  const settles: { ok: (value: NodePreview) => void; no: (error: Error) => void }[] = [];
  const made: Host = {
    asked: [], signals: [],
    resolve: (id, signal) => {
      made.asked.push(id);
      made.signals.push(signal);
      return new Promise((ok, no) => settles.push({ ok, no }));
    },
    answer: (at, value) => settles[at]?.ok(value),
    refuse: (at) => settles[at]?.no(new Error("the host could not say")),
  };
  return made;
}

interface Rig {
  readonly previews: Previews;
  readonly host: Host;
  readonly clock: Clock;
}

function rig(extra: Partial<PreviewDeps> = {}): Rig {
  const made = { host: host(), clock: clock() };
  const previews = createPreviews({ resolver: () => made.host.resolve, schedule: made.clock.schedule, ...extra });
  return { ...made, previews };
}

/** Lets the promise chain inside the previews run to its end. */
const drained = (): Promise<void> => new Promise((done) => setImmediate(done));

/** The inspector asks at once (delay 0); the hover card waits 150 ms. */
async function inspect(subject: Rig, id: string): Promise<void> {
  subject.previews.want("inspector", id);
  subject.clock.run(0);
  await drained();
}

const LONG = "\u{1F600}".repeat(300);

async function capHolds(limits?: PreviewLimits): Promise<boolean> {
  const subject = rig(limits === undefined ? {} : { limits });
  await inspect(subject, "a");
  // Through a variable: a host's object may carry more than the type names, and `url` must not survive.
  const hostile = { title: LONG, text: "x".repeat(5000), icon: "i".repeat(40), url: "https://example.invalid/" };
  subject.host.answer(0, hostile);
  await drained();
  const shown = subject.previews.shown("inspector").preview;
  return shown !== null && [...shown.title].length === 256 && [...(shown.text ?? "")].length === 4096 &&
    [...(shown.icon ?? "")].length === 16 && !("url" in shown) && shown.title === LONG.slice(0, 512);
}

test("a preview is cut to 256, 4096 and 16 code points, a pair never split, and no url kept", async () => {
  assert.equal(await capHolds(), true);
});

test("negative control: with no cap the same check is red", async () => {
  assert.equal(await capHolds({ title: Infinity, text: Infinity, icon: Infinity }), false);
  assert.deepEqual(PREVIEW_LIMITS, { title: 256, text: 4096, icon: 16 });
});

/** True when an answer that arrives after `between` ran is neither shown nor cached. */
async function lateAnswerDropped(between: (subject: Rig) => void): Promise<boolean> {
  const subject = rig();
  await inspect(subject, "a");
  between(subject);
  subject.host.answer(0, { title: "about the last graph" });
  await drained();
  await inspect(subject, "a");
  return subject.previews.shown("inspector").preview === null && subject.host.asked.length === 2;
}

test("an answer about the last graph is dropped: the generation moved", async () => {
  assert.equal(await lateAnswerDropped((subject) => subject.previews.clear()), true);
});

test("negative control: with no new graph between the call and the answer, it is kept", async () => {
  assert.equal(await lateAnswerDropped(() => undefined), false);
});

/** True when the host is asked again for `a` after its first answer to `a` was `settle`. */
async function askedAgain(settle: (subject: Rig) => void): Promise<boolean> {
  const subject = rig();
  await inspect(subject, "a");
  settle(subject);
  await drained();
  await inspect(subject, "b");
  await inspect(subject, "a");
  return subject.host.asked.filter((id) => id === "a").length === 2;
}

test("a rejection is never cached: the next want asks the host again", async () => {
  assert.equal(await askedAgain((subject) => subject.host.refuse(0)), true);
});

test("a timeout is never cached either, and the call is aborted", async () => {
  const subject = rig();
  await inspect(subject, "a");
  assert.equal(subject.clock.run(5000), 1, "one timeout armed for the one call");
  assert.equal(subject.host.signals[0]?.aborted, true);
  subject.host.answer(0, { title: "too late" });
  await drained();
  assert.equal(subject.previews.shown("inspector").preview, null);
});

test("negative control: an answer, unlike a rejection, is cached and not asked for again", async () => {
  assert.equal(await askedAgain((subject) => subject.host.answer(0, { title: "A" })), false);
});

/** True when the first of `count` answered ids has left a cache of the default capacity. */
async function firstEvictedAfter(count: number): Promise<boolean> {
  const subject = rig();
  for (let at = 0; at < count; at += 1) {
    await inspect(subject, `n${at}`);
    subject.host.answer(at, { title: `N${at}` });
    await drained();
  }
  const before = subject.host.asked.length;
  await inspect(subject, "n0");
  return subject.host.asked.length === before + 1;
}

test(`the cache holds ${PREVIEW_CAPACITY} previews and evicts the oldest at entry 257`, async () => {
  assert.equal(PREVIEW_CAPACITY, 256);
  assert.equal(await firstEvictedAfter(257), true);
});

test("negative control: at entry 256 nothing has been evicted yet", async () => {
  assert.equal(await firstEvictedAfter(256), false);
});

test("one call in flight per consumer: moving on aborts the last one, and its answer is dropped", async () => {
  const subject = rig();
  await inspect(subject, "a");
  await inspect(subject, "b");
  assert.deepEqual(subject.host.asked, ["a", "b"]);
  assert.equal(subject.host.signals[0]?.aborted, true, "the inspector's call is aborted on a selection change");
  assert.equal(subject.host.signals[1]?.aborted, false);
  subject.host.answer(0, { title: "A" });
  await drained();
  assert.deepEqual(subject.previews.shown("inspector"), { id: "b", preview: null });
});

test("the hover card waits 150 ms, and a pointer that moves on first asks nothing", async () => {
  const subject = rig();
  subject.previews.want("hover", "a");
  subject.previews.want("hover", "b");
  assert.equal(subject.clock.run(150), 1, "only the last node's debounce is still armed");
  await drained();
  assert.deepEqual(subject.host.asked, ["b"]);
});

test("invalidate forgets one preview and asks again where it is shown", async () => {
  const subject = rig();
  await inspect(subject, "a");
  subject.host.answer(0, { title: "old" });
  await drained();
  subject.previews.invalidate("a");
  await drained();
  assert.deepEqual(subject.host.asked, ["a", "a"]);
  subject.host.answer(1, { title: "new" });
  await drained();
  assert.equal(subject.previews.shown("inspector").preview?.title, "new");
});
