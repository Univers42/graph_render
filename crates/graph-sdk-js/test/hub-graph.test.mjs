import assert from "node:assert/strict";
import { test } from "node:test";
import { createHub } from "../src/hub.ts";
import { InvalidCursorError } from "../src/hub/cursor.ts";
import { InvalidOptionsError, GraphMotorError } from "../src/errors.ts";
import { RemoteError } from "../src/remote/errors.ts";
import { SNAPSHOT_MEDIA_TYPE } from "../src/remote.ts";
import { ColumnId } from "../src/types.ts";

const BASE = "http://hub.test:8081";
const WS = "ops";
const NEVER = () => assert.fail("the path under test must not reach this");

// The contract's own pinned 84-byte example, from `test/snapshot.test.mjs:14-21`, so `layout`
// here decodes a real snapshot instead of asserting that some bytes were passed along.
const hex = (text) => Uint8Array.from(text.trim().split(/\s+/), (b) => Number.parseInt(b, 16));
const PINNED = Uint8Array.of(
  ...hex(`
    47 4D 53 4E  00 00 00 00  03 00 00 00  00 00 00 00  01 00 00 00  02 00 00 00  01 00 00 00
    00 00 00 00  01 00 00 00  03 00 00 00  61 62 63 00
    00 00 00 00  01 00 00 00  65 00 00 00
    00 00 00 00  01 00 00 00
    00 00 80 3F  00 00 20 C0  00 00 00 00  00 00 00 3F`),
  ...hex("00 00 00 00"),
);
const SNAPSHOT_BODY = new TextEncoder().encode(JSON.stringify({ version: 1, source: "ops" }));

function recorder(reply) {
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    return typeof reply === "function" ? reply(url, init, seen.length) : reply;
  };
  return { fetch, seen };
}

function hubOver(reply, apiKey) {
  const rec = recorder(reply);
  return { hub: createHub({ baseUrl: BASE, ...(apiKey === undefined ? {} : { apiKey }), fetch: rec.fetch }), ...rec };
}

test("hub_graph_returns_the_etag_cursor_and_body", async () => {
  const { hub, seen } = hubOver(
    new Response(SNAPSHOT_BODY, { status: 200, headers: { ETag: '"1789000000000000.118"' } }),
  );
  assert.deepEqual(await hub.graph(WS), { cursor: "1789000000000000.118", ingest: '{"version":1,"source":"ops"}' });
  assert.equal(seen[0].url, `${BASE}/v1/workspaces/ops/graph`);
  assert.equal(seen[0].init.method, "GET");
});

test("hub_graph_sends_no_conditional_header_of_its_own", async () => {
  // `If-None-Match` is the caller's business: `graph` has none, so a 304 can only be something
  // the hub chose to send, and it is refused rather than answered as a success.
  const { hub, seen } = hubOver(new Response(SNAPSHOT_BODY, { status: 200, headers: { ETag: '"1.0"' } }));
  await hub.graph(WS);
  assert.equal(seen[0].init.headers["If-None-Match"], undefined);
});

test("hub_graph_refuses_a_304", async () => {
  const { hub } = hubOver(new Response(null, { status: 304 }));
  await assert.rejects(() => hub.graph(WS), (e) => {
    assert.ok(e instanceof RemoteError, String(e));
    assert.equal(e.status, 304, "not answered as a success and not folded into 200");
    assert.match(e.message, /\/v1\/workspaces\/ops\/graph/);
    return true;
  });
});

test("hub_graph_refuses_a_missing_etag", async () => {
  const { hub } = hubOver(new Response(SNAPSHOT_BODY, { status: 200 }));
  await assert.rejects(() => hub.graph(WS), (e) => e instanceof RemoteError && /no ETag/.test(e.message));
});

test("hub_graph_refuses_a_malformed_etag", async () => {
  // §5.2 says the hub always sends a cursor, so a malformed one is a hub this client cannot
  // follow. It is refused by `parseCursor`, which is a `GraphMotorError` like every other
  // refusal in this package.
  const { hub } = hubOver(new Response(SNAPSHOT_BODY, { status: 200, headers: { ETag: '"not-a-cursor"' } }));
  await assert.rejects(() => hub.graph(WS), (e) => e instanceof InvalidCursorError && e instanceof GraphMotorError);
});

test("hub_graph_refuses_an_etag_at_two_to_the_53", async () => {
  const { hub } = hubOver(new Response(SNAPSHOT_BODY, { status: 200, headers: { ETag: '"9007199254740992.1"' } }));
  await assert.rejects(() => hub.graph(WS), InvalidCursorError);
});

