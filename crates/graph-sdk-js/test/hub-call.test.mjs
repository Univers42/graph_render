import assert from "node:assert/strict";
import { test } from "node:test";
import { InvalidOptionsError } from "../src/errors.ts";
import { hubCaller, textOf } from "../src/hub/call.ts";
import { RemoteError } from "../src/remote/errors.ts";

const BASE = "http://hub.test:8081";
const KEY = "hub-key-1";

/** An in-process `fetch` that records every request and answers `reply`. */
function fakeFetch(reply) {
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    return typeof reply === "function" ? reply(url, init) : reply;
  };
  return { fetch, seen };
}

function json(body, status = 200, headers = {}) {
  return new Response(body, { status, headers: { "Content-Type": "application/json", ...headers } });
}

test("hub_call_sends_the_key_as_a_bearer_token", async () => {
  const { fetch, seen } = fakeFetch(json("{}"));
  await hubCaller({ baseUrl: BASE, apiKey: KEY, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(seen[0].init.headers["Authorization"], `Bearer ${KEY}`);
});

test("hub_call_sends_no_authorization_without_a_key", async () => {
  const { fetch, seen } = fakeFetch(json("{}"));
  await hubCaller({ baseUrl: BASE, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal("Authorization" in seen[0].init.headers, false);
});

test("hub_call_refuses_a_redirect_on_every_route", async () => {
  // `src/remote.ts:69`'s rule: a 302 to another host would carry the Authorization header
  // somewhere the key was never meant to go.
  const { fetch, seen } = fakeFetch(json("{}"));
  const caller = hubCaller({ baseUrl: BASE, apiKey: KEY, fetch });
  await caller.call({ method: "GET", route: "/v1/a" });
  await caller.call({ method: "PUT", route: "/v1/b", body: "{}" });
  for (const call of seen) assert.equal(call.init.redirect, "error");
});

test("hub_call_builds_the_url_from_the_base_and_the_route", async () => {
  const { fetch, seen } = fakeFetch(json("{}"));
  await hubCaller({ baseUrl: `${BASE}/prefix/`, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(seen[0].url, `${BASE}/prefix/v1/x`);
});

test("hub_call_answers_a_200_with_the_body_still_unread", async () => {
  const { fetch } = fakeFetch(json("hello"));
  const answer = await hubCaller({ baseUrl: BASE, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(answer.status, 200);
  // The SSE route reads the body itself, so `call` must not have drained it.
  assert.equal(await textOf(answer), "hello");
});

test("hub_call_lowercases_the_answer_headers", async () => {
  const { fetch } = fakeFetch(json("{}", 200, { ETag: '"1.5"', "Content-Type": "application/json" }));
  const answer = await hubCaller({ baseUrl: BASE, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(answer.headers["etag"], '"1.5"');
  assert.equal("ETag" in answer.headers, false);
});

test("hub_call_sets_content_type_only_when_there_is_a_body", async () => {
  const { fetch, seen } = fakeFetch(json("{}"));
  const caller = hubCaller({ baseUrl: BASE, fetch });
  await caller.call({ method: "GET", route: "/v1/a" });
  await caller.call({ method: "POST", route: "/v1/b", body: "{}" });
  assert.equal("Content-Type" in seen[0].init.headers, false);
  assert.equal(seen[1].init.headers["Content-Type"], "application/json");
});

test("hub_call_sets_accept_only_when_asked", async () => {
  const { fetch, seen } = fakeFetch(json("{}"));
  const caller = hubCaller({ baseUrl: BASE, fetch });
  await caller.call({ method: "GET", route: "/v1/a" });
  await caller.call({ method: "GET", route: "/v1/b", accept: "text/event-stream" });
  assert.equal("Accept" in seen[0].init.headers, false);
  assert.equal(seen[1].init.headers["Accept"], "text/event-stream");
});

test("hub_call_json_turns_a_400_body_into_a_typed_refusal", async () => {
  const body = JSON.stringify({ error: "invalid", message: 'collection "issue" is not declared' });
  const { fetch } = fakeFetch(json(body, 400));
  const caller = hubCaller({ baseUrl: BASE, apiKey: KEY, fetch });
  await assert.rejects(
    () => caller.callJson({ method: "POST", route: "/v1/x/batches", body: "{}" }),
    (e) => {
      assert.ok(e instanceof RemoteError, String(e));
      assert.equal(e.status, 400);
      assert.match(e.message, /\/v1\/x\/batches/, "the refusal names the path");
      assert.match(e.message, /collection "issue" is not declared/);
      return true;
    },
  );
});

test("hub_call_json_carries_retry_after_into_the_refusal", async () => {
  const { fetch } = fakeFetch(json("{}", 429, { "Retry-After": "2" }));
  await assert.rejects(
    () => hubCaller({ baseUrl: BASE, fetch }).callJson({ method: "POST", route: "/v1/x", body: "{}" }),
    (e) => e.retryAfter === 2,
  );
});

test("hub_call_json_answers_a_2xx_with_the_body_still_unread", async () => {
  const { fetch } = fakeFetch(json('{"seq":5,"applied":2}'));
  const answer = await hubCaller({ baseUrl: BASE, fetch }).callJson({ method: "POST", route: "/v1/x", body: "{}" });
  assert.deepEqual(JSON.parse(await textOf(answer)), { seq: 5, applied: 2 });
});

test("hub_call_json_defaults_accept_to_json", async () => {
  // `src/hub/call.ts` declares its own `JSON_MEDIA_TYPE` (plan Decision 2): `call` is the raw
  // route the SSE reader uses, `callJson` is the one that is always asking for a document.
  const { fetch, seen } = fakeFetch(json("{}"));
  const caller = hubCaller({ baseUrl: BASE, fetch });
  await caller.callJson({ method: "POST", route: "/v1/a", body: "{}" });
  await caller.callJson({ method: "POST", route: "/v1/b", body: "{}", accept: "application/x-thing" });
  assert.equal(seen[0].init.headers["Accept"], "application/json");
  assert.equal(seen[1].init.headers["Accept"], "application/x-thing");
});

test("hub_call_turns_a_rejected_fetch_into_a_status_0_refusal", async () => {
  const fetch = async () => {
    throw new TypeError("fetch failed");
  };
  await assert.rejects(
    () => hubCaller({ baseUrl: BASE, fetch }).callJson({ method: "GET", route: "/v1/x" }),
    (e) => {
      assert.ok(e instanceof RemoteError, String(e));
      assert.equal(e.status, 0);
      assert.match(e.message, /\/v1\/x/);
      return true;
    },
  );
});

test("hub_call_refuses_a_key_in_a_browser", () => {
  // `remoteConfigOf`'s own rule (`src/remote/options.ts:85-90`), reused rather than re-spelled.
  const fetch = async () => json("{}");
  globalThis.window = {};
  try {
    assert.throws(() => hubCaller({ baseUrl: BASE, apiKey: KEY, fetch }), InvalidOptionsError);
    // The documented escape hatch: the key then ships to every visitor, which is the caller's call.
    assert.equal(hubCaller({ baseUrl: BASE, apiKey: KEY, dangerouslyAllowBrowser: true, fetch }).key, KEY);
    // No key, no refusal: a browser may read a hub started with the auth off.
    assert.equal(hubCaller({ baseUrl: BASE, fetch }).key, undefined);
  } finally {
    delete globalThis.window;
  }
});

test("hub_call_keeps_the_key_out_of_every_refusal", async () => {
  const { fetch } = fakeFetch(json(JSON.stringify({ error: "unauthorized", message: "no grant" }), 403));
  await assert.rejects(
    () => hubCaller({ baseUrl: BASE, apiKey: KEY, fetch }).callJson({ method: "GET", route: "/v1/x" }),
    (e) => !e.message.includes(KEY) && !String(e.stack).includes(KEY),
  );
});

test("hub_call_text_decodes_utf8", async () => {
  const { fetch } = fakeFetch(new Response(new TextEncoder().encode("héllo — ✓\n"), { status: 200 }));
  const answer = await hubCaller({ baseUrl: BASE, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(await textOf(answer), "héllo — ✓\n");
});

test("hub_call_text_is_empty_without_a_body", async () => {
  const { fetch } = fakeFetch(new Response(null, { status: 204 }));
  const answer = await hubCaller({ baseUrl: BASE, fetch }).call({ method: "GET", route: "/v1/x" });
  assert.equal(await textOf(answer), "");
});
