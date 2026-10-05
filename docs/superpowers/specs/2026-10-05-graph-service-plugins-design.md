# graph-hub: a graph service that plugins feed — design

Date 2026-10-05.

**Status: revision 4, re-submitted to the `devil`.** Revision 1 drew a BLOCK with 15 conditions;
§12 maps them. Revision 2 drew a BLOCK with 18 conditions and defects D1–D13; §13 maps them.
Revision 3 drew a BLOCK with defects N1–N16; §14 maps them, and
`docs/measurements/hub-pg-epoch-probe.md` runs the PostgreSQL facts it listed as unknown. The
decision record is `docs/decisions/graph-hub.md`.

This is phase D1 of `docs/decisions/server-and-write-path.md`: the store and the write path. D2,
the Redis snapshot cache, stays where `docs/decisions/snapshot-cache.md` put it.

## 1. What it is for

The user asked on 2026-10-05 for "a complete SDK and API ... a micro-service plugin system that
we can use and track all kinds of data to be represented as a graph".

So the goal is this. A program (a **plugin**) tracks some data: rows of a table, notes, tickets,
commits, hosts and their links. It pushes those records into a named **workspace**. The
**hub**:

- keeps the records;
- numbers every change;
- notifies anyone subscribed, who then reads the changes;
- serves the workspace as an ingest document, or as a laid-out snapshot computed by the existing
  motor service.

Two kinds of reader use it: plugin authors, in any language over HTTP and JSON, and app
developers, through the TypeScript SDK and later the studio.

v1 is done when each of these holds, each one a gate row (§8) with a negative control:

- A plugin built on the SDK registers its manifest and pushes batches. A second client sees
  every change in `seq` order, and resumes after a disconnect without a gap or a duplicate, even
  while other writers commit and after an operator edits the database by hand.
- `POST /v1/workspaces/{ws}/layout` returns bytes identical to graph-server's `POST /v1/layout`
  (`source=contract`) run on the document `GET /v1/workspaces/{ws}/graph` returns at the same
  `<epoch>.<seq>`.
- That document always passes graph-contract's reader (`ingest::read`), whatever the plugins
  pushed or deleted, and in whatever order.
- Every batch the hub acknowledged survives a container-level kill of the hub, and of
  PostgreSQL, with the data on a named volume.
- A key writes only the plugins it is granted; one plugin's writes never refuse another's.
- A `sync` run twice on unchanged input adds no `seq`.
- Peak memory at every cap at once is measured under `scripts/orch/drun` and stays under the hub
  container's 1 GiB.
- These stay green on the merged tree: the rows `svc-*`, every row of `service-supply.rows` with
  its negative controls failing for their original reason, the wasm32 build of graph-core, and
  `codegen --check`.
- The motor workspace's `Cargo.lock`, graph-core's dependency list, and the features the shipped
  graph-server resolves at the `server/` virtual root do not change.

## 2. Decisions

The user's standing instruction is full autonomy with no questions, so each decision below is
the recommended default, recorded here. Each is open to the verdict.

| # | Decision | Why | Rejected |
|---|---|---|---|
| H1 | Two services. graph-server keeps its stateless compute, unchanged. A new **graph-hub** holds the state and asks graph-server for layouts over HTTP. The hub never **runs** motor math. It does **link** graph-core and graph-wasm, through graph-server's lib (H8); no hub code path calls them. | The user asked for micro-services. Compute keeps its own slots and caps, and scales apart from the hub. | The hub lays out in process: one network hop fewer, but a second copy of the slots and caps. |
| H2 | `server/graph-hub` (a binary plus its lib) and `server/graph-store` (a lib), both members of the `server/` workspace, which gains `default-members = ["graph-server"]`. | `server-and-write-path.md` puts graph-store in `server/`, so nothing under `crates/` reaches either. `default-members` keeps a build at the virtual root without `-p` (`scripts/service.sh:73`) selecting graph-server alone, so the hub's features never unify into the shipped binary (D13). | `crates/graph-store`, which would put a database driver into the motor's lockfile. |
| H3 | PostgreSQL 17 through `tokio-postgres`, image built from Debian (`deploy/postgres.Dockerfile`), with `fsync`, `synchronous_commit` and `full_page_writes` on, data on a named volume. | The user chose it on 2026-09-29 (the ADR); `images-from-debian.md`; `server-dependencies.md` already names `tokio-postgres`. | SQLite has fewer moving parts but contradicts the recorded choice. |
| H4 | The stored model is the ingest contract itself: collections that plugins declare, and records. | One data model for the adapters, the service and the hub. graph-contract's writer stays the only producer of canonical ingest bytes (the ADR), including the hub's streamed documents (H14). | A new property-graph model: a second definition of graph input, plus a mapping layer. |
| H5 | A plugin is an out-of-process client. It owns one namespace (its plugin id) in a workspace and **pushes** to the hub. | Any language works and no plugin code runs inside the hub, so a plugin crash cannot take the hub down. | In-process plugins (wasm, dylib) need a sandbox, an ABI and resource limits. Pull or webhook plugins open SSRF, and need a scheduler and retries. |
| H6 | `seq` comes from a row-locked `UPDATE workspaces SET head_seq = head_seq + 1 ... RETURNING head_seq`, inside the batch's `READ COMMITTED` transaction, in the lock order of §5.1. | The ADR's write model. Writers to one workspace serialize on that row, so seqs have no gap and no duplicate, and they become visible in seq order. A rolled-back batch leaves no hole, which a `SEQUENCE` would. | A PostgreSQL `SEQUENCE`; Redis streams (ADR item D2, a stop-and-ask item). |
| H7 | Live fan-out in process, after commit: a `tokio::sync::watch` per workspace carries `(epoch, head_seq)`, updated only when the new pair is greater. SSE sends **notices** (`seq`, `plugin`, `at`), never change bodies; clients read bodies from `/changes`. | One caller, one node. A notice is about 256 bytes, so subscriber memory does not grow with the change size. A monotone watch cannot step back when two commits publish out of order. | PostgreSQL `LISTEN/NOTIFY` (needed only with two hub instances); bodies on the stream (memory grows with the batch size). |
| H8 | The hub has its **own** keys file, `GRAPH_HUB_KEYS_FILE`, in graph-server's format, and reuses `auth::bearer` and `keys::KeySet::{load,parse,name_of}` through a path dependency on graph-server's lib with `default-features = false`, never forwarding `negctl` or `test-hooks`. Grants live in `GRAPH_HUB_GRANTS_FILE`. Auth cannot be turned off. | One credential format and one constant-time check (`keys.rs:118-128`). `KeySet::load` holds the break knob `accept-group-writable` (`keys.rs:74`), which is a constant `false` without `negctl` (`breaks.rs:11-14`); `hub-breaks-off` proves no feature reaches it. Caveat: the path dependency links graph-core and graph-wasm into the hub binary (size and attack surface, no code path); reopen and extract a `graph-keys` crate when a third consumer appears or a hub size or surface budget refuses graph-core (graph-render-4f, D8 (i)). | Sharing graph-server's file (every hub key would be a compute key); a `graph-keys` crate now (D8 (ii): more edits on a gated auth surface to save link size). |
| H9 | The hub's wire types (manifest, batch, change, notice) live in a new `hub` module of graph-contract, built in its default features with no new dependency, and read and written by hand like `ingest`. Their schema derives sit under the existing `codegen` feature, and `graph-cli codegen` writes the JSON schema and TypeScript declarations. | graph-contract is the single source of truth for the wire, and the SDK's types are generated from it. Cost, accepted: `crates/` is fingerprinted, so every change to the hub wire voids the motor's gate evidence until the full gate runs again. That is why the hub wire is frozen once slice 1 lands. | Types written by hand in both the hub and the SDK, which makes two definitions. |
| H10 | The SDK is TypeScript only, in `crates/graph-sdk-js/src/hub*.ts` and `plugin*.ts`. | One caller language today. The wire is HTTP plus JSON, specified in `docs/contract/hub-api.md`, so any language can push. | A Python SDK, which has no caller. |
| H11 | The change log is bounded by count (`GRAPH_HUB_RETAIN`) and by bytes (`GRAPH_HUB_RETAIN_BYTES`) per workspace. A cursor outside what is kept, or from another epoch (H15), gets 410 or `event: resync`, and the client resyncs from `/graph`. | Disk and memory stay bounded (`memory-guard.md`), and a tracker runs for months. | An unbounded log, which grows forever. |
| H12 | **Materialization drops what cannot resolve.** `/graph` and `/layout` leave out link and parent cells naming an absent record; a cell left with no target is removed. A link field naming an unregistered collection is left out of its collection's declaration, **and its cells are left out of every record**. The stored record keeps all of it, so the edge appears once the target arrives. | The reader refuses a dangling reference (`check_link_cells`, `crates/graph-contract/src/ingest/validate/cells.rs:28`) and a cell for an undeclared field (`check_references`, `validate.rs:200`), and independent plugins cannot order their pushes. Writes therefore never depend on another plugin's state, and a key learns nothing about other plugins from a refusal. | Refusing dangling writes and keeping tombstones: pushes would have to be ordered across plugins, and a 422 would reveal whether another plugin's record exists. |
| H13 | Tags are workspace-wide. The hub tag `tag:<value>` joins records from every plugin that uses the value; a tag value containing `:` is refused at write time, as the motor's derivation refuses it (`check_tag`, `crates/graph-core/src/ingest/build/builder.rs:255`). | This is the point of one graph per workspace: a `prod` tag from a hosts plugin and from a tickets plugin meet on one node. A plugin that wants private tags prefixes its values. | Tags per plugin, which needs a change to the motor's derivation. |
| H14 | **Materialization streams.** graph-contract's canonical writer is split into the pieces `to_json` is made of (document head, one collection, one record, document tail), and `to_json` becomes their concatenation, so it stays the single producer. The store keeps each record's qualified canonical text and writes the document from an ordered scan, a page at a time (§5.3). | A whole-document writer needs the document in memory: at the measured ingest peak of 18.25× the body (`server/graph-server/src/config/slots.rs:22`), a 64 MiB workspace is about 1.2 GB, over the 1 GiB container (D1). | Holding the document and paying the 18.25× term; a second writer in the hub (two producers of canonical bytes). |
| H15 | Each workspace has an **epoch**, drawn at creation from `hub_next_epoch()`: one row, `epoch_clock.last`, set to `greatest(last + 1, clock_timestamp() in ms)`. Per-event statement triggers and a `TRUNCATE` trigger, all `ENABLE ALWAYS`, draw a new epoch for every workspace a statement outside the hub's own write paths touched. A restore detector at start bumps every epoch when the database is not the one the hub last ran on (§5.3). ETags, event ids, cursors and `Graph-Seq` all read `<epoch>.<seq>`. | Deleting and recreating a workspace by hand, editing its rows in SQL (§9), or restoring a backup would otherwise reuse or invalidate seqs silently (D3, N1). A new epoch tells every client to resync. The clock floor keeps epochs growing after a restore rewinds `epoch_clock`. Each case is measured in `docs/measurements/hub-pg-epoch-probe.md`. | Forbidding SQL edits: §9 leaves them to operators, so they will happen. A global `SEQUENCE`: a point-in-time recovery rewinds it, so epochs repeat (N1). |

