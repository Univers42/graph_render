import assert from "node:assert/strict";
import { test } from "node:test";
import { createPlugin, SyncRestartError } from "../src/plugin.ts";

const BASE = "http://hub.test:8081";
const WS = "ops";
const PLUGIN = "tracker";
const MANIFEST = {
  version: 1,
  manifestVersion: 1,
  name: "tracker",
  collections: [
    { id: "issue", name: "Issue", titleField: "title", fields: [{ id: "title", name: "Title", role: "title", link: null }] },
  ],
};

/** `GET .../records`, §5.2: `{collection, id, rev}` in byte order, an opaque `next` until the
 * last page, and `plugin_seq` on every page. */
function recordsPage(ids, pluginSeq, next) {
  return {
    plugin_seq: pluginSeq,
    records: ids.map((id) => ({ collection: "issue", id, rev: 1 })),
    ...(next === undefined ? {} : { next }),
  };
}

function json(value, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } });
}

function kindOf(url) {
  if (url.includes("/batches")) return "batches";
  if (url.includes("/records")) return "records";
  return "manifest";
}

/** The three plugin routes over one `fetch`. `script.pages(n)` is the n-th records page,
 * `script.batches(n)` the n-th batch's answer, `script.stored` the manifest the hub keeps. */
function fakeHub(script = {}) {
  const seen = [];
  const counters = { manifest: 0, records: 0, batches: 0 };
  const fetch = async (url, init) => {
    const kind = kindOf(url);
    seen.push({ url, init, kind, n: counters[kind]++ });
    if (kind === "manifest") return json(script.stored ?? MANIFEST, script.manifestStatus ?? 201);
    if (kind === "records") return json(script.pages ? script.pages(seen.at(-1).n) : recordsPage([], "1.0"));
    return script.batches ? script.batches(seen.at(-1).n) : json({ seq: 10, applied: 1 });
  };
  return {
    seen,
    fetch,
    count: (kind) => seen.filter((call) => call.kind === kind).length,
    of: (kind) => seen.filter((call) => call.kind === kind),
    headersOf: (kind, name) => seen.filter((call) => call.kind === kind).map((call) => call.init.headers[name]),
    bodiesOf: (kind) => seen.filter((call) => call.kind === kind).map((call) => JSON.parse(call.init.body)),
  };
}

/** A plugin over a fake hub, with a counting key generator so "one key per batch" is checkable. */
function pluginOver(hub, extra = {}) {
  let minted = 0;
  const keys = [];
  const plugin = createPlugin({
    baseUrl: BASE,
    plugin: PLUGIN,
    manifest: MANIFEST,
    fetch: hub.fetch,
    key: () => {
      minted += 1;
      const key = `key-${minted}`;
      keys.push(key);
      return key;
    },
    ...extra,
  });
  return { plugin, keys, hub };
}

function ingestOf(records) {
  return { version: 1, source: WS, collections: [], records };
}

function record(id, extra = {}) {
  return { collection: "issue", id, deleted: false, updatedAt: 1, values: { title: `t${id}` }, ...extra };
}

function opsOf(body) {
  return body.upserts.length + body.deletes.length;
}

// The order is the plan's list. `negctl-hub-sdk-ifmatch` in `scripts/orch/rows/hub-sdk.rows`
// greps `not ok 5 - plugin_sync_if_match_is_per_plugin`, and node's tap reporter numbers tests
// from 1 in declaration order, so the fifth name here is that one.

test("plugin_sync_upserts_every_desired_record_and_deletes_the_rest", async () => {
  const hub = fakeHub({ pages: () => recordsPage(["1", "2", "3"], "1.0") });
  const { plugin } = pluginOver(hub);
  const answers = await plugin.sync(WS, ingestOf([record("1"), record("2")]));
  assert.equal(hub.count("batches"), 1);
  const [body] = hub.bodiesOf("batches");
  assert.deepEqual(body.upserts.map((up) => up.id), ["1", "2"]);
  assert.deepEqual(body.deletes, [{ collection: "issue", id: "3" }]);
  assert.deepEqual(answers, [{ seq: 10, applied: 1 }]);
});

test("plugin_sync_deletes_a_deleted_row_when_stored_and_drops_it_otherwise", async () => {
  // §7 and N14: the rows file keeps the row, so this is the only place that decides.
  const stored = fakeHub({ pages: () => recordsPage(["1", "2"], "1.0") });
  await pluginOver(stored).plugin.sync(WS, ingestOf([record("1"), record("2", { deleted: true })]));
  const withDelete = stored.bodiesOf("batches")[0];
  assert.deepEqual(withDelete.upserts.map((up) => up.id), ["1"]);
  assert.deepEqual(withDelete.deletes, [{ collection: "issue", id: "2" }], "stored: deleted");

  const absent = fakeHub({ pages: () => recordsPage([], "1.0") });
  await pluginOver(absent).plugin.sync(WS, ingestOf([record("1"), record("2", { deleted: true })]));
  const without = absent.bodiesOf("batches")[0];
  assert.deepEqual(without.upserts.map((up) => up.id), ["1"]);
  assert.deepEqual(without.deletes, [], "not stored: nowhere");
  // §5.2 refuses 422 a batch naming one id in both lists, so never both.
  const upserted = new Set(without.upserts.map((up) => up.id));
  for (const del of without.deletes) assert.equal(upserted.has(del.id), false, `${del.id} twice`);
});

