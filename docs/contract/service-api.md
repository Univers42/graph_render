# Service API v1: graph-motor as an HTTP microservice

Status: **blocked, 2026-10-03** (round 1). The 13 conditions under "Verdict" bind the code and override the
draft text above them. Conditions 1–3 must hold before the re-submission.
It implements `docs/decisions/server-and-write-path.md` with that ADR's first version narrowed. There is
no PostgreSQL, no Redis and no write path yet: each waits for a caller.

## What it is

One container image (`deploy/service.Dockerfile`, `FROM debian:trixie-slim`) that runs one binary,
`graph-server`, from the separate Cargo workspace `server/`. It has two faces:

1. **The compute API** under `/v1/`. A backend posts a graph and gets the finished snapshot back. It is
   computed natively by the same `graph-core` the browser runs as wasm, and the bytes are identical
   (row `svc-digest`). Every `/v1/` route needs an API key.
2. **The embed assets** under `/embed/`: the studio bundle, the SDK ESM and both wasm artifacts
   (`graph_wasm.wasm`, and the threads build). A host page loads `<graph-studio>` from here, and the
   motor then runs in the viewer's browser. These are public code: no key.

A browser never holds an API key. The browser path is the embed (wasm, local); the key path is
backend-to-backend.

## Routes

| Method, path | Auth | Request | Response |
|---|---|---|---|
| `GET /healthz` | none | — | `200 ok`, text. Liveness only |
| `GET /v1/meta` | key | — | `200` JSON `{api:1, abi, version, layouts:[id], posts:[id], analyses:[id]}`, ids in registry order |
| `POST /v1/layout` | key | body = ingest document; query `layout=<id>` (required), `post=<id>[,<id>]` (optional, applied in order), `source=studio\|contract` (default `studio`) | `200` snapshot, binary face (`application/vnd.graph-motor.snapshot`) or canonical JSON (`application/json`) chosen by `Accept` |
| `GET /embed/*` | none | — | static files, see "Headers" |

- `source=studio` reads the provisional ingest (`crates/graph-wasm/src/ingest.rs:164` `read_records`,
  then `ingest/ids.rs:43` `index`). `source=contract` reads the phase-10 contract
  (`crates/graph-wasm/src/contract.rs:75` `derive`). The server calls these same functions through the
  `graph-wasm` rlib. A second parser would be a second place to drift.
- The binary face is `Snapshot::to_bytes` (`docs/contract/binary-layout.md`). The JSON face is
  `graph_contract::canonical_json::to_json`. The server does not compute the hash; the bytes are the
  evidence.

## Errors

Every error body is JSON `{"error": "<code>", "message": "<one line>"}`. `code` is the motor's own
`Code` name where one applies (`IngestInvalid`, `ContractInvalid`, `UnknownLayoutId`, `LayoutFailed`,
...), so the browser SDK and the service name a failure the same way.