test("hub_graph_turns_a_hub_refusal_into_a_typed_error", async () => {
  const body = JSON.stringify({ error: "not_found", message: "no such workspace" });
  const { hub } = hubOver(new Response(body, { status: 404, headers: { "Content-Type": "application/json" } }));
  await assert.rejects(() => hub.graph(WS), (e) => e instanceof RemoteError && e.status === 404);
});

test("hub_layout_posts_the_layout_query_and_decodes_the_snapshot", async () => {
  const { hub, seen } = hubOver(new Response(PINNED, { status: 200 }));
  const snapshot = await hub.layout(WS, { layout: "layout.packing.circle", post: "post.style.bezier" });
  assert.equal(seen[0].url, `${BASE}/v1/workspaces/ops/layout?layout=layout.packing.circle&post=post.style.bezier`);
  assert.equal(seen[0].init.method, "POST");
  assert.equal(seen[0].init.headers["Accept"], SNAPSHOT_MEDIA_TYPE);
  assert.deepEqual([snapshot.nodeIds, snapshot.edgeIds], [["a", "bc"], ["e"]]);
  assert.deepEqual(Array.from(snapshot.column(ColumnId.EdgeSource)), [0]);
});

test("hub_layout_is_never_retried", async () => {
  // §5.3 and §7: the SDK never retries `/layout`. A failed layout reaches the caller as a
  // `RemoteError` and it is the caller's decision whether to try again.
  let calls = 0;
  const { hub } = hubOver(() => {
    calls += 1;
    return new Response("{}", { status: 503, headers: { "Content-Type": "application/json" } });
  });
  await assert.rejects(() => hub.layout(WS, { layout: "layout.tree.tidy" }), (e) => e instanceof RemoteError && e.status === 503);
  assert.equal(calls, 1, "exactly one request, no retry");
});

test("hub_layout_is_never_retried_on_a_transport_error", async () => {
  let calls = 0;
  const { hub } = hubOver(() => {
    calls += 1;
    throw new TypeError("fetch failed");
  });
  await assert.rejects(() => hub.layout(WS, { layout: "layout.tree.tidy" }), (e) => e instanceof RemoteError && e.status === 0);
  assert.equal(calls, 1, "exactly one request, no retry");
});

test("hub_layout_refuses_a_request_it_does_not_understand", async () => {
  // `layoutQueryOf`'s closed shape, reused rather than re-spelled.
  const { hub } = hubOver(new Response(PINNED, { status: 200 }));
  await assert.rejects(() => hub.layout(WS, { layout: "layout.tree.tidy", posts: [] }), InvalidOptionsError);
});

test("hub_layout_turns_an_unknown_layout_into_a_typed_refusal", async () => {
  // The route's own 400, the way `test/remote.test.mjs:82-95` sees the real motor's: a bad
  // layout id is a refusal from the service, and its class reaches the caller typed.
  const body = JSON.stringify({ error: "invalid", message: "unknown layout id" });
  const { hub } = hubOver(new Response(body, { status: 400, headers: { "Content-Type": "application/json" } }));
  await assert.rejects(() => hub.layout(WS, { layout: "layout.does-not-exist" }), (e) => {
    assert.ok(e instanceof RemoteError, String(e));
    assert.equal(e.status, 400);
    assert.match(e.message, /unknown layout id/);
    return true;
  });
});

test("hub_subscribe_is_the_subscribe_module_over_the_same_caller", async () => {
  // One `HubCaller` for all three routes: a bad option is refused by `createHub` itself.
  assert.throws(() => createHub({ baseUrl: "not-a-url", fetch: async () => new Response("{}") }), InvalidOptionsError);
  assert.throws(() => createHub({ baseUrl: BASE, apiKey: "not a token", fetch: async () => new Response("{}") }), InvalidOptionsError);
});

test("hub_subscribe_runs_the_loop_over_the_events_route", async () => {
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    if (url.includes("/events")) {
      return new Response("event: resync\ndata: {}\n\n", { status: 200, headers: { "Content-Type": "text/event-stream" } });
    }
    return NEVER();
  };
  const hub = createHub({ baseUrl: BASE, fetch });
  let resyncs = 0;
  const stop = hub.subscribe(WS, { onChange: NEVER, onResync: () => (resyncs += 1), wait: NEVER });
  for (let i = 0; i < 8 && resyncs === 0; i += 1) {
    await new Promise((resolve) => globalThis.setTimeout(resolve, 0));
  }
  stop();
  assert.equal(resyncs, 1);
  assert.equal(seen[0].url, `${BASE}/v1/workspaces/ops/events`);
});
