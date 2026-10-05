import assert from "node:assert/strict";
import { test } from "node:test";
import { hubCaller } from "../src/hub/call.ts";
import { subscribe } from "../src/hub/subscribe.ts";
import { nextDelay } from "../src/hub/backoff.ts";

const BASE = "http://hub.test:8081";
const WS = "ops";

// `setTimeout` is not one of the eight globals `eslint.config.js:68-78` gives a `.mjs` file.
// Several turns, because the loop's own awaits are all microtasks: one macrotask boundary is
// enough for the microtask queue to drain, and four makes the assertions independent of how
// many awaits a given path happens to have.
async function tick() {
  for (let i = 0; i < 4; i += 1) await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
}

/** A `wait` the test steps. Each call parks until the test releases it, so the subscribe loop
 * advances exactly one backoff at a time and can never race past an assertion — and never spins,
 * which an immediately-resolving `wait` would let it do. */
function paced() {
  const seen = [];
  let release = null;
  const wait = async (ms) => {
    seen.push(ms);
    await new Promise((resolve) => {
      release = resolve;
    });
  };
  // Bounded, so a loop that never parks is a failed assertion and not a hung test.
  const spin = async (done) => {
    for (let spins = 0; spins < 200 && !done(); spins += 1) await tick();
  };
  return {
    seen,
    wait,
    /** Let the loop take exactly one more backoff: release the parked `wait`, then wait for
     * the next one to park. Releasing alone is not enough — every await in the loop is a
     * microtask, so one macrotask turn lets it run all the way to the next `wait`. */
    async step() {
      const before = seen.length;
      await spin(() => release !== null);
      assert.notEqual(release, null, `the loop never reached wait() (saw ${JSON.stringify(seen)})`);
      const go = release;
      release = null;
      go();
      await spin(() => seen.length > before);
      assert.equal(seen.length, before + 1, `the loop did not take exactly one backoff (saw ${JSON.stringify(seen)})`);
    },
    /** Let the loop finish whatever it is parked on — after `stop()`, it exits rather than
     * waiting again, so no growth is expected. */
    async finish() {
      await spin(() => release !== null);
      if (release !== null) {
        const go = release;
        release = null;
        go();
      }
      await tick();
    },
  };
}

function notice(seq) {
  return `event: change\nid: 1.${seq}\ndata: ${JSON.stringify({ seq, plugin: "ops", at: "2026-10-05T00:00:00Z" })}\n\n`;
}

function change(seq) {
  return { at: "2026-10-05T00:00:00Z", kind: "batch", plugin: "ops", seq, upserts: [] };
}

