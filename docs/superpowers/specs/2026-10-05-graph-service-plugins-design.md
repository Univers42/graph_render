# graph-hub: a graph service that plugins feed — design

Date 2026-10-05.

**Status: revision 2, re-submitted to the `devil`.** Revision 1 drew a BLOCK with 15 conditions
the same day; §12 lists each condition and where this revision meets it. The decision record is
`docs/decisions/graph-hub.md`.

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
- streams the changes to anyone subscribed;
- serves the workspace as an ingest document, or as a laid-out snapshot computed by the existing
  motor service.

Two kinds of reader use it: plugin authors, in any language over HTTP and JSON, and app
developers, through the TypeScript SDK and later the studio.

v1 is done when each of these holds, each one a gate row (§8) with a negative control:

- A plugin built on the SDK registers its manifest and pushes batches. A second client sees
  every change over SSE in `seq` order, and resumes after a disconnect without a gap or a
  duplicate, even while other writers commit.
- `POST /v1/workspaces/{ws}/layout` returns bytes identical to graph-server's `POST /v1/layout`
  (`source=contract`) run on the document `GET /v1/workspaces/{ws}/graph` returns at the same
  `seq`.
- That document always passes graph-contract's reader (`ingest::read`), whatever the plugins
  pushed or deleted, and in whatever order.
- Every batch the hub acknowledged survives a `kill -9` of the hub, and a `kill -9` of
  PostgreSQL.
- A key writes only the plugins it is granted.
- Peak memory at the configured caps is measured and stays under the container's limit.
- These stay green: the rows `svc-*`, `lock-parity`, `svc-features` and `cargo-deny-server`, the
  wasm32 build of graph-core, and `codegen --check`.
- The motor workspace's `Cargo.lock` and graph-core's dependency list do not change.

## 2. Decisions

The user's standing instruction is full autonomy with no questions, so each decision below is
the recommended default, recorded here. Each is open to the verdict.

