import assert from "node:assert/strict";
import { test } from "node:test";
import { PUSH_ATTEMPTS, isRetryable, pushOnce, retryDelayMs } from "../src/plugin/push.ts";
import { hubCaller } from "../src/hub/call.ts";
import { RemoteError } from "../src/remote/errors.ts";

const BASE = "http://hub.test:8081";
const WS = "ops";
const PLUGIN = "tracker";
const BATCH = { deletes: [{ collection: "issue", id: "41" }], upserts: [{ collection: "issue", id: "42", updatedAt: 7, values: { title: "Crash" } }] };

/** The batches route, answered from a script of `Response`s. A constant `Response` is cloned
 * per request, because a real hub sends a fresh one and `refusalOf` consumes the body it is
 * given — handing the same instance to a second attempt would fail on a disturbed body. */
function fakeHub(reply) {
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    return typeof reply === "function" ? reply(seen.length) : reply.clone();
  };
  return { caller: hubCaller({ baseUrl: BASE, fetch }), seen };
}

function answer(seq = 5, applied = 2) {
  return new Response(JSON.stringify({ seq, applied }), { status: 200, headers: { "Content-Type": "application/json" } });
}

function refusal(status, headers = {}) {
  return new Response(JSON.stringify({ error: "refused", message: "no" }), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });
}

const never = () => assert.fail("no wait was expected");
const ok = { wait: async () => {} };

test("plugin_push_answers_a_200_with_seq_and_applied", async () => {
  const hub = fakeHub(answer(9, 2));
  assert.deepEqual(await pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), { seq: 9, applied: 2 });
  assert.equal(hub.seen[0].url, `${BASE}/v1/workspaces/ops/plugins/tracker/batches`);
  assert.equal(hub.seen[0].init.method, "POST");
});

test("plugin_push_retries_429", async () => {
  const hub = fakeHub((n) => (n === 1 ? refusal(429) : answer()));
  assert.deepEqual(await pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), { seq: 5, applied: 2 });
  assert.equal(hub.seen.length, 2);
});

test("plugin_push_retries_503", async () => {
  const hub = fakeHub((n) => (n === 1 ? refusal(503) : answer()));
  assert.deepEqual(await pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), { seq: 5, applied: 2 });
  assert.equal(hub.seen.length, 2);
});

test("plugin_push_retries_a_transport_error", async () => {
  let n = 0;
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    n += 1;
    if (n === 1) throw new TypeError("fetch failed");
    return answer();
  };
  const caller = hubCaller({ baseUrl: BASE, fetch });
  assert.deepEqual(await pushOnce(caller, WS, PLUGIN, BATCH, ok), { seq: 5, applied: 2 });
  assert.equal(seen.length, 2);
});

test("plugin_push_gives_up_after_four_attempts", async () => {
  const hub = fakeHub(refusal(503));
  await assert.rejects(() => pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), (e) => {
    assert.ok(e instanceof RemoteError, String(e));
    assert.equal(e.status, 503, "the last refusal, not a wrapped one");
    return true;
  });
  assert.equal(hub.seen.length, PUSH_ATTEMPTS);
  assert.equal(PUSH_ATTEMPTS, 4, "one try and three retries, §7");
});

test("plugin_push_honours_retry_after_over_the_default_delay", async () => {
  const waited = [];
  const hub = fakeHub((n) => (n === 1 ? refusal(429, { "Retry-After": "2" }) : answer()));
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, {
    wait: async (ms) => {
      waited.push(ms);
    },
  });
  assert.deepEqual(waited, [2000], "Retry-After is seconds and wins over 1000 x attempt");
});

test("plugin_push_waits_a_thousand_times_the_attempt_without_retry_after", async () => {
  const waited = [];
  let n = 0;
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    n += 1;
    return n < 4 ? refusal(503) : answer();
  };
  await pushOnce(hubCaller({ baseUrl: BASE, fetch }), WS, PLUGIN, BATCH, {
    wait: async (ms) => {
      waited.push(ms);
    },
  });
  assert.deepEqual(waited, [1000, 2000, 3000]);
  // `retryDelayMs` with no Retry-After: §7 fixes the retry count and the key, not the delay.
  assert.equal(retryDelayMs(new RemoteError("x", { status: 503 }), 1), 1000);
  assert.equal(retryDelayMs(new RemoteError("x", { status: 503 }), 3), 3000);
});

test("plugin_push_reuses_the_idempotency_key", async () => {
  // Review Focus 4: a fresh key per retry applies a batch that already committed, which is the
  // one failure `push` exists to prevent. All four attempts must carry one key, and that key
  // must be minted once for the call rather than once per attempt — so a second call mints a
  // different one, and a call that is given a key sends exactly that.
  const hub = fakeHub(refusal(503));
  await assert.rejects(() => pushOnce(hub.caller, WS, PLUGIN, BATCH, ok));
  const keys = hub.seen.map((call) => call.init.headers["Idempotency-Key"]);
  assert.equal(keys.length, PUSH_ATTEMPTS);
  assert.equal(new Set(keys).size, 1, `one key across every attempt: ${JSON.stringify(keys)}`);

  const other = fakeHub(answer());
  await pushOnce(other.caller, WS, PLUGIN, BATCH, ok);
  assert.notEqual(other.seen[0].init.headers["Idempotency-Key"], keys[0], "a new call mints a new key");
});

