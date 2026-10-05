# graph-hub slice 4 (hub-sdk) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `graph-sdk-js` gains a `hub` reader (`graph`, `layout`, `subscribe`), a `plugin` writer (`register`, `push`, `sync`), an example plugin, and the rows `hub-sync` and `hub-sdk`.

**Architecture:** TypeScript only, inside the existing package. `src/hub.ts` and `src/plugin.ts` are thin
factories over child modules under `src/hub/` and `src/plugin/`; every HTTP call goes through one
`HubCaller` built on `remoteConfigOf`, so the `fetch` injection, the API-key grammar check and the
`dangerouslyAllowBrowser` rule are reused, never re-implemented. The wire types are imported by name
from `crates/graph-contract/generated/hub.d.ts`, which slice 1 generates. Subscribe parses SSE off a
`fetch` body stream, never `EventSource`, because `EventSource` cannot send `Authorization`. Unit
tests run against an in-process fake `fetch`, so they need no network and no hub; the two LIVE rows
are written here and run once slice 3 lands.

**Tech Stack:** TypeScript on `node --test --experimental-strip-types` (node 22), `tsc --noEmit` from
the repo root's installed `typescript`, `eslint`. No new npm dependency. Toolchain only through
`scripts/orch/node-slim.sh`, `scripts/orch/gr`, `scripts/orch/drun`, `scripts/orch/gate.sh`.

**Spec:** `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1; §5.2 routes,
§5.3 cursors / notices / SSE / 410 / busy, §6 `GRAPH_HUB_MAX_BATCH`, §7 all of it, §8 rows `hub-sync`
and `hub-sdk`, §10 slice 4, §14(c) hub-sdk 1–4, §15(c) hub-sdk 5, §16(b) hub-sdk 6). Verdict:
`docs/decisions/graph-hub.md`.

## Global Constraints

- House limits: ≤ 40 lines per function, ≤ 4 parameters, ≤ 300 lines per file. `src/hub.ts` and
  `src/plugin.ts` split into `src/hub/` and `src/plugin/` child modules rather than growing.
- No new npm dependency. `package.json` changes are additive `exports` entries only.
- Every heuristic carries a `Caveat:` line: the busy jitter, the `Retry-After`-less push delay, the
  bounded `sync` restart count, the clean-end-of-stream reconnect.
- Reuse, never duplicate: `remoteConfigOf`, `RemoteConfig`, `FetchLike`, `RemoteOptions`,
  `LayoutRequest` from `src/remote/options.ts`; `RemoteError`, `refusalOf`, `unreachable`, `scrub`
  from `src/remote/errors.ts`; `decodeSnapshot`, `Snapshot`, `SNAPSHOT_MEDIA_TYPE` from
  `src/remote.ts`; `type Ingest` from `src/adapters/rows.ts`. `src/remote*`, `src/snapshot*` are read
  only — never edited.
- The wire types are imported from `crates/graph-contract/generated/hub.d.ts` by their generated
  names, exactly: `HubWire`, `ManifestWire`, `BatchWire`, `UpsertWire`, `DeleteWire`, `AnswerWire`,
  `ChangeWire`, `NoticeWire`, `ErrorWire`, `Collection`, `Field`, `Link`, `Cardinality`, `ChangeKind`,
  `Role`, `StoredRecord`, `StoredDelete`, `JsonValue`. The import path is
  `../../../graph-contract/generated/hub.d.ts` from `src/hub/wire.ts`, the same shape
  `src/types.ts:7` already uses for `snapshot-header.d.ts`.
- The SDK never retries `/layout` (§5.3, §7): a failed layout reaches the caller as `RemoteError`.
- The SDK sends `If-Match` per plugin with the first records page's `plugin_seq`, never a workspace
  `head_seq`; two plugins writing one workspace must not starve each other (§5.1, `hub-authz`).
- No wall-clock in the tested paths: the backoff clock, the push delay and the idempotency-key
  generator are all injectable, and the tests inject them.
- Nothing under `server/**`, `crates/graph-contract/**`, the root `package.json`, the lockfile,
  `docs/contract/delta.md`, `docs/contract/service-api.md`, `push-once.sh`, `push-loop.sh` or
  `.claude/rules/devil/` is touched. `src/index.ts` is not touched either: `./hub` and `./plugin`
  are reached through `exports`, not the barrel.

## Decisions recorded here (spec silent or illustrative)

1. `./hub` **and** `./plugin` are both added to `exports`. The brief names `./hub`; `plugin.ts` needs
   its own subpath too, and the map is flat and additive, so this is the smallest way to reach both.
2. `src/hub/call.ts` declares its own `const JSON_MEDIA_TYPE = "application/json"`. The one in
   `src/remote.ts:20` is module-private and reaching it would mean editing `src/remote*`. Two
   constants hold the same string; a change to one would miss the other. Recorded, not a heuristic.
3. A clean end of the event stream is treated like `busy` (back off and reconnect from the cursor),
   not as an immediate reconnect. The hub closes a stream on `resync`, on `busy`, or when a
   heartbeat write fails; reconnecting at once would hot-loop against the last case.
4. `push` waits `1000 × attempt` ms between retries when the answer carries no `Retry-After`. §7
   fixes the retry count and the key, not the delay; a zero delay would spin on a saturated hub.
   `Retry-After` wins when the hub sends one.
5. `sync` restarts at most 3 times on a 412 or a changed `plugin_seq`, then throws
   `SyncRestartError`. §7 says "starts the sync over" without a bound; unbounded is a hot loop
   against a plugin another writer holds.
6. The records page size defaults to 1 000 and the batch size to 10 000 (`GRAPH_HUB_MAX_BATCH`),
   both overridable, matching the route's default and max of §5.2.
7. The idempotency key defaults to `globalThis.crypto.randomUUID()`, never `node:crypto`: §3 puts
   the SDK in the browser path. Tests inject a counter.
8. `onResync` is called at most once per `subscribe`, and `subscribe` then stops. A second resync
   would read a second whole `/graph` from a pool the first resync already stressed (§5.3).

## Review Focus

1. `9007199254740992.1` parsed as one number is about 9.007 × 10^15 and reads back as a plausible
   epoch; only the split reader sees that the epoch half is at 2^53 and refuses it
   (`hub_cursor_refuses_two_to_the_53`).
2. Two `/changes` pages whose windows overlap must reach `onChange` in `seq` order, and a notice
   that arrives mid-read must raise the target rather than start a second read
   (`hub_subscribe_coalesces_and_orders`).
3. The first-change-is-`cursor + 1` check is the only guard against a silently skipped change; a
   stream that resumes one seq late loses a record and nothing else notices
   (`hub_subscribe_resyncs_on_an_injected_gap`).
4. A fresh `Idempotency-Key` per retry applies a batch that already committed twice, which is the
   one failure `push` exists to prevent (`plugin_push_reuses_the_idempotency_key`).
5. `If-Match` on `plugin_seq` and not on `head_seq`: the latter makes every other plugin's write a
   412 and starves a workspace with more than one writer
   (`plugin_sync_if_match_is_per_plugin`).

---

### Task 1: `src/hub/wire.ts` and `src/hub/cursor.ts`

**Files:**
- Create: `crates/graph-sdk-js/src/hub/wire.ts`, `crates/graph-sdk-js/src/hub/cursor.ts`, `crates/graph-sdk-js/test/hub-cursor.test.mjs`

**Interfaces:**
- `wire.ts` re-exports by name, from `../../../graph-contract/generated/hub.d.ts`:
  `type ManifestWire, BatchWire, UpsertWire, DeleteWire, AnswerWire, ChangeWire, NoticeWire, ErrorWire,
  ChangeKind, Cardinality, Role, JsonValue`; and `Collection, Field, Link, StoredRecord,
  StoredDelete` (declared in the same generated file).
- `cursor.ts`: `export interface Cursor { readonly epoch: number; readonly seq: number }`;
  `export const MAX_SEQ = 9007199254740991`;
  `export function parseCursor(text: string): Cursor` (throws `GraphMotorError` subclass
  `InvalidCursorError`); `export function formatCursor(cursor: Cursor): string`.

- [ ] **Step 1: Failing tests.** In `test/hub-cursor.test.mjs`, flat `test("name", fn)` from
  `node:test`, `node:assert/strict`, importing `../src/hub/cursor.ts` by relative path with the
  extension (the harness `test/adapters.test.mjs:15`):

```js
test("hub_cursor_parses_a_microsecond_epoch", () => {
  assert.deepEqual(parseCursor("1789000000000000.118"), { epoch: 1789000000000000, seq: 118 });
  assert.equal(formatCursor({ epoch: 1789000000000000, seq: 118 }), "1789000000000000.118");
});

test("hub_cursor_accepts_the_ceiling_and_refuses_one_past_it", () => {
  assert.deepEqual(parseCursor("9007199254740991.0"), { epoch: MAX_SEQ, seq: 0 });
  assert.throws(() => parseCursor("9007199254740992.1"), InvalidCursorError);
  assert.throws(() => parseCursor("0.9007199254740992"), InvalidCursorError);
});

test("hub_cursor_refuses_three_halves_and_a_bare_seq", () => {
  assert.throws(() => parseCursor("1.2.3"), InvalidCursorError);
  assert.throws(() => parseCursor("5"), InvalidCursorError);
});

test("hub_cursor_accepts_the_origin", () => {
  assert.deepEqual(parseCursor("0.0"), { epoch: 0, seq: 0 });
});

test("hub_cursor_refuses_a_non_decimal_half", () => {
  for (const bad of ["1.", ".1", "01.1", "+1.2", " 1.2", "1 .2", "1e3.2", "1.2 "]) {
    assert.throws(() => parseCursor(bad), InvalidCursorError, bad);
  }
});
```

- [ ] **Step 2:** `scripts/orch/node-slim.sh node --test --experimental-strip-types crates/graph-sdk-js/test/hub-cursor.test.mjs` → FAIL (`ERR_MODULE_NOT_FOUND` for `../src/hub/cursor.ts`).
- [ ] **Step 3: Implement `wire.ts`.** A pure re-export module, no logic:

```ts
// The hub wire, as graph-contract's codegen writes it. Named exactly as
// `crates/graph-contract/generated/hub.d.ts` names it, so a change to the Rust
// schema surfaces here as a type error instead of as a wrong payload.
export type {
  AnswerWire, BatchWire, Cardinality, ChangeKind, ChangeWire, DeleteWire, ErrorWire,
  Field, JsonValue, Link, ManifestWire, NoticeWire, Role, UpsertWire,
} from "../../../graph-contract/generated/hub.d.ts";
export type { Collection, StoredDelete, StoredRecord } from "../../../graph-contract/generated/hub.d.ts";
```

- [ ] **Step 4: Implement `cursor.ts`.**

```ts
import { GraphMotorError } from "../errors.ts";

/** The largest `epoch` or `seq` JavaScript reads exactly: 2^53 − 1 (§5). */
export const MAX_SEQ = 9007199254740991;

