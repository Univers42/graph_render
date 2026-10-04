# Service API v1: graph-motor as an HTTP microservice

Status: **blocked, 2026-10-04 (round 2, docs/reviews/review-svc-r2.md); round 3 pending**. The 13
conditions under "Verdict" bind the code and override the draft text above them. Conditions 1–3 must
hold before the re-submission.
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
| `GET /v1/meta` | key | — | `200` JSON `{api:1, abi, version, layouts:[id], posts:[id]}`, ids in registry order. No `analyses`: no route serves one |
| `POST /v1/layout` | key | body = ingest document; query `layout=<id>` (required), `post=<id>` (optional, at most one id), `source=studio\|contract` (default `studio`) | `200` snapshot, binary face (`application/vnd.graph-motor.snapshot`) or canonical JSON (`application/json`) chosen by `Accept` |
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

Environment only, and every variable the server reads is listed here (`server/graph-server/src/config.rs`
`NAMES`). An empty value counts as unset, a malformed or out-of-range one refuses the start (exit 2)
naming the variable and never its value, and the start line prints each name with `set` or `unset`.

| Variable | Default | Range | What it does |
|---|---|---|---|
| `GRAPH_PORT` | `8080` | 0–65535 | the listen port; `0` asks the kernel for a free one, which `healthcheck` reads |
| `GRAPH_BIND` | `127.0.0.1` (`0.0.0.0` in the image) | a parsed `IpAddr` | the listen address; anything but loopback refuses `GRAPH_AUTH=off` |
| `GRAPH_API_KEYS_FILE` | unset | a path | the `<name> <sha256-hex>` file; required while `GRAPH_AUTH` is on |
| `GRAPH_AUTH` | `on` | `on` or `off` | `off` serves every request without a key, on a loopback bind only |
| `GRAPH_MAX_BODY` | `67108864` (64 MiB) | 1–`1073741824` (1 GiB) | the largest body, chunked included; the ceiling is the motor's own ingest limit |
| `GRAPH_WORKERS` | `min(cores, memory.max / PER_SLOT_BYTES)` | 1–1024 | the compute slots. `0` slots derived from `memory.max` refuses the start |
| `GRAPH_QUEUE` | `2 × GRAPH_WORKERS` | 0–65536 | requests allowed to wait for a slot; past that, 429 |
| `GRAPH_TIMEOUT_MS` | `30000` | 1–600000 | the wait plus the run, and the shutdown drain |
| `GRAPH_BODY_TIMEOUT_MS` | `10000` | 1–600000 | reading the whole body; past it, 408 |
| `GRAPH_HEADER_TIMEOUT_MS` | `5000` | 1–600000 | receiving the request head, per read |
| `GRAPH_MAX_HEADER_BYTES` | `16384` | `8192`–`1048576` | hyper's read buffer, so the largest head; below 8192 hyper refuses it. Past it, hyper answers 431 with no JSON body |
| `GRAPH_MAX_CONNECTIONS` | `256` | 1–65536 | open connections; the next waits in the kernel backlog. No refusal response |
| `GRAPH_CORS_ORIGINS` | unset | a comma list of `http(s)` origins, no path, no wildcard | the origins allowed on `/v1/`, compared byte for byte |
| `GRAPH_EMBED_DIR` | unset (`/srv/embed` in the image) | a directory holding `VERSION` and `<h>/` | the embed tree, read once at start; unset means every `/embed/` path is 404 |

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

## Gates (each with its negative control where one exists)

Every row below exists in `scripts/orch/rows/`. A row with no negative control says so; where one
exists it names the break (`GM_SVC_BREAK`, under the `negctl` feature) or the `SERVICE_IMAGE_BREAK`
it sets, and the test function it has to turn red.

