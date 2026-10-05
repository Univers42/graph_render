// One batch, sent to the hub's batches route, retried on the three refusals §7 says to retry and
// on nothing else.
//
// The idempotency key is the whole reason this module exists. A batch that committed and whose
// answer was lost must be re-sent under the *same* key, or the hub applies it twice; §5.1 keeps
// that key per (workspace, plugin, key) for 24 h and hashes the body, so the re-send is a replay
// that adds no seq. A fresh key per attempt would defeat exactly that.

import { textOf } from "../hub/call.ts";
import { breaking } from "../hub/wire.ts";
import { RemoteError } from "../remote/errors.ts";
import type { HubCaller } from "../hub/call.ts";
import type { AnswerWire, BatchWire } from "../hub/wire.ts";

/** One try and three retries (§7). */
export const PUSH_ATTEMPTS = 4;

/** How one `pushOnce` is driven. Everything that is not the request is injected. */
export interface PushOptions {
  /** The `Idempotency-Key` to send. Defaults to one `randomUUID()` for the whole call. */
  readonly key?: string;
  /** Waits `ms` between attempts. Defaults to `setTimeout`; injected in every test. */
  readonly wait?: (ms: number) => Promise<void>;
  /** `If-Match: "<epoch>.<plugin_seq>"`, verbatim. §5.2 makes it optional and per plugin. */
  readonly ifMatch?: string;
}

/** Sends `batch` once, retrying only what §7 says to retry, and answers the hub's `{seq, applied}`.
 *
 * The four attempts share one key, so a transport error or a 503 after a commit is a replay and
 * not a second write. `If-Match` is sent on every attempt for the same reason: a 412 is a restart
 * for `sync`, and re-sending the batch under a fresh precondition would hide that.
 */
export async function pushOnce(
  caller: HubCaller,
  workspace: string,
  plugin: string,
  batch: BatchWire,
  options: PushOptions,
): Promise<AnswerWire> {
  const route = batchesRoute(workspace, plugin);
  const body = JSON.stringify(batch);
  // Read once, outside the loop. The `new-key-per-retry` break moves the read inside it and
  // `plugin_push_reuses_the_idempotency_key` is the test that reds; a key the caller gave is
  // the caller's own and the break does not touch it.
  const given = options.key;
  const minted = given ?? randomKey();
  for (let attempt = 1; ; attempt += 1) {
    const key = given ?? (breaking("new-key-per-retry") ? randomKey() : minted);
    try {
      return await send(caller, route, body, key, options);
    } catch (thrown) {
      if (attempt >= PUSH_ATTEMPTS || !isRetryable(thrown)) throw thrown;
      await (options.wait ?? defaultWait)(retryDelayMs(thrown, attempt));
    }
  }
}

// Caveat: the delay is `Retry-After` when the hub sent one and `1000 × attempt` otherwise. §7
// fixes the retry count and the key, not the delay; a zero delay would spin on a hub that is
// answering 503 because its pool is short. `Retry-After` is seconds, per RFC 9110, and it wins
// because it is the only number the hub chose rather than one this SDK guessed.
export function retryDelayMs(error: unknown, attempt: number): number {
  const sent = error instanceof RemoteError ? error.retryAfter : undefined;
  return sent === undefined ? 1000 * attempt : sent * 1000;
}

/** Whether §7 says to try this batch again: 429, 503, and a transport error.
 *
 * Status 0 is `unreachable` (`src/remote/errors.ts:109`), so a connection that dropped mid-batch
 * is retried under the same key and the hub's idempotency row decides whether it was already
 * applied. Everything else is a verdict about this batch — too large, invalid, out of date — and
 * re-sending it unchanged would only earn the same refusal.
 */
export function isRetryable(error: unknown): boolean {
  return error instanceof RemoteError && (error.status === 429 || error.status === 503 || error.status === 0);
}

async function send(caller: HubCaller, route: string, body: string, key: string, options: PushOptions): Promise<AnswerWire> {
  const headers: Record<string, string> = { "Idempotency-Key": key };
  if (options.ifMatch !== undefined) headers["If-Match"] = options.ifMatch;
  const answer = await caller.callJson({ method: "POST", route, body, headers });
  return JSON.parse(await textOf(answer)) as AnswerWire;
}

/** `POST /v1/workspaces/{ws}/plugins/{plugin}/batches` (§5.2). */
function batchesRoute(workspace: string, plugin: string): string {
  return `/v1/workspaces/${encodeURIComponent(workspace)}/plugins/${encodeURIComponent(plugin)}/batches`;
}

// Decision 7: `globalThis.crypto.randomUUID()`, never `node:crypto` — §3 puts this SDK in the
// browser path, and 36 bytes is under §5.1's 128-byte cap.
function randomKey(): string {
  return globalThis.crypto.randomUUID();
}

function defaultWait(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