| # | Decision | Why | Rejected |
|---|---|---|---|
| H1 | Two services. graph-server keeps its stateless compute, unchanged. A new **graph-hub** holds the state and asks graph-server for layouts over HTTP. | The user asked for micro-services. Compute keeps its own slots and caps, and scales apart from the hub. The hub never runs motor math. | The hub links `graph-wasm` and lays out in process: one network hop fewer, but a second copy of the slots and caps. |
| H2 | `server/graph-hub` (a binary plus its lib) and `server/graph-store` (a lib), both members of the `server/` workspace. | `server-and-write-path.md` puts graph-store in `server/`, so nothing under `crates/` reaches either. | `crates/graph-store`, which would put a database driver into the motor's lockfile. |
| H3 | PostgreSQL 17 through `tokio-postgres`, image built from Debian (`deploy/postgres.Dockerfile`), with `fsync` and `synchronous_commit` on. | The user chose it on 2026-09-29 (the ADR); `images-from-debian.md`; `server-dependencies.md` already names `tokio-postgres`. | SQLite has fewer moving parts but contradicts the recorded choice. |
| H4 | The stored model is the ingest contract itself: collections that plugins declare, and records. | One data model for the adapters, the service and the hub. graph-contract's writer stays the only producer of canonical ingest bytes (the ADR). | A new property-graph model: a second definition of graph input, plus a mapping layer. |
| H5 | A plugin is an out-of-process client. It owns one namespace (its plugin id) in a workspace and **pushes** to the hub. | Any language works and no plugin code runs inside the hub, so a plugin crash cannot take the hub down. | In-process plugins (wasm, dylib) need a sandbox, an ABI and resource limits. Pull or webhook plugins open SSRF, and need a scheduler and retries. |
| H6 | `seq` comes from a row-locked `UPDATE workspaces SET head_seq = head_seq + 1 ... RETURNING head_seq`, inside the batch's transaction. | The ADR's write model. Writers to one workspace serialize on that row, so seqs have no gap and no duplicate. A rolled-back batch leaves no hole, which a `SEQUENCE` would. | A PostgreSQL `SEQUENCE`; Redis streams (ADR item D2, a stop-and-ask item). |
| H7 | Live fan-out in process, after commit: a `tokio::sync::watch` per workspace carries only the newest `head_seq`. Subscribers read change bodies from the database (§5.3). | One caller, one node. A notification holds no change body, so memory does not grow with the change size. | PostgreSQL `LISTEN/NOTIFY` (needed only with two hub instances); a broadcast of change bodies (memory grows with the batch size). |
| H8 | The hub has its **own** keys file, `GRAPH_HUB_KEYS_FILE`, in graph-server's format. It reuses graph-server's `keys::KeySet` (`load`, `parse`, `name_of`) through a path dependency on its lib. Grants live in `GRAPH_HUB_GRANTS_FILE`. Auth cannot be turned off. | One credential format and one constant-time check. A hub key does not grant graph-server compute, and the reverse. | Sharing graph-server's file, where every hub key would also be a compute key; scopes inside the keys file, which would change graph-render-4f's format. |
| H9 | The hub's wire types (manifest, batch, change) live in a new `hub` module of graph-contract, built in its default features with no new dependency, and read and written by hand like `ingest`. Their schema derives sit under the existing `codegen` feature, and `graph-cli codegen` writes the JSON schema and TypeScript declarations. | graph-contract is the single source of truth for the wire, and the SDK's types are generated from it. Cost, accepted: `crates/` is fingerprinted, so every change to the hub wire voids the motor's gate evidence until the full gate runs again. That is why the hub wire is frozen once slice 1 lands. | Types written by hand in both the hub and the SDK, which makes two definitions. |
| H10 | The SDK is TypeScript only, in `crates/graph-sdk-js/src/hub*.ts` and `plugin*.ts`. | One caller language today. The wire is HTTP plus JSON, specified in `docs/contract/hub-api.md`, so any language can push. | A Python SDK, which has no caller. |
| H11 | The change log is bounded by count (`GRAPH_HUB_RETAIN`) and by bytes (`GRAPH_HUB_RETAIN_BYTES`) per workspace. A `since` older than the oldest kept change gets 410, and the client resyncs from `/graph`. | Disk and memory stay bounded (`memory-guard.md`), and a tracker runs for months. | An unbounded log, which grows forever. |
| H12 | **Materialization drops what cannot resolve.** `/graph` and `/layout` leave out link and parent cells naming an absent record, and link fields naming an unregistered collection. The stored record keeps them, so the edge appears once the target arrives. | The reader refuses a dangling reference (`check_link_cells`, `crates/graph-contract/src/ingest/validate/cells.rs:28`), and independent plugins cannot order their pushes. Writes therefore never depend on another plugin's state, and a key learns nothing about other plugins from a refusal. | Refusing dangling writes and keeping tombstones: pushes would have to be ordered across plugins, and a 422 would reveal whether another plugin's record exists. |
| H13 | Tags are workspace-wide. The hub tag `tag:<value>` (`crates/graph-core/src/ingest/build.rs`) joins records from every plugin that uses the value. | This is the point of one graph per workspace: a `prod` tag from a hosts plugin and from a tickets plugin meet on one node. A plugin that wants private tags prefixes its values. | Tags per plugin, which needs a change to the motor's derivation. |

## 3. Architecture

```
plugin A ─┐  HTTP + hub key (write:ws/A)                 ┌─ app or studio (SDK createHub)
plugin B ─┼──► graph-hub ──SQL──► PostgreSQL              │
          │      │  ▲ SSE /events, /changes, /graph ──────┤
          │      └── POST /v1/layout?source=contract ──► graph-server   (snapshot bytes relayed)
```

| Unit | Does | Depends on | Never |
|---|---|---|---|
| graph-contract `hub` module | wire types, strict readers, canonical writers, codegen | what graph-contract has | a new dependency |
| `server/graph-store` | SQL schema and migrations; one transaction per batch; reads: materialize a workspace in one snapshot, changes after a seq, one record | `tokio-postgres`, graph-contract | HTTP |
| `server/graph-hub` | routes, keys and grants, limits, SSE, the motor client | graph-store, graph-server's lib (`keys` only), graph-contract, axum, hyper (already pinned) | motor math; edits to graph-server |
| SDK `hub.ts`, `plugin.ts` | reading, subscribing, laying out; registering, pushing, syncing | `fetch`, the existing snapshot reader | Node-only APIs in the browser path |

The hub binds `127.0.0.1` by default, as graph-server does. Publishing it beyond the host, and
TLS, stay stop-and-ask items (`server-and-write-path.md:32-33`, `docs/deploy/service.md:65`).

## 4. Data model