test("plugin_sync_batches_at_max_batch_with_its_own_key", async () => {
  const records = Array.from({ length: 10001 }, (_, i) => record(String(i)));
  const hub = fakeHub({ pages: () => recordsPage([], "1.0"), batches: (n) => json({ seq: 10 + n, applied: 1 }) });
  const { plugin, keys } = pluginOver(hub);
  const answers = await plugin.sync(WS, ingestOf(records));
  assert.equal(hub.count("batches"), 2, "10001 ops is two batches at GRAPH_HUB_MAX_BATCH");
  assert.deepEqual(hub.bodiesOf("batches").map(opsOf), [10000, 1]);
  const sent = hub.headersOf("batches", "Idempotency-Key");
  assert.equal(new Set(sent).size, 2, "one key per batch, so a crash between them loses neither");
  assert.deepEqual(sent, keys);
  assert.equal(answers.length, 2);
});

test("plugin_sync_sends_if_match_with_the_first_pages_plugin_seq", async () => {
  const hub = fakeHub({ pages: () => recordsPage([], "1.40") });
  await pluginOver(hub).plugin.sync(WS, ingestOf([record("1")]));
  assert.equal(hub.headersOf("batches", "If-Match")[0], '"1.40"');
  // §5.2: the header is the cursor, quoted the way `If-Match` quotes an entity tag.
  assert.match(hub.headersOf("batches", "If-Match")[0], /^"\d+\.\d+"$/);
});

test("plugin_sync_if_match_is_per_plugin", async () => {
  // Review Focus 5: `If-Match` on the workspace `head_seq` would make every other plugin's
  // write a 412 and starve a workspace with more than one writer. Another plugin moved
  // `head_seq` to 99 while this plugin's own `plugin_seq` is 40.
  const hub = fakeHub({ pages: () => ({ ...recordsPage([], "1.40"), head_seq: 99 }) });
  const { plugin } = pluginOver(hub);
  const answers = await plugin.sync(WS, ingestOf([record("1")]));
  const ifMatch = hub.headersOf("batches", "If-Match")[0];
  assert.equal(ifMatch, '"1.40"', "the plugin's own seq, not the workspace head");
  assert.equal(ifMatch.includes("99"), false);
  // No 412, so no restart: one records read and one batch.
  assert.equal(hub.count("records"), 1);
  assert.equal(hub.count("batches"), 1);
  assert.deepEqual(answers, [{ seq: 10, applied: 1 }]);
});

test("plugin_sync_advances_the_expected_seq_to_the_batch_seq", async () => {
  // §7: after each batch with `applied > 0` the expected `plugin_seq` becomes that batch's seq.
  const hub = fakeHub({ pages: () => recordsPage([], "1.40"), batches: (n) => json({ seq: 43, applied: n === 0 ? 3 : 0 }) });
  await pluginOver(hub, { maxBatch: 1 }).plugin.sync(WS, ingestOf([record("1"), record("2")]));
  assert.deepEqual(hub.headersOf("batches", "If-Match"), ['"1.40"', '"1.43"']);
});

test("plugin_sync_keeps_the_expected_seq_after_a_batch_that_applied_nothing", async () => {
  const hub = fakeHub({ pages: () => recordsPage([], "1.40"), batches: () => json({ seq: 99, applied: 0 }) });
  await pluginOver(hub, { maxBatch: 1 }).plugin.sync(WS, ingestOf([record("1"), record("2")]));
  assert.deepEqual(hub.headersOf("batches", "If-Match"), ['"1.40"', '"1.40"']);
});

test("plugin_sync_restarts_on_412", async () => {
  // Two concurrent syncs of one plugin: the loser gets a 412 and starts over (§7).
  const hub = fakeHub({
    pages: (n) => recordsPage([], n === 0 ? "1.40" : "1.44"),
    batches: (n) => (n === 0 ? json({ error: "conflict", message: "plugin_seq moved" }, 412) : json({ seq: 12, applied: 1 })),
  });
  const answers = await pluginOver(hub).plugin.sync(WS, ingestOf([record("1")]));
  assert.deepEqual(answers, [{ seq: 12, applied: 1 }], "the restarted run completes");
  assert.equal(hub.count("records"), 2, "the restart re-reads the records route");
  assert.deepEqual(hub.headersOf("batches", "If-Match"), ['"1.40"', '"1.44"'], "the retry reads the new seq");
});

