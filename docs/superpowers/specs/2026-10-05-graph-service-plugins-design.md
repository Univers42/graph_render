# graph-hub: a graph service that plugins feed — design

Date 2026-10-05. Status: **proposed**. A `devil` verdict is owed before any code
(`.claude/rules/devil/risk.md`: public surface, data and schema). This is phase D1 of
`docs/decisions/server-and-write-path.md` (the store and the write path). D2, the Redis
snapshot cache, stays where `docs/decisions/snapshot-cache.md` put it.

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
  duplicate.
- `POST /v1/workspaces/{ws}/layout` returns bytes identical to graph-server's `POST /v1/layout`
  run on the document `GET /v1/workspaces/{ws}/graph` returns.
- Every batch the hub acknowledged survives a `kill -9` of the hub.
- A key writes only the plugins it is granted.
- The rows `motor-alone`, `svc-*` and `lock-parity` stay green, and graph-server's files stay
  unchanged.

## 2. Decisions

The user's standing instruction is full autonomy with no questions, so each decision below is
the recommended default, recorded here. Each is open to the verdict.

| # | Decision | Why | Rejected |
|---|---|---|---|
| H1 | Two services. graph-server keeps its stateless compute, unchanged. A new **graph-hub** holds the state and asks graph-server for layouts over HTTP. | The user asked for micro-services. Compute keeps its own slots and caps, and scales apart from the hub. The hub never runs motor math. | The hub links `graph-wasm` and lays out in process: one network hop fewer, but a second copy of the slots and caps. |
| H2 | `server/graph-hub` (a binary plus its lib) and `server/graph-store` (a lib), both members of the `server/` workspace. | `server-and-write-path.md` puts graph-store in `server/`. Nothing under `crates/` reaches either. graph-render-4f agreed to the layout on 2026-10-05. | `crates/graph-store`, which would put a database driver into the motor's lockfile. |
| H3 | PostgreSQL 17 through `tokio-postgres`, image built from Debian (`deploy/postgres.Dockerfile`). | The user chose it on 2026-09-29 (the ADR); `images-from-debian.md`; `server-dependencies.md` already names `tokio-postgres`. | SQLite has fewer moving parts but contradicts the recorded choice. |
| H4 | The stored model is the ingest contract itself: collections that plugins declare, and records. | One data model for the adapters, the service and the hub. graph-contract's writer stays the only producer of canonical ingest bytes (the ADR). | A new property-graph model: a second definition of graph input, plus a mapping layer. |
| H5 | A plugin is an out-of-process client. It owns one namespace (its plugin id) in a workspace and **pushes** to the hub. | Any language works and no plugin code runs inside the hub, so a plugin crash cannot take the hub down. | In-process plugins (wasm, dylib) need a sandbox, an ABI and resource limits. Pull or webhook plugins open SSRF, and need a scheduler and retries. |
| H6 | Every committed batch takes the next `seq` of its workspace. The change log feeds both `/changes` and SSE `/events`. | The ADR's write model: `head_seq`, `rev + 1`, one change row. | Redis streams, which are ADR item D2 and stop-and-ask. |
| H7 | Live fan-out in process (`tokio::sync::broadcast`) after commit, on one hub instance. | One caller, one node. | PostgreSQL `LISTEN/NOTIFY`, needed only once there are two hub instances. |
| H8 | Auth reuses graph-server's keys (same file, hash and constant-time check) through a path dependency on its lib. Grants live in a second file, `GRAPH_HUB_GRANTS_FILE`. | No second credential format, and graph-server's files stay untouched. | Scopes inside the keys file, which changes graph-render-4f's format and `tests/auth.rs`. |
| H9 | The hub's wire types (manifest, batch, change) live in a new `hub` module of graph-contract, read and written by hand like `ingest`. Their schema derives sit under the existing `codegen` feature, and `graph-cli codegen` writes the JSON schema and TypeScript declarations. | graph-contract is the single source of truth for the wire, and the SDK's types are generated from it. | Types written by hand in both the hub and the SDK: two definitions. |
| H10 | The SDK is TypeScript only, in `crates/graph-sdk-js/src/hub*.ts` and `plugin*.ts`. | One caller language today. The wire is HTTP plus JSON, specified in `docs/contract/hub-api.md`, so any language can push. | A Python SDK, which has no caller. |
| H11 | The change log is bounded: `GRAPH_HUB_RETAIN` changes per workspace. A `since` older than the oldest kept change gets 410, and the client resyncs from `/graph`. | Disk and memory stay bounded (`memory-guard.md`), and a tracker runs for months. | An unbounded log, which grows forever. |

