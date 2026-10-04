// A stand-in for the graph-motor service (`docs/contract/service-api.md` "Routes", "Errors"),
// injected as `fetch`. Its layout answers are the real wasm `Motor`'s own `toBytes`/`toJSON`,
// so the client is tested on the bytes the service sends, not on bytes written by hand.
// Not a server: it checks the bearer key exactly and routes two paths, nothing more.

import { randomBytes } from "node:crypto";
import { GraphMotorError } from "../src/errors.ts";
import { ABI_VERSION } from "../src/wasm.ts";
import { SNAPSHOT_MEDIA_TYPE } from "../src/remote.ts";

/** The service's own `error` names by status, spelled as `server/graph-server/src/error.rs`
 *  spells them. Written out, not imported from the SDK: the fake plays the server, so a name
 *  the SDK spells differently fails a test instead of moving with it. */
export const SERVER_NAMES = Object.freeze({
  400: "BadRequest", 401: "Unauthorized", 404: "NotFound", 406: "NotAcceptable", 429: "Busy", 500: "Internal", 503: "Timeout",
});

/** A key in the service's shape, `gm_` + 43 base64url characters, made per test run. */
export const newKey = () => `gm_${randomBytes(32).toString("base64url")}`;

/** The service's error body, with the headers its contract puts on a 401 (C9) and a 429. */
function refuse(status, error, message) {
  const headers = { "Content-Type": "application/json" };
  if (status === 401) headers["WWW-Authenticate"] = "Bearer";
  if (status === 429) headers["Retry-After"] = "1";
  return new Response(JSON.stringify({ error, message }), { status, headers });
}

const statusOf = (error) => (error.codeName === "IngestTooLarge" ? 413 : 422);

function face(motor, handle, accept, names) {
  if (accept === SNAPSHOT_MEDIA_TYPE) return new Response(motor.toBytes(handle), { status: 200, headers: { "Content-Type": accept } });
  if (accept === "application/json") return new Response(motor.toJSON(handle), { status: 200, headers: { "Content-Type": accept } });
  return refuse(406, names[406], "Accept names neither face");
}

function layoutReply(motor, query, init, names) {
  const layout = query.get("layout") ?? "";
  const post = query.get("post");
  // The server checks the registry before the motor runs, as `query.rs` does.
  if (!motor.layouts().includes(layout)) return refuse(400, "UnknownLayoutId", `no layout ${layout}`);
  if (post !== null && post.includes(",")) return refuse(400, names[400], "post takes at most one id");
  let handle;
  try {
    handle = query.get("source") === "contract" ? motor.buildContract(init.body) : motor.build(init.body);
    motor.layout(handle, layout);
    if (post !== null) motor.post(handle, post);
    return face(motor, handle, init.headers.Accept, names);
  } catch (error) {
    if (!(error instanceof GraphMotorError)) throw error;
    return refuse(statusOf(error), error.codeName, error.message);
  } finally {
    if (handle !== undefined) motor.release(handle);
  }
}

function route(motor, url, init, names) {
  const { pathname, searchParams } = new URL(url);
  if (pathname.endsWith("/v1/meta") && init.method === "GET") {
    const meta = { api: 1, abi: ABI_VERSION, version: "0.0.0-fake", layouts: motor.layouts(), posts: motor.posts() };
    return new Response(JSON.stringify(meta), { status: 200, headers: { "Content-Type": "application/json" } });
  }
  if (pathname.endsWith("/v1/layout") && init.method === "POST") return layoutReply(motor, searchParams, init, names);
  return refuse(404, names[404], "no such route");
}

/**
 * `{ fetch, seen }`: `fetch` answers like the service would for `key`; `seen` records every
 * call. Options bend it:
 * - `canned: { status, error }` answers that refusal to every call with the right key;
 * - `mode: "echo"` puts the presented `Authorization` header in its 401 message (a server that
 *   must not exist, to prove the client cuts the key out);
 * - `mode: "html"` answers a 401 the way a proxy might, with no error body;
 * - `names` replaces {@link SERVER_NAMES}, which is how the negative control renames one.
 */
export function fakeService(motor, key, { mode = "", canned, names = SERVER_NAMES } = {}) {
  const seen = [];
  const fetch = async (url, init) => {
    seen.push({ url, init });
    const presented = init.headers.Authorization;
    if (presented !== `Bearer ${key}`) {
      if (mode === "html") return new Response("<html>401</html>", { status: 401, headers: { "Content-Type": "text/html" } });
      return refuse(401, names[401], mode === "echo" ? `refused ${presented}` : "missing or unknown API key");
    }
    if (canned !== undefined) return refuse(canned.status, canned.error, "canned refusal");
    return route(motor, url, init, names);
  };
  return { fetch, seen };
}