**Workspace.**
- Id `[a-z0-9][a-z0-9-]{0,62}`.
- It is the `source` of the materialized ingest document.
- It has `head_seq` (u64) and `doc_bytes`.

**Plugin.**
- Id `[a-z0-9][a-z0-9-]{0,62}`, so it contains no `.` and no `:`.
- It has a `manifestVersion` (u32) and collections.
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

**Record.**
- Key: (plugin, collection, id).
- `rev` is a u64 the hub assigns, +1 per write.
- `updatedAt` is a u32 the source assigns; the hub carries it.
- `values` is keyed by declared field id. An undeclared field id is 422. Each cell is checked
  against its field's role at write time with graph-contract's cell rules. A tag value containing
  `:` is 422 at write time: the motor's derivation refuses it (`build.rs:55-66`).
- Values are stored as the canonical text graph-contract's writer produces for the record, never
  as `jsonb`, so `-0`, large integers and key order come back byte-identical. A `\u0000` is 422
  at the reader, never a 500 from the database.
- A delete removes the row and writes the delete to the change log.

**Manifest changes.**
- In v1 a manifest only grows: a higher `manifestVersion` may add collections and fields.
- Any change to an existing collection or field, or any removal, is 409.
- The same version with different content is 409.
- The same version with the same content is a 200 that changes nothing.
- A 409 depends only on the plugin's own manifest, never on another plugin's.
- Restructuring a plugin is an operator task in SQL in v1, like deleting a workspace.

## 5. Wire: hub-api v1

Bodies follow the ingest reader's conventions: an unknown member refuses the document, every id
is checked, and output is canonical. `seq` and `rev` are JSON numbers; the hub stops accepting
writes to a workspace at 2^53 − 1, so JavaScript reads them exactly. That ceiling is unreachable
in practice: at 10^6 commits a second it takes 285 years.

### 5.1 Bodies

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
- A batch is atomic: it all commits or none of it does.
- A record appears at most once per batch across `upserts` and `deletes`, else 422.
- `If-Match: "<seq>"` is optional. When the workspace's `head_seq` differs, the answer is 409.
- `Idempotency-Key` takes at most 128 bytes and is stored per (workspace, plugin, key) for 24 h,
  written in the batch's own transaction.
  - A replay returns the stored response and adds no seq.
  - The same key with a different body (sha256) is 422.

A change, as `/changes` returns it and as SSE `data:` carries it:

```json
{"seq":118,"plugin":"tracker","at":"2026-10-05T09:12:44.120Z","upserts":[],"deletes":[]}
```

- Records in a change carry qualified collection ids and their `rev`.
- `at` is PostgreSQL's `clock_timestamp()` read when the batch takes its seq. It is
  informational and not ordered: order is `seq`. The hub may have a clock; the motor never does.

### 5.2 Routes

| Method | Path | Grant | Returns |
|---|---|---|---|
| GET | `/healthz` | none | `ok` |
| GET | `/v1/meta` | any key | `{api:1, version, limits}` |
| PUT | `/v1/workspaces/{ws}` | admin | 201 on create, 200 when it exists |
| GET | `/v1/workspaces` | any key | the workspaces the key may read, each with its `head_seq` |
| PUT | `/v1/workspaces/{ws}/plugins/{plugin}` | write | the stored manifest, 201 or 200 |
| GET | `/v1/workspaces/{ws}/plugins` | read | every manifest |
| POST | `/v1/workspaces/{ws}/plugins/{plugin}/batches` | write | `{seq}` |
| GET | `/v1/workspaces/{ws}/graph` | read | the canonical ingest document, `ETag: "<seq>"`; `If-None-Match` gives 304 |
| GET | `/v1/workspaces/{ws}/records/{plugin}/{collection}/{id}` | read | the record and its `rev` |
| GET | `/v1/workspaces/{ws}/changes?since=&limit=` | read | changes after `since`; 410 when `since` is older than the oldest change kept |
| GET | `/v1/workspaces/{ws}/events` | read | SSE (§5.3) |
| POST | `/v1/workspaces/{ws}/layout?layout=&post=` | read | graph-server's snapshot for the materialized document, `Accept` passed through, plus a `Graph-Seq` header |

**Errors** use graph-server's JSON shape, `{error, message}`. The hub has its own error enum;
graph-server's `error.rs` stays untouched. graph-server's answers to `/layout` map as follows:

