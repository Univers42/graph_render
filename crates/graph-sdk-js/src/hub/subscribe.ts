// `subscribe` — the change-stream reader of §7. It is a loop over one SSE stream at a time:
// `GET /events` says "there is something after your cursor", `/changes` says what, and the
// cursor only ever moves forward. There is no `EventSource` (it cannot send `Authorization`),
// no polling timer, and no second concurrent read of `/changes`.

import { textOf } from "./call.ts";
import { formatCursor, parseCursor, type Cursor } from "./cursor.ts";
import { sseFrames, type SseFrame } from "./sse.ts";
import { busyBackoff, systemRandom } from "./backoff.ts";
import { breaking } from "./wire.ts";
import type { ChangeWire } from "./wire.ts";
import type { HubCaller } from "./call.ts";
import { RemoteError, unreadable } from "../remote/errors.ts";

const JSON_MEDIA_TYPE = "application/json";
const STREAM_MEDIA_TYPE = "text/event-stream";

/** The changes route's default page, §5.2's "default 1 000".
 *
 * Caveat: this is the route's default, not a claim about the hub. A page is a byte-bounded read
 * (`GRAPH_HUB_CHANGES_BYTES`), so a change larger than that bound ends the page early and the
 * drain reads again; the page size bounds a single read, never the number of changes delivered.
 */
const DEFAULT_PAGE = 1000;

/** How a subscriber is driven. Every clock and every source of randomness is injected, so a
 * test drives the whole loop with no wall-clock and no `Math.random` in a tested path. */
export interface SubscribeOptions {
  /** Where to start: `<epoch>.<seq>`. Read into the `?since=` query of the first stream. */
  readonly since?: string;
  /** One change, in `seq` order, exactly once. */
  readonly onChange: (change: ChangeWire) => void;
  /** The cursor is no longer valid: read `/graph` and start over. Called at most once, and
   * `subscribe` stops after it (plan Decision 8). */
  readonly onResync: () => void;
  /** Waits `ms`. Defaults to `setTimeout`; injected in every test. */
  readonly wait?: (ms: number) => Promise<void>;
  /** A uniform float in `[0, 1)`. Defaults to `globalThis.crypto`; injected in every test. */
  readonly random?: () => number;
  /** The `/changes` page size. Defaults to the route's 1 000. */
  readonly page?: number;
  /** A read that failed with something other than a 410 — a 503 from a busy hub, a network
   * error. The loop backs off and reconnects from its cursor either way; this only reports it. */
  readonly onError?: (cause: unknown) => void;
}

/** What one pass of the stream did, so the loop can decide between a backoff and a resync. */
export interface StreamOutcome {
  readonly kind: "resync" | "busy" | "end";
  /** Whether a change reached `onChange` on this pass — the only thing that resets the ladder. */
  readonly delivered: boolean;
}

/** Where the reader is. `cursor` is `""` until the first change is delivered, which is what
 * tells the first `GET /events` to put `since` in the query instead of in `Last-Event-ID`. */
export interface SubscribeState {
  cursor: string;
  seq: number;
  target: number;
  epoch: number;
}

/** Subscribes to `workspace` and returns the function that stops it.
 *
 * The loop is total: `resync` calls `onResync` once and stops (plan Decision 8), and `busy` and a
 * closed stream both back off and reconnect from the cursor (plan Decision 3 — the hub also
 * closes when a heartbeat write fails, so reconnecting at once would hot-loop against that).
 */
export function subscribe(caller: HubCaller, workspace: string, options: SubscribeOptions): () => void {
  const state = stateOf(options.since);
  const backoff = busyBackoff(options.random ?? systemRandom);
  const wait = options.wait ?? defaultWait;
  let steps = 0;
  let stopped = false;
  void loop();
  return () => {
    stopped = true;
  };

  async function loop(): Promise<void> {
    for (;;) {
      if (stopped) return;
      const outcome = await passOf(caller, workspace, state, options);
      if (stopped) return;
      if (outcome.kind === "resync") {
        options.onResync();
        return;
      }
      // §7: "reset to 1 s after a delivered change". `busy` and a closed stream are the two
      // reasons to wait, and neither is a reason to forget that the stream was making progress.
      if (outcome.delivered) steps = 0;
      await wait(backoff(steps));
      steps += 1;
    }
  }
}