## 3. Architecture

```
plugin A ─┐  HTTP + key (write:ws/A)                     ┌─ app or studio (SDK createHub)
plugin B ─┼──► graph-hub ──SQL──► PostgreSQL              │
          │      │  ▲ SSE /events, /changes, /graph ──────┤
          │      └── POST /v1/layout ──► graph-server ────┘  (snapshot bytes relayed)
```

| Unit | Does | Depends on | Never |
|---|---|---|---|
| graph-contract `hub` module | wire types, strict readers, canonical writers, codegen | what graph-contract has | a new dependency |
| `server/graph-store` | SQL schema and migrations; one transaction per batch; reads: materialize a workspace as `graph_contract::ingest::Ingest`, changes since a seq, one record | `tokio-postgres`, graph-contract | HTTP |
| `server/graph-hub` | routes, keys and grants, limits, SSE fan-out, the motor client | graph-store, the graph-server lib (keys and auth only), graph-contract, axum, hyper (already pinned) | motor math; edits to graph-server |
| SDK `hub.ts`, `plugin.ts` | reading, subscribing, laying out; registering, pushing, syncing | `fetch`, the existing snapshot reader | Node-only APIs in the browser path |

## 4. Data model

**Workspace.**
- Id `[a-z0-9][a-z0-9-]{0,62}`.
- It is the `source` of the materialized ingest document.
- It has a `head_seq`.

**Plugin.**
- Id `[a-z0-9][a-z0-9-]{0,62}`, so it contains no `.` and no `:`.
- It has a `manifestVersion` (u32) and collections.
- A plugin's collection ids match `[A-Za-z0-9_-]{1,64}`.
- In the materialized document a collection becomes `<plugin>.<collection>`, so a node id reads
  `<ws>:<plugin>.<collection>:<record>`, which is the contract's own grammar (H5 of the ingest
  contract).