| graph-server | hub | Why |
|---|---|---|
| 200 | 200, bytes relayed unchanged | |
| 413 (body or per-id work caps) | 413 `GraphTooLarge`, with graph-server's message | the workspace is past what the motor accepts |
| 422 | 502 `MaterializeInvalid`, logged as a hub defect | H12 guarantees a readable document; the `hub-materialize` row (§8) proves it |
| 429 or 503 | 503 with `Retry-After` | the motor is busy |
| unreachable or timeout | 502 `MotorUnavailable` | |

**Grants.**
- `GRAPH_HUB_GRANTS_FILE` holds lines of the form `<key-name> <ws|*> <read|write:<plugin>|admin>`.
- A key with no grant is denied by default.
- `admin` implies `read` and every `write`.
- The grants file gets the same permission check as the keys file (mode 0640 or stricter).
- On `SIGHUP` the hub loads both files and swaps them together as one
  `RwLock<Arc<(KeySet, Grants)>>`. If either file fails to load, the old pair stays.

### 5.3 Snapshots, order and the event stream

**Reads.**
- `/graph` and `/layout` read `head_seq` and the records inside one `REPEATABLE READ`
  transaction, so the document is exactly the state at `Graph-Seq`.
- Materialization is a single function over that snapshot. It orders collections and records
  by `(plugin, collection, id)` in byte order (`COLLATE "C"`), then writes the document with
  graph-contract's canonical writer.
- So the same seq gives the same bytes, whatever order the records were inserted in.

**SSE.**
- `GET .../events` takes a cursor: `Last-Event-ID`, else `?since=`, else the current `head_seq`.
- The protocol, per subscriber:
  1. Subscribe to the workspace's `watch` channel first.
  2. Page through `changes` after the cursor, in seq order, one change in memory at a time.
     Each change is sent as event id = seq, and the cursor advances.
  3. When caught up, wait on the `watch`, then go back to step 2.
- No change can be missed. The `watch` only says that `head_seq` moved, and the database is read
  from the cursor, so a change committed during step 2 is read in the next pass.
- No change can be sent twice. The cursor only moves forward.
- When retention has passed the cursor, the stream sends `event: resync` and closes. The SDK
  then reloads `/graph`.
- A comment heartbeat goes out every 15 s.

## 6. Limits

| Env | Default | Over the limit |
|---|---|---|
| `GRAPH_HUB_MAX_BODY` | 4 MiB | 413 |
| `GRAPH_HUB_MAX_BATCH` | 10 000 records | 413 |
| `GRAPH_HUB_MAX_DOC_BYTES` (per workspace) | 64 MiB | 413 on the batch that would cross it |
| `GRAPH_HUB_RETAIN` / `GRAPH_HUB_RETAIN_BYTES` (per workspace) | 100 000 changes / 512 MiB | older changes are pruned in the commit's transaction; a `since` below them gets 410 |
| `GRAPH_HUB_MATERIALIZE` (concurrent `/graph` + `/layout`) | 2 | 503 with `Retry-After` |
| `GRAPH_HUB_MAX_SUBSCRIBERS` | 64 | 429 |
| `GRAPH_HUB_DB_POOL` | 8 (must exceed `GRAPH_HUB_MATERIALIZE`, checked at start) | requests queue, bounded by `GRAPH_HUB_TIMEOUT_MS` (30 000), then 408 |
| `GRAPH_MOTOR_URL`, `GRAPH_MOTOR_KEY_FILE` | `http://127.0.0.1:8080`, none | the hub's own graph-server key |

**doc_bytes.**
- `doc_bytes` is the sum of each stored record's canonical bytes. The batch's transaction keeps
  it current.
- It bounds the materialized document from above: H12 only removes cells, and the collection
  declarations are bounded by the manifest caps.
- The 64 MiB default equals graph-server's `GRAPH_MAX_BODY` default, so a workspace the hub
  accepts fits graph-server's body limit. graph-server's per-id work caps can still answer 413,
  which maps as in §5.2.

**Memory.**
- The budget is the sum of three terms:
  - `GRAPH_HUB_MATERIALIZE` × (the document plus its rows, up to `GRAPH_HUB_MAX_DOC_BYTES` each);
  - `GRAPH_HUB_DB_POOL` × `GRAPH_HUB_MAX_BODY`;
  - `GRAPH_HUB_MAX_SUBSCRIBERS` × one change.
