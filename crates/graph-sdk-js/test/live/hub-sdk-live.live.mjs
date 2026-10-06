// The `hub-sdk` LIVE row (plan Task 18): the example plugin, `subscribe` and `layout` against a real
// hub and motor, both started by `scripts/orch/hub-live.sh --motor`. Its negative control runs this
// file with GM_HUB_SDK_BREAK=1, which makes the example push a record that is not in its rows file.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { setTimeout as sleep } from "node:timers/promises";
import { createHub, InvalidCursorError } from "../../src/hub.ts";
import { startProxy } from "./proxy.mjs";
import { adminKey, createWorkspace, graphOf, hubUrl, idsOf, issues, pluginFor, workspace, writerKey } from "./hub.mjs";

const LIVE = { timeout: 60000 };
const run = promisify(execFile);
const isEvents = (req) => req.url.includes("/events");
const BUSY = { reply: { status: 200, headers: { "content-type": "text/event-stream" }, body: "event: busy\n\n" } };
const now = async () => {};

/** A workspace with a registered plugin, the cursor right after the registration, then `writes` syncs. */
async function written(prefix, writes) {
  const ws = await workspace(prefix);
  const first = issues([{ id: "1", values: { title: "one", state: "open" } }]);
  const plugin = pluginFor(first);
  await plugin.register(ws);
  const since = (await graphOf(ws)).etag;
  for (let n = 1; n <= writes; n += 1) {
    const ids = Array.from({ length: n }, (_, i) => String(i + 1));
    await plugin.sync(ws, issues(ids.map((id) => ({ id, values: { title: `t${id}`, state: "open" } }))));
  }
  return { ws, since };
}

/** Polls `holds` until it is true; a live hub has no event to await for "the subscriber got there". */
async function until(holds, what) {
  for (let i = 0; i < 400; i += 1) {
    if (holds()) return;
    await sleep(50);
  }
  assert.fail(`timed out waiting for ${what}`);
}

test("the example plugin syncs its rows file", LIVE, async () => {
  await createWorkspace("ops");
  const example = "examples/plugins/rows-file/sync.mjs";
  const env = { ...process.env, GRAPH_HUB_URL: hubUrl, GRAPH_HUB_KEY: writerKey };
  await run(process.execPath, ["--experimental-strip-types", example], { env });
  assert.deepEqual(await idsOf("ops"), ["1", "2"], "the example pushed a record that is not in the rows file");
});

test("a gap in the changes is a resync, never a skipped change", LIVE, async () => {
  const { ws, since } = await written("gap", 2);
  const proxy = await startProxy(hubUrl);
  const stops = [];
  try {
    proxy.rule((req) => req.url.includes("/changes"), {
      body: (text) => {
        const page = JSON.parse(text);
        return JSON.stringify({ ...page, changes: page.changes.slice(1) });
      },
    });
    const changes = [];
    let resynced = 0;
    const hub = createHub({ baseUrl: proxy.url, apiKey: adminKey });
    stops.push(hub.subscribe(ws, { since, onChange: (c) => changes.push(c), onResync: () => (resynced += 1), wait: now }));
    await until(() => resynced > 0, "the resync");
    assert.equal(resynced, 1);
    assert.deepEqual(changes, [], "a change after the gap was delivered");
  } finally {
    stops.forEach((stop) => stop());
    await proxy.close();
  }
});

test("a busy stream reconnects from its cursor and delivers", LIVE, async () => {
  const { ws, since } = await written("busy", 1);
  const proxy = await startProxy(hubUrl);
  const stops = [];
  try {
    proxy.rule(isEvents, BUSY);
    const changes = [];
    let resynced = 0;
    const hub = createHub({ baseUrl: proxy.url, apiKey: adminKey });
    stops.push(hub.subscribe(ws, { since, onChange: (c) => changes.push(c), onResync: () => (resynced += 1), wait: now }));
    await until(() => changes.length > 0, "a change after the busy close");
    assert.equal(resynced, 0, "a busy close was treated as a resync");
    assert.ok(proxy.seen.filter((entry) => isEvents(entry)).length >= 2, "the subscriber never reconnected");
  } finally {
    stops.forEach((stop) => stop());
    await proxy.close();
  }
});

test("subscribers sent busy together do not come back together", LIVE, async () => {
  const { ws, since } = await written("jitter", 1);
  const proxy = await startProxy(hubUrl);
  const stops = [];
  try {
    proxy.rule(isEvents, BUSY, Infinity);
    const waits = [];
    const hub = createHub({ baseUrl: proxy.url, apiKey: adminKey });
    const parked = (ms) => {
      waits.push(ms);
      return new Promise(() => {});
    };
    for (let i = 0; i < 8; i += 1) {
      stops.push(hub.subscribe(ws, { since, onChange: () => {}, onResync: () => {}, wait: parked }));
    }
    await until(() => waits.length === 8, "eight first backoffs");
    assert.ok(new Set(waits).size > 1, `eight subscribers drew one delay: ${waits}`);
  } finally {
    stops.forEach((stop) => stop());
    await proxy.close();
  }
});

test("a delivered change resets the backoff ladder", LIVE, async () => {
  const { ws, since } = await written("ladder", 1);
  const proxy = await startProxy(hubUrl);
  const stops = [];
  try {
    proxy.rule(isEvents, BUSY, 2);
    proxy.rule(isEvents, { untilChange: true });
    proxy.rule(isEvents, BUSY, Infinity);
    const waits = [];
    const counted = (ms) => {
      waits.push(ms);
      return waits.length <= 3 ? Promise.resolve() : new Promise(() => {});
    };
    const hub = createHub({ baseUrl: proxy.url, apiKey: adminKey });
    stops.push(hub.subscribe(ws, { since, onChange: () => {}, onResync: () => {}, wait: counted, random: () => 0 }));
    await until(() => waits.length === 4, "four backoffs");
    assert.deepEqual(waits, [500, 1000, 500, 1000]);
  } finally {
    stops.forEach((stop) => stop());
    await proxy.close();
  }
});

test("graph reads an ETag at 2^53 - 1 and refuses one past it", LIVE, async () => {
  const proxy = await startProxy(hubUrl);
  try {
    const at = (epoch) => (headers) => ({ ...headers, etag: headers.etag.replace(/^"\d+\./, `"${epoch}.`) });
    proxy.rule((req) => req.url.endsWith("/graph"), { headers: at("9007199254740991") });
    proxy.rule((req) => req.url.endsWith("/graph"), { headers: at("9007199254740992") });
    const hub = createHub({ baseUrl: proxy.url, apiKey: adminKey });
    const read = await hub.graph("ops");
    assert.match(read.cursor, /^9007199254740991\.\d+$/);
    await assert.rejects(hub.graph("ops"), InvalidCursorError);
  } finally {
    await proxy.close();
  }
});

test("a failed layout is reported once and never retried", LIVE, async () => {
  const proxy = await startProxy(hubUrl);
  try {
    const isLayout = (req) => req.method === "POST" && req.url.includes("/layout");
    proxy.rule(isLayout, { reply: { status: 503, headers: { "content-type": "application/json" }, body: "{}" } });
    await assert.rejects(createHub({ baseUrl: proxy.url, apiKey: adminKey }).layout("ops", { layout: "layout.grid" }));
    assert.equal(proxy.seen.filter((entry) => isLayout(entry)).length, 1, "the layout was retried");
    const snapshot = await createHub({ baseUrl: hubUrl, apiKey: adminKey }).layout("ops", { layout: "layout.grid" });
    assert.equal(snapshot.nodeCount, 2);
  } finally {
    await proxy.close();
  }
});