export interface Cursor {
  readonly epoch: number;
  readonly seq: number;
}

export class InvalidCursorError extends GraphMotorError {
  override readonly name = "InvalidCursor";
}

/** A change-stream position: `<epoch>.<seq>`, two decimal halves below 2^53 (§7). */
export function parseCursor(text: string): Cursor {
  const halves = text.split(".");
  if (halves.length !== 2) throw new InvalidCursorError(`a cursor is <epoch>.<seq>: ${text}`);
  const epoch = halfOf(halves[0]!, text);
  const seq = halfOf(halves[1]!, text);
  return { epoch, seq };
}

export function formatCursor(cursor: Cursor): string {
  return `${cursor.epoch}.${cursor.seq}`;
}

function halfOf(half: string, text: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(half)) throw new InvalidCursorError(`a cursor half is decimal: ${text}`);
  const value = Number(half);
  if (!Number.isSafeInteger(value) || value > MAX_SEQ) {
    throw new InvalidCursorError(`a cursor half is below 2^53: ${text}`);
  }
  return value;
}
```

`Number(half)` on a single decimal half is exact below 2^53 and is not `parseFloat` over the whole
string, which §7 forbids. `InvalidCursorError` extends `GraphMotorError` (`src/errors.ts:48`) so the
existing `catch (e) { e instanceof GraphMotorError }` callers keep working; `GraphMotorError` is not
edited.
- [ ] **Step 5:** the test file PASSes; `scripts/orch/node-slim.sh bash -c 'node node_modules/typescript/bin/tsc -p crates/graph-sdk-js/tsconfig.json --noEmit'` exits 0. Commit (`updated`).

### Task 2: `src/hub/call.ts` — the one caller

**Files:**
- Create: `crates/graph-sdk-js/src/hub/call.ts`, `crates/graph-sdk-js/test/hub-call.test.mjs`

**Interfaces:**
- Consumes: `remoteConfigOf`, `RemoteConfig` from `../remote/options.ts`; `refusalOf`, `unreachable`,
  `RemoteError` from `../remote/errors.ts`; `RemoteOptions`, `FetchLike` from `../remote/options.ts`.
- Produces: `interface HubRequest { readonly method: "GET" | "PUT" | "POST"; readonly route: string;
  readonly accept?: string; readonly body?: string; readonly headers?: Readonly<Record<string, string>> }`;
  `interface HubAnswer { readonly status: number; readonly headers: Readonly<Record<string, string>>;
  readonly body: ReadableStream<Uint8Array> | null }`;
  `interface HubCaller { readonly base: string; readonly key: string | undefined; call(request: HubRequest): Promise<HubAnswer>; callJson(request: HubRequest): Promise<HubAnswer> }`;
  `function hubCaller(options: RemoteOptions): HubCaller`; `function textOf(answer: HubAnswer): Promise<string>`.

- [ ] **Step 1: Failing tests.** An in-process fake `fetch`, injected as `options.fetch`, exactly the
  one-liner `test/remote.test.mjs:54` uses. Cases: the `Authorization` header is
  `Bearer <key>` when a key is given and absent otherwise; `redirect: "error"` is on every request
  (the `remote.ts:69` rule); a 200 answers `{status:200}` with the body still unread;
  `callJson` throws `RemoteError` with `status: 201`? no — **a 400 body `{"error":"invalid","message":"…"}`
  becomes a `RemoteError` whose `status` is 400 and whose message names the path**; a rejecting
  `fetch` becomes a `RemoteError` with `status: 0`; `remoteConfigOf`'s own refusals still fire, so
  `createHub({ baseUrl, apiKey })` in a browser-like global throws `InvalidOptionsError`
  (the `dangerouslyAllowBrowser` rule, `src/remote/options.ts:85-90`); `textOf` decodes UTF-8.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.**

```ts
import { unreachable, type RemoteError } from "../remote/errors.ts";
import { refusalOf } from "../remote/errors.ts";
import { remoteConfigOf, type RemoteConfig, type RemoteOptions } from "../remote/options.ts";

