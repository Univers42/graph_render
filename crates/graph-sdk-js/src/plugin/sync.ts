// `sync` — make the hub's copy of one plugin's collections match an `Ingest` (§7).
//
// The write is a diff, not a snapshot: read the plugin's own stored ids, work out what is
// wanted, and send the difference in batches of at most `GRAPH_HUB_MAX_BATCH`. Two properties
// carry the whole design:
//
//   * `If-Match` carries the plugin's own `plugin_seq`, never the workspace's `head_seq`, so two
//     plugins writing one workspace do not starve each other (§5.1, `hub-authz`).
//   * A 412, or a `plugin_seq` that moved under the read, starts the whole sync over — bounded,
//     because unbounded is a hot loop against whatever else is writing.

import { textOf } from "../hub/call.ts";
import { batchOf, chunkOps, deleteOps, desiredOps, opKey, type SyncKey } from "./batch.ts";
import { pushOnce } from "./push.ts";
import { formatCursor, parseCursor } from "../hub/cursor.ts";
import { breaking } from "../hub/wire.ts";
import { GraphMotorError } from "../errors.ts";
import { RemoteError } from "../remote/errors.ts";
import type { HubCaller } from "../hub/call.ts";
import type { Ingest } from "../adapters/rows.ts";
import type { AnswerWire, BatchWire } from "../hub/wire.ts";

/** The records route's default page, §5.2's "default 1 000".
 *
 * Caveat: this is the route's default, not a bound on the plugin. A plugin with more than a
 * page of records is read over several pages, and every page must agree on `plugin_seq` or the
 * read is thrown away — see `readStored`.
 */
export const DEFAULT_PAGE = 1000;

/** A sync that could not be completed within its restart bound (§7). */
export class SyncRestartError extends GraphMotorError {
  override readonly name = "SyncRestartError";
}

/** "Start the sync over." Private, and deliberately not `SyncRestartError`: one of these is
 * recoverable and the other is the give-up, and the loop must not confuse them. */
class RestartSignal extends GraphMotorError {
  override readonly name = "SyncRestart";
}

/** The plugin's stored ids, and the `plugin_seq` of the **first** records page. */
export interface StoredPage {
  readonly keys: readonly SyncKey[];
  readonly pluginSeq: string;
}

/** Everything `syncOnce` needs that is not the caller, the workspace or the ingest. */
export interface SyncOptions {
  readonly plugin: string;
  readonly maxBatch: number;
  readonly page: number;
  readonly nextKey: () => string;
  readonly wait?: ((ms: number) => Promise<void>) | undefined;
}

/** Runs the bounded restart loop: `syncOnce` until it settles, or `restarts` restarts have not
 * been enough.
 *
 * Caveat: a sync restart is bounded at 3. §7 says a 412 or a changed `plugin_seq`
 * "starts the sync over" without a bound; unbounded, two writers of one plugin
 * livelock each other. Raise `restarts` for a workspace whose plugin is contended.
 */
export async function syncWithRestarts(
  caller: HubCaller,
  options: SyncOptions,
  workspace: string,
  ingest: Ingest,
  restarts: number,
): Promise<readonly AnswerWire[]> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await syncOnce(caller, options, workspace, ingest);
    } catch (thrown) {
      if (!(thrown instanceof RestartSignal)) throw thrown;
      if (attempt >= restarts) {
        throw new SyncRestartError(`sync restarted ${attempt + 1} times and never settled: ${thrown.message}`);
      }
    }
  }
}

/** One full attempt: read the stored ids, work out the difference, push it.
 *
 * `If-Match` is the first page's `plugin_seq` on the first batch and, after each batch that
 * applied something, that batch's own `seq` (§7). `head_seq` is never read: another plugin's
 * writes would turn every batch into a 412.
 */
