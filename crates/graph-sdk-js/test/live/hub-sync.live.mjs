// The `hub-sync` LIVE row (plan Task 18): `sync` against a real hub, under the writer key
// `scripts/orch/hub-live.sh` minted with `* write:rows-file` only. Its negative control runs this
// file with GM_HUB_SDK_BREAK=sync-via-graph and expects test 2, by name, to fail.
import { test } from "node:test";
import assert from "node:assert/strict";
import { startProxy } from "./proxy.mjs";
import { changesOf, graphOf, hubUrl, idsOf, issues, pluginFor, workspace } from "./hub.mjs";

const LIVE = { timeout: 60000 };
const row = (id, title, state = "open") => ({ id, values: { title, state } });
const gone = (id) => ({ id, deleted: true, values: { title: `gone ${id}`, state: "closed" } });
const isBatchPost = (req) => req.method === "POST" && req.url.includes("/batches");

test("a writer key registers and syncs the rows file", LIVE, async () => {
  const ws = await workspace("sync");
  const ingest = issues([row("1", "one"), row("2", "two"), gone("3")]);
  const plugin = pluginFor(ingest);
  await plugin.register(ws);
  await plugin.sync(ws, ingest);
  assert.deepEqual(await idsOf(ws), ["1", "2"]);
});

// The stored read must be the plugin's own records, not `/graph`: `/graph` shows what the document
// materialized, and a link to a record nobody wrote is not what the plugin pushed.
test("a dangling link does not move the seq on a second sync", LIVE, async () => {
  const ws = await workspace("dangling");
  const blocks = { name: "blocks", label: "Blocks", role: "link", link: { collection: "issue", cardinality: "one" } };
  const ingest = issues([{ id: "1", values: { title: "one", state: "open", blocks: "404" } }, row("2", "two")], [blocks]);
  const plugin = pluginFor(ingest);
  await plugin.register(ws);
  await plugin.sync(ws, ingest);
  const first = (await graphOf(ws)).etag;
  await plugin.sync(ws, ingest);
  assert.equal((await graphOf(ws)).etag, first, "the second sync of an unchanged rows file wrote a change");
});

test("a batch the hub committed but whose answer was lost is retried under the same key", LIVE, async () => {
  const ws = await workspace("drop");
  const proxy = await startProxy(hubUrl);
  try {
    proxy.rule(isBatchPost, { drop: true });
    const ingest = issues([row("1", "one"), row("2", "two")]);
    const plugin = pluginFor(ingest, { baseUrl: proxy.url, wait: async () => {} });
    await plugin.register(ws);
    await plugin.sync(ws, ingest);
    const posts = proxy.seen.filter((entry) => entry.method === "POST" && entry.url.includes("/batches"));
    assert.ok(posts.length >= 2, `expected a retry, saw ${posts.length} batch posts`);
    assert.ok(posts[0].key !== null && posts[0].key === posts[1].key, "the retry carried a new idempotency key");
    const batches = (await changesOf(ws)).filter((change) => change.kind === "batch");
    assert.equal(batches.length, 1, "the retried batch was applied twice");
    assert.deepEqual(await idsOf(ws), ["1", "2"]);
  } finally {
    await proxy.close();
  }
});

test("a row deleted from the file is deleted from the hub, and one never written is not sent", LIVE, async () => {
  const ws = await workspace("delete");
  const before = issues([row("1", "one"), row("2", "two"), row("3", "three")]);
  const plugin = pluginFor(before);
  await plugin.register(ws);
  await plugin.sync(ws, before);
  const after = issues([row("1", "one"), row("2", "two"), gone("3"), gone("4")]);
  await plugin.sync(ws, after);
  const batches = (await changesOf(ws)).filter((change) => change.kind === "batch");
  const deleted = (batches.at(-1).deletes ?? []).map((entry) => entry.id);
  assert.ok(deleted.includes("3"), `the last batch deleted ${JSON.stringify(deleted)}, not 3`);
  assert.ok(!deleted.includes("4"), "a delete was sent for a record the hub never had");
  assert.deepEqual(await idsOf(ws), ["1", "2"]);
});

// A is held at its first batch post while B writes the same plugin; A's If-Match is then stale,
// so the hub answers 412 and A must read the stored records again and converge on its own file.
test("two writers of one plugin converge on the one that synced last", LIVE, async () => {
  const ws = await workspace("race");
  const proxy = await startProxy(hubUrl);
  try {
    let reached;
    let release;
    const atPost = new Promise((resolve) => (reached = resolve));
    const held = new Promise((resolve) => (release = resolve));
    proxy.rule(isBatchPost, {
      before: async () => {
        reached();
        await held;
      },
    });
    const fileA = issues([row("1", "A1"), row("2", "A2")]);
    const fileB = issues([row("2", "B2"), row("3", "B3")]);
    const writerA = pluginFor(fileA, { baseUrl: proxy.url, wait: async () => {} });
    await writerA.register(ws);
    const syncA = writerA.sync(ws, fileA);
    await atPost;
    await pluginFor(fileB).sync(ws, fileB);
    release();
    await syncA;
    assert.ok(
      proxy.seen.some((entry) => entry.status === 412),
      "A's stale batch was not refused",
    );
    const { doc } = await graphOf(ws);
    assert.deepEqual(doc.records.map((record) => record.id).sort(), ["1", "2"]);
    const two = JSON.stringify(doc.records.find((record) => record.id === "2").values);
    assert.ok(two.includes("A2") && !two.includes("B2"), `record 2 is ${two}`);
  } finally {
    await proxy.close();
  }
});