- That budget is a claim until slice 3 measures peak RSS at every cap at once and records it in
  `docs/measurements/hub-memory.md`. The defaults then shrink until the peak fits the hub
  container's 1 GiB under `scripts/orch/drun`.
- Caveat: `/graph` and `/layout` hold the whole document in memory, because graph-contract writes
  canonical JSON from a whole document. A streaming writer is the upgrade, once a workspace needs
  more than 64 MiB.

## 7. SDK (TypeScript)

```ts
const hub = createHub({ baseUrl, apiKey });
const { seq, ingest } = await hub.graph("ops");
const snapshot = await hub.layout("ops", { layout: "fa2" });       // existing snapshot reader
const stop = hub.subscribe("ops", { since: seq, onChange, onResync }); // fetch + stream SSE parser

const plugin = createPlugin({ baseUrl, apiKey, plugin: "tracker", manifest });
await plugin.register("ops");
await plugin.push("ops", { upserts, deletes });  // Idempotency-Key per call; 429 and 503 retried 3 times
await plugin.sync("ops", rowsToIngest(rows));    // full state: diff against /graph, push batches
```

- `subscribe` uses `fetch` and parses the stream itself, because `EventSource` cannot send an
  `Authorization` header. It reconnects with `Last-Event-ID`, and on `resync` calls `onResync`.
- `sync` turns any adapter's `Ingest` (`rowsToIngest`, `notionToIngest`) into plugin batches.
  Caveat: it reads the whole workspace graph on every call, O(n); a plugin that knows its own
  changes calls `push` instead.
- Browser use keeps `remote.ts`'s `dangerouslyAllowBrowser` rule: a key held in a page is public
  (`service-api.md`).
- One example plugin, `examples/plugins/rows-file/`: it reads a JSON rows file, passes it through
  `rowsToIngest`, and calls `sync`. The `hub-sdk` row runs it.

## 8. Gates

Every row has a negative control that must turn it red. The breaks sit behind a `negctl` feature,
off in every shipped build, as graph-server's do.

| Row | Slice | Proves | Negative control |
|---|---|---|---|
| `hub-materialize` | 1, 2 | Random op sequences (cross-plugin links, deletes of linked records, links to unregistered collections) always give a document `ingest::read` accepts. Same seq gives same bytes; a permuted insertion order gives the same hash. | `keep-dangling`: materialization keeps unresolved cells |
| `hub-wire` | 1 | readers refuse unknown members, `B.coll` in a batch, a tag with `:`, `\u0000`; `-0` and 2^53 − 1 round-trip byte-identical | `lax-reader` |
| `hub-floor` | 2, 3 | fmt, clippy `-D warnings` and tests over the `server/` workspace | an unformatted file in a scratch copy |
| `hub-seq` | 2 | 100 concurrent writers on one workspace: seqs 1..N, no gap, no duplicate, idempotency rows present | `sequence-seq`: seq from a `SEQUENCE` with one rolled-back batch |
| `hub-pg-durability` | 2 | `fsync` and `synchronous_commit` read `on` in the running image; `kill -9` of PostgreSQL after acks loses no acked seq | an image with `synchronous_commit=off`: the check row fails |
| `hub-durability` | 3 | 200 batches, `kill -9` of the hub right after an ack, restart: every acked seq present, no gap | `ack-before-commit` |
| `hub-roundtrip` | 3 | hub `/layout` bytes = graph-server `/v1/layout?source=contract` on `/graph`'s document at the same seq, for 3 fixtures | `drop-record` |
| `hub-authz` | 3 | the key matrix: `write:A` writing `B.coll` refused; read-only writing 403; unknown key 401; no grant 403; refusals identical whether or not another plugin exists | `skip-grant` |
| `hub-reload` | 3 | `SIGHUP` with a bad grants file keeps the old pair; a good pair swaps both | `reload-keys-only` |
| `hub-events` | 3 | a reconnect while 4 writers commit: no gap, no duplicate; `resync` past retention | `skip-event` |
| `hub-motor-map` | 3 | each §5.2 mapping, against a stub graph-server | `pass-through-422` |
| `hub-idem` | 3 | a replayed key gives the same response and the same `head_seq`; another body is 422 | `no-idem` |
| `hub-limits` | 3 | each 413, 429 and 503 of §6; manifest 409s | `no-cap` |
| `hub-memory` | 3 | peak RSS at every cap at once < 1 GiB, recorded in `docs/measurements/hub-memory.md` | `GRAPH_HUB_MATERIALIZE=16`: peak over the limit |
| `hub-breaks-off` | 3 | graph-server's `breaks::on()` is const-false in the hub's release build | built with `negctl` |
| `hub-sdk` | 4 | SDK tests and the example plugin against live hub, PostgreSQL and graph-server containers | `GM_HUB_SDK_BREAK=1`: the example pushes a wrong record |
| `cargo-deny-server`, `lock-parity`, `svc-features` | 2, 3 | re-run with the new members; the hub image builds `-p graph-hub` only | existing |
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
  or fields. Operators do these in SQL.