const JSON_MEDIA_TYPE = "application/json";
```

`hubCaller` calls `remoteConfigOf(options)` once and closes over the result, returning the three
members above. `callJson` is `call` plus the non-2xx refusal, so the SSE route can use `call` and
read a 410 itself while every JSON route gets the typed refusal for free. Both share one private
`send(config, request)` of ≤ 20 lines: build the URL from `config.base` and `request.route`, add
`Authorization`, `Content-Type` when there is a body, `Accept` when asked, set
`redirect: "error"`, `await config.fetch(...)`, and hand back `{ status, headers, body }` with the
headers lowercased. A thrown `fetch` becomes `unreachable(request.route, cause, config.key)`
(`src/remote/errors.ts:109`), which scrubs the key.
- [ ] **Step 4:** the tests PASS. Commit.

### Task 3: `src/hub/sse.ts` — the frame reader

**Files:**
- Create: `crates/graph-sdk-js/src/hub/sse.ts`, `crates/graph-sdk-js/test/hub-sse.test.mjs`

**Interfaces:**
- Produces: `interface SseFrame { readonly event: string; readonly id: string | undefined;
  readonly data: string }`;
  `function sseFrames(stream: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame>`.

- [ ] **Step 1: Failing tests.** Drive it with a `ReadableStream` built from an array of string chunks
  (the node 22 web-stream constructor, no dependency). Cases: one `event: change` / `id: 1.5` /
  `data: {…}` block; a frame split across three chunks mid-line and mid-block; `\r\n\r\n` line
  endings; a `:` heartbeat comment produces no frame; two `data:` lines join with `\n`; a block with
  no `event:` line defaults to `message`; a block with no `id:` yields `id: undefined`; two frames
  in one chunk; a trailing block with no blank line at end-of-stream is still emitted; a block whose
  `id:` line has no space after the colon is honoured (`id:1.5`).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.** One exported async generator, ≤ 40 lines: a `buffer` string, a
  `TextDecoder` with `{stream: true}`, a `reader = stream.getReader()`, and a loop that splits the
  buffer on the first `\n\n` or `\r\n\r\n`, parses each block's lines (`field: value`, a leading `:`
  is a comment and ignored), and `yield`s one `SseFrame`. On `done`, flush a non-empty final block.
- [ ] **Step 4:** PASS. Commit.

### Task 4: `src/hub/subscribe.ts` and `src/hub/backoff.ts`

**Files:**
- Create: `crates/graph-sdk-js/src/hub/subscribe.ts`, `src/hub/backoff.ts`,
  `crates/graph-sdk-js/test/hub-subscribe.test.mjs`

**Interfaces:**
- `backoff.ts`: `const BUSY_MIN_MS = 1000`, `const BUSY_MAX_MS = 30_000`;
  `function busyBackoff(random: () => number): (steps: number) => number`;
  `function nextDelay(steps: number): number` (the ladder without jitter, exported for the reset test).
- `subscribe.ts`: `interface SubscribeOptions { readonly since?: string; readonly onChange:
  (change: ChangeWire) => void; readonly onResync: () => void; readonly wait?: (ms: number) =>
  Promise<void>; readonly random?: () => number; readonly page?: number }`;
  `function subscribe(caller: HubCaller, workspace: string, options: SubscribeOptions): () => void`;
  exports `interface StreamOutcome { readonly kind: "resync" | "busy" | "end"; readonly delivered: boolean }`
  and `function runStream(caller: HubCaller, workspace: string, state: SubscribeState, options: SubscribeOptions): Promise<StreamOutcome>` so the tests can drive one stream deterministically.

- [ ] **Step 1: Failing tests.** A fake hub over an injected `fetch` that answers the events route
  with a scripted stream and the changes route with scripted pages. Cases, each its own `test(...)`:

```js
test("hub_subscribe_resyncs_on_an_injected_gap", async () => { /* first page's change.seq is cursor.seq + 3 */ });
test("hub_subscribe_resyncs_on_410_from_changes", async () => { /* /changes answers 410 */ });
test("hub_subscribe_resyncs_on_a_resync_event", async () => { /* stream sends `event: resync` */ });
test("hub_subscribe_coalesces_and_orders", async () => { /* three notices, two pages, one read in flight: exactly two /changes reads, onChange in seq order */ });
test("hub_subscribe_reconnects_with_last_event_id", async () => { /* the second events request carries Last-Event-ID: 1.7 */ });
test("hub_subscribe_busy_backs_off_and_does_not_resync", async () => { /* `event: busy`, onResync never called */ });
test("hub_subscribe_backoff_doubles_from_1s_to_30s_with_upper_half_jitter", () => { /* random() fixed at 0.5: 500, 1000, 2000, 4000, 8000, 16000, 15000, 15000 */ });
test("hub_subscribe_resets_the_backoff_after_a_delivered_change", async () => { /* busy, busy, a change, busy: waits 500, 1000, 500, 1000 */ });
test("hub_subscribe_stops", async () => { /* stop() ends the loop */ });
```

- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement `backoff.ts`.** `nextDelay(steps) = min(1000 * 2 ** steps, 30_000)`;
  `busyBackoff(random)` returns `nextDelay(steps) / 2 + random() * (nextDelay(steps) / 2)`, i.e.
  uniformly in the upper half of each step, so `random() = 0.5` lands on the midpoint of the range the
  test pins. Head comment:

```ts
// Caveat: the jitter is uniform over the upper half of each step and comes from
// `globalThis.crypto.getRandomValues` when the caller injects nothing. It is there so
// subscribers the hub sent `busy` together do not come back together (§7); it is not
// a fairness bound, so a fleet larger than the jitter spread still clusters, and the
// caller can inject `random` to test the ladder exactly.
```

- [ ] **Step 4: Implement `subscribe.ts`.** `SubscribeState` is `{ cursor: string; seq: number;
target: number; epoch: number }`. `subscribe` loops: call `runStream`, then on `resync` call
`options.onResync()` once and return the stop function; on `busy` or `end` `await wait(delay)` with
`steps += 1`; on `resync` from a stream that had already delivered a change the `steps` counter
resets to 0. `steps = 0` after any stream in which a change was delivered (§7's "reset to 1 s after a
delivered change"). `runStream` opens
`GET /v1/workspaces/{ws}/events` with `Last-Event-ID: <cursor>` when the cursor is set (else
`?since=`), returns `resync` on a 410, then iterates `sseFrames`: `resync` → return, `busy` → return
`{kind:"busy"}`, `change` → raise `state.target` to the notice's `seq` and, when
`state.target > state.seq`, run the coalesced drain. The drain reads
`GET /v1/workspaces/{ws}/changes?since=<cursor>&limit=<page>`, returns `false` on a 410 (resync), and
otherwise, for each change in page order, refuses a first change whose `seq !== state.seq + 1`,
advances the cursor to `formatCursor({epoch, seq})`, and calls `options.onChange(change)`. Because the
drain is awaited inside the single frame loop, a notice that arrives while a read is in flight stays
in the stream buffer and only raises the target on the next turn — that is the coalescing of §7, and
`hub_subscribe_coalesces_and_orders` is the test that says so.
- [ ] **Step 5:** PASS. Commit.

### Task 5: `src/hub.ts` — `createHub`

**Files:**
- Create: `crates/graph-sdk-js/src/hub.ts`, `crates/graph-sdk-js/test/hub-graph.test.mjs`
- Modify: `crates/graph-sdk-js/package.json` (`exports`, additive only)

**Interfaces:**
- Consumes: `hubCaller`, `HubCaller`, `textOf`; `sseFrames`; `subscribe`;
  `decodeSnapshot`, `Snapshot`, `SNAPSHOT_MEDIA_TYPE` from `./remote.ts`; `LayoutRequest` from
  `./remote/options.ts`; `parseCursor`, `formatCursor`, `Cursor`.
- Produces: `interface HubGraph { readonly cursor: string; readonly ingest: string }`;
  `interface Hub { readonly graph: (workspace: string) => Promise<HubGraph>;
  readonly layout: (workspace: string, request: LayoutRequest) => Promise<Snapshot>;
  readonly subscribe: (workspace: string, options: SubscribeOptions) => () => void }`;
  `function createHub(options: RemoteOptions): Hub`; and a re-export block of every type above so
  `./hub` is one import for a caller.

- [ ] **Step 1: Failing tests.** `graph` returns `{cursor, ingest}` with `cursor` read from the
  `ETag` header and stripped of its quotes, `ingest` the exact UTF-8 body; a 304 is not treated as a
  success and throws `RemoteError` with `status: 304`; a cursor-shaped ETag is validated by
  `parseCursor` before it is handed back; a missing or malformed ETag throws `RemoteError` (the hub
  always sends one, §5.2); `layout` posts to `/v1/workspaces/{ws}/layout` with the `layout`/`post`
  query of `LayoutRequest`, `Accept: application/vnd.graph-motor.snapshot`, and returns
  `decodeSnapshot(bytes)`; **`hub_layout_is_never_retried`**: a fake that answers 503 every time
  records exactly **one** fetch call.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement `hub.ts`.** Three closures over one `hubCaller(options)`:

```ts
export function createHub(options: RemoteOptions): Hub {
  const caller = hubCaller(options);
  const graph = async (workspace: string): Promise<HubGraph> => {
    const answer = await caller.call({ method: "GET", route: graphRoute(workspace), accept: JSON_MEDIA_TYPE });
    const etag = answer.headers["etag"];
    if (etag === undefined) throw unreadable("GET /graph", "no ETag");
    parseCursor(etag.replace(/^"|"$/g, ""));
    return { cursor: etag.replace(/^"|"$/g, ""), ingest: await textOf(answer) };
  };
  const layout = async (workspace: string, request: LayoutRequest): Promise<Snapshot> => {
    const answer = await caller.call({ method: "POST", route: layoutRoute(workspace, request), accept: SNAPSHOT_MEDIA_TYPE });
    return decodeSnapshot(await bytesOf(answer));
  };
  return { graph, layout, subscribe: (workspace, given) => subscribe(caller, workspace, given) };
}
```

`unreadable` is imported from `../remote/errors.ts` (`:114`) and is not re-implemented. The `304`
case is refused rather than answered: `If-None-Match` is the caller's business and `graph` has no
conditional header of its own.
- [ ] **Step 4:** add to `package.json` `exports`, keeping the four existing keys byte-identical:

```json
"./hub": "./src/hub.ts",
"./plugin": "./src/plugin.ts"
```

The `./plugin` target lands in Task 8, so add both keys in this step only if `./src/plugin.ts`
exists; otherwise add `"./hub"` now and `"./plugin"` in Task 8. Run the test file and
`tsc --noEmit`: PASS, exit 0. Commit.

### Task 6: `src/plugin/batch.ts` — an `Ingest` into batches

**Files:**
- Create: `crates/graph-sdk-js/src/plugin/batch.ts`, `crates/graph-sdk-js/test/plugin-batch.test.mjs`

**Interfaces:**
- Consumes: `type Ingest`, `type IngestRecord` from `../adapters/rows.ts`; `UpsertWire`,
  `DeleteWire`, `BatchWire` from `../hub/wire.ts`.
- Produces: `const DEFAULT_MAX_BATCH = 10000` (`GRAPH_HUB_MAX_BATCH`, §6);
  `interface SyncOp { readonly kind: "upsert" | "delete"; readonly collection: string;
  readonly id: string; readonly updatedAt?: number }`;
  `function desiredOps(ingest: Ingest): readonly SyncOp[]`;
  `function deleteOps(stored: Iterable<SyncKey>, wanted: ReadonlySet<string>): readonly SyncOp[]`;
  `interface SyncKey { readonly collection: string; readonly id: string }`;
  `function chunkOps(ops: readonly SyncOp[], maxBatch: number): readonly SyncOp[][]`;
  `function batchOf(ops: readonly SyncOp[]): BatchWire`;
  `function opKey(op: SyncKey): string` (`"<collection> <id>"`, so byte order and set
  membership are one string).

- [ ] **Step 1: Failing tests.** A record marked `deleted: true` whose id **is** stored becomes a
  delete; a `deleted: true` record whose id is **not** stored is left out entirely (§7, `N14`); every
  other record becomes an upsert carrying its own `updatedAt` and `values` verbatim; ops are ordered by
  `(collection, id)` in **byte** order, so `"a-b"` precedes `"a.b"` and `"A"` precedes `"a"`;
  `chunkOps` gives each chunk at most `maxBatch` operations and never reorders;
  `batchOf` writes `upserts` and `deletes` with the collection **unqualified** and drops `updatedAt`
  and `values` from a delete; a 10 001-op ingest over `chunkOps(…, 10000)` gives two chunks, 10 000
  and 1.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.** `desiredOps` walks `ingest.records`, keeps a record with
  `deleted === false` as an upsert and a record with `deleted === true` as a delete op, and sorts
  both kinds by `opKey` with a plain `<` (UTF-8 byte order for these ASCII ids — the same order the
  hub's `COLLATE "C"` scan uses, §5.3). `deleteOps` takes the stored keys, drops those in `wanted`,
  and returns the rest as delete ops in the same byte order. `chunkOps` walks the list once.
  Head comment on `desiredOps`:

```ts
// A record an adapter marks `deleted: true` becomes a delete when its id is stored
// and is left out of the desired set otherwise (§7, N14). Rows that mark records
// deleted still keep them in `ingest.records`, so the decision belongs here and
// nowhere else.
```

- [ ] **Step 4:** PASS. Commit.

### Task 7: `src/plugin/push.ts` — one batch, retried with one key

**Files:**
- Create: `crates/graph-sdk-js/src/plugin/push.ts`, `crates/graph-sdk-js/test/plugin-push.test.mjs`

**Interfaces:**
- Produces: `const PUSH_ATTEMPTS = 4` (one try plus three retries, §7);
  `interface PushOptions { readonly key?: string; readonly wait?: (ms: number) => Promise<void> }`;
  `function pushOnce(caller: HubCaller, workspace: string, plugin: string, batch: BatchWire, options: PushOptions): Promise<AnswerWire>`;
  `function retryDelayMs(error: unknown, attempt: number): number`;
  `function isRetryable(error: unknown): boolean`.

- [ ] **Step 1: Failing tests.** Table-driven over the fake, the shape `test/remote.test.mjs:82-95`
  uses. Cases: a 200 gives `AnswerWire` `{seq, applied}`; **429, 503 and a rejected `fetch` (status 0)
  are each retried and the second attempt succeeds**; every attempt carries **the same**
  `Idempotency-Key` (`plugin_push_reuses_the_idempotency_key` asserts the four headers are equal and
  that a fresh key was minted once); **400, 409, 412, 413 and 422 are not retried**; four failures
  give up and throw the last `RemoteError`; `Retry-After: 2` is honoured over the default delay;
  `retryDelayMs` returns `1000 * attempt` otherwise; the body is the canonical
  `{"deletes":…,"upserts":…}` the contract writes, with keys in byte order.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement.** `isRetryable` is `error instanceof RemoteError && (error.status === 429 ||
  error.status === 503 || error.status === 0)` — status 0 is `unreachable`, a transport error. The
  loop runs at most `PUSH_ATTEMPTS` times, sends `Idempotency-Key` from `options.key` when given and
  from one `randomUUID()` read outside the loop otherwise, and `await wait(retryDelayMs(...))`
  between tries. Head comment on `retryDelayMs`:

```ts
// Caveat: the delay is `Retry-After` when the hub sent one and `1000 × attempt`
// otherwise. §7 fixes the retry count and the key, not the delay; a zero delay would
// spin on a hub that is answering 503 because its pool is short.
```

- [ ] **Step 4:** PASS. Commit.

### Task 8: `src/plugin/sync.ts` and `src/plugin.ts` — `createPlugin`

**Files:**
- Create: `crates/graph-sdk-js/src/plugin/sync.ts`, `crates/graph-sdk-js/src/plugin.ts`,
  `crates/graph-sdk-js/test/plugin-sync.test.mjs`
- Modify: `crates/graph-sdk-js/package.json` (`exports`, the `"./plugin"` key if Task 5 added only `"./hub"`)

**Interfaces:**
- Consumes: `hubCaller`, `HubCaller`, `textOf`; `parseCursor`, `formatCursor`;
  `type Ingest` from `../adapters/rows.ts`; `desiredOps`, `deleteOps`, `chunkOps`, `batchOf`,
  `opKey`, `DEFAULT_MAX_BATCH` from `./batch.ts`; `pushOnce` from `./push.ts`;
  `ManifestWire`, `AnswerWire` from `../hub/wire.ts`.
- Produces: `class SyncRestartError extends GraphMotorError` (the bounded-restart give-up);
  `interface PluginOptions extends RemoteOptions { readonly plugin: string; readonly manifest:
  ManifestWire; readonly maxBatch?: number; readonly page?: number; readonly restarts?: number;
  readonly key?: () => string; readonly wait?: (ms: number) => Promise<void> }`;
  `interface Plugin { readonly register: (workspace: string) => Promise<ManifestWire>;
  readonly push: (workspace: string, batch: BatchWire, options?: PushOptions) => Promise<AnswerWire>;
  readonly sync: (workspace: string, ingest: Ingest) => Promise<readonly AnswerWire[]> }`;
  `function createPlugin(options: PluginOptions): Plugin`;
  `function readStored(caller: HubCaller, workspace: string, plugin: string, page: number): Promise<StoredPage>`;
  `interface StoredPage { readonly keys: readonly SyncKey[]; readonly pluginSeq: string }`.

- [ ] **Step 1: Failing tests.** A fake hub serving the records route with scripted pages and the
  batches route with scripted answers. Cases:

```js
test("plugin_sync_upserts_every_desired_record_and_deletes_the_rest", async () => { /* 3 upserts, 1 delete of a stored id no longer wanted */ });
test("plugin_sync_deletes_a_deleted_row_when_stored_and_drops_it_otherwise", async () => { /* stored → in the batch's deletes; absent → nowhere */ });
test("plugin_sync_batches_at_max_batch_with_its_own_key", async () => { /* 10 001 ops, maxBatch 10 000: two POSTs, two distinct Idempotency-Keys */ });
test("plugin_sync_sends_if_match_with_the_first_pages_plugin_seq", async () => { /* If-Match: "1.40" */ });
test("plugin_sync_if_match_is_per_plugin", async () => { /* another plugin's batch moved head_seq to 99 and plugin_seq to 40: no 412 */ });
test("plugin_sync_advances_the_expected_seq_after_a_batch_that_applied", async () => { /* applied 3 → next If-Match "1.43"; applied 0 → unchanged */ });
test("plugin_sync_restarts_on_412", async () => { /* the first batch answers 412, the run completes */ });
test("plugin_sync_restarts_when_a_later_page_carries_another_plugin_seq", async () => { /* page 2 plugin_seq 41 ≠ page 1's 40 */ });
test("plugin_sync_gives_up_after_three_restarts", async () => { /* SyncRestartError */ });
test("plugin_sync_sends_no_deletes_when_nothing_is_stored", async () => { /* one batch, deletes: [] */ });
test("plugin_register_puts_the_manifest_and_returns_the_stored_one", async () => { /* PUT /v1/workspaces/ops/plugins/tracker */ });
```

- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: Implement `sync.ts`.** `readStored` pages
  `GET /v1/workspaces/{ws}/plugins/{plugin}/records?limit=<page>` following `next` until it is absent,
  and returns the keys plus the `plugin_seq` of the **first** page; a later page carrying a different
  `plugin_seq` throws the restart signal. `syncOnce(caller, options, workspace, ingest)`:

  1. `const first = await readStored(...)`, `let expected = first.pluginSeq`;
  2. `const wanted = new Set(desiredOps(ingest).filter((op) => op.kind === "upsert").map(opKey))`;
  3. `const ops = [...desiredOps(ingest), ...deleteOps(first.keys, wanted)]` — a `deleted: true`
     record is already an op in the first list, so it is never also a delete in the second;
  4. for each chunk of `chunkOps(ops, maxBatch)`, `pushOnce(caller, workspace, plugin,
     batchOf(chunk), { key: nextKey(), wait })` — but sent through one extra private call that adds
     `If-Match: <expected>` to the request headers. `pushOnce` takes the key through `PushOptions`;
     the header goes on the `HubRequest` inside `syncOnce` by giving `pushOnce` an optional
     `ifMatch` on `PushOptions`. **Change `PushOptions` to
     `{ key?, wait?, ifMatch? }` and Task 7's test asserts the `If-Match` header appears verbatim when
     it is given.**
  5. on a 412, or a changed `plugin_seq`, restart from step 1; on `applied > 0`, set
     `expected = formatCursor({ epoch: firstCursor.epoch, seq: answer.seq })` (§7: "after each batch
     with `applied > 0`, the expected `plugin_seq` becomes that batch's seq").
- [ ] **Step 4: Implement `plugin.ts`.** `createPlugin` builds one `hubCaller(options)` and returns
  three closures: `register` PUTs `manifest` to `/v1/workspaces/{ws}/plugins/{plugin}` with
  `Content-Type: application/json` and returns the parsed `ManifestWire` (201 or 200, §5.2);
  `push` delegates to `pushOnce`; `sync` runs the restart loop, counting attempts against
  `options.restarts ?? 3`, and throws `SyncRestartError` past it. Head comment on the loop:

```ts
// Caveat: a sync restart is bounded at 3. §7 says a 412 or a changed `plugin_seq`
// "starts the sync over" without a bound; unbounded, two writers of one plugin
// livelock each other. Raise `restarts` for a workspace whose plugin is contended.
```
- [ ] **Step 5:** add the `"./plugin"` key to `package.json` `exports` if it is not there; the tests
  PASS; `tsc --noEmit` exits 0. Commit.

### Task 9: `examples/plugins/rows-file/`

**Files:**
- Create: `examples/plugins/rows-file/package.json`, `sync.mjs`, `rows.json`, `README.md`,
  `crates/graph-sdk-js/test/hub-example.test.mjs`

**Interfaces:**
- Consumes: `rowsToIngest` from `crates/graph-sdk-js/src/adapters/rows.ts`; `createPlugin` from
  `crates/graph-sdk-js/src/plugin.ts`; `GRAPH_HUB_SDK_BREAK` (the `hub-sdk` negative control, §8).

- [ ] **Step 1: Failing test.** `test/hub-example.test.mjs` runs `sync.mjs` as a child process
  against an **in-process** fake: it sets `GRAPH_HUB_URL`, `GRAPH_HUB_KEY` and
  `GRAPH_HUB_PLUGIN`, and `sync.mjs` must therefore read those three names. Cases: it exits 0 on
  `rows.json` against a fake that answers every route; **with `GM_HUB_SDK_BREAK=1` it pushes a
  wrong record** — a record whose `title` is `"wrong"` — and the test asserts the batch body carries
  it (this is the §8 negative control, proved without a live hub; the LIVE `hub-sdk` row re-proves it
  against containers).
- [ ] **Step 2:** run, FAIL (`ENOENT` on `sync.mjs`).
- [ ] **Step 3: Write the files.** `package.json`: `{"name": "@graph-motor/example-rows-file",
  "private": true, "type": "module"}`. `rows.json`: a `RowsSource` with one table, one `title` field,
  three records (one with `"deleted": true`), small enough to read in the test.
  `sync.mjs`, ≤ 60 lines and no Node-only API:

```js
#!/usr/bin/env node
// Reads a JSON rows file, maps it with `rowsToIngest` and calls `sync`. The rows
// file is this directory's `rows.json` unless GRAPH_HUB_ROWS says otherwise.
import { readFile } from "node:fs/promises";
import { createPlugin } from "../../../crates/graph-sdk-js/src/plugin.ts";
import { rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";

const rows = JSON.parse(await readFile(process.env.GRAPH_HUB_ROWS ?? new URL("rows.json", import.meta.url), "utf8"));
const source = rowsToIngest(breakOf(rows));
const plugin = createPlugin({
  baseUrl: process.env.GRAPH_HUB_URL ?? "http://127.0.0.1:8081",
  apiKey: process.env.GRAPH_HUB_KEY,
  plugin: process.env.GRAPH_HUB_PLUGIN ?? "rows-file",
  manifest: manifestOf(source),
});
await plugin.register(source.source);
const answers = await plugin.sync(source.source, source);
process.stdout.write(`${answers.length} batches\n`);

// GM_HUB_SDK_BREAK=1 is the `hub-sdk` negative control (§8): the example pushes a
// record that is not in the rows file, so the live row must go red for that reason.
function breakOf(rows) {
  return process.env.GM_HUB_SDK_BREAK === "1"
    ? { ...rows, tables: rows.tables.map((t, i) => (i === 0 ? { ...t, rows: [...t.rows, wrongRow(t)] } : t)) }
    : rows;
}
```

with `manifestOf` building a `ManifestWire` from `source.collections` (fields carry `link: null`
for every non-link role, `role` as the wire's lowercase name) and `wrongRow` a record with
`id: "wrong"` and `title: "wrong"`. `README.md`: 20 lines — what it does, the four environment
names, the exact `hub-sdk` row that runs it, and the Caveat that the rows file is read on every run
so a run is only as fresh as the file.
- [ ] **Step 4:** the test PASSes with and without the break (the break case asserts the wrong
  record **is** sent, so the row is what turns red under `GM_HUB_SDK_BREAK=1`).
  `scripts/orch/node-slim.sh node --check examples/plugins/rows-file/sync.mjs` exits 0. Commit.

### Task 10: `scripts/orch/rows/hub-sdk.rows`

**Files:**
- Create: `scripts/orch/rows/hub-sdk.rows`

- [ ] **Step 1: Write the file.** The rows are `name|expect|cmd`, `expect` is `0` or `nonzero`
  (`scripts/orch/gate.sh:2`), every command goes through `scripts/orch/node-slim.sh`,
  `scripts/orch/gr` or `scripts/orch/drun` — never a bare `node`, `npm`, `cargo` or `docker run` —
  and env goes in through `bash -c 'env K=V …'`, which is the only way `node-slim.sh` takes it
  (`scripts/orch/node-slim.sh` forwards `"$@"` as the container command). A `negctl-*` row expects
  `0` and passes only when the run **fails**, with `test $? -ne 0` plus a `grep` on the TAP line, the
  shape `scripts/orch/rows/sdk-remote.rows:15-16` uses.

```
# Rows for graph-hub slice 4 (hub-sdk). The unit rows need no hub: they run the SDK
# tests against an in-process fake `fetch`. The two LIVE rows need slice 3's
# containers (graph-hub, PostgreSQL, graph-server) and are marked as such.
# Run: scripts/orch/gate.sh <logdir> scripts/orch/rows/hub-sdk.rows
#
# negctl-*: each break must turn exactly one named test red, checked by grepping its
# TAP line, so a red run that fails anywhere else (a missing build, a decode error)
# turns the control red too.
hub-sdk-types|0|scripts/orch/node-slim.sh bash -c 'node node_modules/typescript/bin/tsc -p crates/graph-sdk-js/tsconfig.json --noEmit'
hub-sdk-lint|0|scripts/orch/node-slim.sh bash -c 'node node_modules/eslint/bin/eslint.js crates/graph-sdk-js --max-warnings=0'
hub-sdk-unit|0|scripts/orch/node-slim.sh node --test --experimental-strip-types crates/graph-sdk-js/test/hub-cursor.test.mjs crates/graph-sdk-js/test/hub-call.test.mjs crates/graph-sdk-js/test/hub-sse.test.mjs crates/graph-sdk-js/test/hub-subscribe.test.mjs crates/graph-sdk-js/test/hub-graph.test.mjs crates/graph-sdk-js/test/hub-example.test.mjs crates/graph-sdk-js/test/plugin-batch.test.mjs crates/graph-sdk-js/test/plugin-push.test.mjs crates/graph-sdk-js/test/plugin-sync.test.mjs
negctl-hub-sdk-cursor|0|scripts/orch/node-slim.sh bash -c 'out=$(env GM_HUB_SDK_BREAK=parsefloat node --test --test-reporter=tap --experimental-strip-types crates/graph-sdk-js/test/hub-cursor.test.mjs 2>&1); rc=$?; printf "%s\n" "$out"; test $rc -ne 0 && grep -q "^not ok 8 - hub_cursor_accepts_the_ceiling_and_refuses_one_past_it" <<<"$out"'
negctl-hub-sdk-gap|0|scripts/orch/node-slim.sh bash -c 'out=$(env GM_HUB_SDK_BREAK=no-gap-check node --test --test-reporter=tap --experimental-strip-types crates/graph-sdk-js/test/hub-subscribe.test.mjs 2>&1); rc=$?; printf "%s\n" "$out"; test $rc -ne 0 && grep -q "^not ok 1 - hub_subscribe_resyncs_on_an_injected_gap" <<<"$out"'
negctl-hub-sdk-key|0|scripts/orch/node-slim.sh bash -c 'out=$(env GM_HUB_SDK_BREAK=new-key-per-retry node --test --test-reporter=tap --experimental-strip-types crates/graph-sdk-js/test/plugin-push.test.mjs 2>&1); rc=$?; printf "%s\n" "$out"; test $rc -ne 0 && grep -q "^not ok 8 - plugin_push_reuses_the_idempotency_key" <<<"$out"'
negctl-hub-sdk-ifmatch|0|scripts/orch/node-slim.sh bash -c 'out=$(env GM_HUB_SDK_BREAK=head-seq-if-match node --test --test-reporter=tap --experimental-strip-types crates/graph-sdk-js/test/plugin-sync.test.mjs 2>&1); rc=$?; printf "%s\n" "$out"; test $rc -ne 0 && grep -q "^not ok 5 - plugin_sync_if_match_is_per_plugin" <<<"$out"'
hub-sdk-example|0|scripts/orch/node-slim.sh node --test --experimental-strip-types crates/graph-sdk-js/test/hub-example.test.mjs
negctl-hub-sdk-example|0|scripts/orch/node-slim.sh bash -c 'out=$(env GM_HUB_SDK_BREAK=1 node --test --test-reporter=tap --experimental-strip-types crates/graph-sdk-js/test/hub-example.test.mjs 2>&1); rc=$?; printf "%s\n" "$out"; test $rc -ne 0 && grep -q "the example pushed a record that is not in the rows file" <<<"$out"'
sdk-remote-regression|0|scripts/orch/node-slim.sh node --test --experimental-strip-types crates/graph-sdk-js/test/remote.test.mjs crates/graph-sdk-js/test/snapshot.test.mjs
# ---- LIVE rows: these run once slice 3 (hub-api) lands. They need
# scripts/orch/hub.sh up, plus its PostgreSQL and the existing graph-server. ----
hub-sync|0|scripts/orch/timed scripts/orch/gate.sh <logdir>/hub-sync scripts/orch/rows/hub-sync.rows
hub-sdk|0|scripts/orch/timed scripts/orch/gate.sh <logdir>/hub-sdk scripts/orch/rows/hub-sdk-live.rows
```

  The four `negctl-*` unit rows need the four breaks honoured in the source: `GM_HUB_SDK_BREAK=parsefloat`
  in `cursor.ts` (use one `parseFloat` over the whole string), `no-gap-check` in `subscribe.ts`
  (drop the `cursor + 1` test), `new-key-per-retry` in `push.ts` (mint the key inside the loop),
  `head-seq-if-match` in `sync.ts` (send the workspace `head_seq`). Each is read from
  `globalThis.GM_HUB_SDK_BREAK` through one private `breaking(name)` helper in
  `src/hub/wire.ts`, so the four knobs are four lines, and the shipped path is a `process.env` read
  that is `undefined` in a browser.

- [ ] **Step 2:** `scripts/orch/gate.sh <logdir> scripts/orch/rows/hub-sdk.rows` → every unit row
  exits as declared and each `negctl-*` row exits 0 because its test went red. **The two LIVE rows
  are expected to fail here and are left unrun, marked "runs once slice 3 lands";** report them as
  `not run` in the return block rather than as a pass. Commit.

## Self-review

- Spec §7 bullet → task, each with a named test:
  - `createHub` / `graph` / `layout` / `subscribe` surface → Task 5 (`hub_graph_returns_the_etag_cursor_and_body`, `hub_layout_is_never_retried`), Task 4 (`hub_subscribe_*`).
  - `subscribe` uses `fetch`, no `EventSource` → Task 3 (`sseFrames`), Task 4.
  - notices coalesced, `onChange` in `seq` order → Task 4 (`hub_subscribe_coalesces_and_orders`).
  - 410 / `event: resync` / first change not `cursor + 1` → Task 4 (`hub_subscribe_resyncs_on_410_from_changes`, `…_on_a_resync_event`, `…_on_an_injected_gap`).
  - reconnects with `Last-Event-ID` → Task 4 (`hub_subscribe_reconnects_with_last_event_id`).
  - `busy` backoff 1 s → 30 s, upper-half jitter, reset after a change → Task 4 (`…_backs_off_and_does_not_resync`, `…_backoff_doubles_from_1s_to_30s_with_upper_half_jitter`, `…_resets_the_backoff_after_a_delivered_change`).
  - cursor `<epoch>.<seq>`, split on `.`, each half below 2^53, never `parseFloat` over the string → Task 1 (five `hub_cursor_*` tests).
  - `push` retries 429 / 503 / transport up to 3 times with the same key → Task 7 (`plugin_push_reuses_the_idempotency_key`, `plugin_push_retries_*`, `plugin_push_gives_up_after_four_attempts`).
  - `layout` never retried → Task 5 (`hub_layout_is_never_retried`).
  - `sync` needs only `write:<plugin>`, reads ids from the records route → Task 8 (`plugin_sync_upserts_every_desired_record_and_deletes_the_rest`; the grant is slice 3's `hub-sync` row).
  - upserts every desired record, deletes the ids no longer wanted, batches ≤ `GRAPH_HUB_MAX_BATCH`, each with its own key → Task 6 (`chunk_ops`), Task 8 (`plugin_sync_batches_at_max_batch_with_its_own_key`).
  - `If-Match` with the first page's `plugin_seq`, restart on 412 or a changed `plugin_seq`, expected seq advances after `applied > 0` → Task 8 (`plugin_sync_sends_if_match_with_the_first_pages_plugin_seq`, `plugin_sync_if_match_is_per_plugin`, `plugin_sync_advances_the_expected_seq_after_a_batch_that_applied`, `plugin_sync_restarts_on_412`, `plugin_sync_restarts_when_a_later_page_carries_another_plugin_seq`).
  - an adapter's `deleted: true` becomes a delete when stored, else left out → Task 6 (`plugin_sync_deletes_a_deleted_row_when_stored_and_drops_it_otherwise`; `rows.ts:215`, `notion.ts:259`).
  - `sync` turns any adapter's `Ingest` into batches → Task 8 (`type Ingest` from `../adapters/rows.ts`).
  - browser use keeps `dangerouslyAllowBrowser` → Task 2 (`hub_call_refuses_a_key_in_a_browser`).
  - example plugin `examples/plugins/rows-file/` → Task 9 (`hub-example.test.mjs`, `hub-sdk` row).
- §8 row `hub-sdk` clauses → task: injected gap (Task 4), `busy` without `onResync` (Task 4), 8
  subscribers at once reconnecting at non-identical times (Task 4's ladder, proven by `random()`
  varying; the LIVE row runs the 8-subscriber case), backoff reset (Task 4), a cursor at 2^53 − 1 exact
  and one at 2^53 refused (Task 1), a failed `/layout` not retried (Task 5), negative control
  `GM_HUB_SDK_BREAK=1` (Tasks 9 and 10).
- §8 row `hub-sync` clauses → task: `write:<plugin>` only (slice 3's row; the SDK needs no read
  grant, which the unit tests show by never calling `/graph`), a second `sync` adding no seq and a
  dangling link (Task 8: identical upserts are no-ops and a link to an unregistered collection is a
  cell the hub keeps), a transport error retried with the same key (Task 7), `deleted: true` (Task
  6), two concurrent syncs (`If-Match`, 412, restart — Task 8), negative control `sync-via-graph`
  (Task 10's LIVE rows file).
- §14(c) hub-sdk 1 (`N14`) → Task 6; 2 (`N16`, records paging) → Task 8; 3 (`/layout` not retried)
  → Task 5; 4 (gap injection) → Task 4. §15(c) hub-sdk 5 (busy backoff, µs cursor parsed exactly)
  → Tasks 4 and 1. §16(b) hub-sdk 6 (8 subscribers not identical) → Task 4 and the LIVE row.
- §12 condition 14 (u64 seq in JavaScript, no key in a browser) → Task 1 (`MAX_SEQ`) and Task 2.
- Names used across tasks, each defined once: `HubCaller`, `HubRequest`, `HubAnswer`, `hubCaller`,
  `textOf`, `Cursor`, `parseCursor`, `formatCursor`, `MAX_SEQ`, `InvalidCursorError`, `SseFrame`,
  `sseFrames`, `busyBackoff`, `nextDelay`, `SubscribeOptions`, `SubscribeState`, `subscribe`,
  `runStream`, `StreamOutcome`, `Hub`, `HubGraph`, `createHub`, `Ingest`, `SyncKey`, `SyncOp`,
  `desiredOps`, `deleteOps`, `chunkOps`, `batchOf`, `opKey`, `DEFAULT_MAX_BATCH`, `PushOptions`,
  `pushOnce`, `isRetryable`, `retryDelayMs`, `PUSH_ATTEMPTS`, `SyncRestartError`, `StoredPage`,
  `readStored`, `PluginOptions`, `Plugin`, `createPlugin`, `breaking`.
- Limits honoured: the batch size is `GRAPH_HUB_MAX_BATCH` (10 000, `DEFAULT_MAX_BATCH`), the records
  page size is the route's default 1 000 and its max 10 000 (`options.page`), and the `Idempotency-Key`
  stays under the 128-byte cap of §5.1 (`randomUUID()` is 36).
- Not run here, and named as such: `hub-sync` and the live half of `hub-sdk` need slice 3's
  containers. UNKNOWN is FAIL, so they are reported as `not run`, never as passes.