| Row (rows file) | Passes when | Negative control |
|---|---|---|
| `svc-fmt`, `svc-clippy`, `root-fmt` (`svc-floor.rows`) | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` and the root fmt are clean | — |
| `svc-test` (`svc-floor.rows`) | `cargo test --workspace --no-fail-fast` in `server/` | — (the `svc-auth`, `svc-log`, `svc-embed` and `svc-slots` rows below are its armed controls) |
| `svc-digest` (`service-supply.rows`) | `/v1/layout` binary bytes equal the wasm build's `gm_snapshot_bytes` for every row of the committed manifest | `negctl-svc-digest` (`--break`), one response byte flipped |
| `svc-digest-wasm` (`service-supply.rows`) | the same bytes out of the wasm build of the same tree, and the JSON face round-trips | `negctl-svc-digest-wasm` |
| `svc-features` (`service-supply.rows`) | `cargo tree -e features` in `server/` shows neither `probe` nor `threads` | `negctl-svc-features` |
| `lock-parity` (`service-supply.rows`) | `graph-wasm`'s whole normal closure is one version with the same features in both lockfiles | `negctl-lock-parity-version`, `negctl-lock-parity-feature`, in a scratch copy |
| `svc-preauth` (`service.rows`) | a keyless body is refused before a byte of it is buffered; a head sent at 1 byte/s is cut at `GRAPH_HEADER_TIMEOUT_MS` | `negctl-preauth` (`body-before-auth`), `negctl-slow-headers` (`no-header-timeout`) |
| `svc-auth` (`service.rows`) | no key, a wrong key and a truncated key are all 401 with `WWW-Authenticate: Bearer`; a good key is 200; a second `Authorization` header and a key in the query are 400 | `negctl-svc-auth` (`any-key`), red on `a_missing_wrong_or_truncated_key_is_401_with_a_challenge` |
| `svc-log` (`service.rows`) | one JSON line per request carrying id, key name, route, status, ms, layout, post, n and m; no line holds the key, its hash, or the `Authorization` | `negctl-svc-log` (`log-header`), red on `no_line_holds_the_key_or_its_hash` |
| `svc-slots` (`service.rows`) | a run answered 503 at its deadline keeps its slot until it ends (workers + queue are 429 until then); a panic is a 500 and the next run a 200 | `negctl-svc-slots` (`drop-permit`), red on `a_timed_out_run_keeps_its_slot_until_it_ends` |
| `svc-exit2` (`service.rows`) | every refused key file and every refused setting is exit 2, naming the line number or the variable and never the line, the path or the value | `negctl-exit2` (`accept-group-writable`) |
| `svc-sighup` (`service.rows`) | `SIGHUP` parses the whole file, then swaps the key set in one step; a bad or empty file keeps the old set | `negctl-sighup` (`ignore-sighup`) |
| `svc-embed` (`service.rows`) | a file is served public and immutable under its content version, with `nosniff` and the JS MIME; every path outside the table is 404, a symlink escape included | `negctl-svc-embed` (`follow-symlinks`), red on `every_path_outside_the_files_is_404` |
| `svc-caps` (`service.rows`) | cap + 1 for every registry id is a 413 `IngestTooLarge` in under a second | `negctl-caps` (`lift-caps`) |
| `svc-shutdown` (`service.rows`) | SIGTERM stops new connections at once, the request in flight finishes with its normal status, exit 0 within `GRAPH_TIMEOUT_MS` | `negctl-shutdown` (`no-drain`) |
| `svc-healthcheck` (`service.rows`) | `graph-server healthcheck` exits 0 only on a 200 from `/healthz` inside its 2 s budget | `negctl-healthcheck` (`always-healthy`) |
| `svc-memory` (`service.rows`) | 8 GiB holds one slot: the container starts and logs `listening` | `negctl-memory` (1 GiB: exit 2, `GRAPH_WORKERS: unset, and memory.max holds no slot`) |
| `svc-caps-time` (`service-limits.rows`) | every time-bound id at its cap answers 200 within `GRAPH_TIMEOUT_MS` on the image's reference CPU | `negctl-svc-caps-time` (one cap's rung halved) |
| `svc-limits` (`service-limits.rows`) | a body over `GRAPH_MAX_BODY` is 413 and a flood past the queue is 429, and one slot at the largest cap fits `PER_SLOT_BYTES` under `docker --memory` at that limit, with no exit 137 | `negctl-svc-limits` (one cap doubled in a scratch copy of the tsv) |
| `svc-image` (`service-image.rows`) | the image builds, runs as a numeric non-root user, `/healthz` is 200, the wasm carries COEP/CORP and `application/wasm`, and the image report row `svc-no-leak` (in `scripts/service-image.sh`) finds no key file, `.env`, `.git` or scratch path | `negctl-svc-image` (`headers`, stripped), `negctl-svc-image-leak` (`leak`, a planted `.env` and `.git`) |
| `svc-sdk-live` (`service-image.rows`) | `./remote`'s `meta()` and `layout()` against the image, the typed 401 and 400, and parity with the local wasm build | `negctl-svc-sdk-live` (`SERVICE_IMAGE_BREAK=key`, a well-formed key the file does not hold) |

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
   - Row `svc-slots` (named `svc-limits` in round 1; `svc-limits` is now the memory-budget row of
     condition 3):
     - After a timed-out run, workers + queue more requests get 429 until that run ends.
     - A test-only fault hook gives 500, then 200.
     - Negctl: `negctl-svc-slots`, the `drop-permit` break at the timeout.
6. **One post, named errors, Accept.**
   - `post=` takes at most one id. A wasm post reads the layout's geometry, never the previous pass's, so
     a chain has no oracle.
   - The contract names every `error` the server emits, server-only ones too. `Code` and the SDK's
     `CODE_NAMES` grow additively.
   - No `Accept`, or `*/*`, means binary. `q=0` excludes a face. Anything else is 406.
   - Row `svc-sdk-live` (`scripts/orch/rows/service-image.rows`): one case per status checks the typed
     name, and `post=a,b` is 400. Negctl: `negctl-svc-sdk-live`, a well-formed key the key file does not
     hold.
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
    - Rows: the develop full gate and `svc-sdk-live`.

Round 2 (re-submission once 1–3 hold): see "Round 2: BLOCK" below.

### Round 2: **BLOCK, 9 conditions** (independent review, 2026-10-04, `docs/reviews/review-svc-r2.md`)

The verdict is still BLOCK: the memory budget is replaced, not closed, and the published 4 GiB run shape
could not start the service at all. Nine new conditions, each checkable by a row with a negative control.
This job answers six of them; three belong to `scripts/orch/rows/service-limits.rows` and to the develop
gate, and they are listed here so nothing is left unclaimed.

| n | Condition | Answered by |
|---|---|---|
| 1 | `svc-limits` exists and proves the budget fits the container, with no exit 137 | `svc-limits` + `negctl-svc-limits`, `scripts/orch/rows/service-limits.rows` (named here; that row belongs to the sibling job and was not run for this one) |
| 2 | the 4 GiB run shape is resolved in code or in docs, not both ways | `scripts/service.sh run` passes `DRUN_MEM=8g`, and `docs/deploy/service.md` "Run" publishes `--memory 8g` with 8 GiB named the minimum; `docs/decisions/memory-guard.md` decision 1 names the service as the exception to the 4 GiB default |
| 3 | the caps are re-measured on the image's reference CPU, or the time-bound caps are lowered by the one-rung margin the measurement declares | `svc-caps-time` + `negctl-svc-caps-time`, `scripts/orch/rows/service-limits.rows` (the sibling job) |
| 4 | one `negctl-` row per unrowed break | `negctl-svc-auth` (`any-key`, `src/auth.rs` `check`), `negctl-svc-log` (`log-header`, `src/observe.rs` `observe`), `negctl-svc-embed` (`follow-symlinks`, `src/embed.rs` `Tree::visit`), `negctl-svc-slots` (`drop-permit`, `src/layout.rs` `compute`); each names the test function it turns red |
| 5 | condition 1's process clause is run: the develop full gate and `hashgate --seeds 1000` on the commit carrying the `graph-wasm` seam | not run here, and owed on develop: `server/` and `deploy/` are unfingerprinted, so this branch proves nothing about the seam |
| 6 | `service-image.rows` runs, and the build context's exclusion control is named | `service-image.rows` passed 5/5 on image `graph-motor:5f01f980602fc2dd` (2026-10-04); `deploy/service.Dockerfile.dockerignore` deleted, and `docs/deploy/service.md` "Build" names the staged context as the control that `svc-no-leak` checks |
| 7 | the contract's stale rows, routes and variables are reconciled | this document: "Routes" drops `analyses` and says one `post` id, "Configuration" lists all 14 `GRAPH_*` variables with default and range, "Gates" names only rows that exist, and "As built" cites every source reference by symbol |
| 8 | `Caveat:` on the nine constants and the two `config.rs` default blocks | `src/serve.rs` `RUNTIME_GRACE`, `src/health.rs` `BUDGET` and `MAX_STATUS_LINE`, `src/observe.rs` `MAX_REQUEST_ID`, `src/error.rs` `MAX_MESSAGE`, `src/keys.rs` `KEY_BYTES`, `MAX_FILE_BYTES`, `MAX_KEYS`, `MAX_NAME` and the mode mask, and one block above each of `src/config.rs` `read_limits` and `read_connections` naming every default literal in them |
| 9 | `config/tests.rs` asserts the real 4 GiB row directly | with option (b), `default_workers(4 << 30)` is asserted beside `PER_SLOT_BYTES - 1`; `src/config/tests.rs` belongs to the sibling job. The derivation is pinned either way by `default_workers_is_the_smaller_of_cores_and_memory_slots` |

### As built (2026-10-04, conditions 3, 4, 9, 11, 12)

Where each condition now holds in `server/`, and the row in `scripts/orch/rows/service.rows` that proves
it. Every source reference is by **symbol**, not by line: the line numbers drifted twice, and a reference
that can drift is not evidence. Rows that were not run here are named as such.

**3. Memory budget.** The worker count is `min(cores, floor(memory.max / PER_SLOT_BYTES))` at
`server/graph-server/src/config/slots.rs` `default_workers`, with `PER_SLOT_BYTES` = 4_635_677_069: the
`source=contract` ingest term binds (`docs/measurements/service-caps.md` "Memory per slot"), so a 4 GiB
container holds no slot and 8 GiB holds one; an unset `GRAPH_WORKERS` with a `memory.max` holding no slot
is refused by `src/config.rs` `read_workers`, exit 2 through `src/main.rs` `refuse`. The slots are the
gate's semaphore (`src/gate.rs` `Gate::new`). Rows `svc-memory` and `negctl-memory` both PASS: 8 GiB starts
and logs `listening`, 1 GiB exits 2 with `GRAPH_WORKERS: unset, and memory.max holds no slot`. The
derivation itself is proved by `src/config/tests.rs`
`default_workers_is_the_smaller_of_cores_and_memory_slots`. No log line carries the worker count:
`src/config.rs` `start_line` logs `set`/`unset` per variable and no value, by the rule in condition 9.
What these two rows do **not** prove is the fit — that a slot at the largest cap stays inside
`PER_SLOT_BYTES`. That is `svc-limits`, in `scripts/orch/rows/service-limits.rows`.

**4. Request order and connection limits.** In `src/layout.rs` `serve` the order is the pre-auth query-pair
scan, then `auth::check`, then `query::layout` and `query::face`, then `gate::admit`, and only then
`body::read`. The scan (`query::pairs`) is deliberately before auth: it is the only pre-auth work, it is a
`String` bounded by `GRAPH_MAX_HEADER_BYTES`, and it is where a key in the query is refused. The body read
is `src/body.rs` `read`, bounded by `GRAPH_MAX_BODY` and `GRAPH_BODY_TIMEOUT_MS` for chunked bodies too.
The header timeout and the header-size cap are set per connection in `src/serve.rs` `Acceptor::spawn`, and
the connection cap is a pre-accept permit in `src/serve.rs` `Acceptor::next`. Rows `svc-preauth`,
`negctl-preauth` (break `body-before-auth`) and `negctl-slow-headers` (break `no-header-timeout`).

**9. Auth.** `src/auth.rs` `check` is the only key check, and `src/observe.rs` `mark_sensitive` flags every
`Authorization` value in the middleware before any route, handler or log line can read it — asserted by
the unit test `the_authorization_reaching_the_handler_is_marked_sensitive` in `src/observe.rs` `tests`,
which runs a probe layer inside `observe` and reads the flag exactly as a handler would. A key file that is
not 0640 or stricter is refused by `src/keys.rs` `KeySet::load` (no group write or exec, and nothing at all
for others, read included, because the file holds hashes), and every refusal at start is exit 2 through
`src/main.rs` `refuse`. `SIGHUP` re-reads and swaps the whole set: `src/serve.rs` `spawn_reload` installs the
handler even with auth off, and `src/app.rs` `reload_keys` parses the whole file before the swap, so a bad
file keeps the old set. Rows `svc-auth` + `negctl-svc-auth` (break `any-key`), `svc-log` + `negctl-svc-log`
(break `log-header`), `svc-exit2` + `negctl-exit2` (break `accept-group-writable`), `svc-sighup` +
`negctl-sighup` (break `ignore-sighup`).

**11. Image.** The `HEALTHCHECK` command is `src/main.rs` `healthcheck` over `src/health.rs` `healthcheck`,
which is true only on a 200 from `src/lib.rs` `healthz` inside `BUDGET`. `scripts/service.sh` `build` stages
`bin/graph-server`, `embed/<version>/` and `embed/VERSION` into `target/service/stage` and makes that
directory the whole build context, so the staged path list is the exclusion control; row `svc-no-leak` in
`scripts/service-image.sh` checks the image it produces. There is no `.dockerignore` to keep in step with
it: `deploy/service.Dockerfile.dockerignore`, which docker never read, is deleted.
`scripts/orch/rows/service-image.rows` passed 5/5 on image `graph-motor:5f01f980602fc2dd` (2026-10-04):
`svc-image`, `negctl-svc-image`, `negctl-svc-image-leak`, `svc-sdk-live`, `negctl-svc-sdk-live`. Rows
`svc-healthcheck` + `negctl-healthcheck` (break `always-healthy`) cover the command in-process.

**12. Operability.** `SIGTERM` and `SIGINT` break the accept loop in `src/serve.rs` `serve`, the listener is
dropped there so a new connection is refused at once, and `src/serve.rs` `drain` waits up to
`GRAPH_TIMEOUT_MS` before the process exits 0. Rows `svc-shutdown`, `negctl-shutdown` (break `no-drain`). The
`X-Request-Id` and JSON-log halves of this condition have rows of their own: `svc-log` + `negctl-svc-log`
(break `log-header`), with the id validation and the sensitive marking asserted in `src/observe.rs` `tests`
and `src/observe.rs` `Line::render`.