function page(seqs) {
  return new Response(JSON.stringify(seqs.map(change)), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function gone() {
  return new Response(JSON.stringify({ error: "cursor", message: "outside what is kept" }), {
    status: 410,
    headers: { "Content-Type": "application/json" },
  });
}

function sse(text) {
  return new Response(text, { status: 200, headers: { "Content-Type": "text/event-stream" } });
}

/** The two hub routes `subscribe` reads. `events(n)` is the body of the n-th `GET /events`
 * (0-based) and `changes(n)` the n-th `GET /changes`, so a script can be a function of the
 * call index and an endless ladder needs no endless array. */
function fakeHub(script) {
  const seen = [];
  let events = 0;
  let changes = 0;
  const fetch = async (url, init) => {
    const call = { url, init: init.headers ?? {} };
    seen.push(call);
    if (url.includes("/events")) {
      call.kind = "events";
      // A script may hand back a whole `Response` (to answer a refusal or a 410) or just the
      // SSE text, which is what every other script does.
      const body = script.events(events++);
      return body instanceof Response ? body : sse(body);
    }
    if (url.includes("/changes")) {
      call.kind = "changes";
      return await script.changes(changes++);
    }
    return new Response("{}", { status: 500 });
  };
  return {
    caller: hubCaller({ baseUrl: BASE, fetch }),
    seen,
    count: (kind) => seen.filter((c) => c.kind === kind).length,
    eventsSeen: () => seen.filter((c) => c.kind === "events"),
    changesSeen: () => seen.filter((c) => c.kind === "changes"),
  };
}

const BUSY = "event: busy\ndata: {}\n\n";
const RESYNC = "event: resync\ndata: {}\n\n";
const NEVER = () => assert.fail("the path under test must not reach this");

// The order of the first nine is the plan's list; `negctl-hub-sdk-gap` in
// `scripts/orch/rows/hub-sdk.rows` greps `not ok 1 - hub_subscribe_resyncs_on_an_injected_gap`
// and node's tap reporter numbers tests from 1 in declaration order.

test("hub_subscribe_resyncs_on_an_injected_gap", async () => {
  // The first change is `cursor + 3`, not `cursor + 1`: something was skipped and no other
  // check in this package would notice.
  const hub = fakeHub({ events: () => notice(9), changes: () => page([9]) });
  const resynced = [];
  const stop = subscribe(hub.caller, WS, {
    since: "1.6",
    onChange: NEVER,
    onResync: () => resynced.push("resync"),
    wait: NEVER,
  });
  await tick();
  stop();
  assert.deepEqual(resynced, ["resync"]);
  assert.equal(hub.count("changes"), 1);
});

test("hub_subscribe_resyncs_on_410_from_changes", async () => {
  const hub = fakeHub({ events: () => notice(7), changes: () => gone() });
  const resynced = [];
  const stop = subscribe(hub.caller, WS, {
    since: "1.6",
    onChange: NEVER,
    onResync: () => resynced.push("resync"),
    wait: NEVER,
  });
  await tick();
  stop();
  assert.deepEqual(resynced, ["resync"]);
});

test("hub_subscribe_resyncs_on_a_resync_event", async () => {
  const hub = fakeHub({ events: () => RESYNC, changes: () => page([]) });
  const resynced = [];
  const stop = subscribe(hub.caller, WS, {
    since: "1.6",
    onChange: NEVER,
    onResync: () => resynced.push("resync"),
    wait: NEVER,
  });
  await tick();
  stop();
  assert.deepEqual(resynced, ["resync"]);
  // A resync is the cursor's own word: no `/changes` read is spent discovering it.
  assert.equal(hub.count("changes"), 0);
});

test("hub_subscribe_coalesces_and_orders", async () => {
  // Three notices, two pages, and the first drain is in flight while notices 6 and 7 are still
  // in the stream buffer. A read per notice would be three reads; raising a target makes it two
  // reads and `seq` order.
  const hub = fakeHub({
    events: () => notice(5) + notice(6) + notice(7),
    changes: (n) => page(n === 0 ? [5, 6] : [7]),
  });
  const gate = paced();
  const delivered = [];
  const stop = subscribe(hub.caller, WS, {
    // `since` 1.4, so 5 is `cursor + 1` and the first change is not a gap.
    since: "1.4",
    onChange: (c) => delivered.push(c.seq),
    onResync: NEVER,
    wait: gate.wait,
  });
  await tick();
  assert.deepEqual(delivered, [5, 6, 7]);
  assert.equal(hub.count("changes"), 2, "one drain in flight, one more for the last notice");
  assert.equal(hub.changesSeen()[1].url, `${BASE}/v1/workspaces/ops/changes?since=1.6&limit=1000`);
  stop();
  await gate.finish();
});

test("hub_subscribe_reconnects_with_last_event_id", async () => {
  const hub = fakeHub({ events: (n) => (n === 0 ? notice(7) : BUSY), changes: () => page([7]) });
  const gate = paced();
  const stop = subscribe(hub.caller, WS, {
    since: "1.6",
    onChange: () => {},
    onResync: NEVER,
    wait: gate.wait,
    random: () => 0,
  });
  await gate.step();
  const [first, second] = hub.eventsSeen();
  // The first open has no cursor of its own yet, so `since` goes in the query.
  assert.equal(first.url, `${BASE}/v1/workspaces/ops/events?since=1.6`);
  assert.equal(first.init["Last-Event-ID"], undefined);
  assert.equal(second.url, `${BASE}/v1/workspaces/ops/events`);
  assert.equal(second.init["Last-Event-ID"], "1.7");
  stop();
  await gate.finish();
});

test("hub_subscribe_busy_backs_off_and_does_not_resync", async () => {
  const hub = fakeHub({ events: () => BUSY, changes: () => page([]) });
  const gate = paced();
  const stop = subscribe(hub.caller, WS, {
    onChange: NEVER,
    onResync: NEVER,
    wait: gate.wait,
    random: () => 0,
  });
  await gate.step();
  assert.deepEqual(gate.seen, [500]);
  // §5.3: busy carries no `id:` line and the cursor is still valid, so a resync — which would
  // read the whole `/graph` from a pool that is already short — never happens.
  assert.equal(hub.count("changes"), 0);
  stop();
  await gate.finish();
});

test("hub_subscribe_backoff_doubles_from_1s_to_30s_with_upper_half_jitter", async () => {
  // `random()` fixed at 0, so each rung is its floor `nextDelay(steps) / 2`: 1 s doubling to
  // 30 s. See `busyBackoff`: the plan's "0.5" cannot produce the list it pins beside it.
  const hub = fakeHub({ events: () => BUSY, changes: () => page([]) });
  const gate = paced();
  const stop = subscribe(hub.caller, WS, { onChange: () => {}, onResync: NEVER, wait: gate.wait, random: () => 0 });
  for (let i = 0; i < 8; i += 1) await gate.step();
  assert.deepEqual(gate.seen, [500, 1000, 2000, 4000, 8000, 15000, 15000, 15000]);
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5, 6, 7].map(nextDelay),
    [1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000],
    "the un-jittered ladder",
  );
  stop();
  await gate.finish();
});

test("hub_subscribe_resets_the_backoff_after_a_delivered_change", async () => {
  // busy, busy, a change, busy: waits 500, 1000, then 500 again — §7's "reset to 1 s after a
  // delivered change", the only thing that shortens the ladder.
  const hub = fakeHub({
    events: (n) => [BUSY, BUSY, notice(1), BUSY][n] ?? BUSY,
    changes: () => page([1]),
  });
  const gate = paced();
  const stop = subscribe(hub.caller, WS, { onChange: () => {}, onResync: NEVER, wait: gate.wait, random: () => 0 });
  for (let i = 0; i < 4; i += 1) await gate.step();
  assert.deepEqual(gate.seen, [500, 1000, 500, 1000]);
  stop();
  await gate.finish();
});

test("hub_subscribe_stops", async () => {
  const hub = fakeHub({ events: () => BUSY, changes: () => page([]) });
  const gate = paced();
  const stop = subscribe(hub.caller, WS, { onChange: NEVER, onResync: NEVER, wait: gate.wait, random: () => 0 });
  await gate.step();
  const opened = hub.count("events");
  stop();
  await gate.finish();
  assert.equal(hub.count("events"), opened, "no stream is opened after stop()");
  assert.deepEqual(gate.seen, [500]);
});