// A thrown read is a closed stream, not the end of the subscription: `loop` runs detached, so a
// rejection out of it would be unhandled, and Node ends the process on one. A 503 from a hub
// whose read permits are taken is the common case.
async function passOf(
  caller: HubCaller,
  workspace: string,
  state: SubscribeState,
  options: SubscribeOptions,
): Promise<StreamOutcome> {
  try {
    return await runStream(caller, workspace, state, options);
  } catch (cause) {
    options.onError?.(cause);
    return { kind: "end", delivered: false };
  }
}

/** One pass of the stream, from opening `GET /events` to whatever ended it.
 *
 * Exported so a test can drive exactly one stream and read the outcome, with no loop, no
 * backoff and no second stream to reason about.
 */
export async function runStream(
  caller: HubCaller,
  workspace: string,
  state: SubscribeState,
  options: SubscribeOptions,
): Promise<StreamOutcome> {
  const opened = await open(caller, workspace, state, options);
  if (opened.kind !== "open") return opened.outcome;
  let delivered = false;
  for await (const frame of sseFrames(opened.body)) {
    if (frame.event === "resync") return { kind: "resync", delivered };
    if (frame.event === "busy") return { kind: "busy", delivered };
    if (frame.event !== "change") continue;
    if (options.since === undefined && state.epoch === 0) placeAt(state, frame);
    const notice = noticeSeq(frame);
    if (notice > state.target) state.target = notice;
    if (state.target <= state.seq) continue;
    // Awaited inside this one frame loop, so a notice that arrives while a read is in flight
    // stays in the stream buffer and only raises the target on the next turn. That is the
    // coalescing of §7: notices raise a target, they never start a second read.
    if (!(await drain(caller, workspace, state, options))) return { kind: "resync", delivered };
    delivered = true;
  }
  return { kind: "end", delivered };
}

type Opened = { kind: "open"; body: ReadableStream<Uint8Array> } | { kind: "closed"; outcome: StreamOutcome };

// `GET /events` with `Last-Event-ID` once there is a cursor, `?since=` on the first open. A 410
// is a resync. Any other non-2xx is treated as a closed stream and reconnected from the cursor.
//
// Caveat: a 401 or a 403 is not distinguished from a closed stream here, so a key that lost its
// grant reconnects at the top of the ladder for as long as the caller keeps the subscription
// alive rather than throwing once. The ladder caps the rate at one attempt per 15 s, and a
// subscriber that must fail fast is a caller's own `onResync`/abort, not this loop.
async function open(
  caller: HubCaller,
  workspace: string,
  state: SubscribeState,
  options: SubscribeOptions,
): Promise<Opened> {
  const resuming = state.cursor !== "";
  const route = resuming ? eventsRoute(workspace) : sinceRoute(workspace, options.since);
  const headers = resuming ? { "Last-Event-ID": state.cursor } : undefined;
  const answer = await caller.call({ method: "GET", route, accept: STREAM_MEDIA_TYPE, headers });
  if (answer.status === 410) return { kind: "closed", outcome: { kind: "resync", delivered: false } };
  if (answer.status < 200 || answer.status >= 300) return { kind: "closed", outcome: { kind: "end", delivered: false } };
  if (answer.body === null) return { kind: "closed", outcome: { kind: "end", delivered: false } };
  return { kind: "open", body: answer.body };
}

