// `./remote` against a fake service that serves the real wasm build's bytes
// (`fake-service.mjs`). Gate row `svc-sdk` (`docs/contract/service-api.md` "Gates").
//
// Negative controls, by `GM_SDK_REMOTE_BREAK` (any other value is refused, so a typo cannot
// turn a control into a pass):
// - `wrong-key` sends the parity cases a key the fake does not hold, so each one must fail on
//   the typed 401, never on a decode (the spec's `svc-sdk` row);
// - `rename-name` makes the fake answer 429 with a renamed `error`, so the per-status case for
//   429 must fail on its name (verdict C6).
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { inspect } from "node:util";
import { CODE_NAMES, GraphMotorError, InvalidOptionsError } from "../src/errors.ts";
import { ColumnId } from "../src/index.ts";
import { createRemote, RemoteError } from "../src/remote.ts";
import { CASES, caseDifferences, documentOf, loadMotor, runLocal } from "../scripts/parity.mjs";
import { SERVER_NAMES, fakeService, newKey } from "./fake-service.mjs";

const BREAK = process.env.GM_SDK_REMOTE_BREAK ?? "";
if (!["", "wrong-key", "rename-name"].includes(BREAK)) throw new Error(`GM_SDK_REMOTE_BREAK must be unset, "wrong-key" or "rename-name"`);
const NAMES = BREAK === "rename-name" ? { ...SERVER_NAMES, 429: `${SERVER_NAMES[429]}Renamed` } : SERVER_NAMES;

const BASE = "http://graph.test";
const motor = await loadMotor();
const key = newKey();

function remoteOver(service, apiKey = key, baseUrl = BASE) {
  return createRemote({ baseUrl, apiKey, fetch: service.fetch });
}

for (const testCase of CASES) {
  test(`remote.layout decodes to the wasm Motor's columns: ${testCase.name}`, async () => {
    const remote = remoteOver(fakeService(motor, key), BREAK === "wrong-key" ? newKey() : key);
    assert.deepEqual(await caseDifferences(remote, motor, testCase), []);
  });
}

test("format: json returns the canonical JSON face, parsed", async () => {
  const testCase = CASES[0];
  const document = await documentOf(testCase);
  const json = await remoteOver(fakeService(motor, key)).layout(document, { ...testCase.request, format: "json" });
  const { handle } = runLocal(motor, document, testCase.request);
  assert.deepEqual(json, JSON.parse(motor.toJSON(handle)));
  motor.release(handle);
});

test("meta() reads the service's registries, and drops a field it does not know", async () => {
  const meta = await remoteOver(fakeService(motor, key)).meta();
  assert.deepEqual(meta, { api: 1, abi: meta.abi, version: "0.0.0-fake", layouts: motor.layouts(), posts: motor.posts() });
  const body = JSON.stringify({ ...meta, analyses: [], later: true });
  const fetch = async () => new Response(body, { status: 200, headers: { "Content-Type": "application/json" } });
  assert.deepEqual(await createRemote({ baseUrl: BASE, fetch }).meta(), meta);
});

test("README: a remote layout reads like a local one", async () => {
  const service = fakeService(motor, key);
  const document = await documentOf(CASES[0]);
  const remote = createRemote({ baseUrl: "http://graph.test", apiKey: key, fetch: service.fetch });
  const meta = await remote.meta();
  const snapshot = await remote.layout(document, { layout: "layout.tree.tidy" });
  const x = snapshot.column(ColumnId.NodeX); // Float32Array, one entry per node, in nodeIds order
  assert.ok(meta.layouts.includes("layout.tree.tidy"));
  assert.equal(x.length, snapshot.nodeCount);
  assert.equal(snapshot.nodeIds.length, snapshot.nodeCount);
});

test("the key travels in Authorization only, and the request names the face and the query", async () => {
  const service = fakeService(motor, key);
  await remoteOver(service, key, `${BASE}/api/`).layout(await documentOf(CASES[1]), CASES[1].request);
  const [{ url, init }] = service.seen;
  assert.equal(url, `${BASE}/api/v1/layout?layout=layout.packing.circle&post=post.style.bezier`);
  assert.equal(init.headers.Authorization, `Bearer ${key}`);
  assert.equal(init.headers.Accept, "application/vnd.graph-motor.snapshot");
  assert.equal(init.redirect, "error");
});

// One case per status the service answers (verdict C6). `name` is the server's spelling; the
// real motor answers the 400 layout, 401 and 422 cases, a canned body the rest.
const STATUS_CASES = [
  { status: 400, name: "UnknownLayoutId", request: { layout: "layout.does-not-exist" } },
  { status: 400, name: SERVER_NAMES[400], canned: NAMES[400] },
  { status: 401, name: SERVER_NAMES[401], apiKey: newKey() },
  { status: 404, name: SERVER_NAMES[404], canned: NAMES[404] },
  { status: 406, name: SERVER_NAMES[406], canned: NAMES[406] },
  { status: 413, name: "IngestTooLarge", canned: "IngestTooLarge" },
  { status: 422, name: "IngestInvalid", document: "{\"version\":1}" },
  { status: 429, name: SERVER_NAMES[429], canned: NAMES[429] },
  { status: 500, name: SERVER_NAMES[500], canned: NAMES[500] },
  { status: 503, name: SERVER_NAMES[503], canned: NAMES[503] },
];