## 3. Architecture

```
plugin A ─┐  HTTP + hub key (write:ws/A)                 ┌─ app or studio (SDK createHub)
plugin B ─┼──► graph-hub ──SQL──► PostgreSQL              │
          │      │  ▲ SSE notices, /changes, /graph ──────┤
          │      └── POST /v1/layout?source=contract ──► graph-server   (body and snapshot streamed)
```

| Unit | Does | Depends on | Never |
|---|---|---|---|
| graph-contract `hub` module | wire types, strict readers, canonical writers, the in-memory materialization model, codegen | what graph-contract has | a new dependency |
| graph-contract `ingest::write` | the stream pieces (H14); `to_json` built from them | unchanged | a second producer |
| `server/graph-store` | SQL schema, migrations, the epoch clock, triggers and restore detector; one transaction per batch; the streamed materializer; changes after a cursor; the plugin's own records; retention and the idempotency sweeper | `tokio-postgres`, graph-contract | HTTP |
| `server/graph-hub` | routes, keys and grants, semaphores and limits, SSE notices, the streaming relay to graph-server | graph-store, graph-server's lib (`bearer`, `keys`; links graph-core and graph-wasm, H1, H8), graph-contract, axum, hyper (already pinned) | calling motor code; edits to graph-server beyond `pub fn bearer` |
| SDK `hub.ts`, `plugin.ts` | reading, subscribing, laying out; registering, pushing, syncing | `fetch`, the existing snapshot reader | Node-only APIs in the browser path |

The hub binds `127.0.0.1` by default, as graph-server does. Publishing it beyond the host, and
TLS, stay stop-and-ask items (`server-and-write-path.md:32-33`, `docs/deploy/service.md:65`).

## 4. Data model

**Workspace.**
- Id `[a-z0-9][a-z0-9-]{0,62}`.
- It is the `source` of the materialized ingest document.
- It has `epoch` (H15), `head_seq` (u64) and `doc_bytes`.

**Plugin.**
- Id `[a-z0-9][a-z0-9-]{0,62}`, so it contains no `.` and no `:`.
- It has a `manifestVersion` (u32), collections, `plugin_seq` (the seq of its last change) and
  `plugin_bytes`.
- A plugin's collection ids match `[A-Za-z0-9_-]{1,64}`.
- In the materialized document a collection becomes `<plugin>.<collection>`, so a node id reads
  `<ws>:<plugin>.<collection>:<record>`, which is the contract's own grammar. Neither part may
  contain `.` or `:`, so two plugins can never produce the same qualified id.

