# Service API v1: graph-motor as an HTTP microservice

Status: **draft, 2026-10-03**. It needs a devil verdict, because it is a public surface over untrusted
input. The verdict's conditions are appended under "Verdict" and bind the code.
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

| Status | When |
|---|---|
| 400 | malformed query, unknown `source`, unknown layout or post id |
| 401 | no key, or a key that matches no stored hash. The body never says which |
| 406 | `Accept` names neither face |
| 413 | body over `GRAPH_MAX_BODY` (default 64 MiB), or over the motor's own ingest limits (`docs/decisions/wasm-ingest-limits.md`) |
| 422 | the motor refused the document or the run |
| 429 | every compute slot is busy and the wait queue is full; `Retry-After: 1` |
| 503 | the request ran past `GRAPH_TIMEOUT_MS` (default 30000) |

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

(pending)
