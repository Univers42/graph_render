// The hub reader: `graph` (the canonical document and its cursor), `layout` (a snapshot of the
// materialized workspace) and `subscribe` (the change stream, `src/hub/subscribe.ts`).
//
// Three closures over one `hubCaller(options)`, so the `fetch` injection, the API-key grammar
// check and the `dangerouslyAllowBrowser` rule are `remoteConfigOf`'s and every route is spelled
// in exactly one place. Reached as `@graph-motor/sdk-js/hub` through `exports`; `src/index.ts`
// is untouched, so this is additive and the barrel stays the Motor's.

import { bytesOf, hubCaller, textOf } from "./hub/call.ts";
import { parseCursor } from "./hub/cursor.ts";
import { subscribe, type SubscribeOptions } from "./hub/subscribe.ts";
import { decodeSnapshot, SNAPSHOT_MEDIA_TYPE, type Snapshot } from "./remote.ts";
import { layoutQueryOf, type LayoutRequest, type RemoteOptions } from "./remote/options.ts";
import { unreadable } from "./remote/errors.ts";

const JSON_MEDIA_TYPE = "application/json";

/** What `graph` read: the cursor the hub stamped the document with, and the document itself. */
export interface HubGraph {
  /** The `ETag`, quotes stripped: `<epoch>.<seq>`, a position to `subscribe` from. */
  readonly cursor: string;
  /** The canonical ingest document, exactly as the hub wrote it. */
  readonly ingest: string;
}

/** The hub's reader. Three routes; a `workspace` id is never a URL fragment the SDK builds. */
export interface Hub {
  /** The canonical ingest document and the cursor it is at. */
  graph(workspace: string): Promise<HubGraph>;
  /** A snapshot of the materialized workspace. Never retried (§5.3, §7). */
  layout(workspace: string, request: LayoutRequest): Promise<Snapshot>;
  /** The change stream, and the function that stops it. */
  subscribe(workspace: string, options: SubscribeOptions): () => void;
}

/** Builds the hub reader. Every option is `createRemote`'s, and every refusal in them is
 * `remoteConfigOf`'s: this call makes no request, and an unusable configuration is refused
 * before the first one. */
export function createHub(options: RemoteOptions): Hub {
  const caller = hubCaller(options);
  const graph = async (workspace: string): Promise<HubGraph> => {
    const route = graphRoute(workspace);
    // `callJson`, not `call`: a 304 is not a success, and a 404 or a 401 should be the hub's
    // own typed refusal rather than a guess from a missing header.
    const answer = await caller.callJson({ method: "GET", route, accept: JSON_MEDIA_TYPE });
    const etag = answer.headers["etag"];
    if (etag === undefined) throw unreadable(route, "the hub sent no ETag");
    const cursor = unquoted(etag);
    // A cursor the client cannot read exactly is a cursor it must not hand to `subscribe`.
    parseCursor(cursor);
    return { cursor, ingest: await textOf(answer) };
  };
  const layout = async (workspace: string, request: LayoutRequest): Promise<Snapshot> => {
    const route = layoutRoute(workspace, request);
    // `callJson` for the refusal, not the media type: §7 says a failed `/layout` reaches the
    // caller as a `RemoteError` and is never retried, so a 503 or a 401 must not be handed to
    // `decodeSnapshot` as if it were a snapshot. The `Accept` is still the snapshot's own.
    const answer = await caller.callJson({ method: "POST", route, accept: SNAPSHOT_MEDIA_TYPE });
    return decodeSnapshot(await bytesOf(answer));
  };
  return { graph, layout, subscribe: (workspace, given) => subscribe(caller, workspace, given) };
}

/** `GET /v1/workspaces/{ws}/graph` (§5.2). */
function graphRoute(workspace: string): string {
  return `/v1/workspaces/${encodeURIComponent(workspace)}/graph`;
}

/** `POST /v1/workspaces/{ws}/layout`, with `layoutQueryOf`'s own checked query. */
function layoutRoute(workspace: string, request: LayoutRequest): string {
  return `/v1/workspaces/${encodeURIComponent(workspace)}/layout?${layoutQueryOf(request).toString()}`;
}

// §5.2 writes the ETag as `"<epoch>.<seq>"`; the quotes are HTTP's, not the cursor's.
function unquoted(etag: string): string {
  return etag.replace(/^"|"$/g, "");
}

export { parseCursor, formatCursor, InvalidCursorError, MAX_SEQ } from "./hub/cursor.ts";
export type { Cursor } from "./hub/cursor.ts";
export type { SseFrame } from "./hub/sse.ts";
export type { StreamOutcome, SubscribeState } from "./hub/subscribe.ts";
export type {
  AnswerWire, BatchWire, Cardinality, ChangeKind, ChangeWire, Collection, DeleteWire, ErrorWire,
  Field, JsonValue, Link, ManifestWire, NoticeWire, Role, StoredDelete, StoredRecord, UpsertWire,
} from "./hub/wire.ts";
