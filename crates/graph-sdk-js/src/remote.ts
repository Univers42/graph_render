// `@graph-motor/sdk-js/remote`: the graph-motor HTTP service (`docs/contract/service-api.md`)
// behind the SDK's own types. The binary face is read by the SDK's snapshot reader, so a
// remote layout answers `column(ColumnId.NodeX)` exactly as `Motor#column` does for the same
// run, and every failure is a `GraphMotorError`.
//
// For Node and server-side callers: an `apiKey` in a browser is refused unless the caller
// says otherwise (`dangerouslyAllowBrowser`). The key goes in `Authorization` only.

import { decodeSnapshot, type Snapshot } from "./snapshot.ts";
import { RemoteError, refusalOf, unreachable, unreadable } from "./remote/errors.ts";
import { layoutQueryOf, remoteConfigOf, type LayoutRequest, type RemoteConfig, type RemoteOptions } from "./remote/options.ts";
import { InvalidOptionsError } from "./errors.ts";

export { RemoteError, SERVICE_ERROR_NAMES, type RemoteErrorInit, type RemoteErrorName } from "./remote/errors.ts";
export type { FetchLike, LayoutRequest, RemoteOptions } from "./remote/options.ts";
export { decodeSnapshot, SnapshotRefusedError, type Note, type RefusalCode, type Snapshot } from "./snapshot.ts";

/** The binary face's media type, which `layout` asks for unless told `format: "json"`. */
export const SNAPSHOT_MEDIA_TYPE = "application/vnd.graph-motor.snapshot";
const JSON_MEDIA_TYPE = "application/json";

/** `GET /v1/meta`: the service's motor, and the registries its routes serve, in registry
 *  order. No `analyses`: no route runs one (verdict C12). Fields a newer service adds are
 *  dropped, not refused. */
export interface RemoteMeta {
  readonly api: 1;
  readonly abi: number;
  readonly version: string;
  readonly layouts: readonly string[];
  readonly posts: readonly string[];
}

/** An ingest document: a JSON string sent as is, or a value sent as `JSON.stringify(doc)`. */
export type IngestDocument = string | object;

export interface Remote {
  meta(): Promise<RemoteMeta>;
  /** The canonical JSON face, parsed. */
  layout(doc: IngestDocument, request: LayoutRequest & { readonly format: "json" }): Promise<unknown>;
  /** The binary face, decoded. */
  layout(doc: IngestDocument, request: LayoutRequest): Promise<Snapshot>;
}

interface Call {
  readonly method: "GET" | "POST";
  readonly path: string;
  readonly query?: URLSearchParams;
  readonly accept: string;
  readonly body?: string;
}

const UTF8 = new TextDecoder();

function headersOf(config: RemoteConfig, call: Call): Record<string, string> {
  const headers: Record<string, string> = { Accept: call.accept };
  if (call.body !== undefined) headers["Content-Type"] = JSON_MEDIA_TYPE;
  if (config.key !== undefined) headers["Authorization"] = `Bearer ${config.key}`;
  return headers;
}

function mediaTypeOf(response: Response): string {
  return (response.headers.get("content-type") ?? "").split(";")[0]?.trim().toLowerCase() ?? "";
}

/** One request, answered by a 200 in the face it asked for, as bytes; anything else throws. */
async function send(config: RemoteConfig, call: Call): Promise<Uint8Array> {
  const route = `${call.method} ${call.path}`;
  const url = `${config.base}${call.path}${call.query === undefined ? "" : `?${call.query.toString()}`}`;
  const init: RequestInit = { method: call.method, headers: headersOf(config, call), redirect: "error" };
  if (call.body !== undefined) init.body = call.body;
  let response: Response;
  try {
    response = await config.fetch(url, init);
  } catch (cause) {
    throw unreachable(route, cause, config.key);
  }
  if (response.status !== 200) throw await refusalOf(response, route, config.key);
  const type = mediaTypeOf(response);
  if (type !== call.accept) throw unreadable(route, `asked for ${call.accept}, got ${type === "" ? "no content type" : type.slice(0, 80)}`);
  try {
    return new Uint8Array(await response.arrayBuffer());
  } catch (cause) {
    throw unreachable(route, cause, config.key);
  }
}

function parsed(bytes: Uint8Array, route: string): unknown {
  try {
    return JSON.parse(UTF8.decode(bytes));
  } catch {
    throw unreadable(route, "the body is not JSON");
  }
}

const isIdList = (value: unknown): boolean => Array.isArray(value) && value.every((id) => typeof id === "string");

function metaOf(value: unknown): RemoteMeta {
  const meta = (typeof value === "object" && value !== null ? value : {}) as Partial<Record<keyof RemoteMeta, unknown>>;
  const fits =
    meta.api === 1 && typeof meta.abi === "number" && typeof meta.version === "string" &&
    isIdList(meta.layouts) && isIdList(meta.posts);
  if (!fits) throw unreadable("GET /v1/meta", "not an api 1 meta document");
  const { api, abi, version, layouts, posts } = meta as RemoteMeta;
  return { api, abi, version, layouts, posts };
}

function bodyOf(doc: IngestDocument): string {
  if (typeof doc === "string") return doc;
  if (typeof doc !== "object" || doc === null) throw new InvalidOptionsError("layout's document must be a JSON string or an object");
  return JSON.stringify(doc);
}

async function layoutOf(config: RemoteConfig, doc: IngestDocument, request: LayoutRequest): Promise<unknown> {
  const query = layoutQueryOf(request);
  const json = request.format === "json";
  const call: Call = { method: "POST", path: "/v1/layout", query, accept: json ? JSON_MEDIA_TYPE : SNAPSHOT_MEDIA_TYPE, body: bodyOf(doc) };
  const bytes = await send(config, call);
  return json ? parsed(bytes, "POST /v1/layout") : decodeSnapshot(bytes);
}

/** A client for one service. Options are checked here, once: a bad `baseUrl`, a malformed
 *  key, or a key in a browser throws `InvalidOptionsError` before any request. */
export function createRemote(options: RemoteOptions): Remote {
  const config = remoteConfigOf(options);
  const meta = async (): Promise<RemoteMeta> =>
    metaOf(parsed(await send(config, { method: "GET", path: "/v1/meta", accept: JSON_MEDIA_TYPE }), "GET /v1/meta"));
  const layout = (doc: IngestDocument, request: LayoutRequest): Promise<unknown> => layoutOf(config, doc, request);
  return { meta, layout: layout as Remote["layout"] };
}