test("plugin_sync_restarts_when_a_later_page_carries_another_plugin_seq", async () => {
  // §7: a later records page with another `plugin_seq` starts the sync over, because the ids it
  // listed were read under a different view of this plugin's history.
  const hub = fakeHub({
    pages: (n) => (n === 0 ? recordsPage(["1"], "1.40", "cursor-1") : recordsPage(n === 1 ? ["2"] : [], "1.41")),
  });
  const answers = await pluginOver(hub).plugin.sync(WS, ingestOf([record("1")]));
  assert.equal(hub.count("records"), 3, "two pages, then one page for the restarted run");
  assert.equal(hub.headersOf("batches", "If-Match")[0], '"1.41"', "the restarted run uses the new seq");
  assert.deepEqual(answers, [{ seq: 10, applied: 1 }]);
});

test("plugin_sync_gives_up_after_three_restarts", async () => {
  let calls = 0;
  const hub = fakeHub({
    pages: () => recordsPage([], "1.40"),
    batches: () => {
      calls += 1;
      return json({ error: "conflict", message: "plugin_seq moved" }, 412);
    },
  });
  const plugin = createPlugin({ baseUrl: BASE, plugin: PLUGIN, manifest: MANIFEST, fetch: hub.fetch, key: () => "k", wait: async () => {} });
  await assert.rejects(() => plugin.sync(WS, ingestOf([record("1")])), (e) => {
    assert.ok(e instanceof SyncRestartError, String(e));
    assert.equal(e.name, "SyncRestartError");
    return true;
  });
  // Plan Decision 5: bounded at 3, so one try and three restarts, never an unbounded hot loop.
  assert.equal(calls, 4);
});

test("plugin_sync_honours_its_own_restart_bound", async () => {
  let calls = 0;
  const hub = fakeHub({
    pages: () => recordsPage([], "1.40"),
    batches: () => {
      calls += 1;
      return json({}, 412);
    },
  });
  const plugin = createPlugin({ baseUrl: BASE, plugin: PLUGIN, manifest: MANIFEST, fetch: hub.fetch, key: () => "k", wait: async () => {}, restarts: 1 });
  await assert.rejects(() => plugin.sync(WS, ingestOf([record("1")])), SyncRestartError);
  assert.equal(calls, 2, "one try and one restart");
});

test("plugin_sync_sends_no_deletes_when_nothing_is_stored", async () => {
  const hub = fakeHub({ pages: () => recordsPage([], "1.0") });
  await pluginOver(hub).plugin.sync(WS, ingestOf([record("1")]));
  assert.deepEqual(hub.bodiesOf("batches")[0].deletes, []);
});

test("plugin_sync_sends_no_batch_when_nothing_is_desired_and_nothing_is_stored", async () => {
  const hub = fakeHub({ pages: () => recordsPage([], "1.0") });
  const answers = await pluginOver(hub).plugin.sync(WS, ingestOf([]));
  assert.deepEqual(answers, []);
  assert.equal(hub.count("batches"), 0, "an empty batch is not a write");
});

test("plugin_register_puts_the_manifest_and_returns_the_stored_one", async () => {
  const stored = { ...MANIFEST, manifestVersion: 7 };
  const hub = fakeHub({ stored });
  assert.deepEqual(await pluginOver(hub).plugin.register(WS), stored);
  const [call] = hub.of("manifest");
  assert.equal(call.url, `${BASE}/v1/workspaces/ops/plugins/tracker`);
  assert.equal(call.init.method, "PUT");
  assert.equal(call.init.headers["Content-Type"], "application/json");
  assert.deepEqual(JSON.parse(call.init.body), MANIFEST);
});

test("plugin_register_accepts_a_200_for_an_existing_plugin", async () => {
  const hub = fakeHub({ manifestStatus: 200 });
  assert.deepEqual(await pluginOver(hub).plugin.register(WS), MANIFEST);
});

test("plugin_push_delegates_to_push_once", async () => {
  const hub = fakeHub();
  const { plugin } = pluginOver(hub);
  const batch = { deletes: [], upserts: [{ collection: "issue", id: "1", updatedAt: 1, values: {} }] };
  assert.deepEqual(await plugin.push(WS, batch), { seq: 10, applied: 1 });
  assert.equal(hub.headersOf("batches", "Idempotency-Key")[0], "key-1");
});

test("plugin_refuses_a_key_in_a_browser_before_any_request", () => {
  // `createPlugin` builds its caller first, so `remoteConfigOf`'s rule stands.
  globalThis.window = {};
  try {
    assert.throws(
      () => createPlugin({ baseUrl: BASE, apiKey: "hub-key-1", plugin: PLUGIN, manifest: MANIFEST, fetch: async () => json({}) }),
      /browser|apiKey/,
    );
  } finally {
    delete globalThis.window;
  }
});