**Links.**
- In a manifest, `link.collection` is either `coll` (the plugin's own collection) or
  `other.coll` (another plugin's collection, which must already be registered, else 422). The
  hub rewrites both to qualified ids.
- A link to a record that has not been pushed yet, or has been deleted, is legal: the motor
  states it and drops it (`crates/graph-core/src/ingest/tests/build/links.rs:4`). The edge
  appears once the record arrives.

**Record.**
- Key: (plugin, collection, id).
- `rev` is a u64 that the hub assigns, +1 per write.
- `updatedAt` is a u32 the source assigns; the hub carries it.
- `values` is keyed by declared field id. An undeclared field id is 422.
- A delete removes the row and logs the delete in the change log.

**Manifest changes.**
- A higher `manifestVersion` may add collections and fields, and may change a field's role.
- Removing a collection or a field that holds records, or that another plugin links to, is 409.
- The same version with different content is 409.
- The same version with the same content is a 200 that changes nothing.

## 5. Wire: hub-api v1

Bodies follow the ingest reader's conventions: an unknown member refuses the document, every id
is checked, and output is canonical.

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
- The plugin comes from the URL, never from the body, and collections are plugin-local ids.
- A batch is atomic: it all commits or none of it does.
- A record appears at most once per batch across `upserts` and `deletes`, else 422.
- `If-Match: "<seq>"` is optional. When the workspace's `head_seq` differs, the answer is 409:
  a plugin that reads, then writes, can detect a concurrent writer.
- `Idempotency-Key` takes at most 128 bytes and is stored per (workspace, plugin, key) for 24 h.
  - A replay returns the stored response and adds no seq.
  - The same key with a different body (sha256) is 422.

A change, as `/changes` returns it and as SSE `data:` carries it:

```json
{"seq":118,"plugin":"tracker","at":"2026-10-05T09:12:44.120Z","upserts":[],"deletes":[]}
```

- Records in a change carry qualified collection ids and their `rev`.
- `at` is PostgreSQL's commit-time clock. The hub may have a clock; the motor never does.

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
| GET | `/v1/workspaces/{ws}/events` | read | SSE: event id = seq; resume from `Last-Event-ID` or `?since=`; a comment heartbeat every 15 s |
| POST | `/v1/workspaces/{ws}/layout?layout=&post=` | read | graph-server's answer to the materialized document, with `Accept` passed through and a `Graph-Seq` header |

Errors use graph-server's JSON shape, `{error, message}`. The hub's own codes (`Forbidden` 403,
`Conflict` 409, `Gone` 410, plus the shared ones) are listed in `hub-api.md`. The hub has its own
error enum; graph-server's `error.rs` stays untouched.

**Grants.**
- `GRAPH_HUB_GRANTS_FILE` holds lines of the form `<key-name> <ws|*> <read|write:<plugin>|admin>`.
- A key with no grant is denied by default.
- `admin` implies `read` and every `write`.
- `SIGHUP` reloads keys and grants together, atomically.

## 6. Limits

| Env | Default | Over the limit |
|---|---|---|
| `GRAPH_HUB_MAX_BODY` | 16 MiB | 413 |
| `GRAPH_HUB_MAX_BATCH` | 10 000 records | 413 |
| `GRAPH_HUB_MAX_RECORDS` (per workspace) | 1 000 000 | 413 on the batch that would cross it |
| `GRAPH_HUB_RETAIN` (changes per workspace) | 100 000 | older changes are pruned in the commit's transaction; `since` below them gets 410 |
| `GRAPH_HUB_MAX_SUBSCRIBERS` | 64 | 429 |
| SSE buffer per subscriber | 1 024 changes | the stream closes; the SDK resumes from `Last-Event-ID` |
| `GRAPH_HUB_DB_POOL` | 8 | requests queue, bounded by `GRAPH_HUB_TIMEOUT_MS` (30 000), then 408 |

- Each container is capped at 1 GiB, the hub and PostgreSQL alike, under `scripts/orch/drun`.
- The hub reaches graph-server at `GRAPH_MOTOR_URL` with its own key (`GRAPH_MOTOR_KEY_FILE`).
- Materializing for `/layout` stops with 413 once the document would exceed
  `GRAPH_MOTOR_MAX_BODY` (default 64 MiB, graph-server's `GRAPH_MAX_BODY` default), so the hub
  never sends a body graph-server would refuse.
- Caveat: `/graph` and `/layout` hold the whole materialized `Ingest` in memory, because
  graph-contract writes canonical JSON from a whole document. Memory is bounded by the 64 MiB cap
  on `/layout` and by `GRAPH_HUB_MAX_RECORDS` on `/graph`. A streaming writer is the upgrade, once
  a workspace needs it.

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
  `Authorization` header.
- On 410, `subscribe` calls `onResync`.
- `sync` turns any adapter's `Ingest` (`rowsToIngest`, `notionToIngest`) into plugin batches.
  Caveat: it reads the whole workspace graph on every call, O(n); a plugin that knows its own
  changes calls `push` instead.
- Browser use keeps `remote.ts`'s `dangerouslyAllowBrowser` rule: a key held in a page is public.
- One example plugin, `examples/plugins/rows-file/`: it reads a JSON rows file, passes it through
  `rowsToIngest`, and calls `sync`. The `hub-sdk` row runs it.

## 8. Gates

Every row has a negative control that must turn it red. The breaks sit behind a `negctl` feature,
off in every shipped build, as graph-server's do.

| Row | Proves | Negative control |
|---|---|---|
| `hub-floor` | fmt, clippy `-D warnings` and tests over the `server/` workspace | an unformatted file in a scratch copy |
| `hub-roundtrip` | hub `/layout` bytes = graph-server `/v1/layout` on `/graph`'s document, for 3 fixtures | `drop-record`: materialization loses one record |
| `hub-durability` | 200 batches, `kill -9` right after an ack, restart: every acked seq present, no gap | `ack-before-commit` |
| `hub-authz` | the key matrix: another plugin 403, read-only writing 403, unknown key 401, no grant 403 | `skip-grant` |
| `hub-events` | seqs 1..N in order; reconnect with `Last-Event-ID` gives no gap and no duplicate; 410 past retention | `skip-event` |
| `hub-idem` | a replayed key gives the same response and the same `head_seq`; another body is 422 | `no-idem` |
| `hub-limits` | each 413 of §6; 409 on removing a linked collection | `no-cap` |
| `hub-sdk` | SDK tests and the example plugin against live hub, PostgreSQL and graph-server containers | `GM_HUB_SDK_BREAK=1`: the example pushes a wrong record |
| `codegen --check` | the hub schema and TypeScript declarations are current | existing |
| `motor-alone`, `svc-*`, `lock-parity` | unchanged | existing |

## 9. Not in v1

Each of these waits for a caller.

- The Redis cache and the snapshot cache (`snapshot-cache.md`, D2, a stop-and-ask item).
- Two or more hub instances, and with them `LISTEN/NOTIFY`.
- Pull or webhook plugins, in-process plugins, plugin distribution.
- Edge properties. Edges come from link fields; a relation with attributes is a record in its
  own collection.
- A query language, traversal or search route, and time travel (`/graph?at=`).
- Deleting a workspace over HTTP (an operator does it in SQL).
- TLS (a reverse proxy does it), per-key quotas, a Python SDK.
- `pgvector`, which the images ADR lists but nothing calls.
- Live studio integration. It needs host-api work in `packages/graph-studio/src/host/*`, which
  graph-render-4f owns, so it is a later slice agreed with them.

## 10. Slices

Each slice has its own branch and worktree and runs as an OpenCode job with an exact brief, in
this order.

1. **hub-contract.** graph-contract's `hub` module: the types, strict readers, canonical
   writers, and codegen of the schema and the TypeScript declarations. Tests. It touches
   `crates/`, so the full gate runs on develop after landing.
2. **hub-store.** `server/graph-store`, `deploy/postgres.Dockerfile`, migrations, the
   transactional batch apply, materialize, changes and retention. Tests run against a PostgreSQL
   container under `drun`. It also adds the `tokio-postgres` pins and a `server-dependencies.md`
   amendment listing everything the driver pulls in.
3. **hub-api.** `server/graph-hub`: routes, keys and grants, limits, SSE and the motor client.
   It also adds `deploy/hub.Dockerfile`, `scripts/orch/rows/hub.rows` with every §8 row,
   `docs/contract/hub-api.md` and `docs/deploy/hub.md`.
4. **hub-sdk.** `hub.ts`, `plugin.ts`, the example plugin, the `./hub` package export, and the
   `hub-sdk` row.
5. **hub-report.** `docs/reports/hub-dod.md`: every row run on the landing commit.

Slice 4 can start once slice 1 lands (generated types). Slices 2 and 3 are sequential.

## 11. Unknowns, to settle in the slice that meets them

- What `tokio-postgres` pulls in, and whether `cargo deny` and `cargo audit` pass on it
  (slice 2).
- Whether graph-server's `keys` and `auth` functions can be called without its `App`
  (slice 3). If a change to graph-server is needed, graph-render-4f makes it.
- The hub's HTTP client to graph-server needs `hyper-util`'s `client-legacy` feature: a feature
  added to a crate already pinned (slice 3).
- Which axum feature SSE needs at the pinned 0.8.9 (slice 3).
- Ownership: the one-line `members` addition to `server/Cargo.toml` and the shared
  `server/Cargo.lock` were agreed with graph-render-4f; any other edit under
  `server/graph-server` goes to them.
