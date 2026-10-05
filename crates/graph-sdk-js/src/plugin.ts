// The hub's writer: `register` (a plugin's declaration), `push` (one batch) and `sync` (make the
// hub's copy match an `Ingest`).
//
// Reached as `@graph-motor/sdk-js/plugin` through `exports`; `src/index.ts` is untouched. A
// plugin needs only `write:<plugin>`: `sync` reads the plugin's own ids from the records route
// and never calls `/graph`, which is what lets one key with a single write grant run a sync.

import { hubCaller, textOf } from "./hub/call.ts";
import { DEFAULT_MAX_BATCH } from "./plugin/batch.ts";
import { pushOnce, type PushOptions } from "./plugin/push.ts";
import { syncWithRestarts, DEFAULT_PAGE, type SyncOptions } from "./plugin/sync.ts";
import type { HubCaller } from "./hub/call.ts";
import type { Ingest } from "./adapters/rows.ts";
import type { RemoteOptions } from "./remote/options.ts";
import type { AnswerWire, BatchWire, ManifestWire } from "./hub/wire.ts";

const JSON_MEDIA_TYPE = "application/json";

/** Everything `createPlugin` takes: `createRemote`'s options, plus what this plugin is. */
export interface PluginOptions extends RemoteOptions {
  /** The plugin's id. It is in every route and never in a body (§5.2). */
  readonly plugin: string;
  /** What this plugin's collections mean. Sent by `register`, and re-sent by every `sync`. */
  readonly manifest: ManifestWire;
  /** `GRAPH_HUB_MAX_BATCH`, §6. Defaults to 10 000. */
  readonly maxBatch?: number;
  /** The records route's page size. Defaults to the route's 1 000. */
  readonly page?: number;
  /** How many times `sync` may start over before it gives up. Defaults to 3. */
  readonly restarts?: number;
  /** Mints each batch's `Idempotency-Key`. Defaults to `globalThis.crypto.randomUUID()`. */
  readonly key?: () => string;
  /** Waits `ms` between a push's retries. Defaults to `setTimeout`. */
  readonly wait?: (ms: number) => Promise<void>;
}

/** The plugin's writer. */
export interface Plugin {
  /** `PUT /v1/workspaces/{ws}/plugins/{plugin}`: the stored manifest, 201 or 200 (§5.2). */
  register(workspace: string): Promise<ManifestWire>;
  /** One batch, retried under one key. `options` overrides the key, the wait and `If-Match`. */
  push(workspace: string, batch: BatchWire, options?: PushOptions): Promise<AnswerWire>;
  /** Make the hub's copy match `ingest`, and answer what each batch did. */
  sync(workspace: string, ingest: Ingest): Promise<readonly AnswerWire[]>;
}

/** Builds the plugin writer over one `HubCaller`, so the three routes share the `fetch`
 * injection, the API-key grammar check and the `dangerouslyAllowBrowser` rule. */
export function createPlugin(options: PluginOptions): Plugin {
  const caller = hubCaller(options);
  const nextKey = options.key ?? (() => globalThis.crypto.randomUUID());
  const syncOptions: SyncOptions = {
    plugin: options.plugin,
    maxBatch: options.maxBatch ?? DEFAULT_MAX_BATCH,
    page: options.page ?? DEFAULT_PAGE,
    nextKey,
    wait: options.wait,
  };
  return {
    register: (workspace) => register(caller, options, workspace),
    push: (workspace, batch, given) =>
      pushOnce(caller, workspace, options.plugin, batch, { key: nextKey(), wait: options.wait, ...given }),
    sync: (workspace, ingest) => syncWithRestarts(caller, syncOptions, workspace, ingest, options.restarts ?? 3),
  };
}

/** Declares what this plugin's collections mean, and answers the hub's stored copy.
 *
 * `PUT`, not `POST`: the route creates the plugin when it is not there (201) and replaces the
 * declaration when it is (200), so a plugin that re-registers after a `manifestVersion` bump
 * does not need to know whether it already exists.
 */
async function register(caller: HubCaller, options: PluginOptions, workspace: string): Promise<ManifestWire> {
  const route = `/v1/workspaces/${encodeURIComponent(workspace)}/plugins/${encodeURIComponent(options.plugin)}`;
  const answer = await caller.callJson({
    method: "PUT",
    route,
    accept: JSON_MEDIA_TYPE,
    body: JSON.stringify(options.manifest),
  });
  return JSON.parse(await textOf(answer)) as ManifestWire;
}

export { pushOnce, PUSH_ATTEMPTS, isRetryable, retryDelayMs } from "./plugin/push.ts";
export type { PushOptions } from "./plugin/push.ts";
export { DEFAULT_MAX_BATCH, batchOf, chunkOps, deleteOps, desiredOps, opKey } from "./plugin/batch.ts";
export type { SyncKey, SyncOp } from "./plugin/batch.ts";
export { readStored, syncOnce, syncWithRestarts, SyncRestartError, DEFAULT_PAGE } from "./plugin/sync.ts";
export type { StoredPage, SyncOptions } from "./plugin/sync.ts";
export type { Ingest, IngestRecord } from "./adapters/rows.ts";
export type {
  AnswerWire, BatchWire, ChangeWire, Collection, DeleteWire, Field, Link, ManifestWire, NoticeWire,
  StoredDelete, StoredRecord, UpsertWire,
} from "./hub/wire.ts";