| Status | `error` | When |
|---|---|---|
| 400 | `BadRequest` | malformed, unknown or repeated query parameter, unknown `source`, more than one `post`, a key in the query, a second `Authorization` header |
| 400 | `UnknownLayoutId` | no layout has the `layout` id |
| 400 | `IndexOutOfRange` | no POST pass has the `post` id (the motor's own name for it) |
| 401 | `Unauthorized` | no key, or a key that matches no stored hash. The body never says which; `WWW-Authenticate: Bearer` |
| 404 | `NotFound` | no such route, a wrong method on a route, any `/embed/` path not in the versioned tree |
| 406 | `NotAcceptable` | `Accept` excludes both faces |
| 408 | `Timeout` | the body did not arrive within `GRAPH_BODY_TIMEOUT_MS` |
| 413 | `IngestTooLarge` | body over `GRAPH_MAX_BODY` (default 64 MiB), over the motor's own ingest limits (`docs/decisions/wasm-ingest-limits.md`), or nodes or edges over the id's work cap |
| 422 | `IngestInvalid`, `ContractInvalid`, `LayoutFailed`, `PostFailed` | the motor refused the document or the run |
| 429 | `Busy` | every compute slot is busy and the wait queue is full; `Retry-After: 1` |
| 500 | `Internal` | a run panicked, or the motor answered a code the service never expects (its name is in the message) |
| 503 | `Timeout` | the request ran past `GRAPH_TIMEOUT_MS` (default 30000) |

Implementation note (graph-server): the `error` column, 404, 408 and 500 were added here because
the server had to name them; a malformed request line or a header block past
`GRAPH_MAX_HEADER_BYTES` is refused by hyper itself (400 or 431) with no JSON body.

## API keys

- A key is `gm_` + 43 base64url characters (32 random bytes from `/dev/urandom`). It is sent as
  `Authorization: Bearer <key>`.
- The server stores **no key**. `GRAPH_API_KEYS_FILE` holds one line per key: `<name> <sha256-hex>`,
  and `#` starts a comment. On every request the server hashes the presented key and compares it to
  every stored hash in constant time; a request is never refused faster for a near miss.
- `graph-server keygen <name>` prints the new key once to stdout, and its file line to stderr. Nothing
  is written to disk; rotation means editing the file and sending `SIGHUP` (re-read) or restarting.
- Every request logs the key's **name**, never the key or its hash.
- An empty or missing key file refuses to start (exit 2), unless `GRAPH_AUTH=off`. That mode exists
  only for local dev and prints a warning on every start.

## Limits and scheduling

- Layout runs are CPU work. They go to `spawn_blocking` behind a semaphore of `GRAPH_WORKERS` slots
  (default: cores). At most `GRAPH_QUEUE` waiters queue (default 2 × slots); past that, 429.
- `GRAPH_TIMEOUT_MS` bounds the wait plus the run. Caveat: a run cannot be preempted. A timed-out run
  goes on to its end on its slot while its response is already a 503, so a flood of over-budget graphs
  holds slots for the length of the slowest run. The ingest limits bound how slow a run can be.
- No state crosses requests: each request builds, runs, encodes and drops. No session and no cache in
  v1.

## Headers

- `/embed/*` sends `Cross-Origin-Resource-Policy: cross-origin`, `Cross-Origin-Embedder-Policy:
  require-corp`, `Cross-Origin-Opener-Policy: same-origin`, the right `Content-Type` (`.wasm` is
  `application/wasm`), and `Cache-Control: public, max-age=31536000, immutable` under the versioned path
  `/embed/<version>/`. The threads build needs the **host page** to be cross-origin isolated as well;
  the server cannot do that for the host. Without isolation the studio falls back to one thread: about
  700 ms/tick at 1M nodes, against 158–170 ms with 8 threads (`docs/measurements/perf-p3-browser.md`).
- CORS: off by default. `GRAPH_CORS_ORIGINS` (a comma list) allows those origins on `/v1/`. It is for a
  host backend behind a browser proxy, and still never for a key in page code.
- No TLS in the server. It sits behind the host's proxy, and remote exposure is stop-and-ask
  (`server-and-write-path.md`).

## Configuration (environment only)

`GRAPH_PORT` (8080), `GRAPH_BIND` (0.0.0.0 in the image, 127.0.0.1 otherwise), `GRAPH_API_KEYS_FILE`,
`GRAPH_AUTH`, `GRAPH_MAX_BODY`, `GRAPH_WORKERS`, `GRAPH_QUEUE`, `GRAPH_TIMEOUT_MS`, `GRAPH_CORS_ORIGINS`,
`GRAPH_EMBED_DIR` (`/srv/embed` in the image). The process prints the variable names and set/unset at
start, never a value of the key file.

## The image

- `scripts/service.sh build` stages the artifacts with the house toolchain images (`ge-rust` through
  `scripts/orch/gr`, the threads build through `scripts/orch/wasm-threads.sh`, and the studio through
  `scripts/studio.sh build`), so no second copy of the rustup or node recipe exists. The image only
  copies them in: `debian:trixie-slim`, a non-root user, the binary and `/srv/embed`, and a
  `HEALTHCHECK` on `/healthz`. The binary is built against trixie's glibc, inside `ge-rust`, which is
  itself trixie.
- `deploy/` and `server/` are not fingerprinted. Building the image voids no motor gate evidence.

## SDK

`crates/graph-sdk-js` gains `./remote`: `createRemote({ baseUrl, apiKey, fetch? })` returns `meta()`
and `layout(doc, { layout, post?, source?, format? })`. The result is the same snapshot type the wasm
path gives, decoded from the binary face by the SDK's existing reader, and it throws the same typed
errors, keyed by `error`. It is for Node and server-side callers. In a browser it refuses to run with
an `apiKey` unless `dangerouslyAllowBrowser: true` is passed.

## Gates (each with its negative control)

| Row | Passes when | Negative control |
|---|---|---|
| `svc-floor` | `server/`: fmt, clippy `-D warnings`, test | — |
| `svc-digest` | `/v1/layout` binary bytes equal the wasm build's `gm_snapshot_bytes` for the same fixture and layout, on every fixture in `fixtures/` | flip one byte of the response: red |
| `svc-auth` | no key, a wrong key and a truncated key are all 401; a good key is 200 | `GRAPH_AUTH=off` with the row expecting 401: red |
| `svc-limits` | a body over `GRAPH_MAX_BODY` is 413; a flood past the queue gets 429 | raise the limit: red |
| `svc-image` | the image builds, runs as non-root, `/healthz` 200, `/embed/<v>/graph_wasm.wasm` carries COEP/CORP and `application/wasm` | serve without the headers: red |
| `svc-sdk` | `remote.layout` decodes to the same columns as the wasm `Motor` for 3 fixtures | wrong key: the typed 401 error, not a decode |
| `lock-parity` | every crate `graph-core` links resolves to one version in both lockfiles | edit one version in a scratch copy: red |

## Verdict

### Round 1: **BLOCK** (independent review, 2026-10-03)

Two load-bearing claims are false in the tree. First, the rlib path to the three readers (Routes, the
`source=` bullet) does not exist. Second, "the ingest limits bound how slow a run can be" (Limits) is
untrue. An unproven safety claim rules BLOCK (`risk.md`). Re-submit when conditions 1–3 hold. Every
condition binds, every row has a negative control, and where a condition and the draft above disagree,
the condition wins.

| Axis (5 = worst) | Score | Why |
|---|---|---|
| Blast radius | 3 | a new workspace, but it needs a motor change in `graph-wasm`, and it adds a public API and an SDK entry |
| Reversibility | 3 | the routes, the key-file format and `./remote` become shipped surface |
| Cost on failure | 4 | one keyed request can OOM-abort the process; bodies buffer before auth; auth off on 0.0.0.0 is open compute |
| Confidence | **5, the worst** | the native ingest path does not compile today, and nothing bounds a run's work |

1. **Native seam, motor first.**
   - `ingest::read_records`, `ingest::index`, `contract::derive` and the run-to-snapshot step become
     `pub` and native. Today they are private and `cfg(any(test, wasm32))`, at
     `crates/graph-wasm/src/lib.rs:149-159`.
   - The change goes on its own branch, through the merge floor and `hashgate --seeds 1000`.
   - `server/` links `graph-wasm` without `probe` or `threads`; `probe` is a data race natively
     (`ingest/phases.rs:66`, `:112`).
   - Row `svc-features`: `cargo tree -e features` in `server/` shows neither feature. Negctl: enable
     `probe`.
2. **Work caps.**
   - After ingest and before the run, an n or m over the layout's or the post's cap is 413
     `IngestTooLarge`.
   - A cap is at most the id's `scale_ceiling`. It is also at most the size measured to finish within
     `GRAPH_TIMEOUT_MS` on the image's reference CPU (`docs/measurements/`). For scale: neato is O(n²)
     and took 251 s at 10k nodes; spectral allocates n×n.
   - Row `svc-caps`: cap + 1 for each registry id is a 413 in under 1 s. Negctl: lift one cap.
3. **Memory budget.**
   - Admission comes before the body read. So `GRAPH_WORKERS × (body + ingest + run peak at cap)` is the
     whole budget, and it fits the container limit. The arithmetic lives in this doc.
   - Row `svc-limits`: every slot busy at cap under `docker --memory` at that limit, with no exit 137.
     Negctl: double one cap.
4. **Request order and connection limits.**
   - The order is auth, then the query check, then admission, then a streamed body read. That read holds
     `GRAPH_MAX_BODY` for chunked bodies too.
   - The header and body read timeouts, the header-size cap and the connection cap are `GRAPH_*`
     variables.
   - Row `svc-preauth`:
     - 50 keyless 64 MiB bodies get 401, with RSS under baseline + 16 MiB.
     - A chunked body over the limit is 413.
     - Headers sent at 1 byte/s are cut at the timeout.
     - Negctl: put the body extractor before auth.
5. **Slots and faults.**
   - The permit moves into the blocking closure, so a run that answered 503 keeps its slot until it ends.
   - A panic or a `JoinError` is 500 `Internal`, and the process lives (`panic = "unwind"`). 500 joins
     the table.
   - Row `svc-limits`:
     - After a timed-out run, workers + queue more requests get 429 until that run ends.
     - A test-only fault hook gives 500, then 200.
     - Negctl: drop the permit at the timeout.
6. **One post, named errors, Accept.**
   - `post=` takes at most one id. A wasm post reads the layout's geometry, never the previous pass's, so
     a chain has no oracle.
   - The contract names every `error` the server emits, server-only ones too. `Code` and the SDK's
     `CODE_NAMES` grow additively.
   - No `Accept`, or `*/*`, means binary. `q=0` excludes a face. Anything else is 406.
   - Row `svc-sdk`: one case per status checks the typed name, and `post=a,b` is 400. Negctl: rename one
     name.
7. **Digest manifest.**
   - `svc-digest` reads a committed `(fixture, source, layout, post)` manifest. It covers both sources,
     every registry id whose cap admits the fixture, and a component of 257–700 nodes for spectral's
     LOBPCG branch.
   - The JSON face round-trips to the same bytes.
   - `server/` sets no `target-cpu` or float flag that the workspace lacks.
   - Negctl: flip a byte; drop a registry id from the manifest.
8. **lock-parity with features.**
   - The row compares `graph-wasm`'s whole normal closure in both workspaces: crate, version and enabled
     features.
   - Negctl: change one version, or add one feature, in a scratch copy.
9. **Auth.**
   - `Authorization` is marked sensitive in the trace layer.
   - A key in the query string, or a second `Authorization` header, is 400.
   - `Bearer` is case-insensitive, and a 401 sends `WWW-Authenticate: Bearer`.
   - At start, each of these is exit 2 naming the line number, never the line: a malformed line, a
     duplicate name, a control character in a name, a non-hex hash, or a group- or world-writable file.
   - SIGHUP parses the whole file, then swaps the key set atomically. A bad or empty file keeps the old
     set.
   - `GRAPH_AUTH=off` refuses a non-loopback bind.
   - Rows `svc-auth` (one case each) and `svc-log` (no key or hash in the logs; negctl: log the header).
10. **Embed.**
    - The version segment is content-derived (the artifacts' hash), not the static `0.1.0`. Unversioned
      paths are 404 or `no-cache`.
    - Responses add `nosniff` and a JavaScript MIME.
    - `..`, encoded traversal, a symlink out of `GRAPH_EMBED_DIR`, dotfiles and directories are 404,
      never a listing.
    - `<graph-studio>` makes a module Worker from `import.meta.url` (`element.ts:88`), which browsers
      refuse cross-origin. So v1 is same-origin only: the host proxies `/embed/`. (This matches
      condition 13 of `host-api.md`.)
    - Row `svc-embed`: one curl case each, plus a proxied browser load that draws a node. Negctl: serve a
      symlink escape.
11. **Image.**
    - The base image is pinned by digest (ge-rust's trixie-slim), and the user is a numeric non-root UID.
    - The `HEALTHCHECK` runs `graph-server healthcheck`, because slim has no curl.
    - The build context is the staging dir, and the docs publish on 127.0.0.1.
    - Row `svc-image`: `id -u` ≠ 0, health `healthy`, and no key file, `.env`, `.git` or scratch path in
      the image.
12. **Operability.**
    - `X-Request-Id` is validated (at most 128 printable characters) or generated, then echoed.
    - Each request writes one JSON log line: id, key name, route, status, duration, layout, post, n and
      m.
    - SIGTERM stops new connections, drains for up to `GRAPH_TIMEOUT_MS`, then exits 0.
    - Preflight needs no key, and is answered for configured origins only.
    - `/v1/meta` drops `analyses`, because no route serves them. `/metrics` is YAGNI.
    - Row `svc-shutdown`: SIGTERM mid-run still finishes that response. Negctl: no drain.
13. **Evidence scope.**
    - "Not fingerprinted" holds for `server/` and `deploy/` only. Condition 1 and `./remote` live under
      the fingerprinted `crates/`, so they owe the full gate on develop.
    - `fetch` stays inside `./remote`, and the caller can inject it.
    - Rows: the develop full gate and `svc-sdk`.

Round 2 (re-submission once 1–3 hold): pending.

### As built (2026-10-04, conditions 3, 4, 9, 11, 12)

Where each condition now holds in `server/`, and the row in `scripts/orch/rows/service.rows` that
proves it. Not-run rows are named as such.

**3. Memory budget.** The worker count is `min(cores, floor(memory.max / PER_SLOT_BYTES))` at
`server/graph-server/src/config/slots.rs:24`, with `PER_SLOT_BYTES` = 4_635_677_069: the
`source=contract` ingest term binds (`docs/measurements/service-caps.md` "Memory per slot"), so a
4 GiB container holds no slot and 8 GiB holds one; an unset `GRAPH_WORKERS` with a `memory.max` holding
no slot is a refusal at `src/config.rs:166-174`, exit 2 via `src/main.rs:69`. The slots are the
gate's semaphore (`src/gate.rs:24`). Rows `svc-memory` / `negctl-memory` cover the container, and
the derivation itself is proved by `src/config/tests.rs:73`.
No log line carries the worker count: `config::start_line` logs `set`/`unset` per variable and no
value, by the rule in condition 9. `svc-memory` and `negctl-memory` are NOT RUN: no `graph-server` image
exists yet (`scripts/orch/drun` has only `ge-rust` and the oracle images), so the rows assert the
start and the refusal rather than a logged figure.

**4. Request order and connection limits.** The order is auth, query, admission, then the streamed
body read: `src/layout.rs:59`, `:60-63`, `:64`, `:65`. The body read is `src/body.rs:12`, bounded by
`GRAPH_MAX_BODY` and `GRAPH_BODY_TIMEOUT_MS` for chunked bodies too. The header timeout and the
header-size cap are per connection at `src/serve.rs:112-113`, and the connection cap is a
pre-accept permit at `src/serve.rs:94`. Rows `svc-preauth`, `negctl-preauth` (break
`body-before-auth`) and `negctl-slow-headers` (break `no-header-timeout`).

**9. Auth.** `src/auth.rs:11` is the only key check; a group- or world-writable key file is refused
at `src/keys.rs:60` and every refusal at start is exit 2 (`src/main.rs:69`). `SIGHUP` re-reads and
swaps the whole set at `src/serve.rs:126-135`. Rows `svc-exit2`, `negctl-exit2` (break
`accept-group-writable`), `svc-sighup`, `negctl-sighup` (break `ignore-sighup`).

**11. Image.** The `HEALTHCHECK` command is `src/main.rs:61` over `src/health.rs:15`, which is true
only on a 200 from `src/lib.rs:45`; the image itself is not built here. Rows `svc-healthcheck`,
`negctl-healthcheck` (break `always-healthy`).

**12. Operability.** `SIGTERM` and `SIGINT` break the accept loop (`src/serve.rs:50-51`), the
listener is dropped at `src/serve.rs:68` so a new connection is refused at once, and
`src/serve.rs:144` drains for up to `GRAPH_TIMEOUT_MS` before the process exits 0. Rows
`svc-shutdown`, `negctl-shutdown` (break `no-drain`). The `X-Request-Id` and JSON-log halves of this
condition are not covered by this job.