**Links.**
- In a manifest, `link.collection` is either `coll` (the plugin's own collection) or
  `other.coll` (another plugin's). The hub qualifies both.
- Registration does not need the target to be registered. Materialization drops what does not
  resolve (H12).
- A batch names only plugin-local collection ids. `B.coll` in a batch fails the id grammar: 422.
- The store keeps a `links` table, `(ws, src_qcoll, src_id, field, target_qcoll, target_id)`,
  written in the batch's transaction, so materialization finds unresolved cells by an index
  anti-join instead of reading every record twice.

**Record.**
- Key: (plugin, collection, id).
- `rev` is a u64 the hub assigns, +1 per write that changes the record.
- `updatedAt` is a u32 the source assigns; the hub carries it.
- `values` is keyed by declared field id. An undeclared field id is 422. Each cell is checked
  against its field's role at write time with graph-contract's cell rules. A tag value containing
  `:` is 422 (H13).
- Values are stored as the qualified canonical text graph-contract's writer produces for the
  record, never as `jsonb`, so `-0`, large integers and key order come back byte-identical.
- A record whose canonical text exceeds `GRAPH_HUB_MAX_RECORD_BYTES` (1 MiB) is 413.
- `\u0000` in a body is 422 at the hub's reader; a percent-decoded NUL (`%00`) in a path id is
  400. Neither reaches the database.
- An upsert whose canonical text equals the stored text is a **no-op**: no rev, no seq, no
  notice. Deleting an absent record is a no-op too.
- A delete removes the row and its `links` rows, and writes the delete to the change log.

**Manifests.**
- In v1 a manifest only grows: a higher `manifestVersion` may add collections and fields.
- Any change to an existing collection or field, or any removal, is 409.
- The same version with different content is 409. The same version with the same content is a
  200 that takes no seq.
- A manifest that changes takes a seq and logs a change of kind `manifest`, so subscribers learn
  of new collections in order with the records that use them.
- The manifest PUT keeps the batch's lock order (§5.1 step 1, N9): it takes the workspace row
  `FOR UPDATE` and `SET LOCAL hub.writer = '1'` (H15) first. Under that lock it checks the
  64-plugin cap, updates `doc_bytes` and refuses (413, rollback) past `GRAPH_HUB_MAX_DOC_BYTES`,
  and only then takes a seq.
- Creating a workspace is `INSERT ... ON CONFLICT (id) DO NOTHING` under `hub.writer`, with
  `epoch = hub_next_epoch()` and `doc_bytes` set to the head and tail lengths (§6). It answers 201
  when the row was inserted, 200 when it existed, and takes no seq.
- Caps, each a 413: at most 64 collections per plugin, 256 fields per collection, 256 KiB of
  manifest text, and 64 plugins per workspace.
- A 409 or 413 depends only on the plugin's own manifest and the workspace's plugin count, never
  on another plugin's content.
- Restructuring a plugin is an operator task in SQL in v1, like deleting a workspace (H15
  covers both).

## 5. Wire: hub-api v1

Bodies follow the ingest reader's conventions: an unknown member refuses the document, every id
is checked, and output is canonical. `seq` and `rev` are JSON numbers; the hub stops accepting
writes to a workspace at 2^53 − 1, so JavaScript reads them exactly. That ceiling is unreachable
in practice: at 10^6 commits a second it takes 285 years. A cursor is the string
`<epoch>.<seq>`; a bare seq is 400.

### 5.1 Bodies and the writer transaction

Manifest (`PUT .../plugins/{plugin}`); a field is the contract's `Field`:

```json
{"version":1,"manifestVersion":3,"name":"Issue tracker",
 "collections":[{"id":"issue","name":"Issue","titleField":"title","fields":[]}]}
```

Batch (`POST .../plugins/{plugin}/batches`):

```json
{"version":1,
 "upserts":[{"collection":"issue","id":"42","updatedAt":7,"values":{"title":"Crash on load"}}],
 "deletes":[{"collection":"issue","id":"41"}]}
```

Batch rules:
- The plugin comes from the URL, never from the body.
- A batch is atomic: it all commits or none of it does. Two batches are not atomic together; each
  carries its own `Idempotency-Key`.
- A record appears at most once per batch across `upserts` and `deletes`, else 422.
- `If-Match: "<epoch>.<plugin_seq>"` is optional and per plugin: when the plugin's last change is
  not that one, the answer is 412. Another plugin's writes never change it.
- `Idempotency-Key` takes at most 128 bytes and is stored per (workspace, plugin, key) for 24 h,
  written in the batch's own transaction.
  - A replay returns the stored response and adds no seq.
  - The same key with a different body (sha256) is 422.
- The answer is `{seq, applied}`: the seq the batch took, or the current `head_seq` with
  `applied: 0` when every operation was a no-op.

The writer transaction, `READ COMMITTED`, in this order:
1. `SELECT ... FROM workspaces WHERE id = $1 FOR UPDATE`; `SET LOCAL hub.writer = '1'` (H15).
2. Look up the idempotency row; a hit returns its stored response.
3. Check `If-Match` against `plugin_seq`.
4. Apply: compare the SHA-256 of each upsert's canonical text with the stored `text_sha256`,
   reading only the batch's ids (at most `GRAPH_HUB_MAX_BATCH` × 32 B = 320 000 B; `sha2` 0.10.9
   is already in `server/Cargo.lock:458`); write the changed ones, their `links` rows and their
   revs; delete the present ones; update `plugin_bytes` and `doc_bytes`
   and refuse (413, rollback) past `GRAPH_HUB_MAX_PLUGIN_BYTES` or `GRAPH_HUB_MAX_DOC_BYTES`.
5. When anything applied: bump `head_seq`, insert the change, set `plugin_seq`.
6. Prune the change log past `GRAPH_HUB_RETAIN` and `GRAPH_HUB_RETAIN_BYTES`.
7. Insert the idempotency row with the response, then commit. Only then is the response sent and
   the watch updated.

An all-no-op batch still takes step 1's lock and stores its idempotency row. A unique violation
on the idempotency key (not expected under step 1's lock; defensive) rolls back and retries once,
which then finds the stored response in step 2. A sweeper deletes idempotency rows older than
24 h every 10 minutes, a bounded batch at a time. It runs under `SET LOCAL hub.writer = '1'`
(H15), so it never moves an epoch.

**A change** is stored as a header row `(ws, seq, plugin, at, kind, bytes, ops)` plus one row per
operation `(ws, seq, ord, op, qcoll, id, rev, text)`. As `/changes` returns it:

```json
{"seq":118,"plugin":"tracker","at":"2026-10-05T09:12:44.120Z","kind":"batch","upserts":[],"deletes":[]}
```

- Records in a change carry qualified collection ids and their `rev`. A `manifest` change carries
  the plugin's new manifest.
- `at` is PostgreSQL's `clock_timestamp()` read when the batch takes its seq. It is
  informational and not ordered: order is `seq`. The hub may have a clock; the motor never does.
- A change's canonical bytes are counted in step 5. A change over `max_change` =
  `GRAPH_HUB_MAX_BODY` + 96 × `GRAPH_HUB_MAX_BATCH` is 413. Each operation adds at most the
  qualified prefix (64 bytes) and a `rev` member (at most 26) to what the body carried, so a body
  under `GRAPH_HUB_MAX_BODY` cannot reach it; the check makes the bound a fact the start check
  (§6) can lean on.

### 5.2 Routes

| Method | Path | Grant | Returns |
|---|---|---|---|
| GET | `/healthz` | none | `ok` |
| GET | `/v1/meta` | any key | `{api:1, version, limits}` |
| PUT | `/v1/workspaces/{ws}` | admin | 201 on create, 200 when it exists |
| GET | `/v1/workspaces` | any key | the workspaces the key may read, each with `epoch` and `head_seq` |
| PUT | `/v1/workspaces/{ws}/plugins/{plugin}` | `write:<plugin>` | the stored manifest, 201 or 200 |
| GET | `/v1/workspaces/{ws}/plugins` | read | every manifest |
| POST | `/v1/workspaces/{ws}/plugins/{plugin}/batches` | `write:<plugin>` | `{seq, applied}` |
| GET | `/v1/workspaces/{ws}/plugins/{plugin}/records?cursor=&limit=` | `write:<plugin>` or read | the plugin's own records as `{collection, id, rev}`, in byte order, at most `limit` (default 1 000, max 10 000), and an opaque `next` cursor until the last page; every page carries `plugin_seq` as `"<epoch>.<plugin_seq>"` |
| GET | `/v1/workspaces/{ws}/graph` | read | the canonical ingest document, streamed, `ETag: "<epoch>.<seq>"`; `If-None-Match` gives 304 |
| GET | `/v1/workspaces/{ws}/records/{plugin}/{collection}/{id}` | read | the record and its `rev` |
| GET | `/v1/workspaces/{ws}/changes?since=&limit=` | read | the changes after `since`, at most `limit` and at most `GRAPH_HUB_CHANGES_BYTES` (never less than one change); 410 when `since` is from another epoch or outside what is kept (§5.3) |
| GET | `/v1/workspaces/{ws}/events` | read | SSE notices (§5.3) |
| POST | `/v1/workspaces/{ws}/layout?layout=&post=` | read | graph-server's snapshot for the materialized document, streamed, `Accept` passed through, plus a `Graph-Seq: <epoch>.<seq>` header |

**Order of refusals.** Authorization comes before existence: no or unknown key is 401; a key
with no grant covering the path's workspace and plugin is 403; only then is a missing workspace,
plugin or record 404. A key learns that a workspace exists only if it may read or write it.

**Errors** use graph-server's JSON shape, `{error, message}`. The hub has its own error enum;
graph-server's `error.rs` stays untouched. A request that waits past `GRAPH_HUB_TIMEOUT_MS` for a
pool connection or a semaphore permit gets 503 with `Retry-After`. graph-server's answers to
`/layout` (statuses from `server/graph-server/src/error.rs:38-102` and `motor.rs:18-19, 27-47`) map as
follows:

| graph-server | hub | Why |
|---|---|---|
| 200 | 200, bytes streamed unchanged | |
| 400 | 400, relayed | the caller's `layout` or `post` is invalid |
| 401 | 502 `MotorAuth` | the hub's own motor key is wrong: a hub defect, logged |
| 406 | 406, relayed | the caller's `Accept` |
| 408 | 502 `MotorBodyTimeout`, no `Retry-After` | the hub's upload missed graph-server's 10 s body timeout (`body.rs:20`); the same request would miss it again |
| 413 (body or per-id work caps) | 413 `GraphTooLarge`, with graph-server's message | the workspace is past what the motor accepts |
| 422 with `error` `IngestInvalid` or `ContractInvalid` | 502 `MaterializeInvalid`, logged as a hub defect | the document did not read (`motor.rs:18-19, 27-38`); `hub-materialize` proves H12 makes it readable |
| 422 with `error` `LayoutFailed` or `PostFailed` | 422, relayed | the caller's choice of layout or post failed on this graph (`motor.rs:40-47`) |
| 429 | 503 with `Retry-After` | the motor's queue is full (`error.rs:59`) |
| 503 | 503, relayed with graph-server's `error`, no `Retry-After` | `Timeout` (`error.rs:71`); the SDK never retries `/layout` |
| 500 | 502 `MotorError` | |
| unreachable, or no answer within `GRAPH_HUB_MOTOR_TIMEOUT_MS` | 502 `MotorUnavailable` | |
| any other status (404 `error.rs:78`, 405, …) | 502 `MotorError`, logged | |

`GRAPH_HUB_MOTOR_TIMEOUT_MS` defaults to 45 000, above graph-server's `GRAPH_TIMEOUT_MS` plus
`GRAPH_BODY_TIMEOUT_MS` (30 000 + 10 000, `config.rs:195-196`), so graph-server answers first.

Caveat: graph-server's 503 `Timeout` covers both an admission wait (`gate.rs:39-42`) and a run
that overran (`layout.rs:94`, `error.rs:71`). An overrun is deterministic and keeps its slot
(`layout.rs:76-79`), so retrying it only adds load; the hub cannot tell the two apart and relays
the 503 without `Retry-After`. A distinct code for an overrun is graph-render-4f's call (§11).

**Grants.**
- `GRAPH_HUB_GRANTS_FILE` holds lines of the form `<key-name> <ws|*> <read|write:<plugin>|admin>`.
- A key with no grant is denied by default.
- `admin` implies `read` and every `write`.
- The grants file gets the same permission check as the keys file (mode 0640 or stricter).
- On `SIGHUP` the hub loads both files and swaps them together as one
  `RwLock<Arc<(KeySet, Grants)>>`. If either file fails to load, the old pair stays.
- The hub does what graph-server's `auth::check` does beyond `bearer`, without reusing it: a
  second `Authorization` header is 400, and every other refusal of a credential is the same 401.

### 5.3 Snapshots, order and the event stream

**Cursors.** A cursor `<epoch>.<seq>` is valid when its epoch is the workspace's and
`low − 1 ≤ seq ≤ head_seq`, where `low` is the oldest change kept (`head_seq + 1` when none is).
Anything else, including `seq > head_seq`, is 410 on `/changes` and `event: resync` on the
stream. The SDK also requires the first change it receives to be `cursor + 1`, and resyncs
otherwise.

**`/graph`.**
- One `REPEATABLE READ, READ ONLY` transaction reads `epoch` and `head_seq`, then streams the
  document from it, so the document is exactly the state at its ETag. The manifests' collection
  declarations are streamed in qualified-id order from the same transaction, holding one plugin's
  dropped-field set at a time.
- Order: the head; the collections, sorted by qualified id; the records, from one portal scan
  ordered by `(qcoll COLLATE "C", id COLLATE "C")` with a fixed fetch size
  (`GRAPH_HUB_FETCH_ROWS`, through `client.bind` and `query_portal`); the tail.
- The unresolved `(qcoll, id, field, target)` tuples come from an anti-join of `links` against
  the records, in the same order, read through a second portal with the same
  `GRAPH_HUB_FETCH_ROWS`, and are merged into the scan.
- A record with no unresolved cell and no dropped field is copied verbatim from its stored
  text. Any other record is parsed, pruned (H12; a cell left empty is removed) and rewritten
  with the record piece of the canonical writer.
- So the same `<epoch>.<seq>` gives the same bytes, whatever order the records were inserted
  in. Cost: O(n) over the records, plus O(L log n) for the L link cells.
- `hub-materialize` proves the streamed bytes equal `to_json` over the in-memory model of the
  same records, and that `ingest::read` accepts them.
- Caveat: the snapshot lives for the whole response, so a slow client pins one pool connection
  and the database's xmin horizon (vacuum waits) until `GRAPH_HUB_STREAM_DEADLINE_MS` cuts the
  connection. The SDK sees a short body, and retries.

**`/layout`.**
- The same streamed document is the request body to graph-server (chunked; graph-server reads a
  chunked body in full before computing, `server/graph-server/src/body.rs:12-24`).
- The snapshot spans the upload only: the transaction commits when the last chunk is sent,
  before the hub awaits graph-server's answer. The answer is streamed back to the caller.
- `drop-record`, the negative control of `hub-roundtrip`, sits in this relay alone, so `/graph`
  and `/layout` cannot change together.
- `/layout` takes a `GRAPH_HUB_LAYOUTS` permit (default 1), not a `GRAPH_HUB_READS` one.
- Caveat: graph-server admits a request before it reads the body (`layout.rs:64-65`,
  `gate.rs:39-42`), so while it waits for a slot the hub holds the snapshot, the xmin horizon and
  a pool connection, for up to 30 s. `GRAPH_HUB_LAYOUTS` = 1 keeps that to one connection.
- Slice 3 measures the upload of a `GRAPH_HUB_MAX_DOC_BYTES` workspace at the minimum record size
  against graph-server's 10 s body timeout (`body.rs:20`) and records it in
  `docs/measurements/hub-memory.md`. A miss is a stop.
- The SDK never retries `/layout`.

**`/changes`.**
- One `REPEATABLE READ, READ ONLY` transaction checks the cursor, reads the headers after it in
  seq order, summed until `GRAPH_HUB_CHANGES_BYTES`, then the operations of those seqs in
  `(seq, ord)` order. At least one change is returned, which the start check (§6) bounds.
- The single snapshot is what keeps a concurrent prune from removing operations between the two
  reads: under `READ COMMITTED` the probe read `headers|10` then `ops|2` across a prune, under
  `REPEATABLE READ` `headers|10` then `ops|10` (`docs/measurements/hub-pg-epoch-probe.md`).
- A header whose operation count differs from its stored `ops` is never sent: the page fails
  with 500 and the hub logs it. Row `hub-changes-snapshot` proves it.

**SSE.**
- `GET .../events` takes a cursor: `Last-Event-ID`, else `?since=`, else the current
  `<epoch>.<head_seq>`.
- The protocol, per subscriber:
  1. Subscribe to the workspace's `watch` first.
  2. Page through change **headers** after the cursor, in seq order, and send each as
     `event: change`, `id: <epoch>.<seq>`, `data: {"seq":..,"plugin":..,"at":..}`. The cursor
     advances.
  3. When caught up, wait on the `watch`, then go back to step 2.
- No change can be missed: the `watch` only says that the head moved, and the database is read
  from the cursor. Writers commit in seq order under the row lock (H6), so a seq is never
  visible before a smaller one.
- No change can be sent twice: the cursor only moves forward.
- When the cursor stops being valid, the stream sends `event: resync` and closes.
- A comment heartbeat goes out every 15 s, and re-reads the epoch: an epoch changed by a manual
  edit sends `resync` even when no hub write followed it.
- `GRAPH_HUB_TIMEOUT_MS` bounds the time to the first byte. Once the stream's headers are sent,
  only the heartbeat's failed write ends it.

**Epoch triggers.** The facts below are measured in `docs/measurements/hub-pg-epoch-probe.md`.
- `hub_next_epoch()` runs `UPDATE epoch_clock SET last = greatest(last + 1,
  (extract(epoch FROM clock_timestamp()) * 1000)::bigint) RETURNING last` on the one-row table
  `epoch_clock(one bool PRIMARY KEY CHECK (one), last bigint)`.
- Every hub table carries one statement-level trigger per event: `AFTER INSERT` with
  `REFERENCING NEW TABLE`, `AFTER UPDATE` with `REFERENCING OLD TABLE ... NEW TABLE`, `AFTER
  DELETE` with `REFERENCING OLD TABLE`, and `AFTER TRUNCATE FOR EACH STATEMENT`, which bumps every
  workspace. PostgreSQL 17 refuses transition tables on a multi-event trigger (`transition tables
  cannot be specified for triggers with more than one event`). The workspace column's name is
  the trigger's `TG_ARGV[0]`.
- Every trigger is `ENABLE ALWAYS`: with plain `ENABLE`, `session_replication_role = replica`
  skips it. The probe measured `TRUNCATE` (+2), `\copy` (+1) and the replica role with `ALWAYS`
  (+1) each moving the epoch.
- The guard skips the bump when `current_setting('hub.writer', true) = '1' OR pg_trigger_depth()
  > 1`. The hub sets `hub.writer` with `SET LOCAL` on every one of its write paths: the batch, the
  manifest PUT, the workspace create and the sweeper. The depth guard keeps the trigger on
  `workspaces` from firing on its own update.
- A workspace deleted and created again by hand draws a fresh, larger epoch.
- **Restore detector.** `hub_meta(system_identifier, timeline_id, datoid)` holds
  `pg_control_system().system_identifier`, `pg_control_checkpoint().timeline_id` and the
  database's `pg_database.oid`, all readable without superuser. At start the hub compares them;
  on a mismatch it bumps every workspace's epoch and rewrites `hub_meta`. `pg_restore` loads the
  data before it creates the triggers (`pg_restore -l` lists them in that order), and a restore
  into a new database gets a new oid, which the detector keys on.
- Runbook (`docs/deploy/hub.md`): after any restore, run
  `UPDATE workspaces SET epoch = hub_next_epoch()`.
- Caveat: the detector misses `ALTER TABLE ... DISABLE TRIGGER`, `pg_restore --disable-triggers`,
  `pg_restore --clean` into the same database, and a point-in-time recovery whose host clock was
  also stepped back. Only the runbook catches those.

## 6. Limits and memory

| Env | Default | Over the limit |
|---|---|---|
| `GRAPH_HUB_MAX_BODY` | 4 MiB | 413 |
| `GRAPH_HUB_MAX_BATCH` | 10 000 operations | 413 |
| `GRAPH_HUB_MAX_RECORD_BYTES` | 1 MiB of canonical text | 413 |
| `GRAPH_HUB_MAX_PLUGIN_BYTES` (per plugin) | 16 MiB | 413 on the batch that would cross it |
| `GRAPH_HUB_MAX_DOC_BYTES` (per workspace) | 64 MiB | 413 on the batch that would cross it |
| `GRAPH_HUB_RETAIN` / `GRAPH_HUB_RETAIN_BYTES` (per workspace) | 100 000 changes / 512 MiB | older changes are pruned in the commit's transaction; a cursor below them gets 410 |
| `GRAPH_HUB_CHANGES_BYTES` (one `/changes` page) | 8 MiB | the page ends; at least one change per page |
| `GRAPH_HUB_WRITERS` (batches and manifests being read and applied at once) | 2 | waits, then 503 with `Retry-After` |
| `GRAPH_HUB_READS` (`/graph`, `/changes`, records pages, `GET /plugins`, one record, `/workspaces` at once) | 2 | waits, then 503 with `Retry-After` |
| `GRAPH_HUB_LAYOUTS` (`/layout` at once) | 1 | waits, then 503 with `Retry-After` |
| `GRAPH_HUB_BODY_TIMEOUT_MS` (a request body read in full) | 10 000 | 408 |
| `GRAPH_HUB_MAX_CONNECTIONS` | 256 | the next connection waits in the backlog |
| `GRAPH_HUB_HEADER_TIMEOUT_MS` | 5 000 | the connection is closed |
| `GRAPH_HUB_MAX_HEADER_BYTES` | 16 384 | 431 |
| `GRAPH_HUB_FETCH_ROWS` (portal page) | 32 records | |
| `GRAPH_HUB_MAX_SUBSCRIBERS` / `GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY` | 64 / 8 | 429 |
| `GRAPH_HUB_DB_POOL` | 8 | waits up to `GRAPH_HUB_TIMEOUT_MS` (30 000), then 503 with `Retry-After` |
| `GRAPH_HUB_STREAM_DEADLINE_MS` (`/graph` and `/layout` bodies) | 120 000 | the connection is cut |
| `GRAPH_HUB_MOTOR_TIMEOUT_MS` | 45 000 | 502 `MotorUnavailable` |
| `GRAPH_MOTOR_URL`, `GRAPH_MOTOR_KEY_FILE` | `http://127.0.0.1:8080`, none | the hub's own graph-server key |

The connection and header limits mirror graph-server's (`config.rs:28-30, 68-76`, defaults
`config.rs:234-236`).

**Start checks.** The hub refuses to start when `GRAPH_HUB_DB_POOL` ≤ `GRAPH_HUB_WRITERS` +
`GRAPH_HUB_READS` + `GRAPH_HUB_LAYOUTS` (SSE header pages and the sweeper need the rest), when
`GRAPH_HUB_RETAIN_BYTES` or `GRAPH_HUB_CHANGES_BYTES` is below `max_change` (§5.1), when
`GRAPH_HUB_MOTOR_TIMEOUT_MS` ≤ 40 000, or when the database's `server_encoding` is not `UTF8` or
its `pg_database.datcollate` is not `C` (the image runs `initdb --encoding=UTF8 --locale=C`, so
`COLLATE "C"` order is UTF-8 byte order). Then it runs the restore detector (§5.3).

**doc_bytes.**
- `doc_bytes` is the sum of every stored record's qualified canonical text and every collection
  declaration's, plus their separators and the document's head and tail lengths (graph-contract's
  `hub` module exposes both). The batch, the manifest PUT and the workspace create keep it
  current, each in its own transaction. That is the
  length of the document before H12 prunes anything, so it bounds the streamed document from
  above, exactly.
- The manifest caps (§4) bound the declarations: at most 64 plugins × 256 KiB.
- `GRAPH_HUB_MAX_DOC_BYTES` is shared by the plugins of a workspace. Each plugin is held to
  `GRAPH_HUB_MAX_PLUGIN_BYTES`; with more than four plugins at their quota, they compete for the
  workspace's cap. Caveat: a fifth plugin can be refused because four others are full, and it
  learns only that the workspace is full, never by whom.
- The 64 MiB default equals graph-server's `GRAPH_MAX_BODY` default, so a workspace the hub
  accepts fits graph-server's body limit. graph-server's per-id work caps can still answer 413,
  which maps as in §5.2.

**Memory.** Nothing holds a whole document (H14). The budget:

```
M = base
  + WRITERS × MAX_BODY × F_w
  + (READS + LAYOUTS) × (max(2 × FETCH_ROWS × MAX_RECORD_BYTES + MAX_RECORD_BYTES × F_w,
                             2 × CHANGES_BYTES, 64 × 256 KiB, MAX_RECORD_BYTES) + IO_BUF)
  + MAX_CONNECTIONS × MAX_HEADER_BYTES
  + MAX_SUBSCRIBERS × 256 B
  + DB_POOL × conn_buf
```

- `F_w` is the peak of reading a JSON body into values, relative to its size. The planning value
  is graph-server's measured 18.25 (`slots.rs:22`). Caveat: that was measured on graph-server's
  ingest reader, not the hub's batch reader; slice 3 measures `F_w` on the hub and replaces it.
- The `MAX_RECORD_BYTES × F_w` term is one pruned record parsed at a time.
- The `2 ×` on `FETCH_ROWS` is the second portal, the anti-join (§5.3). `64 × 256 KiB` is
  `GET /plugins`; `MAX_RECORD_BYTES` alone is one record's route. The batch's hash compare
  (at most 320 000 B, §5.1) sits inside `MAX_BODY × F_w`.
- At the defaults, planning arithmetic only: 146 MiB for writers, 164.5 MiB for reads and
  82.25 MiB for layouts (82.25 MiB per permit), 4 MiB for headers, 16 KiB for subscribers, 8 MiB
  for the pool at a planned 1 MiB `conn_buf`: about 405 MiB plus `base` and the `IO_BUF` terms.
- Slice 3 measures peak RSS at every cap at once under `scripts/orch/drun` and records it in
  `docs/measurements/hub-memory.md`. The defaults shrink until the peak fits the hub container's
  1 GiB.

## 7. SDK (TypeScript)

```ts
const hub = createHub({ baseUrl, apiKey });
const { cursor, ingest } = await hub.graph("ops");                      // cursor = "<epoch>.<seq>"
const snapshot = await hub.layout("ops", { layout: "fa2" });            // existing snapshot reader
const stop = hub.subscribe("ops", { since: cursor, onChange, onResync }); // notices + /changes

const plugin = createPlugin({ baseUrl, apiKey, plugin: "tracker", manifest });
await plugin.register("ops");
await plugin.push("ops", { upserts, deletes });  // Idempotency-Key per call
await plugin.sync("ops", rowsToIngest(rows));    // full state of this plugin only
```

- `subscribe` uses `fetch` and parses the stream itself, because `EventSource` cannot send an
  `Authorization` header. Notices are coalesced: while a `/changes` read is in flight, newer
  notices only raise its target, and one more read follows. Each change goes to `onChange` in
  seq order. A 410, an `event: resync`, or a first change that is not `cursor + 1` calls
  `onResync`. It reconnects with `Last-Event-ID`. `hub-sdk` tests the first-change check by
  injecting a gap.
- `push` retries 429, 503 and transport errors (no answer) up to 3 times with the **same**
  `Idempotency-Key`, so a batch that committed before the connection dropped is not applied
  twice.
- `layout` is never retried automatically: graph-server's `error` reaches the caller (§5.2).
- `sync` needs only `write:<plugin>`. It reads the plugin's own ids from the records route,
  pushes every desired record as upserts (identical ones are no-ops on the hub: no seq, no
  notice), and deletes the listed ids it no longer wants, in batches of at most
  `GRAPH_HUB_MAX_BATCH`, each with its own key. A crash between batches leaves part of the state;
  the next `sync` completes it. Caveat: `sync` re-sends every record on every call (bandwidth
  O(n)); a plugin that knows its own changes calls `push`.
- A record an adapter marks `deleted: true` (`rows.ts:94, 215`; `notion.ts:104, 259`) becomes a
  delete when its id is stored, and is otherwise left out of the desired set.
- `sync` sends `If-Match` with the `plugin_seq` of the first records page. A later page with
  another `plugin_seq`, or a 412, starts the sync over. After each batch with `applied > 0`, the
  expected `plugin_seq` becomes that batch's seq.
- `sync` turns any adapter's `Ingest` (`rowsToIngest`, `notionToIngest`) into plugin batches.
- Browser use keeps `remote.ts`'s `dangerouslyAllowBrowser` rule: a key held in a page is public
  (`service-api.md`).
- One example plugin, `examples/plugins/rows-file/`: it reads a JSON rows file, passes it through
  `rowsToIngest`, and calls `sync`. The `hub-sdk` row runs it.

## 8. Gates

Every row has a negative control that must turn it red, and a negative control that stays green
is recorded as a failure. The breaks sit behind a `negctl` feature, off in every shipped build,
as graph-server's do.

| Row | Slice | Proves | Negative control |
|---|---|---|---|
| `hub-wire` | 1 | readers refuse unknown members, `B.coll` in a batch, a tag with `:`, `\u0000`, manifests past each cap; `-0` and 2^53 − 1 round-trip byte-identical | `lax-reader` |
| `hub-materialize` | 1, 2 | Random op sequences (cross-plugin links, deletes of linked records, links to unregistered collections) always give a document `ingest::read` accepts; the streamed bytes equal `to_json` over the in-memory model; same `<epoch>.<seq>`, same bytes; a permuted insertion order gives the same hash; `to_json` equals the concatenation of its pieces on every ingest fixture | `keep-dangling`: unresolved cells kept; `keep-cells`: a dropped field's cells kept |
| `hub-floor` | 2, 3 | fmt, clippy `-D warnings` and tests over the `server/` workspace, without a live PostgreSQL | an unformatted file in a scratch copy |
| `hub-virtual-root` | 2, 3, 5 | `cargo tree --manifest-path server/Cargo.toml -e features --locked`, with no `-p`, is byte-identical to the pre-slice-2 baseline; graph-server's lock entries keep their versions | a scratch copy without `default-members`: graph-server's subtree shows `hyper-util`'s `client-legacy` (grepped) |
| `svc-supply` | 2, 3 | every row of `scripts/orch/rows/service-supply.rows` on the merged tree; each negative control (`--break`, `--break-version`, `--break-feature`) exits 1 **and** prints its original message (grepped) | existing |
| `hub-seq` | 2 | 100 concurrent writers on one workspace: seqs 1..N, no gap, no duplicate, idempotency rows present; an all-no-op batch takes no seq | `sequence-seq`: seq from a `SEQUENCE` with one rolled-back batch, step 1's lock removed, and the commits of odd seqs delayed |
| `hub-epoch` | 2 | on the real schema, each of a manual `INSERT`, `UPDATE`, `DELETE`, `TRUNCATE`, `COPY`, a write under `session_replication_role = replica`, and a delete and re-create of a workspace changes the epoch; the restore detector fires on a new database oid; a cursor from before is 410. Reverse: a hub batch, a manifest PUT, a workspace create and a sweeper run leave the epoch unchanged | the trigger dropped; `trigger-enable-origin`: the triggers `ENABLE` instead of `ENABLE ALWAYS` |
| `hub-changes-snapshot` | 2 | a prune committed between the header and operation reads of a `/changes` page still yields every operation of every returned header | `changes-read-committed` |
| `hub-pg-durability` | 2 | `fsync`, `synchronous_commit` and `full_page_writes` read `on` in the running image; 200 acked batches, the PostgreSQL container killed (`docker kill`) and started again on the same named volume: every acked seq present | `synchronous_commit=off` with `wal_writer_delay=10s`: acked commits lost, the row fails |
| `hub-durability` | 3 | 200 batches, the hub container killed right after an ack, restarted: every acked seq present, no gap | `ack-before-commit`: the ack is sent, then a forced 500 ms delay before the commit |
| `hub-roundtrip` | 3 | hub `/layout` bytes = graph-server `/v1/layout?source=contract` on `/graph`'s document at the same `<epoch>.<seq>`, for 3 fixtures | `drop-record`, in the relay only |
| `hub-authz` | 3 | the key matrix: `write:A` writing `B.coll` refused; read-only writing 403; unknown key 401; a second `Authorization` header 400; no grant 403 before any 404; refusals identical whether or not the workspace or another plugin exists; `If-Match` on plugin A unaffected by plugin B's writes | `skip-grant` |
| `hub-reload` | 3 | `SIGHUP` with a bad grants file keeps the old pair; a good pair swaps both | `reload-keys-only` |
| `hub-events` | 3 | a reconnect while 4 writers commit: no gap, no duplicate; `resync` past retention, for `since > head`, and after an epoch change; the per-key subscriber cap | `skip-event`; and `sequence-seq` must turn it red too (visibility order) |
| `hub-motor-map` | 3 | every row of the §5.2 map (the 408, the 422 split on `error`, the relayed 503, the default row), the motor timeout, and the pool-wait 503, against a stub graph-server | `layoutfailed-as-502` |
| `hub-idem` | 3 | a replayed key gives the same response and the same `head_seq`; another body is 422; the sweeper removes rows past 24 h | `no-idem` |
| `hub-limits` | 3 | each 413, 429 and 503 of §6; each start check, the encoding and collation one included; the connection cap, the header timeout, 431, the body-timeout 408 and the `GRAPH_HUB_LAYOUTS` permit; manifest 409s and caps | `no-cap` |
| `hub-memory` | 3 | peak RSS at every cap at once < 1 GiB under `drun`, `F_w` measured, both recorded in `docs/measurements/hub-memory.md` | `GRAPH_HUB_WRITERS` sized from the measured `F_w` so that `WRITERS × MAX_BODY × F_w` > 1 GiB, `GRAPH_HUB_DB_POOL` just above the start check, under `drun --memory 1g --memory-swap 1g`, every body held in flight behind a barrier: exit 137 or a peak over the limit |
| `hub-breaks-off` | 3 | `cargo tree --manifest-path server/Cargo.toml -e normal,build,features -i graph-server -p graph-hub` on the release build shows neither `negctl` nor `test-hooks` on graph-server's edge | the edge given `features = ["negctl"]` in a scratch copy |
| `hub-sync` | 4 | `sync` with a key holding only `write:A` works; a second `sync` on the same input, with a dangling link in it, adds no seq; a transport error mid-push is retried with the same key and applied once; a `deleted: true` row is deleted when stored and dropped otherwise; two concurrent syncs of one plugin lose no fresh record (`If-Match`, 412, start over) | `sync-via-graph`: diff against `/graph`, which re-pushes the dangling record each time |
| `hub-sdk` | 4 | SDK tests and the example plugin against live hub, PostgreSQL and graph-server containers; an injected gap in the change stream calls `onResync`; a failed `/layout` is not retried | `GM_HUB_SDK_BREAK=1`: the example pushes a wrong record |
| `cargo-deny-server`, `lock-parity`, `svc-features` | 2, 3 | re-run with the new members (inside `svc-supply`); the hub image builds `-p graph-hub` only | existing |
| `codegen --check`, graph-core wasm32 build, `svc-*` | 1–5 | unchanged | existing |
| `motor-lock` | 1–5 | `git diff --exit-code <base> -- Cargo.lock` and graph-core's `[dependencies]` unchanged | a scratch dependency added |

The CLAUDE.md row `motor-alone` is not defined in any rows file today
(`docs/reviews/review-studio.md:201-202`). This spec claims `motor-lock` instead and leaves
`motor-alone` to its own work.

## 9. Not in v1

Each of these waits for a caller.

- The Redis cache and the snapshot cache (`snapshot-cache.md`, D2, a stop-and-ask item).
- Two or more hub instances, and with them `LISTEN/NOTIFY`.
- Pull or webhook plugins, in-process plugins, plugin distribution.
- Edge properties. Edges come from link fields; a relation with attributes is a record in its
  own collection.
- A query language, traversal or search route, and time travel (`/graph?at=`).
- Over HTTP: deleting a workspace or a plugin, and changing or removing a manifest's collections
  or fields. Operators do these in SQL, and the epoch (H15) tells clients.
- TLS and publishing beyond the host (stop-and-ask), per-key write quotas, a Python SDK.
- `pgvector`, which the images ADR lists but nothing calls.
- Live studio integration. It needs host-api work in `packages/graph-studio/src/host/*`, which
  graph-render-4f owns, so it is a later slice agreed with them.

## 10. Slices

Each slice has its own branch and worktree and runs as an OpenCode job with an exact brief, in
this order.

1. **hub-contract.** graph-contract's `hub` module: the types, strict readers (caps, `\u0000`,
   id grammar), canonical writers, the cell checks used at write time, the in-memory
   materialization model (pure, H12 with the dropped fields' cells), and codegen of the schema
   and the TypeScript declarations. The stream pieces in `ingest/write.rs`, with `to_json` built
   from them and its bytes unchanged on every existing fixture. Rows `hub-wire` and the pure half
   of `hub-materialize`. The model is sorted in byte order (collections by qualified id, records
   by `(qcoll, id)`), the pieces concatenate to `to_json`, and the head and tail lengths are
   exposed for `doc_bytes`. It touches `crates/`, so the full gate runs on develop after landing.
2. **hub-store.** In this order, each its own commit:
   1. `default-members = ["graph-server"]` in `server/Cargo.toml`, with the virtual-root
      `cargo tree` diff (`hub-virtual-root`) next to condition (b)'s in the slice report.
   2. The scratch copies of `scripts/orch/lock-parity.sh` (`scratch_setup`) and
      `scripts/orch/svc-features.sh`: both loop over the workspace members, copy each one's
      `Cargo.toml` and link each one's `src` with the computed relative path; nothing else
      changes. graph-render-4f receives the diff before it lands.
   3. `server/graph-store`, `deploy/postgres.Dockerfile` (`initdb --encoding=UTF8 --locale=C`),
      migrations (tables, `links`, `text_sha256`, `epoch_clock`, `hub_next_epoch()`, `hub_meta`,
      the per-event and `TRUNCATE` triggers `ENABLE ALWAYS`), the writer transaction (§5.1), the
      streamed materializer, changes, the plugin's records, retention and the sweeper.

   Rows `hub-virtual-root`, `svc-supply`, `hub-seq`, `hub-epoch`, `hub-changes-snapshot`,
   `hub-pg-durability`, and the
   database half of `hub-materialize`. Tests run against a PostgreSQL container under `drun`. It
   also adds the `tokio-postgres` pins and a `server-dependencies.md` amendment listing everything
   the driver pulls in, and runs `cargo-deny-server` and `lock-parity`.
3. **hub-api.** `server/graph-hub`, and `pub fn bearer` in graph-server in the same commit as
   its first caller, doc comment kept. Routes, keys and grants, semaphores, limits and start
   checks, SSE notices and the streaming relay. It also adds `deploy/hub.Dockerfile`,
   `scripts/orch/rows/hub.rows` with every §8 row, `docs/contract/hub-api.md`,
   `docs/deploy/hub.md` and `docs/measurements/hub-memory.md`, with the `/layout` upload measured
   against graph-server's body timeout (§5.3). `hub-virtual-root` re-runs with `client-legacy`
   present.
4. **hub-sdk.** `hub.ts`, `plugin.ts`, the example plugin, the `./hub` package export, and the
   rows `hub-sync` and `hub-sdk`.
5. **hub-report.** `docs/reports/hub-dod.md`: every row and every negative control run on the
   landing commit, the condition (b) diff, and the virtual-root diffs of slices 2, 3 and 5.

Slice 4 can start once slice 1 lands (generated types); its live rows run once slice 3 lands.
Slices 2 and 3 are sequential.

## 11. Unknowns and dependencies, settled in the slice that meets them

- What `tokio-postgres` pulls in, and whether `cargo deny` passes on it (slice 2). A failure is
  a stop.
- The hub parses `Authorization: Bearer` (RFC 6750 §2.1) with graph-server's private `bearer`
  (`server/graph-server/src/auth.rs:34`). graph-render-4f confirmed on 2026-10-05 that it becomes
  `pub` in slice 3, in the commit that adds its first caller, doc comment kept. The hub reuses
  `bearer` and `keys::KeySet::{load,parse,name_of}` only, never `auth::check` (it carries the
  `any-key` break knob, `auth.rs:24`, and the compute `App`).
- D8 (i), agreed with graph-render-4f: the path dependency with `default-features = false`, never
  forwarding `negctl` or `test-hooks`; `hub-breaks-off` proves it; the graph-core and graph-wasm
  linkage is stated in H1 and H8 with its Caveat and reopen trigger.
- D13, agreed: `default-members` in slice 2, with the virtual-root diff as the binding check.
- D7, agreed: this design's author makes the scratch-copy fix in slice 2 and sends graph-render-4f
  the diff before it lands.
- N4: a distinct graph-server code for a run that overran is graph-render-4f's call. Until then
  the hub relays graph-server's 503 with its `error` (§5.2).
- The hub's HTTP client to graph-server needs `hyper-util`'s `client-legacy` feature: a feature
  added to a crate already pinned (slice 3). `default-members` keeps it out of the shipped
  graph-server; `hub-virtual-root` and condition (b) prove that.
- Which axum feature SSE needs at the pinned 0.8.9, and whether a streamed request body needs any
  feature beyond `http1` (slice 3).
- Records this spec leans on that are not final:
  - `server-dependencies.md:3` is "proposed", pending its own verdict.
  - `service-api.md:3` read "blocked"; graph-render-4f has updated it to PROCEED-WITH-CONDITIONS
    (round 3, `docs/reviews/review-svc-r3.md`).
  - Neither is this spec's to change. Slice 2's amendment cites them as they stand.
- Ownership: graph-render-4f agreed on 2026-10-05 to the `members` addition to
  `server/Cargo.toml` and the shared `server/Cargo.lock`, with conditions (a)–(e) recorded
  verbatim in `docs/decisions/graph-hub.md`. In short: additive edits, `panic = "unwind"` kept;
  `cargo tree -p graph-server -e normal,build --locked` byte-identical before and after, and no
  version change in graph-server's lock entries; `svc-clippy`/`svc-test` (`--workspace`) pass
  without a live PostgreSQL, so every DB test sits behind its own row and negative control; the
  graph-server image still builds `--bin graph-server` only and `svc-image` stays green;
  `cargo-deny-server`, `lock-parity`, `svc-features` stay green. Their second reply (D13, D7, D8,
  `bearer` in slice 3) is recorded verbatim there too. Any other edit under `server/graph-server`
  goes to them.
- `server-and-write-path.md:32-33` defers streams and remote access. This spec lifts "streams"
  only in the sense of an SSE notice feed. It adds no Redis stream, and leaves remote access
  stop-and-ask (§3).

## 12. Verdict conditions, revision 1 → 2

| # | Condition (2026-10-05 BLOCK) | Met by |
|---|---|---|
| 1 | one dangling-reference policy, tested | H12, `hub-materialize` |
| 2 | tag value with `:` → 422 at write | §4 Record, `hub-wire` |
| 3 | `hub` module in default features, no new dependency, cost stated | H9, `motor-lock`, `codegen --check` |
| 4 | row-locked seq, idempotency row in the same transaction | H6, §5.1, `hub-seq` |
| 5 | one `REPEATABLE READ` snapshot, byte-order materialization | §5.3, `hub-materialize` |
| 6 | canonical text, not `jsonb`; `\u0000` → 422 | §4 Record, `hub-wire` |
| 7 | `fsync` and `synchronous_commit` on; PostgreSQL crash test | H3, `hub-pg-durability` |
| 8 | memory in bytes, concurrent materialization cap, measured peak | §6, `hub-memory` |
| 9 | byte-bounded SSE, defined resume protocol | H7, §5.3, `hub-events` |
| 10 | hub-only keys file, one swapped pair, 0640 grants | H8, §5.2 Grants, `hub-reload` |
| 11 | authz matrix, no veto across plugins, no existence leak, no auth-off | H12, §4 Manifests, `hub-authz` |
| 12 | `source=contract`, mapped motor answers | §5.2, `hub-motor-map` |
| 13 | deny, lock-parity, svc-features, `-p graph-hub` image, breaks off | §8 rows, `hub-breaks-off` |
| 14 | u64 seq in JavaScript; no key in a browser | §5 (2^53 − 1 ceiling), §7 |
| 15 | no `motor-alone` claim; deferrals lifted in writing; 4f agreement recorded | §8 note, §11 |

## 13. Verdict conditions and defects, revision 2 → 3

Conditions of the 2026-10-05 revision 2 BLOCK, per slice; [L] marks those that lift the block.

| # | Condition | Met by |
|---|---|---|
| 1 [L] | dropping a link field also drops its cells | H12, §5.3 `/graph`, `hub-materialize` (`keep-cells`) |
| 2 | `\u0000` in path ids; the `builder.rs:255` citation | §4 Record (`%00` → 400), H13 |
| 3 | define the manifest caps | §4 Manifests, `hub-wire`, `hub-limits` |
| 4 [L] | writer isolation, lock order, unique-violation path, sweeper | §5.1 writer transaction, `hub-seq`, `hub-idem` |
| 5 [L] | epoch, gap check, `since > head`, SQL edits bump | H15, §5.3 Cursors and Epoch triggers, `hub-epoch`, `hub-events` |
| 6 | named volume, container-level kill, `full_page_writes` | H3, `hub-pg-durability` |
| 7 [L] | lock-parity and svc-features scratch copies | §10 slice 2 step 2, `svc-supply` |
| 8 [L] | `default-members`, virtual-root `cargo tree` diff | H2, §10 slice 2 step 1, `hub-virtual-root` |
| 9 | close the snapshot before the motor call | §5.3 `/layout` |
| 10 [L] | memory: streaming, byte-capped `/changes` under a semaphore, streamed relay, a negative control that passes the start check, measured peak | H14, §5.3, §6 Memory, `hub-memory` |
| 11 [L] | `If-Match` per plugin, quotas, subscriber caps per key, authorize first | §5.1, §5.2 Order of refusals, §6, `hub-authz`, `hub-events` |
| 12 | every graph-server status, motor timeout, 503 not 408 | §5.2 map, §6, `hub-motor-map` |
| 13 | `drop-record` in the relay, ack delay, `sequence-seq` in `hub-events` | §5.3 `/layout`, §8 |
| 14 [L] | auth reuse conditions 1–4, `hub-breaks-off` through `cargo tree`, linkage stated | H1, H8, §5.2 Grants, §11, `hub-breaks-off`, `hub-authz` |
| 15 [L] | `sync` against the plugin's own records; identical upserts are no-ops | §4 Record, §5.2 records route, §7, `hub-sync` |
| 16 | retry transport errors with the same key | §7 `push`, `hub-sync` |
| 17 | every row and negative control on the landing commit | §8 preamble, §10 slice 5 |
| 18 | record the verdict and 4f's slice-3 confirmation | `docs/decisions/graph-hub.md` |

| Defect | Fix |
|---|---|
| D1 memory bound false | H14 streaming; `F_w` term for writers; `/changes` byte cap under `GRAPH_HUB_READS`; streamed relay; negative control `WRITERS=64 DB_POOL=70` passes the start check (§6, §8) |
| D2 `sync` churn and read grant | records route under `write:<plugin>`; no-op upserts (§4, §5.2, §7) |
| D3 resume holes | H15 epoch and triggers; cursor validity; first-seq check; monotone watch; heartbeat re-reads the epoch (H7, §5.3) |
| D4 `drop-record` placement | in the relay only (§5.3) |
| D5 vetoes, quota, subscribers, 403/404 | per-plugin `If-Match` and quota, shared cap stated with a Caveat, per-key subscriber cap, authorize first (§5.1, §5.2, §6) |
| D6 ack race | forced delay in `ack-before-commit`, named volume, container kill (§8) |
| D7 scratch gates | slice 2 step 2, `svc-supply` with grepped messages |
| D8 motor linked, `--print cfg` | linkage stated (H1, H8); `hub-breaks-off` through `cargo tree -i graph-server -p graph-hub` |
| D9 408 and retries | pool wait 503 with `Retry-After`; same-key retry of transport errors (§5.2, §7) |
| D10 manifest caps | defined (§4 Manifests) |
| D11 `hub-seq` visibility | `sequence-seq` also runs against `hub-events` (§8) |
| D12 absent delete, SSE timeout, RETAIN start check | no-op delete (§4); time to first byte only (§5.3); start checks (§6) |
| D13 virtual-root unification | `default-members` (H2), `hub-virtual-root` |

## 14. Verdict conditions and defects, revision 3 → 4

The 2026-10-05 revision 3 BLOCK (`docs/decisions/graph-hub.md`) found 16 defects (N1–N16) and left
eight revision 2 conditions partly met. The PostgreSQL facts behind N1, N2 and N12 are measured in
`docs/measurements/hub-pg-epoch-probe.md`.

**(a) Revision 2 conditions that were partly met.**

| # | Where it is met now |
|---|---|
| 4 [L] | §4 Manifests (the manifest PUT's lock order and the workspace create), §5.1 sweeper |
| 5 [L] | H15, §5.3 `/changes` and Epoch triggers, `hub-epoch`, `hub-changes-snapshot` |
| 8 [L] | `hub-virtual-root` on slices 2, 3 and 5, its negative control on graph-server's subtree |
| 9 | §5.3 `/layout`: `GRAPH_HUB_LAYOUTS` and its Caveat |
| 10 [L] | §6 limits and memory: every route under a permit, connection and header terms, the anti-join portal |
| 12 | §5.2 map: 408, the 422 split, the 429/503 split, the default row |
| 13 | `hub-seq`'s `sequence-seq` removes the lock and delays odd commits |
| 15 [L] | §7 `sync`: `deleted`, `If-Match` on `plugin_seq` |

Condition 18 holds: the revision 3 verdict is recorded in the ADR.

**(b) Defects.**

| Defect | Fix |
|---|---|
| N1 trigger DDL refused; `TRUNCATE`, replica and restore bypass | one trigger per event plus `TRUNCATE`, `ENABLE ALWAYS`; `epoch_clock` floor; `hub.writer` on the batch, manifest PUT, workspace create and sweeper; `hub_meta` restore detector; runbook and Caveat; reverse check in `hub-epoch` (§5.3, §8) |
| N2 `/changes` headers without operations | one `REPEATABLE READ, READ ONLY` transaction; stored `ops` count checked; `hub-changes-snapshot` (§5.1, §5.3, §8) |
| N3 422 from the caller's layout read as a hub defect | 422 split on `error` (§5.2) |
| N4 408 and 503 retried | 408 → 502 `MotorBodyTimeout`; 503 relayed without `Retry-After`; the SDK never retries `/layout`; the upload measured in slice 3; an overrun code is graph-render-4f's call (§5.2, §5.3, §7, §11) |
| N5 `/layout` holds a snapshot while graph-server queues | `GRAPH_HUB_LAYOUTS` = 1 and a Caveat (§5.3, §6) |
| N6 M not total | every route under `READS` or `LAYOUTS`; connection and header limits; the anti-join portal; the hash compare; manifests streamed (§5.1, §5.3, §6) |
| N7 `hub-memory` negative control cannot fail | sized from the measured `F_w`, `--memory 1g`, a barrier (§8) |
| N8 `doc_bytes` misses the head, the tail and manifests | head and tail lengths; the manifest PUT and the workspace create keep it (§4, §6) |
| N9 manifest PUT lock order | workspace row first; create by `ON CONFLICT DO NOTHING` (§4) |
| N10 virtual-root checked once | slices 2, 3 and 5; `client-legacy` grepped (§8, §10) |
| N11 `sequence-seq` cannot fail | removes the lock, delays odd commits (§8) |
| N12 byte order assumed | `initdb --encoding=UTF8 --locale=C` and a start check (§6, §10) |
| N13 no body timeout | `GRAPH_HUB_BODY_TIMEOUT_MS`, 408 (§6) |
| N14 adapters' `deleted` unmapped | a delete when stored, else dropped (§7, `hub-sync`) |
| N15 unlisted graph-server statuses | the default row, 502 `MotorError` (§5.2) |
| N16 records paging not a snapshot | `plugin_seq` per page, `If-Match`, start over on 412 (§5.2, §7, `hub-sync`) |
| H14 byte-equality precondition | hub-contract condition 1 below (§10 slice 1) |

**(c) Conditions per slice** ([L] lifts the revision 3 block).

| Slice | Condition | Where |
|---|---|---|
| hub-contract | 1. collections sorted by qualified id, records by `(qcoll, id)`, byte order; `hub-materialize` builds its reference from that order | §10 slice 1 |
| | 2. the streamed pieces concatenate to `to_json` | §10 slice 1 |
| | 3. head and tail lengths exposed | §6 `doc_bytes`, §10 slice 1 |
| hub-store | 1. [L] N1 in full, the reverse check included | §5.3 Epoch triggers, `hub-epoch` |
| | 2. [L] N2 and the prune-interleave test | §5.3 `/changes`, `hub-changes-snapshot` |
| | 3. N9, N12, N8 | §4 Manifests, §6 |
| | 4. the anti-join as a portal | §5.3 `/graph` |
| | 5. `hub-virtual-root` negative control on graph-server's subtree | §8 |
| | 6. the `svc-supply` diff to graph-render-4f before landing, negative-control messages grepped | §10 slice 2, §11 D7 |
| hub-api | 1. [L] N6: every route bounded, connection and header limits, every M term | §6 |
| | 2. N3, N4, N15 | §5.2, §5.3 |
| | 3. N5, N13 | §5.3 `/layout`, §6 |
| | 4. N7, the peak in `docs/measurements/hub-memory.md` | §8 `hub-memory` |
| | 5. N11 | §8 `hub-seq` |
| | 6. `hub-breaks-off` with `-e normal,build,features` | §8 |
| | 7. `hub-virtual-root` re-run with `client-legacy` present | §10 slice 3 |
| | 8. `pub fn bearer` lands with its first caller | §10 slice 3 |
| hub-sdk | 1. N14 and its `hub-sync` case | §7, §8 |
| | 2. N16 | §7, §8 |
| | 3. `/layout` not retried | §7, `hub-sdk` |
| | 4. the first-change check tested by gap injection | §7, `hub-sdk` |
| hub-report | 1. every row and negative control on the landing commit; a green negative control is a failure | §8 preamble, §10 slice 5 |
| | 2. the (b) diff and the virtual-root diffs of slices 2, 3 and 5 | §10 slice 5 |
| | 3. the verdict recorded in the ADR | `docs/decisions/graph-hub.md` |
