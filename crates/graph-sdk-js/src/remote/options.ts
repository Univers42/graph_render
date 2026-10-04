// `createRemote`'s and `layout`'s closed option shapes (C16, as `options.ts` does for the
// motor): an unknown key or a malformed value is refused before any request is made. No
// message here quotes the URL or the key: either could carry a secret.

import { InvalidOptionsError } from "../errors.ts";

export type FetchLike = (url: string, init: RequestInit) => Promise<Response>;

export interface RemoteOptions {
  /** The service root, `http:` or `https:`, with an optional path prefix; no credentials,
   *  query or fragment. A trailing slash is ignored. */
  readonly baseUrl: string;
  /** Sent as `Authorization: Bearer <apiKey>`, never in a URL. Omit it only for a service
   *  started with `GRAPH_AUTH=off`. */
  readonly apiKey?: string | undefined;
  /** Defaults to `globalThis.fetch`. */
  readonly fetch?: FetchLike | undefined;
  /** Allow an `apiKey` in a browser-like global. The key then ships to every visitor. */
  readonly dangerouslyAllowBrowser?: boolean | undefined;
}

export interface LayoutRequest {
  /** A layout id from `meta().layouts`, e.g. `"layout.tree.tidy"`. */
  readonly layout: string;
  /** One post id from `meta().posts`, applied after the layout. The service takes at most
   *  one (verdict C6); chain posts on a local `Motor`. */
  readonly post?: string | undefined;
  /** `"studio"` (default) for the provisional ingest, `"contract"` for the contract document. */
  readonly source?: "studio" | "contract" | undefined;
  /** `"binary"` (default) decodes the binary face; `"json"` returns the canonical JSON, parsed. */
  readonly format?: "binary" | "json" | undefined;
}

export interface RemoteConfig {
  readonly base: string;
  readonly key: string | undefined;
  readonly fetch: FetchLike;
}

const REMOTE_KEYS: ReadonlySet<string> = new Set(["baseUrl", "apiKey", "fetch", "dangerouslyAllowBrowser"]);
const LAYOUT_KEYS: ReadonlySet<string> = new Set(["layout", "post", "source", "format"]);
// RFC 6750 `b64token`. Checked here because a fetch implementation that refuses a header
// value may quote it in its own error (undici does).
const BEARER_TOKEN = /^[A-Za-z0-9\-._~+/]+=*$/;

function refuseUnknown(value: object, allowed: ReadonlySet<string>, what: string): void {
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) throw new InvalidOptionsError(`${what}: unknown option "${key}"`);
  }
}

/** The root every route is appended to: origin plus path, trailing slashes dropped. */
export function baseOf(raw: unknown): string {
  if (typeof raw !== "string") throw new InvalidOptionsError("baseUrl must be a string");
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new InvalidOptionsError("baseUrl is not an absolute URL");
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new InvalidOptionsError("baseUrl must be http: or https:");
  if (url.username !== "" || url.password !== "") {
    throw new InvalidOptionsError("baseUrl must not carry credentials; pass apiKey instead");
  }
  if (url.search !== "" || url.hash !== "") throw new InvalidOptionsError("baseUrl must not carry a query or a fragment");
  return url.origin + url.pathname.replace(/\/+$/, "");
}

/** Caveat: a heuristic. A server runtime that defines `window` or `document` (a jsdom test
 *  environment, say) is refused too; it passes `dangerouslyAllowBrowser: true`. A web worker
 *  has neither, so `WorkerGlobalScope` counts as a browser as well. */
function isBrowserLike(): boolean {
  return "window" in globalThis || "document" in globalThis || "WorkerGlobalScope" in globalThis;
}

function keyOf(options: RemoteOptions): string | undefined {
  const { apiKey, dangerouslyAllowBrowser } = options;
  if (dangerouslyAllowBrowser !== undefined && typeof dangerouslyAllowBrowser !== "boolean") {
    throw new InvalidOptionsError("dangerouslyAllowBrowser must be a boolean");
  }
  if (apiKey === undefined) return undefined;
  if (typeof apiKey !== "string" || !BEARER_TOKEN.test(apiKey)) {
    throw new InvalidOptionsError("apiKey must be a bearer token (RFC 6750 b64token); its value is not shown");
  }
  if (dangerouslyAllowBrowser !== true && isBrowserLike()) {
    throw new InvalidOptionsError(
      "createRemote refuses an apiKey in a browser: the key would ship to every visitor. " +
        "Call the service from a server, or pass dangerouslyAllowBrowser: true",
    );
  }
  return apiKey;
}

function fetchOf(given: unknown): FetchLike {
  const chosen: unknown = given ?? globalThis.fetch;
  if (typeof chosen !== "function") throw new InvalidOptionsError("fetch must be a function, and there is no global fetch");
  const call = chosen as FetchLike;
  // Called bare, never as a method: a browser's own fetch throws on a foreign `this`.
  return (url, init) => call(url, init);
}

export function remoteConfigOf(options: RemoteOptions): RemoteConfig {
  if (typeof options !== "object" || options === null) throw new InvalidOptionsError("createRemote takes an options object");
  refuseUnknown(options, REMOTE_KEYS, "createRemote");
  return { base: baseOf(options.baseUrl), key: keyOf(options), fetch: fetchOf(options.fetch) };
}

function checkId(id: unknown, what: string): string {
  if (typeof id !== "string" || id === "" || id.includes(",")) {
    throw new InvalidOptionsError(`${what} must be a non-empty id without a comma`);
  }
  return id;
}

/** `/v1/layout`'s query string for `request`, after checking its shape. */
export function layoutQueryOf(request: LayoutRequest): URLSearchParams {
  if (typeof request !== "object" || request === null) throw new InvalidOptionsError("layout takes a request object");
  refuseUnknown(request, LAYOUT_KEYS, "layout");
  const query = new URLSearchParams({ layout: checkId(request.layout, "layout") });
  if (request.post !== undefined) query.set("post", checkId(request.post, "post (one id; the service takes at most one)"));
  if (request.source !== undefined) {
    if (request.source !== "studio" && request.source !== "contract") {
      throw new InvalidOptionsError(`source must be "studio" or "contract"`);
    }
    query.set("source", request.source);
  }
  if (request.format !== undefined && request.format !== "binary" && request.format !== "json") {
    throw new InvalidOptionsError(`format must be "binary" or "json"`);
  }
  return query;
}