export async function syncOnce(
  caller: HubCaller,
  options: SyncOptions,
  workspace: string,
  ingest: Ingest,
): Promise<readonly AnswerWire[]> {
  const first = await readStored(caller, workspace, options.plugin, options.page);
  const epoch = parseCursor(first.pluginSeq).epoch;
  const desired = desiredOps(ingest);
  // Every key this run already names — an upsert *and* a `deleted: true` row — so `deleteOps`
  // cannot add the same id a second time and earn a 422 (§5.2).
  const wanted = new Set(desired.map(opKey));
  const ops = [...desired, ...deleteOps(first.keys, wanted)];
  const answers: AnswerWire[] = [];
  let expected = first.pluginSeq;
  for (const chunk of chunkOps(ops, options.maxBatch)) {
    if (chunk.length === 0) continue;
    const answer = await pushChunk(caller, options, workspace, batchOf(chunk), expected);
    answers.push(answer);
    // §7: the expected `plugin_seq` becomes that batch's seq once it applied something. An
    // all-no-op batch takes the same lock and stores its idempotency row but does not move the
    // seq, so advancing on it would make the next batch's own `If-Match` wrong.
    if (answer.applied > 0) expected = formatCursor({ epoch, seq: answer.seq });
  }
  return answers;
}

/** The plugin's own stored ids, and the `plugin_seq` of the first page.
 *
 * §5.2: the route pages with an opaque `next` and carries `plugin_seq` on every page. A later
 * page that disagrees with the first means the plugin was written from under the read and the
 * ids below are a mix of two histories, so this throws the restart signal rather than a diff
 * built from half of each.
 */
export async function readStored(caller: HubCaller, workspace: string, plugin: string, page: number): Promise<StoredPage> {
  const keys: SyncKey[] = [];
  let cursor: string | undefined;
  let pluginSeq: string | undefined;
  for (;;) {
    const answer = await caller.callJson({ method: "GET", route: recordsRoute(workspace, plugin, page, cursor) });
    const found = JSON.parse(await textOf(answer)) as RecordsPageWire;
    pluginSeq ??= found.plugin_seq;
    if (found.plugin_seq !== pluginSeq) {
      throw new RestartSignal(`plugin_seq moved under the read: ${pluginSeq} then ${found.plugin_seq}`);
    }
    for (const record of found.records) keys.push({ collection: bareCollection(plugin, record.collection), id: record.id });
    if (found.next === undefined || found.next === null) return { keys, pluginSeq };
    cursor = found.next;
  }
}

/** The route names a collection as `<plugin>.<collection>` (the id it is stored under) and a
 * batch body must name it bare (`a_qualified_collection_in_a_body_is_422`), so a stored key is
 * stripped here, once, before `deleteOps` compares it with the ingest's own. */
function bareCollection(plugin: string, collection: string): string {
  const prefix = `${plugin}.`;
  if (!collection.startsWith(prefix)) {
    throw new GraphMotorError(`records of plugin ${plugin} named collection ${collection}, outside ${prefix}*`);
  }
  return collection.slice(prefix.length);
}

async function pushChunk(
  caller: HubCaller,
  options: SyncOptions,
  workspace: string,
  batch: BatchWire,
  expected: string,
): Promise<AnswerWire> {
  // The `head-seq-if-match` break sends the workspace head instead of this plugin's own seq.
  // §5.1 makes the real `If-Match` per plugin; `plugin_sync_if_match_is_per_plugin` is the test
  // that reds, because a head precondition is a 412 for every other plugin's write.
  const ifMatch = breaking("head-seq-if-match") ? '"0.999"' : `"${expected}"`;
  try {
    return await pushOnce(caller, workspace, options.plugin, batch, {
      key: options.nextKey(),
      wait: options.wait,
      ifMatch,
    });
  } catch (thrown) {
    if (thrown instanceof RemoteError && thrown.status === 412) throw new RestartSignal("If-Match did not hold");
    throw thrown;
  }
}

/** `GET /v1/workspaces/{ws}/plugins/{plugin}/records?cursor=&limit=` (§5.2). */
function recordsRoute(workspace: string, plugin: string, page: number, cursor: string | undefined): string {
  const query = new URLSearchParams({ limit: String(page) });
  if (cursor !== undefined) query.set("cursor", cursor);
  return `/v1/workspaces/${encodeURIComponent(workspace)}/plugins/${encodeURIComponent(plugin)}/records?${query.toString()}`;
}

interface RecordsPageWire {
  readonly plugin_seq: string;
  readonly records: readonly { readonly collection: string; readonly id: string; readonly rev: number }[];
  readonly next?: string | null;
}