for (const c of STATUS_CASES) {
  test(`HTTP ${c.status} is the typed ${c.name}`, async () => {
    const canned = c.canned === undefined ? undefined : { status: c.status, error: c.canned };
    const remote = remoteOver(fakeService(motor, key, { canned, names: NAMES }), c.apiKey ?? key);
    const document = c.document ?? (await documentOf(CASES[0]));
    const error = await remote.layout(document, c.request ?? CASES[0].request).catch((thrown) => thrown);
    assert.ok(error instanceof RemoteError && error instanceof GraphMotorError, String(error));
    assert.equal(error.status, c.status);
    assert.equal(error.codeName, c.name, `${c.status} names ${c.name}`);
    assert.equal(error.code, CODE_NAMES.includes(c.name) ? CODE_NAMES.indexOf(c.name) : undefined);
    assert.equal(error.retryAfter, c.status === 429 ? 1 : undefined);
  });
}

test("an unknown status, or a name the SDK does not type, is still a RemoteError", async () => {
  const proxy = async () => new Response("<html>bad gateway</html>", { status: 502, headers: { "Content-Type": "text/html" } });
  const gateway = await createRemote({ baseUrl: BASE, fetch: proxy }).meta().catch((error) => error);
  assert.ok(gateway instanceof RemoteError && gateway.status === 502 && gateway.codeName === undefined);
  assert.ok(!gateway.message.includes("bad gateway"), gateway.message);
  const renamed = await remoteOver(fakeService(motor, key, { canned: { status: 429, error: "Overloaded" } })).meta().catch((error) => error);
  assert.ok(renamed instanceof RemoteError && renamed.status === 429 && renamed.codeName === undefined);
  assert.match(renamed.message, /HTTP 429 Overloaded/);
});

test("post takes one id: a list or a comma is refused before any request", async () => {
  const service = fakeService(motor, key);
  const remote = remoteOver(service);
  for (const post of ["post.style.bezier,post.route.grid", ["post.style.bezier"], ""]) {
    await assert.rejects(remote.layout("{}", { layout: "layout.tree.tidy", post }), InvalidOptionsError);
  }
  assert.equal(service.seen.length, 0);
});

test("a body that is not the service's error shape leaves codeName undefined", async () => {
  await assert.rejects(remoteOver(fakeService(motor, key, { mode: "html" }), newKey()).meta(), (error) => {
    assert.equal(error.status, 401);
    assert.equal(error.codeName, undefined);
    assert.ok(!error.message.includes("html"));
    return true;
  });
});

test("a 200 in the wrong face is refused, not decoded", async () => {
  const fetch = async () => new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
  const remote = createRemote({ baseUrl: BASE, apiKey: key, fetch });
  await assert.rejects(remote.layout("{}", { layout: "layout.tree.tidy" }), (error) => error instanceof RemoteError && error.status === 200);
});

/** Every text an error carries: its message, its string form and its inspected form. */
const textsOf = (error) => [error.message, String(error), inspect(error, { depth: 8 }), JSON.stringify(error)];

test("the key is in no error text and in no URL", async () => {
  const presented = newKey();
  const echo = fakeService(motor, key, { mode: "echo" });
  const leaky = async () => {
    throw new TypeError(`fetch failed for Bearer ${key}`, { cause: new Error(`header ${key}`) });
  };
  const echoed = await remoteOver(echo, presented).meta().catch((error) => error);
  const thrown = await createRemote({ baseUrl: BASE, apiKey: key, fetch: leaky }).meta().catch((error) => error);
  assert.equal(echoed.status, 401);
  assert.equal(thrown.status, 0);
  for (const text of textsOf(echoed)) assert.ok(!text.includes(presented), text);
  for (const text of textsOf(thrown)) assert.ok(!text.includes(key), text);
  assert.ok(echo.seen.every(({ url }) => !url.includes(presented)));
});

test("a malformed key is refused without being quoted", () => {
  const bad = `${key}\n`;
  assert.throws(() => createRemote({ baseUrl: BASE, apiKey: bad }), (error) => error instanceof InvalidOptionsError && !error.message.includes(key));
});

test("baseUrl: http or https, no credentials, no query, a trailing slash ignored", () => {
  for (const baseUrl of ["ftp://graph.test", "http://user:secret@graph.test", "http://graph.test/?a=1", "http://graph.test/#x", "graph.test", 7]) {
    assert.throws(() => createRemote({ baseUrl, apiKey: key }), (error) => error instanceof InvalidOptionsError && !error.message.includes("secret"));
  }
  assert.doesNotThrow(() => createRemote({ baseUrl: "https://graph.test///", apiKey: key }));
});

test("unknown options are refused, on createRemote and on layout", async () => {
  assert.throws(() => createRemote({ baseUrl: BASE, apikey: key }), InvalidOptionsError);
  const remote = remoteOver(fakeService(motor, key));
  await assert.rejects(remote.layout("{}", { layout: "layout.tree.tidy", posts: [] }), InvalidOptionsError);
  await assert.rejects(remote.layout("{}", { layout: "layout.tree.tidy", source: "rows" }), InvalidOptionsError);
});

test("a key in a browser-like global is refused unless dangerouslyAllowBrowser", () => {
  for (const name of ["window", "document"]) {
    globalThis[name] = {};
    try {
      assert.throws(() => createRemote({ baseUrl: BASE, apiKey: key }), InvalidOptionsError);
      assert.doesNotThrow(() => createRemote({ baseUrl: BASE, apiKey: key, dangerouslyAllowBrowser: true }));
      assert.doesNotThrow(() => createRemote({ baseUrl: BASE }));
    } finally {
      delete globalThis[name];
    }
  }
});