- TLS and publishing beyond the host (stop-and-ask), per-key quotas, a Python SDK.
- `pgvector`, which the images ADR lists but nothing calls.
- Live studio integration. It needs host-api work in `packages/graph-studio/src/host/*`, which
  graph-render-4f owns, so it is a later slice agreed with them.

## 10. Slices

Each slice has its own branch and worktree and runs as an OpenCode job with an exact brief, in
this order.

1. **hub-contract.** graph-contract's `hub` module: the types, strict readers, canonical
   writers, the cell checks used at write time, the materialization function (pure, over an
   in-memory record set, H12), and codegen of the schema and the TypeScript declarations. Rows
   `hub-wire` and the pure half of `hub-materialize`. It touches `crates/`, so the full gate runs
   on develop after landing.
2. **hub-store.** `server/graph-store`, `deploy/postgres.Dockerfile`, migrations, the
   transactional batch apply (H6), the snapshot read, changes and retention. Rows `hub-seq` and
   `hub-pg-durability`, plus the database half of `hub-materialize`. Tests run against a
   PostgreSQL container under `drun`. It also adds the `tokio-postgres` pins and a
   `server-dependencies.md` amendment listing everything the driver pulls in, and runs
   `cargo-deny-server` and `lock-parity`.
3. **hub-api.** `server/graph-hub`: routes, keys and grants, limits, SSE and the motor client.
   It also adds `deploy/hub.Dockerfile`, `scripts/orch/rows/hub.rows` with every §8 row,
   `docs/contract/hub-api.md`, `docs/deploy/hub.md` and `docs/measurements/hub-memory.md`.
4. **hub-sdk.** `hub.ts`, `plugin.ts`, the example plugin, the `./hub` package export, and the
   `hub-sdk` row.
5. **hub-report.** `docs/reports/hub-dod.md`: every row run on the landing commit.

Slice 4 can start once slice 1 lands (generated types). Slices 2 and 3 are sequential.

## 11. Unknowns and dependencies, settled in the slice that meets them

- What `tokio-postgres` pulls in, and whether `cargo deny` passes on it (slice 2). A failure is
  a stop.
- The hub parses `Authorization: Bearer` (RFC 6750 §2.1) with graph-server's private `bearer`
  (`server/graph-server/src/auth.rs:34`). graph-render-4f agreed (ADR) that slice 3 makes it
  `pub` in the same commit as its first caller, doc comment kept. The hub reuses `bearer` and
  `keys::KeySet::{load,parse,name_of}` only, never `auth::check` (it carries the `any-key` break
  knob, `auth.rs:24`, and the compute `App`). The hub never carries a second copy.
- The hub's HTTP client to graph-server needs `hyper-util`'s `client-legacy` feature: a feature
  added to a crate already pinned (slice 3, re-run `svc-features`).
- Which axum feature SSE needs at the pinned 0.8.9 (slice 3).
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
  `cargo-deny-server`, `lock-parity`, `svc-features` stay green. Any other edit under
  `server/graph-server` goes to them.
- `server-and-write-path.md:32-33` defers streams and remote access. This spec lifts "streams"
  only in the sense of an SSE change feed. It adds no Redis stream, and leaves remote access
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
| 11 | authz matrix, no veto across plugins, no existence leak, no auth-off | H12, §4 Manifest, `hub-authz` |
| 12 | `source=contract`, mapped motor answers | §5.2, `hub-motor-map` |
| 13 | deny, lock-parity, svc-features, `-p graph-hub` image, breaks off | §8 rows, `hub-breaks-off` |
| 14 | u64 seq in JavaScript; no key in a browser | §5 (2^53 − 1 ceiling), §7 |
| 15 | no `motor-alone` claim; deferrals lifted in writing; 4f agreement recorded | §8 note, §11 |
