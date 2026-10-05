// The one way the hub SDK speaks HTTP. Every route in `src/hub.ts` and `src/plugin.ts` goes
// through the `HubCaller` this builds, so the `fetch` injection, the API-key grammar check and
// the `dangerouslyAllowBrowser` rule are `remoteConfigOf`'s (reused, never re-implemented) and
// there is exactly one place a header is spelled.

import { refusalOf, unreachable } from "../remote/errors.ts";
import { remoteConfigOf, type RemoteConfig, type RemoteOptions } from "../remote/options.ts";

// Plan Decision 2: `src/remote.ts:20`'s copy is module-private and reaching it would mean
// editing `src/remote*`, which is read-only here. Two constants hold the same string.
const JSON_MEDIA_TYPE = "application/json";

// `fetch` bodies a `Response` may not carry, whatever the hub sent. `new Response(body, …)`
// throws on these, and 304 is exactly what `GET /graph` answers for `If-None-Match`.
const NULL_BODY_STATUSES: ReadonlySet<number> = new Set([204, 205, 304]);

/** One request to the hub: a method, the route under the base, and at most a body. */
export interface HubRequest {
  readonly method: "GET" | "PUT" | "POST";
  readonly route: string;
  /** The `Accept` header. Absent means "say nothing", which is not the same as asking for
   * anything: `callJson` supplies the JSON default, `call` leaves the stream route alone. */
  readonly accept?: string;
  /** The request body, already the wire's canonical text. */
  readonly body?: string;
  /** Extra headers, e.g. `Idempotency-Key` or `If-Match`. */
  readonly headers?: Readonly<Record<string, string>>;
}

/** One answer: a status, lowercased headers, and a body stream nobody has read yet. */
export interface HubAnswer {
  readonly status: number;
  readonly headers: Readonly<Record<string, string>>;
  readonly body: ReadableStream<Uint8Array> | null;
}

/** The hub's HTTP surface: `call` leaves the body alone, `callJson` refuses a non-2xx. */
export interface HubCaller {
  readonly base: string;
  readonly key: string | undefined;
  call(request: HubRequest): Promise<HubAnswer>;
  callJson(request: HubRequest): Promise<HubAnswer>;
}

/** Builds the hub's caller from the same options `createRemote` takes, and reuses every check
 * in them: a malformed `baseUrl`, an `apiKey` that is not an RFC 6750 token and a key offered
 * to a browser-like global are all refused here, before any request is made. */
export function hubCaller(options: RemoteOptions): HubCaller {
  const config = remoteConfigOf(options);
  const call = (request: HubRequest): Promise<HubAnswer> => send(config, request);
  return {
    base: config.base,
    key: config.key,
    call,
    callJson: async (request: HubRequest) => refuseNonSuccess(await call(withJsonAccept(request)), request.route, config.key),
  };
}

/** The answer's body as UTF-8 text: the canonical ingest document, a manifest, an SSE body. */
export async function textOf(answer: HubAnswer): Promise<string> {
  if (answer.body === null) return "";
  return new TextDecoder().decode(await new Response(answer.body).arrayBuffer());
}

/** The answer's body as bytes: what `decodeSnapshot` takes (`src/snapshot.ts:140`). */
export async function bytesOf(answer: HubAnswer): Promise<Uint8Array> {
  if (answer.body === null) return new Uint8Array(0);
  return new Uint8Array(await new Response(answer.body).arrayBuffer());
}

// The one private sender, shared by `call` and `callJson` so a header cannot differ between them.
async function send(config: RemoteConfig, request: HubRequest): Promise<HubAnswer> {
  const headers: Record<string, string> = { ...request.headers };
  if (config.key !== undefined) headers["Authorization"] = `Bearer ${config.key}`;
  if (request.body !== undefined) headers["Content-Type"] = JSON_MEDIA_TYPE;
  if (request.accept !== undefined) headers["Accept"] = request.accept;
  const init: RequestInit = { method: request.method, headers, redirect: "error" };
  if (request.body !== undefined) init.body = request.body;
  try {
    const response = await config.fetch(config.base + request.route, init);
    return { status: response.status, headers: lowercased(response.headers), body: response.body };
  } catch (cause) {
    // `unreachable` scrubs the key, so a transport error never quotes it.
    throw unreachable(request.route, cause, config.key);
  }
}

// `callJson` is `call` plus the typed refusal, so the SSE route can use `call` and read a 410
// itself while every JSON route gets the refusal for free.
async function refuseNonSuccess(answer: HubAnswer, route: string, key: string | undefined): Promise<HubAnswer> {
  if (answer.status >= 200 && answer.status < 300) return answer;
  throw await refusalOf(asResponse(answer), route, key);
}

// The answer as a `Response`, because `refusalOf` reads the body and the `retry-after` header
// off one.
function asResponse(answer: HubAnswer): Response {
  const body = NULL_BODY_STATUSES.has(answer.status) ? null : answer.body;
  return new Response(body, { status: answer.status, headers: { ...answer.headers } });
}

function withJsonAccept(request: HubRequest): HubRequest {
  return request.accept === undefined ? { ...request, accept: JSON_MEDIA_TYPE } : request;
}

function lowercased(headers: Headers): Readonly<Record<string, string>> {
  const out: Record<string, string> = {};
  headers.forEach((value, name) => {
    out[name.toLowerCase()] = value;
  });
  return out;
}