test("plugin_push_mints_one_key_with_random_uuid", async () => {
  // Decision 7: `globalThis.crypto.randomUUID()`, never `node:crypto` — §3 puts the SDK in the
  // browser path, and 36 bytes is well under §5.1's 128-byte cap.
  const hub = fakeHub(refusal(503));
  await assert.rejects(() => pushOnce(hub.caller, WS, PLUGIN, BATCH, ok));
  const [key] = hub.seen.map((call) => call.init.headers["Idempotency-Key"]);
  assert.match(key, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  assert.ok(key.length <= 128);
});

test("plugin_push_uses_the_key_from_its_options", async () => {
  const hub = fakeHub(answer());
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, { key: "mine", wait: never });
  assert.equal(hub.seen[0].init.headers["Idempotency-Key"], "mine");
});

test("plugin_push_does_not_retry_a_client_refusal", async () => {
  // 400, 409, 412, 413 and 422 are the caller's mistake or the hub's verdict: retrying a 412
  // re-reads nothing and a 413 re-sends the same too-large body for ever.
  for (const status of [400, 409, 412, 413, 422]) {
    const hub = fakeHub(refusal(status));
    await assert.rejects(() => pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), (e) => e.status === status);
    assert.equal(hub.seen.length, 1, `${status} is not retried`);
  }
});

test("plugin_push_does_not_retry_an_unauthorised_or_not_found", async () => {
  for (const status of [401, 403, 404, 500]) {
    const hub = fakeHub(refusal(status));
    await assert.rejects(() => pushOnce(hub.caller, WS, PLUGIN, BATCH, ok), (e) => e.status === status);
    assert.equal(hub.seen.length, 1, `${status} is not retried`);
  }
});

test("plugin_push_gives_up_on_four_transport_errors", async () => {
  const fetch = async () => {
    throw new TypeError("fetch failed");
  };
  await assert.rejects(() => pushOnce(hubCaller({ baseUrl: BASE, fetch }), WS, PLUGIN, BATCH, ok), (e) => e.status === 0);
});

test("plugin_push_sends_if_match_when_given", async () => {
  // §5.2: `If-Match: "<epoch>.<plugin_seq>"` is per plugin, so another plugin's writes never
  // make it a 412. The header is sent verbatim, quotes and all.
  const hub = fakeHub(answer());
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, { key: "k", ifMatch: '"1.40"', wait: never });
  assert.equal(hub.seen[0].init.headers["If-Match"], '"1.40"');
});

test("plugin_push_sends_no_if_match_when_none_is_given", async () => {
  const hub = fakeHub(answer());
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, { key: "k", wait: never });
  assert.equal("If-Match" in hub.seen[0].init.headers, false);
});

test("plugin_push_sends_the_canonical_batch_body", async () => {
  // The hub hashes this exact text for the idempotency key, so it must be the canonical order:
  // `deletes` before `upserts`, and every object's keys sorted by bytes.
  const hub = fakeHub(answer());
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, { key: "k", wait: never });
  assert.equal(
    hub.seen[0].init.body,
    '{"deletes":[{"collection":"issue","id":"41"}],"upserts":[{"collection":"issue","id":"42","updatedAt":7,"values":{"title":"Crash"}}]}',
  );
  assert.equal(hub.seen[0].init.headers["Content-Type"], "application/json");
});

test("plugin_push_classifies_what_is_retryable", () => {
  assert.equal(isRetryable(new RemoteError("x", { status: 429 })), true);
  assert.equal(isRetryable(new RemoteError("x", { status: 503 })), true);
  // Status 0 is `unreachable`: a transport error, which §7 retries with the same key so a
  // batch that committed before the connection dropped is not applied twice.
  assert.equal(isRetryable(new RemoteError("x", { status: 0 })), true);
  for (const status of [400, 401, 403, 404, 409, 412, 413, 422, 500]) {
    assert.equal(isRetryable(new RemoteError("x", { status })), false, String(status));
  }
  assert.equal(isRetryable(new TypeError("not a RemoteError")), false);
});

test("plugin_push_takes_the_plugin_from_the_url_only", async () => {
  // §5.2: "The plugin comes from the URL, never from the body." The body is a BatchWire, which
  // has no plugin member to put one in.
  const hub = fakeHub(answer());
  await pushOnce(hub.caller, WS, PLUGIN, BATCH, { key: "k", wait: never });
  assert.equal(JSON.parse(hub.seen[0].init.body).plugin, undefined);
});