// Reads `/changes` until the target is reached. `false` means the cursor is no longer good —
// a 410, or a first change that is not `cursor + 1` — and the caller resyncs.
//
// The `cursor + 1` test is the only guard against a silently skipped change: a stream that
// resumes one seq late, or a page that starts past the cursor, loses a record and nothing else
// in this package notices. `negctl-hub-sdk-gap` turns it off and this function is what reds.
async function drain(
  caller: HubCaller,
  workspace: string,
  state: SubscribeState,
  options: SubscribeOptions,
): Promise<boolean> {
  for (;;) {
    if (state.target <= state.seq) return true;
    const changes = await readChanges(caller, workspace, state, options);
    if (changes === null) return false;
    if (changes.length === 0) return true;
    for (const change of changes) {
      if (change.seq !== state.seq + 1 && !breaking("no-gap-check")) return false;
      state.seq = change.seq;
      state.cursor = formatCursor({ epoch: state.epoch, seq: change.seq });
      options.onChange(change);
    }
  }
}

// One `/changes` page, or `null` for a 410. Every other refusal is the typed `RemoteError`
// `callJson` already built, key scrubbed and all, so a 401 is not re-spelled here.
async function readChanges(
  caller: HubCaller,
  workspace: string,
  state: SubscribeState,
  options: SubscribeOptions,
): Promise<ChangeWire[] | null> {
  const limit = options.page ?? DEFAULT_PAGE;
  // The position, not `state.cursor`: the cursor is `""` until the first delivery, and the hub
  // refuses an empty `since` (`routes/changes.rs` `since_of`).
  const since = formatCursor({ epoch: state.epoch, seq: state.seq });
  const route = `/v1/workspaces/${encodeURIComponent(workspace)}/changes?since=${encodeURIComponent(since)}&limit=${limit}`;
  try {
    const answer = await caller.callJson({ method: "GET", route, accept: JSON_MEDIA_TYPE });
    // `{epoch, head_seq, next, bytes, changes}` (hub-api.md, `routes/changes.rs` `page_body`).
    const page = JSON.parse(await textOf(answer)) as { changes?: unknown };
    if (!Array.isArray(page.changes)) throw unreadable(route, "the page has no changes array");
    return page.changes as ChangeWire[];
  } catch (cause) {
    if (cause instanceof RemoteError && cause.status === 410) return null;
    throw cause;
  }
}

// A notice carries `data: {"seq":..,"plugin":..,"at":..}` and `id: <epoch>.<seq>` (§5.3), and
// the two are the same position. The `id` is read as the fallback rather than a bad body being
// ignored: a notice this client drops is a change nobody comes back for, and `0` is a seq that
// can never be. A frame with neither is a frame this client cannot place, and the only safe
// answer to that is the target it already has.
function noticeSeq(frame: SseFrame): number {
  return seqOf(frame.data) ?? idOf(frame)?.seq ?? 0;
}

// No `since`: the hub starts the stream at the workspace's head (`events.rs` `cursor_of`), so the
// first notice's `id` names the epoch, and the change before it is where this subscriber stands.
// A notice without a readable `id` leaves the epoch at 0, and the hub answers that cursor's first
// `/changes` with a 410, which is a resync.
function placeAt(state: SubscribeState, frame: SseFrame): void {
  const at = idOf(frame);
  if (at === undefined || at.seq === 0) return;
  state.epoch = at.epoch;
  state.seq = at.seq - 1;
  state.target = state.seq;
}

function idOf(frame: SseFrame): Cursor | undefined {
  if (frame.id === undefined) return undefined;
  try {
    return parseCursor(frame.id);
  } catch {
    return undefined;
  }
}

function seqOf(data: string): number | undefined {
  try {
    const seq: unknown = (JSON.parse(data) as { seq?: unknown }).seq;
    return typeof seq === "number" ? seq : undefined;
  } catch {
    return undefined;
  }
}

function stateOf(since: string | undefined): SubscribeState {
  if (since === undefined) return { cursor: "", seq: 0, target: 0, epoch: 0 };
  const { epoch, seq } = parseCursor(since);
  return { cursor: "", seq, target: seq, epoch };
}

function eventsRoute(workspace: string): string {
  return `/v1/workspaces/${encodeURIComponent(workspace)}/events`;
}

function sinceRoute(workspace: string, since: string | undefined): string {
  return since === undefined ? eventsRoute(workspace) : `${eventsRoute(workspace)}?since=${encodeURIComponent(since)}`;
}

function defaultWait(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
