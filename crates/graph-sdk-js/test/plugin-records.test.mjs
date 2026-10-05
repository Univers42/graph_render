// The records route on its own: `readStored` paging, and the `plugin_seq` rule that makes a
// paged read safe. `test/plugin-sync.test.mjs` has the plan's eleven cases, which
// `negctl-hub-sdk-ifmatch` pins by index, so this file holds the cases that are not in that list.
//
// `readStored` is the only reason this file exists: §7's "a later page with another
// `plugin_seq` starts the sync over" is a property of the read, and it is worth pinning without
// a whole `sync` in the way.

import assert from "node:assert/strict";
import { test } from "node:test";
import { hubCaller } from "../src/hub/call.ts";
import { readStored } from "../src/plugin/sync.ts";

const BASE = "http://hub.test:8081";
const WS = "ops";
const PLUGIN = "tracker";

function json(value) {
  return new Response(JSON.stringify(value), { status: 200, headers: { "Content-Type": "application/json" } });
}

/** §5.2: `{collection, id, rev}` in byte order, an opaque `next` until the last page, and
 * `plugin_seq` on every page. */
function recordsPage(ids, pluginSeq, next) {
  return {
    plugin_seq: pluginSeq,
    records: ids.map((id) => ({ collection: "issue", id, rev: 1 })),
    ...(next === undefined ? {} : { next }),
  };
}

test("plugin_read_stored_follows_next_until_it_is_absent", async () => {
  const urls = [];
  const caller = hubCaller({
    baseUrl: BASE,
    fetch: async (url) => {
      urls.push(url);
      // `next` only on the first page: the route is done when it is absent (§5.2).
      return json(urls.length === 1 ? recordsPage(["1", "2"], "1.40", "cursor-1") : recordsPage(["3"], "1.40"));
    },
  });
  const page = await readStored(caller, WS, PLUGIN, 25);
  assert.equal(urls.length, 2);
  assert.match(urls[0], /\/v1\/workspaces\/ops\/plugins\/tracker\/records\?limit=25$/);
  assert.match(urls[1], /cursor=cursor-1/, "the opaque next is followed verbatim");
  assert.deepEqual(page.keys, [
    { collection: "issue", id: "1" },
    { collection: "issue", id: "2" },
    { collection: "issue", id: "3" },
  ]);
  assert.equal(page.pluginSeq, "1.40", "the first page's plugin_seq, not the last one's");
});

test("plugin_read_stored_reads_one_page_when_there_is_no_next", async () => {
  let calls = 0;
  const caller = hubCaller({
    baseUrl: BASE,
    fetch: async () => {
      calls += 1;
      return json(recordsPage([], "1.0"));
    },
  });
  const page = await readStored(caller, WS, PLUGIN, 1000);
  assert.equal(calls, 1);
  assert.deepEqual(page.keys, []);
  assert.equal(page.pluginSeq, "1.0");
});

test("plugin_read_stored_refuses_a_page_whose_plugin_seq_moved", async () => {
  // §7: the ids a second page lists were read under a different view of this plugin's history,
  // so a diff built from them is a mix of two histories and must not be sent.
  let calls = 0;
  const caller = hubCaller({
    baseUrl: BASE,
    fetch: async () => {
      calls += 1;
      return calls === 1 ? json(recordsPage(["1"], "1.40", "cursor-1")) : json(recordsPage(["2"], "1.41"));
    },
  });
  await assert.rejects(() => readStored(caller, WS, PLUGIN, 1000), (e) => {
    assert.equal(e.name, "SyncRestart", "the restart signal, not the give-up");
    assert.match(e.message, /plugin_seq moved under the read/);
    assert.match(e.message, /1\.40 then 1\.41/);
    return true;
  });
  assert.equal(calls, 2, "the read stops at the page that disagrees");
});

test("plugin_read_stored_takes_its_page_size_and_its_plugin", async () => {
  const urls = [];
  const caller = hubCaller({
    baseUrl: BASE,
    fetch: async (url) => {
      urls.push(url);
      return json(recordsPage([], "7.3"));
    },
  });
  const page = await readStored(caller, WS, "a b/c", 10);
  // The workspace and plugin are percent-encoded, never pasted into the path raw.
  assert.equal(urls[0], `${BASE}/v1/workspaces/ops/plugins/a%20b%2Fc/records?limit=10`);
  assert.equal(page.pluginSeq, "7.3");
});
