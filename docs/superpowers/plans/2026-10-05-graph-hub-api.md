# graph-hub slice 3 (hub-api) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `server/` gains `graph-hub`, a binary plus a lib that owns the hub's HTTP surface and nothing else: the twelve routes of §5.2, the keys file and the grants file, the concurrency gates and every limit of §6 with the start checks, the SSE notice stream, the streaming relay of the materialized document to graph-server, `deploy/hub.Dockerfile`, `scripts/orch/rows/hub.rows`, `docs/contract/hub-api.md`, `docs/deploy/hub.md` and `docs/measurements/hub-memory.md`. The hub never runs motor math, never writes SQL, and never computes an epoch.

**Architecture:** One lib crate plus a thin `main`, both members of the `server/` workspace behind `default-members = ["graph-server"]` (slice 2). Every byte of state lives in `graph-store` and every byte of canonical text in `graph-contract`; the hub owns the request, the credential, the permit, the byte cap and the status. One `RwLock<Arc<(KeySet, Grants)>>` holds the credential pair so `SIGHUP` swaps both or neither. Four semaphores (`WRITERS`, `READS`, `LAYOUTS`, and a per-key writer permit) plus two subscriber counters are the whole concurrency story; each route takes exactly one permit, and `GRAPH_HUB_TIMEOUT_MS` bounds the wait, never the work. `/graph` and `/layout` stream the store's `Document` chunk by chunk; `/layout` feeds those chunks straight into the relay's request body, so no document is ever whole in memory.

**Tech Stack:** Rust 2024 (`rust-version = "1.98"`, `server/graph-server/Cargo.toml:8`), axum 0.8.9 at `default-features = false, features = ["http1", "tokio"]` (the same set graph-server pins, `server/graph-server/Cargo.toml:20`), `hyper-util` 0.1.21 with `client-legacy`, `hyper` 1.11.1 with `client`, `tokio` 1.53.2, `graph-store` and `graph-contract` by path, `graph-server` by path with `default-features = false` and **no** feature forwarded. Tests reach PostgreSQL through `scripts/orch/hub-pg.sh` and the motor through an in-process router on an ephemeral port; every command through `scripts/orch/gr`, every container through `scripts/orch/drun`.

**Spec:** `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1; §2 H1/H2/H7/H8/H9/H14, §3, §4, §5.1, §5.2 every route, §5.3, §6, §7 what the SDK expects of the wire, §8 every row whose slice column names 3, §10 slice 3, §11, and the conditions of §12–§16 that name the API, keys, grants, semaphores, limits, start checks, SSE, the relay, `bearer`, `client-legacy`, `hub.Dockerfile`, idempotency or memory). Verdict and the conditions of `docs/decisions/graph-hub.md:101-155`.

## Global Constraints

- **Owner of `server/` is graph-render-4f, and these are binding** (`docs/decisions/graph-hub.md:105-116`, their second reply `:131-152`):
  - (a) Edits to `server/Cargo.toml` and to graph-server are **additive**; `panic = "unwind"` stays in `[profile.dev]` and `[profile.release]` (`server/Cargo.toml:11-12,14-15`) untouched.
  - (b) `scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -p graph-server -e normal,build --locked` is **byte-identical** before and after (paths stripped, sorted), and no graph-server entry in `server/Cargo.lock` changes version. Measured today with the full hub edge in place: identical (Measured facts, fact 5).
  - (c) `svc-clippy` and `svc-test` (`scripts/orch/rows/svc-floor.rows:3-4`) run `--workspace` and must pass **with no PostgreSQL running**. Every database test of this crate therefore sits behind `db-tests`, its own row and its own negative control; nothing under `tests/` may run under a plain `cargo test`.
  - (d) `scripts/service.sh:73` still builds `--bin graph-server` only; `svc-image` (`scripts/orch/rows/service-image.rows`) and its two negative controls stay green. This slice adds `deploy/hub.Dockerfile` for `--bin graph-hub` and never touches `scripts/service.sh`.
  - (e) `cargo-deny-server`, `lock-parity` and `svc-features` stay green, and their three negative controls still exit 1 **for their original reason** (grepped, not trusted by exit code alone).
- **The only edit to `server/graph-server/**` is `server/graph-server/src/auth.rs:34`**: `fn bearer` becomes `pub fn bearer`, doc comment (`/// The token of a `Bearer` credential (RFC 6750 §2.1; the scheme is case-insensitive).`, `auth.rs:33`) kept byte for byte, in the **same commit as its first caller** (`docs/decisions/graph-hub.md:117-119,151-152`). Nothing else under `server/graph-server/**` is edited, not even a comment.
- The hub reuses `graph_server::auth::bearer` and `graph_server::keys::KeySet::{load, parse, name_of}` and **nothing else** from the compute crate (`docs/decisions/graph-hub.md:120-121`): never `auth::check` (it carries the `any-key` break, `auth.rs:24`, and the compute `App`), never `app::App`, never `serve`. The path dependency is `default-features = false` and forwards neither `negctl` nor `test-hooks`; row `hub-breaks-off` reads the edge with `cargo tree -e normal,build,features -i graph-server -p graph-hub`. The one exception is **test-only**: `tests/support/motor.rs` drives `graph_server::config::Settings` + `graph_server::app::App` + `graph_server::router` in process, through a `[dev-dependencies]` edge, so `hub-roundtrip` compares the real motor's bytes. No dev-dependency code reaches the `graph-hub` lib or binary (resolver 2, `server/Cargo.toml:6`).
- graph-core and graph-wasm are linked into the hub binary through that dependency and no hub code path calls them (H1, H8). Caveat: size and attack surface grow with no code path; the reopen trigger is a third credential consumer or a hub size/surface budget refusing graph-core (`docs/decisions/graph-hub.md:149-150`).
- `hub.Dockerfile` follows `deploy/service.Dockerfile` exactly: pinned debian digest, `COPY` only, no build stage, `USER 10001:10001`, `EXPOSE`, a `HEALTHCHECK` that execs the binary's own `healthcheck` subcommand, exec-form `ENTRYPOINT`. The cargo build lives in `scripts/hub.sh` through `scripts/orch/gr`, because `deploy/service.Dockerfile:2-3` keeps every build out of the image on purpose.
- `hyper-util`'s `client-legacy` and `hyper`'s `client` are features on crates `server/graph-server/Cargo.toml:24-25` already pins; they add eleven crates to **graph-hub's** closure and **zero** to the virtual root's (`default-members`), which is what row `hub-virtual-root` greps (Measured facts, facts 3 and 4). Any change to that feature set re-runs facts 2–5 and `cargo deny` before it lands.
- `scripts/orch/gr` has no network option and its containers sit on Docker's default bridge, so a test container reaches PostgreSQL by the container's **bridge IP**, never by `127.0.0.1` and never by a host port. Every container is created through `scripts/orch/drun`; `scripts/orch/drun-check.sh` is a row.
- Workspace and plugin ids `[a-z0-9][a-z0-9-]{0,62}`; collection ids `[A-Za-z0-9_-]{1,64}`, qualified `<plugin>.<collection>`; record ids non-empty and without `:`. `seq`, `rev` and `epoch` stop at `MAX_SEQ` = 2^53 − 1. A cursor is `<epoch>.<seq>`; a bare seq is 400 (`HubError::Cursor` → 400, `crates/graph-contract/src/hub/error.rs:68-71` on develop).
- The hub is the only place a `HubError` becomes a status: it calls `HubError::status()` and never re-derives the class, and it never writes canonical text — `manifest_json`, `answer_json`, `notice_json`, `change_json` and `manifest_change_json` are graph-contract's, and the store calls the last two. The hub writes exactly one body of its own: the batch answer's `Retry-After`-free `{seq, applied}` is `answer_json(seq, applied)`, and the SSE notice is `notice_json(&ChangeHead { seq, plugin, at })`.
- Order of refusals is fixed and tested: **authorization, then existence** (§5.2). No key or an unknown key is 401; a key with no grant covering the path's workspace and plugin is 403; only then may a missing workspace, plugin or record be 404. A key learns that a workspace exists only if it may read or write it.
- House limits: ≤ 40 lines per function, ≤ 4 parameters (a request struct when more would be needed), ≤ 300 lines per file; `BTreeMap` never `HashMap` in anything hashed, ordered or granted; a doc comment on every `pub` item; clippy `-D warnings`; a `Caveat:` line on every heuristic, timeout, retry, semaphore size, cap and backoff.
- Never touch `server/graph-server/**` beyond `auth.rs:34`, `server/graph-store/**` (slice 2's: if the API needs a store change it is a "decision needed", not an edit), `crates/**`, `docs/contract/delta.md`, `docs/contract/service-api.md`, `push-once.sh`, `push-loop.sh`, `.claude/rules/devil/`. Never open `/goinfre/dlesieur/refs/igraph-*`.

## Measured facts

Every command below was run on 2026-10-05 through `scripts/orch/gr` or `scripts/orch/drun` against a scratch copy of `server/`. `$GM_SCRATCH` is outside the git top-level, so the copy is mounted at `/s` and the worktree at `/w`:

```
S=$GM_SCRATCH/tmp/plan-hub-api      # /home/dlesieur/goinfre/tmp/plan-hub-api
# server/ manifests copied, each member's src a symlink to /w, the root workspace manifest copied
# (crates/graph-contract inherits `edition` from it), plus this runner:
scripts/orch/drun --rm --pull never --memory 8g --memory-swap 8g \
  -v "$PWD:/w" -v "$S:/s" -v "$PWD/crates:/s/crates" -v "$PWD/fixtures:/s/fixtures" \
  -v "$PWD/docs:/s/docs" -w /s -v ge-cargo-registry:/opt/cargo/registry ge-rust "$@"
```

1. **axum 0.8.9's SSE needs no feature of its own; the heartbeat needs `tokio`.** `axum::response::sse::{Sse, Event, Sse::new}` compiles with the hub's feature set `default-features = false, features = ["http1", "tokio"]`. In axum 0.8.9 there is no `sse` feature: `pub mod sse;` at `axum-0.8.9/src/response/mod.rs:7` carries no `#[cfg]`, and the crate's `[features]` table has no `sse` key. What is gated is the keep-alive: `Sse::keep_alive` and `KeepAliveStream` are `#[cfg(feature = "tokio")]` (`axum-0.8.9/src/response/sse.rs:73,574,587,604`), and `sse.rs:580` holds a `tokio::time::Sleep` unconditionally. Isolated probe (its own workspace, so graph-server's axum edge cannot unify `tokio` into it), with `axum = { default-features = false, features = ["http1"] }`:
   ```
   error[E0599]: no method named `keep_alive` found for struct `Sse<S>` in the current scope
     --> src/main.rs:32:19
   error: could not compile `sse-probe` (bin "sse-probe") due to 1 previous error
   ```
   With `features = ["http1", "tokio"]` the same file compiles (`Finished dev profile`). The hub's edge therefore reads `features = ["http1", "tokio"]` — the set graph-server already pins at `server/graph-server/Cargo.toml:20` — and the 15 s comment heartbeat §5.3 requires is inside it.
2. **A streamed request body and a streamed response body need no feature beyond `http1`.** A handler taking `axum::body::Body` (the request side of `/layout`'s peer routes and of every write route) and a response built with `Body::from_stream(stream::iter(vec![Ok(Bytes), …]))` both compile with `default-features = false, features = ["http1", "tokio"]` and no other feature. `cargo check -p graph-hub` on the copy: `Finished dev profile`. So the relay's chunked upload and the streamed answer need nothing new on the axum side.
3. **What `client-legacy` adds to graph-hub's closure.** `cargo tree --manifest-path server/Cargo.toml -p graph-hub -e normal,build --prefix none`, with and without `hyper-util`'s `client-legacy` and `hyper`'s `client` (identical in every other respect), differ by exactly eleven crates:
   ```
   want v0.3.1            try-lock v0.2.5       futures-channel v0.3.34   futures-util v0.3.34
   httparse v1.10.1       libc v0.2.190         socket2 v0.6.5           tracing v0.1.44
   pin-project-lite v0.2.17  tracing-core v0.1.36   once_cell v1.21.4
   ```
   Six of the eleven (`futures-channel`, `futures-util`, `httparse`, `libc`, `socket2`, `pin-project-lite`) are already in graph-server's own closure, so five are new to the workspace: `once_cell 1.21.4`, `tracing 0.1.44`, `tracing-core 0.1.36`, `try-lock 0.2.5`, `want 0.3.1`. `server/Cargo.lock` gains those five entries plus the two path members and their edges; **no graph-server entry changes version.**
   Caveat: a new `tracing` in the lock is a dependency-graph fact, not a plan to emit spans; the hub logs JSON lines through its own `log` sink like graph-server does (`server/graph-server/src/app.rs:15`).
4. **The `tokio` feature on the hub's axum edge adds no crate, because graph-server's edge already unifies it.** Same tree, hub edge `features = ["http1"]` versus `["http1", "tokio"]`: byte-identical after paths are stripped. And `client-legacy` appears **once** in `cargo tree -e features --locked -p graph-hub` and **zero** times in `cargo tree --manifest-path server/Cargo.toml -e features --locked` with no `-p` (the virtual root, i.e. `default-members = ["graph-server"]`). That grep pair is the slice-3 form of row `hub-virtual-root`.
5. **Condition (b) holds with the hub edge present.** `cargo tree -e normal,build --locked -p graph-server --prefix none` in the repository against the same command on the copy, both piped through `sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort`: **no difference** (130 lines each). Caveat: the scratch copy lives at `/s` and the repository at `/w`, so the strip **must** remove the parenthesised path, not only a trailing ` (/…)`; with the store plan's narrower `sed -e 's| (/[^)]*)$||'` the single line `graph-core v0.1.0 (/w/crates/graph-core) (*)` differs and the row would fail for the wrong reason. The slice-3 row uses the wider strip.
6. **`cargo deny` passes on the copy with the hub edge.** `GR_IMAGE=ge-audit scripts/orch/drun … ge-audit cargo deny --manifest-path server/Cargo.toml --config /w/deny.toml check advisories bans licenses sources` → exit **0**, last line `advisories ok, bans ok, licenses ok, sources ok`. It re-reports the pre-existing `hashbrown` duplicate (`0.15.5` through `petgraph`, `0.17.1` through `indexmap`), which predates this slice; `deny.toml:42` keeps `multiple-versions = "warn"`, and no new duplicate is introduced. **A `cargo deny` failure on any added crate is a stop**, not a warning to waive.
7. **`hub-breaks-off`'s command works and its control bites.** `cargo tree -e normal,build,features -i graph-server -p graph-hub --prefix none` on the copy: one line, `graph-server v0.1.0`, and **zero** lines matching `negctl|test-hooks`. The same command on a scratch copy whose hub edge reads `features = ["negctl"]` prints `graph-server feature "negctl"`.

## Decisions recorded here (spec silent, illustrative, or impossible as written)

1. **How the hub's tests reach graph-server for the `/layout` relay: an in-process router on an ephemeral port, never the built binary.** `server/graph-server/tests/common/child.rs:72` spawns `env!("CARGO_BIN_EXE_graph-server")`, and cargo defines that variable only for test targets of the crate that owns the binary — a different crate cannot resolve it (measured by reading `child.rs:72` and `server/graph-server/Cargo.toml`, which declares no `[[bin]]` and no `[lib]`, so the implicit `src/lib.rs` is the library). `scripts/service.sh:73` builds the binary in a separate image step, so the artifact is not there when `cargo test -p graph-hub` runs either. So `tests/support/motor.rs` provides two fixtures and reuses `child.rs`'s conventions verbatim rather than its code: **loopback only** (`127.0.0.1`, `child.rs:47` sets `GRAPH_PORT=0`), **the port learned from the listener** (`TcpListener::bind(("127.0.0.1", 0))` then `local_addr()`, which is what `child.rs:99-101` parses out of the readiness line), **a bounded wait** (`PATIENCE = 10 s`, `child.rs:18`) and **a `Drop` that stops the server** (`child.rs:200-205`). `support::motor::real()` returns a `graph_server::router(Arc::new(App::from_settings(&settings, log)?))` served by `hyper_util::server::conn::auto` or `axum::serve`; `support::motor::stub(script)` is a hand-written axum router that answers one scripted `(status, error, message, body)` per request, which is what `hub-motor-map` needs. The hub's own tests speak HTTP with `hyper_util::client::legacy::Client`, already a normal dependency (fact 3), so no HTTP client crate is added. Caveat: the in-process motor shares the test binary's event loop and its 8 GiB container, so a `/layout` timing measured here is a floor, not the production figure; `docs/measurements/hub-memory.md` measures the upload across containers, not in process.
2. **Where the key and grant model lives.** `server/graph-hub/src/auth.rs` (the request's credential and the authorization decision, nothing else), `src/keys.rs` (load both files, hold the pair, `SIGHUP`), `src/grants.rs` (the grants file's grammar and the `BTreeMap` lookup), `src/gate.rs` (the four semaphores and the two subscriber counters), `src/app.rs` (the state every handler takes: store, keyring, gates, config, watch registry, log sink). Every file stays under 300 lines: `auth.rs` holds `credential()` and `authorize()` only, `keys.rs` holds `Keyring::{load, current, reload}` and nothing about HTTP, `grants.rs` holds `Grants::parse` and `Grants::allows`. The full file list is in each task's **Files**.
3. **Which rows file holds what, and which need PostgreSQL.** `scripts/orch/rows/hub.rows` holds every §8 row whose slice column names 3 — `hub-durability`, `hub-roundtrip`, `hub-authz`, `hub-reload`, `hub-events`, `hub-motor-map`, `hub-idem`, `hub-limits`, `hub-memory`, `hub-breaks-off` — plus the rows the shared columns name (`hub-floor`, `hub-virtual-root`, `svc-supply`, `cargo-deny-server`, `lock-parity`, `svc-features`, and `svc-image` under condition (d)), plus `hub-start-check`, `hub-routes`, `hub-upload-timeout` and `hub-image`, which exist only to make §5.2 and §5.3 checkable. Every §8 row of slices 1 and 2 (`hub-wire`, `hub-materialize`, `hub-seq`, `hub-epoch`, `hub-changes-snapshot`, `hub-pg-durability`, the store's retention row) stays in the slice that created it and is re-run by the report slice; `hub-idem`'s sweeper half is the one case §8 splits across slices, and it lives here because the sweeper is spawned by this crate. **Every row that boots the hub needs PostgreSQL** and starts its own container with `scripts/orch/hub-pg.sh reset && … start`, because the first database connection runs the restore detector and the start check reads `server_encoding` (§5.3, §6). The rows that need no database are the static ones: `drun-check`, `hub-floor`, `clippy-hub`, `hooks-gated-hub`, `hub-breaks-off`, `hub-virtual-root`, `svc-supply`, `cargo-deny-server`, `lock-parity`, `svc-features`, `svc-image`, `hub-image`, and `negctl-hub-breaks-off`. Every database row has its own negative control, each of which turns **its own subject** red and is checked by grep, never by exit code alone.
4. **How `docs/measurements/hub-memory.md` measures the `/layout` upload against graph-server's body timeout** (§5.3, `N4`, §13 condition 12). Row `hub-upload-timeout`, whose whole command is `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh upload`, and the file records: **command** the expansion `scripts/orch/hub-mem.sh upload` runs, which is `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" GRAPH_HUB_MAX_DOC_BYTES=67108864 GRAPH_HUB_MAX_RECORD_BYTES=1048576 scripts/orch/drun --rm --memory 1g --memory-swap 1g -v "$PWD:/w" -e GRAPH_HUB_DB_URL -e GRAPH_HUB_MOTOR_URL ge-rust bash -c 'scripts/orch/gr cargo run --release --manifest-path server/Cargo.toml -p graph-hub -- upload-measurement'`, which fills one workspace to exactly `GRAPH_HUB_MAX_DOC_BYTES` with the **smallest record the contract admits** (one collection, one scalar field, one-character id, so the record count is the highest the cap allows and the per-record overhead is the lowest), then times the relay's upload alone — the instant the last chunk is written to the socket, not the motor's answer — over 5 runs after one warm-up. **Input size**: one `GRAPH_HUB_MAX_DOC_BYTES` = 64 MiB canonical document, the size that equals graph-server's own `GRAPH_MAX_BODY` default (§6). **Pass condition**: the slowest of the 5 uploads finishes in **under 8 000 ms**, two seconds under graph-server's `GRAPH_BODY_TIMEOUT_MS` default of 10 000 (`server/graph-server/src/config.rs:196`, enforced at `server/graph-server/src/body.rs:20`), with no 408 in the motor's log and `Graph-Seq` equal to the `/graph` ETag at the same cursor. A miss is a **stop** (§5.3), not a retune: the knob is the relay's chunk size, which the file names next to the number. Caveat: this is loopback between two containers on one host, so it measures the hub's own streaming cost and the socket, not a network; the record count and the bytes per chunk go in the file beside the time.
5. **`GRAPH_HUB_DB_URL` is this slice's name for the database URL.** §6's table never names it, and `StoreConfig { url, … }` (`server/graph-store/src/config.rs`, slice 2 Task 3) has an empty `url` in `defaults()`. The hub reads `GRAPH_HUB_DB_URL`, passes it to `Store::connect`, and **refuses to start** when it is empty: a hub with no database is a misconfiguration, and §5.3's detector-on-every-connection means a silent fallback would be a hub that serves nothing. The URL carries the role and its password, so it is a secret: the deploy doc says mode 0600, and `docs/deploy/hub.md` shows it as a file-sourced env var, never a command-line argument.
6. **`GRAPH_HUB_MOTOR_KEY_FILE` holds the plaintext key, one per line.** A graph-server keys file stores `name <sha256-hex>` and nothing else (`server/graph-server/src/keys.rs:89-116`, `:194-232`), so the secret cannot be recovered from one; `KeySet::name_of` goes the other way. The relay therefore reads the first line of `GRAPH_HUB_MOTOR_KEY_FILE`, trims it, refuses a line over 256 bytes or one holding a NUL, and sends it as `Authorization: Bearer`. Caveat: that file is a live credential; mode 0600, and `docs/deploy/hub.md` says so where it names the file. `keys::keygen` prints the pair (`keys.rs:174`), so the file is created from that output and never from the keys file.
7. **Both credential files are required at start.** `GRAPH_HUB_KEYS_FILE` and `GRAPH_HUB_GRANTS_FILE` are named by H8 and §5.2; a missing keys file is refused (auth cannot be turned off), and so is a missing grants file, because a hub that starts with keys and no grants answers 403 to everything and looks like an authorization bug rather than a deployment mistake. `KeySet::load` already applies the 0640-or-stricter check (`keys.rs:74`), and `Grants::load` applies the same one, which §5.2 requires.
8. **The hub owns the NUL check on path segments.** §4 says a percent-decoded NUL in a path id is 400, and the body NUL rule is graph-contract's (`HubError::Nul`, `crates/graph-contract/src/hub/strict.rs:31`). Measured on develop: `check_record_id` (`crates/graph-contract/src/hub/ids.rs:52-63`) only tests for empty and for `:`, so it **accepts** `\0`, and `check_workspace_id`/`check_plugin_id`/`check_collection_id` reject it only incidentally, byte by byte. `auth.rs`'s path extractor therefore refuses `\0` in every segment with 400 before calling any `check_*_id`. This is the hub's own line, not a graph-contract change, because `crates/**` is out of this slice's paths.
9. **The hub's own `error` strings are graph-contract's wire class names; graph-server's are relayed verbatim.** `crates/graph-contract/src/hub/schema/wire.rs:161-166` fixes `error` to one of `invalid`, `conflict`, `too_large`, `cursor`, `internal` for the hub's own faults, and §5.2 names the relay's five: `MotorAuth`, `MotorBodyTimeout`, `GraphTooLarge`, `MaterializeInvalid`, `MotorError`, `MotorUnavailable`. So `HubApiError::code()` returns a class name for a hub fault and the §5.2 name for a motor fault, and a relayed body keeps graph-server's `error` string unchanged (400, 406, the 422 split's relayed arm, and the 503 all relay). The body is `{"error", "message"}`, graph-server's shape (`server/graph-server/src/error.rs:107-112`, `docs/contract/service-api.md:43`).
10. **The sweeper is a task in the hub's runtime.** `graph_store::sweeper::{run, interval_ms}` are the store's; the hub owns the loop that calls them every `interval_ms(&config)` (default 600 000) and cancels it on shutdown, because the store exports no runtime of its own and must not grow one. One `tokio::time::interval`, one `Store` clone per batch, `MissedTickBehavior::Skip` with its Caveat (a slow batch delays the next sweep; it never runs two at once).
11. **`hyper` gains `client` and `hyper-util` gains `client-legacy`, and no other feature changes.** Measured (facts 2–4): the SSE and streamed-body halves need nothing on axum, and the relay needs exactly these two. The hub's edge states them explicitly rather than relying on `default-features = true`, so `hub-breaks-off` and `hub-virtual-root` can grep the edge.
12. **`event: busy`, not `event: resync`, on a mid-stream pool timeout.** This is revision 5's accepted deviation from the verdict (§15(b), condition 11 of §16(b)): the cursor is still valid, a resync would make every subscriber read a whole `/graph` from a pool that is already short, so the hub frees the subscriber slot, sends `event: busy` with **no `id:` line**, and closes; the SDK reconnects from its cursor (§7's jittered backoff) and only a cursor that went invalid in the meantime gets `resync`.

## Review Focus

1. **Authorization is decided before existence, and the two 403s are byte-identical.** A key with no grant covering the path's workspace and plugin is 403 whether or not the workspace, the plugin or the record is there; the body and the `error` string are the same bytes in all three cases, so a probe cannot enumerate workspaces. Pinned by `no_grant_is_403_before_any_404`, `refusal_bytes_are_identical_with_and_without_the_workspace`, and `a_key_learns_nothing_from_another_plugins_records` (Task 3).
2. **`/layout` is byte-identical to the motor on `/graph`'s document at the same cursor, and the snapshot is closed before the answer is awaited.** The relay streams the store's `Document` into the request body, so no whole document exists, and the store's transaction ends when the last chunk is sent. Pinned by `layout_bytes_equal_motor_bytes_at_the_same_cursor` over the three fixtures, `the_snapshot_closes_before_the_motor_answer_is_awaited`, and `layout_never_holds_a_whole_document` (Task 8).
3. **`event: busy` carries no `id:` line, its subscriber slot is free before the close, and the reconnect from the cursor reads no `/graph`.** The revision-5 deviation is easy to get half right: an `id:` on `busy` would make the SDK's `Last-Event-ID` skip a change, and a slot freed after the close would let a reconnect find the cap full. Pinned by `busy_carries_no_id_line`, `the_busy_slot_is_free_before_the_close`, `the_busy_reconnect_reads_no_graph`, and `a_cursor_pruned_during_the_backoff_gets_resync` (Task 7).
4. **Every cap is a fact, not a hope: the start check refuses a pool at or below the permits and a changes page below `max_change`, and the peak at every cap at once fits 1 GiB.** `GRAPH_HUB_DB_POOL <= WRITERS + READS + LAYOUTS` and `CHANGES_BYTES < max_change` are refusals, not warnings, because a page that cannot hold one change is an unbounded read. Pinned by `start_check_refuses_a_pool_at_or_below_the_permits`, `start_check_refuses_changes_bytes_below_max_change`, `start_check_refuses_a_motor_timeout_at_or_below_forty_thousand`, and `peak_rss_at_every_cap_fits_one_gib` (Tasks 2 and 10).
5. **The hub's reuse of the compute crate is `bearer` plus `KeySet`, and no feature crosses the edge.** The shipped `graph-server` must show neither `negctl` nor `test-hooks` on its edge, `auth::check` must appear nowhere in the hub, and the `pub` on `bearer` must have its first caller in the same commit. Pinned by `the_hub_source_never_names_auth_check`, `the_release_tree_has_no_negctl_and_no_test_hooks`, and `bearer_has_a_hub_caller_in_this_commit` (Task 1).

---

### Task 1: the member, the crate skeleton, `pub fn bearer`, the image and the static rows

**Files:**
- Modify: `server/Cargo.toml` (`members`, line 7: `"graph-hub"` after `"graph-store"`), `server/Cargo.lock` (regenerated)
- Modify: `server/graph-server/src/auth.rs` (line 34 only: `fn bearer` → `pub fn bearer`)
- Create: `server/graph-hub/Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/app.rs`, `src/error.rs`, `src/breaks.rs`, `src/hooks.rs`, `tests/health.rs`, `tests/support/mod.rs`, `tests/support/motor.rs`
- Create: `deploy/hub.Dockerfile`, `scripts/hub.sh` (`chmod 0755`), `scripts/orch/rows/hub.rows`

**Interfaces:**
- Produces: `graph_hub::{App, HubApiError, MotorFault, router, breaks::on, Hooks}`; `App::from_settings(&Settings, LogSink) -> Result<Arc<App>, String>`; `router(app: Arc<App>) -> axum::Router` with `/healthz` and the JSON 404 fallback; `HubApiError::{code, status, retry_after}` and `impl IntoResponse for HubApiError`; `MotorFault` (§5.2's relay column, Task 8 fills its mapping); `breaks::on(name)` behind `negctl` reading `GM_HUB_BREAK`; `Hooks` behind `test-hooks`.
- Consumes from graph-contract (develop, exact names): `hub::{VERSION, Limits, MAX_PLUGINS, MAX_MANIFEST_BYTES, MAX_SEQ, MAX_COLLECTIONS, MAX_FIELDS, Cursor, HubError, Manifest, Growth, Model, Stored, Applied, ChangeHead, answer_json, change_json, manifest_change_json, notice_json, max_change, check_change, check_collection_id, check_plugin_id, check_record_id, check_workspace_id, qualify, growth, manifest_json, read_manifest, read_batch, breaks::on}` and `hub::batch::{Batch, Delete, Upsert}`, `hub::prune::{kept_collection, prune_record}`. `HubError::status()` is the only status the hub derives from a wire fault (`crates/graph-contract/src/hub/error.rs:66-73`).
- Consumes from graph-store (slice 2's plan, verbatim — every name this slice uses): `graph_store::{Store, StoreConfig, StoreError, Hooks as StoreHooks}`; `Store::connect(&StoreConfig) -> Result<Store, StoreError>`; `Store::ping(&self) -> Result<(), StoreError>`; `StoreConfig { url, pool, retain, retain_bytes, changes_bytes, fetch_rows, max_batch, max_body, max_record_bytes, max_plugin_bytes, max_doc_bytes, last_seen, timeout_ms, stream_deadline_ms, sweeper_interval_ms, idem_ttl_ms }` with `defaults()` and `check()`; `StoreError::{Hub(HubError), Db(DbError), NoDatabase, Serialization { retried }, Busy { retry_after }, Gone, Eof}` with `retry_after() -> Option<u64>` and `code() -> &'static str`; `Store::{create_workspace, put_manifest, apply_batch}`; `ManifestWrite { ws, plugin, manifest, limits }`; `ManifestWritten { status, seq, growth }`; `BatchWrite { ws, plugin, manifest, batch, idem, if_match, limits }`; `BatchOutcome { seq, applied, response }`; `changes::{page, cursor_state}`; `ChangesReq { ws, since, limit, max_bytes }`; `ChangePage { epoch, head_seq, next, changes, bytes }`; `Change { seq, plugin, at, kind, upserts, deletes }`; `ChangeOp`; `CursorState::{Valid, Gone}`; `records::page`; `RecordsReq { ws, plugin, after, limit }`; `RecordsPage { plugin_seq, rows, next }`; `materialize::open(&Store, ws) -> Result<Document<'_>, StoreError>`; `Document::{head, next, tail, cursor}`; `sweeper::{run, interval_ms}`; `graph_store::breaks::on` with `drop-record` **reserved for this slice's relay** (slice 2 declares it and never reads it).
- Consumes from graph-server: `graph_server::auth::bearer` (the only item the lib uses) and, **in `tests/support/motor.rs` only**, `config::Settings`, `app::App::from_settings`, `router`.

- [ ] **Step 1: The failing test.** `tests/health.rs`, three tests and no database:

```rust
//! The two routes that need nothing: liveness, and the shape of every refusal.
#![cfg(not(feature = "db-tests"))]

mod support;

use support::*;

/// `/healthz` answers without a key, or the image's HEALTHCHECK cannot probe the hub.
#[tokio::test]
async fn healthz_needs_no_key() {
    let hub = hub_with_env(&[]);
    let reply = hub.get("/healthz").await;
    assert_eq!(reply.code(), 200);
    assert_eq!(reply.body(), "ok");
}

/// An unknown path is the JSON 404 shape, never axum's empty body, so a client can parse it.
#[tokio::test]
async fn an_unknown_route_is_the_json_404_shape() {
    let hub = hub_with_env(&[]);
    let reply = hub.get("/v1/nope").await;
    assert_eq!(reply.code(), 404);
    assert_eq!(reply.error(), "NotFound");
    assert!(!reply.message().is_empty());
}

/// The hub's own source never names the compute crate's `auth::check` (graph-render-4f's reuse
/// limit, `docs/decisions/graph-hub.md:120-121`): it carries the `any-key` break and the compute
/// `App`. A source grep is the only thing that can hold a negative.
#[test]
fn the_hub_source_never_names_auth_check() {
    let hits = support::grep_this_crate("auth::check");
    assert!(hits.is_empty(), "graph-hub must not call auth::check: {hits:?}");
}
```

- [ ] **Step 2: Run it red.** `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-hub --test health` → FAIL: `error: package ID specification `graph-hub` did not match any packages` (no member yet).
- [ ] **Step 3: `server/graph-hub/Cargo.toml`**, the edge measured in facts 1–4:

```toml
[package]
name = "graph-hub"
description = "graph-hub: routes, keys and grants, gates and limits, SSE notices, the layout relay."
version = "0.1.0"
edition = "2024"
rust-version = "1.98"
publish = false

[dependencies]
graph-contract = { path = "../../crates/graph-contract" }
graph-store = { path = "../graph-store" }
# `bearer` and `keys::KeySet` only (H8, docs/decisions/graph-hub.md:120-121), and no feature
# forwarded: `default-features = false` keeps `negctl` and `test-hooks` off graph-server's edge,
# which row `hub-breaks-off` reads with `cargo tree -i graph-server -p graph-hub`.
graph-server = { path = "../graph-server", default-features = false }
# `http1` + `tokio` is graph-server's own set (:20) and is all SSE and a streamed body need
# (axum 0.8.9 gates `Sse::keep_alive` on `tokio`, nothing else in `response::sse`).
axum = { version = "=0.8.9", default-features = false, features = ["http1", "tokio"] }
http-body-util = "=0.1.5"
# `client` + `client-legacy` are the relay's whole cost: eleven crates in graph-hub's closure,
# none in the virtual root's (`default-members`).
hyper = { version = "=1.11.1", default-features = false, features = ["http1", "client"] }
hyper-util = { version = "=0.1.21", default-features = false, features = ["tokio", "client-legacy", "http1"] }
percent-encoding = "=2.3.2"
serde_json = { version = "=1.0.151", default-features = false, features = ["std"] }
sha2 = { version = "=0.10.9", default-features = false, features = ["std"] }
subtle = { version = "=2.6.1", default-features = false, features = ["std"] }
tokio = { version = "=1.53.2", features = ["rt-multi-thread", "net", "signal", "time", "sync", "io-util", "macros"] }

[dev-dependencies]
# Test-only: the real motor's router in process for `hub-roundtrip`, and the hub's own router
# through `oneshot`. Resolver 2 keeps both out of the lib and the binary.
graph-server = { path = "../graph-server" }
tower = { version = "=0.5.3", default-features = false, features = ["util"] }
futures-util = { version = "=0.3.34", default-features = false }

[features]
# The negative controls' hub-side breaks (`src/breaks.rs`). Off in every shipped build.
negctl = []
# The test-only seams (`src/hooks.rs`); graph-server reaches its own through a self
# dev-dependency (server/graph-server/Cargo.toml:41), the hub reaches these through its rows.
test-hooks = []
# Every test that opens a database connection. `#![cfg(feature = "db-tests")]` keeps condition (c):
# `svc-test` runs `--workspace` with no PostgreSQL and must pass.
db-tests = []

[lints.rust]
missing_docs = "warn"
unsafe_code = "forbid"
```

- [ ] **Step 4: `src/lib.rs`, `src/app.rs`, `src/error.rs`, `src/breaks.rs`, `src/hooks.rs`, `src/main.rs`.** `lib.rs` is the module list, the `pub use`s and the router, mirroring `server/graph-server/src/lib.rs:31-42`:

```rust
// Every route of §5.2 lands in this table; the grant each one needs is in the table's own module.
pub fn router(app: Arc<App>) -> Router {
    let api = Router::new()
        .route("/v1/meta", get(meta::meta))
        .route("/v1/workspaces", get(workspaces::list).post(workspaces::create))
        .route("/v1/workspaces/{ws}", put(workspaces::put))
        .route("/v1/workspaces/{ws}/plugins", get(plugins::list))
        .route("/v1/workspaces/{ws}/plugins/{plugin}", put(plugins::put))
        .route("/v1/workspaces/{ws}/plugins/{plugin}/batches", post(batches::post))
        .route("/v1/workspaces/{ws}/plugins/{plugin}/records", get(records::page))
        .route("/v1/workspaces/{ws}/records/{plugin}/{collection}/{id}", get(records::one))
        .route("/v1/workspaces/{ws}/graph", get(graph::get))
        .route("/v1/workspaces/{ws}/changes", get(changes::get))
        .route("/v1/workspaces/{ws}/events", get(events::get))
        .route("/v1/workspaces/{ws}/layout", post(layout::post))
        .with_state(app);
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/v1", api)
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
}
```

  `error.rs` holds the one enum and the two tables, both `match`es so a new variant cannot compile without a status:

```rust
/// The hub's own error enum. The body is graph-server's shape
/// (`{"error", "message"}`, server/graph-server/src/error.rs:107-112).
#[derive(Debug)]
pub enum HubApiError {
    /// 400: the request itself is wrong (a bare seq as a cursor, a `%00` in a path id).
    BadRequest(&'static str),
    /// 401: no key, or a key that is not in the file. One variant, so the two are identical bytes.
    Unauthorized(&'static str),
    /// 403: a key with no grant covering this workspace and plugin. Never reveals existence.
    Forbidden(&'static str),
    /// 404: reached only after authorization said yes.
    NotFound(&'static str),
    /// 406: relayed from the motor.
    NotAcceptable(String),
    /// 408: the request body did not arrive within `GRAPH_HUB_BODY_TIMEOUT_MS`.
    BodyTimeout,
    /// 409: a manifest that would shrink or change.
    Conflict(String),
    /// 410: a cursor outside what is kept, or from another epoch.
    Gone(String),
    /// 413: over one of §6's caps, or a relayed 413 with the motor's message.
    TooLarge { what: &'static str, limit: u64 },
    /// 422: a wire fault graph-contract refused. `HubError::status()` already said 422.
    Invalid { path: String, what: String },
    /// 429 on a subscriber cap, 503 on a permit or pool wait. Carries `Retry-After`.
    Busy { retry_after: u64 },
    /// 502: the motor's fault, mapped by §5.2's table.
    Motor(MotorFault),
    /// 500: a hub defect. Logged whole, never sent whole.
    Internal(String),
}

impl HubApiError {
    /// The wire `error` string: a graph-contract class for a hub fault, §5.2's name for a motor
    /// fault, and graph-server's own string when the motor's answer is relayed unchanged.
    pub fn code(&self) -> &'static str { /* one match per variant, no wildcard */ }
    /// The HTTP status, from the same match as `code`.
    pub fn status(&self) -> u16 { /* 400 401 403 404 406 408 409 410 413 422 429 500 502 503 */ }
    /// `Retry-After` in seconds, when §5.2 or §6 gives one.
    pub fn retry_after(&self) -> Option<u64> { /* Busy only */ }
}
```

  `MotorFault` is the relay column of §5.2, one variant per row: `Auth`, `BodyTimeout`, `TooLarge { message }`, `MaterializeInvalid`, `Error`, `Unavailable`, plus `Relayed { status, error, message }` for the rows that pass graph-server's answer through (400, 406, the 422 `LayoutFailed`/`PostFailed` arm, the 503). Every `MotorFault` is 502 except `Relayed` and `TooLarge`; `auth.rs` never constructs one and `relay/map.rs` is the only module that does. `breaks.rs` is the table (Step 7 of Task 2 lists the names as they are added) shaped exactly like `crates/graph-contract/src/hub/breaks.rs`: `#[cfg(feature = "negctl")] pub fn on(name: &str) -> bool` reading `GM_HUB_BREAK` comma-separated, and a `pub const fn on(_name: &str) -> bool { false }` twin. `hooks.rs` is three no-op seams: `pause_after_admit(route)`, `before_ack(seq)`, `count_headers(n)`. `main.rs` parses `Settings`, opens the store, builds `App`, installs the `SIGHUP` handler and serves, mirroring `server/graph-server/src/main.rs` and `serve.rs`; the `healthcheck` subcommand probes its own `/healthz` exactly as `server/graph-server/src/health.rs:30-46` does, because the image has no `curl`.
- [ ] **Step 5: The one graph-server edit, in the same commit as its first caller.** `server/graph-server/src/auth.rs:34`, `fn bearer(value: &str) -> Option<&str>` → `pub fn bearer(value: &str) -> Option<&str>`, line 33's doc comment untouched. The first caller is `src/auth.rs` (Task 3); Task 1's `app.rs` already calls it once from `credential_of` so the `pub` is never alone, which is the whole point of `docs/decisions/graph-hub.md:117-119`.
- [ ] **Step 6: Green, then the whole workspace with no database.**

```
scripts/orch/hub-pg.sh stop
scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-hub --test health
scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml -p graph-hub --all-targets -- -D warnings
scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast
```
  all PASS, the last with no PostgreSQL running (condition (c)).
- [ ] **Step 7: `deploy/hub.Dockerfile`** — the same seven-part shape as `deploy/service.Dockerfile`, `COPY` only:

```dockerfile
# The hub's runtime image. COPY-only, like deploy/service.Dockerfile:2-3: every cargo invocation
# lives in scripts/hub.sh through scripts/orch/gr, so no second rustup recipe exists.
FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a
COPY bin/graph-hub /usr/local/bin/graph-hub
ENV GRAPH_HUB_BIND=0.0.0.0 GRAPH_HUB_PORT=8080
USER 10001:10001
EXPOSE 8080
# No curl in the base: the binary probes its own /healthz, as graph-server's does.
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 CMD ["/usr/local/bin/graph-hub", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/graph-hub"]
```

- [ ] **Step 8: `scripts/hub.sh`** (`chmod 0755`, ≤ 300 lines). Header ≤ 30 lines: what it is, the verbs `image|run|keygen|upload-measurement|test`, exit codes 0/1/2, and the Caveat that `test` needs a database and `upload-measurement` needs two. `image` stages `target/hub/release/graph-hub` into `target/hub-stage/bin/` and runs `docker build -q -f deploy/hub.Dockerfile -t "graph-hub:$(content_hash)"` with the stage as the context, the tag being the first 16 hex of `sha256sum` over the sorted stage exactly as `scripts/service.sh:44-46,92` does; the build itself is `scripts/orch/gr bash -c 'cd server && CARGO_TARGET_DIR=/w/target/hub cargo build --release --locked --bin graph-hub'`, so `svc-image`'s `--bin graph-server` line is untouched. `run` is `scripts/orch/drun --rm --read-only --cap-drop ALL --security-opt no-new-privileges -p 127.0.0.1:${HUB_PORT:-8080}:8080` with the keys, grants and motor-key files bind-mounted read-only. `test` is `scripts/orch/hub-pg.sh url` into `GRAPH_HUB_DB_URL` and on to `scripts/orch/gr`. `scripts/orch/drun-check.sh` exits 0.
- [ ] **Step 9: The rows this task creates** (the whole file is written in Task 11):

```
hub-breaks-off|0|d=target/hub-breaks; mkdir -p $d; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build,features -i graph-server -p graph-hub --prefix none >$d/tree.txt && grep -c 'graph-server v' $d/tree.txt && ! grep -qE 'negctl|test-hooks' $d/tree.txt
negctl-hub-breaks-off|0|d=target/hub-breaks; c=target/hub-breaks-off; rm -rf $c; mkdir -p $c/server/graph-server $c/server/graph-hub/src; ln -s $(realpath --relative-to=$c crates) $c/crates; ln -s $(realpath --relative-to=$c/server/graph-server server/graph-server/src) $c/server/graph-server/src; cp server/Cargo.lock $c/server/; cp server/graph-server/Cargo.toml $c/server/graph-server/; cp server/graph-hub/Cargo.toml $c/server/graph-hub/; cp server/graph-hub/src/lib.rs $c/server/graph-hub/src/; printf 'members = ["graph-server", "graph-hub"]\n' >$c/server/Cargo.toml; sed -i 's|default-features = false }|default-features = false, features = ["negctl"] }|' $c/server/graph-hub/Cargo.toml; grep -q 'features = \["negctl"\]' $c/server/graph-hub/Cargo.toml || { echo "negctl-hub-breaks-off: the injected feature is gone" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $c/server/Cargo.toml -e normal,build,features -i graph-server -p graph-hub --prefix none >$d/ctl.txt && grep -q 'graph-server feature "negctl"' $d/ctl.txt
hub-virtual-root|0|base=f1a23521; d=target/hub-tree; git cat-file -e $base^{commit} || { echo "hub-virtual-root: $base is not in this clone" >&2; exit 2; }; rm -rf $d; mkdir -p $d/base/server/graph-server; ln -s $(realpath --relative-to=$d/base crates) $d/base/crates; ln -s $(realpath --relative-to=$d/base/server/graph-server server/graph-server/src) $d/base/server/graph-server/src; git show $base:server/Cargo.toml >$d/base/server/Cargo.toml; git show $base:server/Cargo.lock >$d/base/server/Cargo.lock; git show $base:server/graph-server/Cargo.toml >$d/base/server/graph-server/Cargo.toml; strip() { sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/base-server.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/new-server.txt; diff -u $d/base-server.txt $d/new-server.txt || { echo "hub-virtual-root: condition (b) moved" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e features --locked --prefix none | strip >$d/base-root.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/new-root.txt; diff -u $d/base-root.txt $d/new-root.txt && scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked -p graph-hub --prefix none | strip >$d/hub-root.txt && test "$(grep -c 'client-legacy' $d/hub-root.txt)" = 1 && test "$(grep -c 'client-legacy' $d/new-root.txt)" = 0
negctl-hub-virtual-root|0|base=f1a23521; d=target/hub-tree; rm -rf $d/ctl; mkdir -p $d/ctl/server/graph-server $d/ctl/server/graph-hub/src; ln -s $(realpath --relative-to=$d/ctl crates) $d/ctl/crates; ln -s $(realpath --relative-to=$d/ctl/server/graph-server server/graph-server/src) $d/ctl/server/graph-server/src; cp server/Cargo.lock $d/ctl/server/Cargo.lock; cp server/graph-server/Cargo.toml $d/ctl/server/graph-server/; cp server/graph-hub/Cargo.toml $d/ctl/server/graph-hub/; cp server/graph-hub/src/lib.rs $d/ctl/server/graph-hub/src/; grep -v '^default-members' server/Cargo.toml | sed 's|^members = .*|members = ["graph-server", "graph-hub"]|' >$d/ctl/server/Cargo.toml; strip() { sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/real-root.txt; scripts/orch/gr cargo tree --manifest-path $d/ctl/server/Cargo.toml -e features --locked --prefix none | strip >$d/ctl-root.txt; diff -q $d/base-root.txt $d/real-root.txt || { echo "negctl-hub-virtual-root: the real tree moved, so this control proves nothing" >&2; exit 1; }; diff -u $d/base-root.txt $d/ctl-root.txt >$d/ctl.diff; test -s $d/ctl.diff && grep -q 'client-legacy' $d/ctl.diff && grep -q 'graph-hub v' $d/ctl.diff
hub-floor|0|scripts/orch/gr cargo fmt --manifest-path server/Cargo.toml --all --check && scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings && scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast
negctl-hub-floor|0|d=target/hub-floor; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-hub/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-hub/Cargo.toml $d/server/graph-hub/; cp server/graph-hub/src/lib.rs $d/server/graph-hub/src/; printf 'pub fn unformatted( ) ->u8{7}\n' >>$d/server/graph-hub/src/lib.rs; scripts/orch/gr cargo fmt --manifest-path $d/server/Cargo.toml --all --check
clippy-hub|0|scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml -p graph-hub --all-targets --features db-tests,negctl,test-hooks -- -D warnings
negctl-clippy-hub|0|d=target/hub-clippy; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-hub/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-hub/Cargo.toml $d/server/graph-hub/; cp -r server/graph-hub/src/. $d/server/graph-hub/src/; printf '/// Undocumented on purpose.\npub fn undocumented() {}\n' >>$d/server/graph-hub/src/lib.rs; rc=0; scripts/orch/gr cargo clippy --manifest-path $d/server/Cargo.toml -p graph-hub --all-targets --features db-tests,negctl,test-hooks -- -D warnings >target/hub-clippy-negctl.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'missing documentation' target/hub-clippy-negctl.log
hooks-gated-hub|0|d=target/hooks-gated-hub; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --release --bin graph-hub -- --print cfg >$d/release.cfg 2>/dev/null && ! grep -qx 'feature="test-hooks"' $d/release.cfg && ! grep -qx 'feature="negctl"' $d/release.cfg && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --profile test --test health --features test-hooks -- --print cfg >$d/test.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/test.cfg
negctl-hooks-gated-hub|0|d=target/hooks-gated-hub; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --release --bin graph-hub --features test-hooks -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg
hub-image|0|scripts/hub.sh image && test -x target/hub-stage/bin/graph-hub
negctl-hub-image|0|HUB_IMAGE_BREAK=bin scripts/hub.sh image; rc=$?; test $rc -eq 1 && ! test -x target/hub-stage/bin/graph-hub && grep -q 'graph-server' target/hub-image/negctl-bin.log
drun-check|0|scripts/orch/drun-check.sh
svc-supply|0|scripts/orch/svc-digest.sh && scripts/orch/svc-digest-wasm.sh && scripts/orch/svc-features.sh && scripts/orch/lock-parity.sh
negctl-svc-supply-features|0|scripts/orch/svc-features.sh --break; test $? -eq 1
negctl-svc-supply-lock-version|0|scripts/orch/lock-parity.sh --break-version; test $? -eq 1
negctl-svc-supply-lock-feature|0|scripts/orch/lock-parity.sh --break-feature; test $? -eq 1
svc-supply-messages|0|scripts/orch/svc-features.sh --break >target/svc-features-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q 'graph-wasm is linked with probe in' target/svc-features-negctl.log && scripts/orch/lock-parity.sh --break-version >target/lock-parity-version-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after moving indexmap" target/lock-parity-version-negctl.log && scripts/orch/lock-parity.sh --break-feature >target/lock-parity-feature-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after adding probe" target/lock-parity-feature-negctl.log
cargo-deny-server|0|GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config deny.toml check advisories bans licenses sources
negctl-deny-license-server|0|sed '/^    "MIT",$/d' deny.toml >target/deny-negctl-server.toml; grep -q '"MIT"' target/deny-negctl-server.toml && exit 1; rc=0; GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config target/deny-negctl-server.toml check licenses >target/deny-negctl-server.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'MIT - MIT License:' target/deny-negctl-server.log
lock-parity|0|scripts/orch/lock-parity.sh
negctl-lock-parity-version|0|scripts/orch/lock-parity.sh --break-version; test $? -eq 1
negctl-lock-parity-feature|0|scripts/orch/lock-parity.sh --break-feature; test $? -eq 1
svc-features|0|scripts/orch/svc-features.sh
negctl-svc-features|0|scripts/orch/svc-features.sh --break; test $? -eq 1
svc-image|0|timeout 2400 scripts/service-image.sh
negctl-svc-image|0|SERVICE_IMAGE_BREAK=headers timeout 2400 scripts/service-image.sh; rc=$?; t=target/service-image/negctl-headers/table.md; test $rc -eq 1 && grep -q 'svc-healthy` .*| PASS |$' $t && grep -q 'svc-embed-graph-studio.js` .*| FAIL |$' $t
negctl-svc-image-leak|0|SERVICE_IMAGE_BREAK=leak timeout 2400 scripts/service-image.sh; rc=$?; t=target/service-image/negctl-leak/table.md; test $rc -eq 1 && grep -q 'svc-no-leak` .*| FAIL |$' $t
```

  `hub-virtual-root` is the store's row with the strip widened (fact 5's Caveat) plus the two `client-legacy` greps of fact 4: exactly one under `-p graph-hub`, zero in the virtual root. Its control drops `default-members`, which lets the hub's `client-legacy` unify into the root tree, and requires the differing lines to name both `graph-hub` and `client-legacy`. `hub-image`'s control sets `HUB_IMAGE_BREAK=bin`, which makes `scripts/hub.sh image` build `--bin graph-server` instead, so the staged binary is graph-server's and the row's own assertion fails. `negctl-hub-floor` and `negctl-clippy-hub` are slice 2's two controls pointed at this crate: an unformatted line and an undocumented `pub fn` in a scratch copy of `server/graph-hub`, each read from the log by grep. Commit.

- [ ] **Step 10: the shared rows §8's shared columns name.** The rows above re-run on the merged tree with this slice's member in place, and each keeps its own control, which is what conditions (d) and (e) and §8's `svc-supply` / `cargo-deny-server` rows ask for. Nothing here is a new gate: `drun-check` is the tree-wide bare-`docker run` check, `svc-supply` and its four controls are `scripts/orch/rows/service-supply.rows` verbatim, `cargo-deny-server` with its license control is slice 2's, `lock-parity` and `svc-features` with their `--break*` controls are theirs, and `svc-image` with its two controls is condition (d)'s. Each of the three `--break*` controls is checked by **grepping its original message** (`svc-supply-messages`), never by the exit code alone, exactly as `docs/decisions/graph-hub.md:136-141` requires. Run the whole block once: `scripts/orch/gate.sh target/gate-hub scripts/orch/rows/hub.rows` and every row of this task exits 0. Commit.

### Task 2: settings, limits and the start checks

**Files:**
- Create: `src/config.rs`, `src/config/{env,check}.rs`, `tests/start.rs`
- Modify: `server/graph-hub/src/main.rs` (call `Settings::from_env` then `check`), `src/app.rs` (hold the settings), `src/breaks.rs` (`no-start-check`)
- Modify: `scripts/orch/rows/hub.rows` (`hub-start-check` and its control)

**Interfaces:**
- Produces: `Settings { bind, port, db_url, keys_file, grants_file, motor_url, motor_key_file, store: StoreConfig, limits: Limits, connections: Connections, gates: Gates, subscribers: Subscribers, sse_page, motor_timeout, timeout }` with `Settings::from_env(lookup: Lookup<'_>) -> Result<Self, ConfigError>`; `ConfigError { name, reason }` whose `Display` is `"{name}: {reason}"`; `Limits { max_body, max_batch, max_record_bytes, max_plugin_bytes, max_doc_bytes, retain, retain_bytes, changes_bytes, body_timeout, stream_deadline, motor_timeout, timeout, sse_page, fetch_rows, last_seen }`; `Connections { max_connections, header_timeout, max_header_bytes }`; `Gates { writers, readers, layouts, writers_per_key }`; `Subscribers { max, per_key }`; `Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>`; `Settings::check(&self, db: &Store) -> Result<(), ConfigError>` — the §6 start checks, the two that need a connection among them.
- Every `GRAPH_HUB_*` name in §6's table appears exactly once in `config/env.rs`'s `NAMES` list, and `defaults_match_section_6` spells every default out as a literal so a spec change that renames a default fails here.
- Consumes: `graph_store::StoreConfig::{defaults, check}` and `Store::ping`; nothing new from graph-contract.

- [ ] **Step 1: Failing tests in `tests/start.rs`** (behind `db-tests`, because two of the checks read the database):

`config_defaults_match_section_6` (all 26 of §6's defaults as literals, plus `GRAPH_HUB_DB_URL` empty and the two credential paths unset); `every_hub_env_name_is_read` (each name in `NAMES` is honoured: a non-UTF-8 value is `is not UTF-8`, a malformed number `is malformed`, a range failure `is out of range` — the three messages `server/graph-server/src/config.rs:250-291` uses); `start_check_refuses_a_pool_at_or_below_the_permits` (`DB_POOL = WRITERS + READS + LAYOUTS` and one less, both refused; one more accepted); `start_check_refuses_changes_bytes_below_max_change` (`CHANGES_BYTES = max_change(limits) - 1` refused, `= max_change` accepted, where `max_change` is graph-contract's over `Limits::DEFAULT`); `start_check_refuses_retain_bytes_below_max_change`; `start_check_refuses_a_motor_timeout_at_or_below_forty_thousand` (40 000 refused, 40 001 accepted); `start_check_refuses_a_missing_keys_or_grants_file`; `start_check_refuses_an_empty_db_url`; `start_check_refuses_a_wrong_encoding` (a database created with `LATIN1`, read through `Store::ping`'s connection, refused — the §6 check `server_encoding = 'UTF8'`); `start_check_refuses_a_wrong_collation` (`datcollate <> 'C'`, same shape); `start_check_refuses_an_unreachable_database`; `start_check_passes_on_the_hub_image_database` (the `C`/UTF-8 database `scripts/orch/hub-pg.sh` starts, accepted).
- [ ] **Step 2: run, FAIL.** `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests --test start` → FAIL (no `config` module).
- [ ] **Step 3: `src/config/env.rs`.** The shape of `server/graph-server/src/config.rs` with the hub's own names: a `NAMES` const listing every `GRAPH_HUB_*` variable for `Settings::start_line` (names only, never values — `config.rs:125-133`), and a private `Env<'a>` with `text`, `number`, `parsed` and `millis` over `Lookup`. `millis` range is `1..=600_000` as in graph-server, so a 10-minute ceiling applies to every duration; a longer one is a config change with a review, not a bigger number. Each default carries its own `Caveat:` doc line naming what the number is not, following `server/graph-server/src/config.rs:157-180`.
- [ ] **Step 4: `src/config/check.rs`.** `Settings::check` runs §6's five refusals in a fixed order and names the offending variable in every message: (1) `GRAPH_HUB_DB_URL` empty; (2) `GRAPH_HUB_KEYS_FILE` or `GRAPH_HUB_GRANTS_FILE` unset (Decision 7); (3) `DB_POOL <= WRITERS + READS + LAYOUTS`; (4) `RETAIN_BYTES < max_change` or `CHANGES_BYTES < max_change`; (5) `MOTOR_TIMEOUT_MS <= 40_000`; then, over a live connection, (6) `server_encoding <> 'UTF8'`; (7) `pg_database.datcollate <> 'C'`. Each function under 40 lines: `check_numbers`, `check_pool`, `check_pages`, `check_motor`, `check_database`, each one `match`-free and testable alone. The refusal happens **before** the listener is bound, so a hub that fails a check never answers a request, and `main` exits 2 with the message on stderr, as `server/graph-server/src/main.rs:35-38` does.
- [ ] **Step 5: `Settings::check` is not bypassable in a test.** The `no-start-check` break makes `check` return `Ok(())` after logging the refusals it skipped; that is what the negative control turns on, so a green `start_check_refuses_*` can never be an unchecked `check`. Tests PASS; `svc-clippy` and `svc-test` still pass with PostgreSQL stopped. Commit.
- [ ] **Step 6: Row.**

```
hub-start-check|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test start
negctl-no-start-check|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-start-check scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test start; test $? -ne 0
```

### Task 3: keys, grants, authorization and `SIGHUP`

**Files:**
- Create: `src/keys.rs`, `src/grants.rs`, `src/auth.rs`, `src/auth/{path,grant}.rs`, `tests/authz.rs`, `tests/reload.rs`, `tests/support/keys.rs`
- Modify: `src/app.rs` (hold `Keyring`), `src/main.rs` (install the `SIGHUP` task), `src/breaks.rs` (`skip-grant`, `reload-keys-only`), `scripts/orch/rows/hub.rows` (`hub-authz`, `negctl-skip-grant`, `hub-reload`, `negctl-reload-keys-only`)

**Interfaces:**
- Produces: `Keyring { pair: RwLock<Arc<(KeySet, Grants)>> }` with `Keyring::load(keys: &Path, grants: &Path) -> Result<Self, KeyFileError>`, `Keyring::current(&self) -> Arc<(KeySet, Grants)>`, `Keyring::reload(&self) -> Result<bool, String>` (false = the old pair stayed, logged); `Grants { map: BTreeMap<String, Grant> }` with `Grants::load(path: &Path) -> Result<Self, String>`, `Grants::parse(text: &str) -> Result<Self, String>`, `Grants::allows(&self, key: &str, ws: &str, need: Need) -> bool`; `Grant { ws: String, mode: Mode }` with `Mode::{Read, Write(String), Admin}`; `Need::{Read, Write(&'static str), Admin}`; `auth::credential(&HeaderMap) -> Result<&str, HubApiError>` (401 for none, unknown, or a credential `graph_server::auth::bearer` refuses; **400** for a second `Authorization` header); `auth::authorize(&App, &HeaderMap, &Path, Need) -> Result<Credential, HubApiError>`; `Credential { key: String, ws: String, plugin: Option<String> }`; `auth::path_ids(&Request) -> Result<Ids, HubApiError>` refusing `\0` in any segment with 400 (Decision 8) and running `check_workspace_id` / `check_plugin_id` / `check_collection_id` on the rest.
- The grant line grammar is §5.2's, verbatim: `<key-name> <ws|*> <read|write:<plugin>|admin>`, one per line, `#` comments and blank lines skipped, a name 1–64 of `[A-Za-z0-9._-]`, a plugin id checked with `check_plugin_id`. A malformed line is a **load** failure, not a skip: a grants file that half-parses is a security bug, and `Keyring::load` refuses the pair.
- Consumes: `graph_server::auth::bearer`, `graph_server::keys::{KeySet, KeyFileError}` with `KeySet::{load, parse, name_of}`; `graph_contract::hub::check_{workspace,plugin,collection}_id`. `KeySet::name_of` is the constant-time lookup (`server/graph-server/src/keys.rs:118-128`); `KeySet` has no enumerable key list (`entries` is private), so a grant lookup is by name, which is why a key with no grant is denied by default without the hub ever knowing the key exists.

- [ ] **Step 1: Failing tests in `tests/authz.rs`.** `the_key_matrix_of_spec_5_2` as a table-driven test over the whole matrix of §5.2's "Order of refusals" plus §8's `hub-authz` row: `write:A` pushing `B.coll` is 422 (graph-contract's `check_collection_id`, not an authorization fault); a read-only key on `POST …/batches` is 403; an unknown key is 401; no `Authorization` header is 401; a **second** `Authorization` header is 400; a key with no grant on an existing workspace is 403; a key with no grant on a **non-existent** workspace is 403 with the same bytes; `GET /v1/workspaces` lists only the workspaces the key may read, each with `epoch` and `head_seq`; `If-Match` on plugin A is unaffected by plugin B's writes; `a_key_learns_nothing_from_another_plugins_records` (a key granted `read` on the workspace gets A's records and B's, but a key granted `write:B` only is refused A's records route with 403, never 404); `refusal_bytes_are_identical_with_and_without_the_workspace` (the two 403 bodies compared byte for byte); `a_percent_encoded_nul_in_a_path_id_is_400`; `a_bare_seq_as_a_cursor_is_400`.
  Named for Review Focus 1: `no_grant_is_403_before_any_404`, `refusal_bytes_are_identical_with_and_without_the_workspace`, `a_key_learns_nothing_from_another_plugins_records`.
- [ ] **Step 2: Failing tests in `tests/reload.rs`.** `a_sighup_with_a_bad_grants_file_keeps_the_old_pair` (write a malformed line, `SIGHUP`, the old key still works and the old grants still apply); `a_sighup_with_a_bad_keys_file_keeps_the_old_pair`; `a_good_pair_swaps_both` (new key accepted, old key refused, in one `SIGHUP`); `a_group_writable_grants_file_is_refused` (mode 0644 → refusal, the check `server/graph-server/src/keys.rs:74` applies to the keys file, applied here to the grants file as §5.2 requires); `a_relay_still_holds_a_key_set_across_a_sighup` (an in-flight request's `Arc` is unaffected by the swap).
- [ ] **Step 3: run, FAIL.** `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests --test authz --test reload` → FAIL.
- [ ] **Step 4: `src/auth/path.rs`.** The `{ws}`, `{plugin}`, `{collection}`, `{id}` extractors, one function each so none exceeds 40 lines. Each: reject an empty segment, reject `\0` (400, Decision 8), then `check_*_id` (422 with `HubError`'s own `path` and message, because a malformed id is a wire fault, not an authorization one). `{collection}` uses `check_collection_id`, which is what refuses `B.coll` in a **path**; `B.coll` inside a **body** is `Batch::check`'s job in the store, and the hub only relays the store's `HubError`.
- [ ] **Step 5: `src/auth/grant.rs` and `src/grants.rs`.** `Grants::allows` is a `BTreeMap` lookup on the key's **name** (from `name_of`), then a scan of that key's few lines for one whose `ws` is the workspace or `*` and whose `mode` covers the need. `admin` covers everything, `read` covers reads only, `write:<plugin>` covers that plugin's writes and every read. `BTreeMap`, never `HashMap`: a grants file is hashed by nothing but must be diffable and deterministic in every log line that names a grant. ≤ 4 parameters per function: `allows(&self, key, ws, need)` is three, and the plugin rides inside `Need::Write(&'static str)`.
- [ ] **Step 6: `src/auth.rs` and `src/keys.rs`.** `credential` reads `headers.get_all(AUTHORIZATION)`: zero → 401, more than one → **400** (RFC 9110 says a message with two of the same field is invalid, and §5.2 requires exactly this), one → `graph_server::auth::bearer(value).ok_or(Unauthorized)`, then `keyring.current().0.name_of(token)`; `None` → 401. `Keyring::reload` loads both into locals, and only when both are `Ok` does it take the write lock and swap the `Arc`; any failure logs and returns `Err`, leaving the old pair. `main` installs one `tokio::signal::unix::signal(SIGHUP)` task calling it — one task, not a handler, so the swap happens off the accept path; the listener is untouched, so no connection is dropped (mirroring `server/graph-server/src/app.rs:85`).
- [ ] **Step 7: tests PASS. Commit.** Then the rows:

```
hub-authz|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test authz
negctl-skip-grant|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=skip-grant scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test authz; test $? -ne 0
hub-reload|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test reload
negctl-reload-keys-only|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=reload-keys-only scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test reload; test $? -ne 0
```

`skip-grant` makes `Grants::allows` return `true` whatever it read, so `no_grant_is_403_before_any_404` and the whole matrix go red. `reload-keys-only` swaps the keys file and leaves the grants file as it was, so `a_good_pair_swaps_both` and `a_sighup_with_a_bad_grants_file_keeps_the_old_pair` go red. Commit.

### Task 4: the gates, the body reader and the connection limits

**Files:**
- Create: `src/gate.rs`, `src/gate/{permit,subscribers}.rs`, `src/body.rs`, `tests/limits.rs`, `tests/support/gate.rs`
- Modify: `src/config.rs` (the gate fields reach `Gates`), `src/breaks.rs` (`no-cap`), `src/error.rs` (429 and 503 with `Retry-After`), `scripts/orch/rows/hub.rows` (`hub-limits`, `negctl-no-cap`)

**Interfaces:**
- Produces: `Gate::new(permits: usize) -> Self`; `Gate::admit(&self, deadline: Instant) -> Result<OwnedSemaphorePermit, HubApiError>` (a `timeout_at` around `acquire_owned`; on expiry `Busy { retry_after: 1 }` → 503 with `Retry-After: 1`) — the same shape as `server/graph-server/src/gate.rs:31-44`, renamed from `admit`'s 429 because the hub's queue is unbounded and its wait is a timeout, not a queue-full; `Gate::free(&self) -> usize`; `KeyGate` (a per-key writer permit, `GRAPH_HUB_WRITERS_PER_KEY`, one per key name, `BTreeMap<String, Arc<Semaphore>>` with the entry dropped when its `Weak` no longer resolves, so an unbounded key count cannot leak); `Subscribers::admit(&self, key) -> Result<Subscriber, HubApiError>` (429 at the per-key cap then the global cap, §6) and `Subscribers::release(&self, key)`; `body::read(req: Request, limit: u64, budget: Duration) -> Result<Bytes, HubApiError>` → 413 over the cap with no byte read when `Content-Length` declares it, 408 on the budget, 400 when the body cannot be read (mirroring `server/graph-server/src/body.rs:12-24`).
- Consumes: nothing new.

- [ ] **Step 1: Failing tests in `tests/limits.rs`.** Every row of §6's limits, one test each: `writers_two_and_a_third_waits_then_gets_503_with_retry_after`; `readers_two_and_a_third_waits_then_503`; `layouts_one_and_a_second_waits_then_503`; `a_second_batch_from_one_key_waits_on_writers_per_key_while_another_keys_batch_proceeds` (R7, and the pair of keys must be distinguishable by which one gets the 503); `put_workspaces_takes_a_writers_permit` (§15(c) condition 9's correction); `a_body_over_the_cap_is_413_without_reading_a_byte` (a `Content-Length` over `GRAPH_HUB_MAX_BODY`); `a_chunked_body_over_the_cap_is_413_mid_stream`; `a_body_that_stalls_past_the_body_timeout_is_408`; `a_header_over_the_cap_is_431`; `a_header_read_past_the_header_timeout_closes_the_connection`; `the_connection_cap_queues_the_next_connection`; `the_subscriber_cap_per_key_is_429` and `the_subscriber_cap_in_total_is_429`; `a_permit_wait_past_the_timeout_is_503_not_500`; `a_pool_wait_past_the_timeout_is_503_with_retry_after`; `every_limit_default_is_the_section_6_value` (so §6 and the code cannot drift).
- [ ] **Step 2: run, FAIL.**
- [ ] **Step 3: `src/gate.rs`.** Three `Gate`s (`writers`, `readers`, `layouts`) plus the per-key one, each created from `Gates` in `App::from_settings`. `admit` is `timeout_at(deadline, self.slots.clone().acquire_owned())` where `deadline = Instant::now() + timeout`; `Elapsed` → `Busy { retry_after: 1 }`; `AcquireError` (a closed semaphore, only reachable at shutdown) → `Internal`. Every `Caveat:` names what the permit does **not** bound: a `READS` permit bounds concurrent materializations, not the bytes one of them streams, and `GRAPH_HUB_STREAM_DEADLINE_MS` is what cuts that (§6's Caveat).
- [ ] **Step 4: `src/body.rs`.** `read` wraps the request's `Body` in `http_body_util::Limited` at `max_body`, collects it under `timeout(body_timeout)`, and maps: a declared `Content-Length` over the cap → `TooLarge { what: "body", limit }` before a single byte is read (so a 5 GiB upload costs nothing); a length overflow mid-stream → the same 413; an elapsed budget → `BodyTimeout` (408); any other read error → `BadRequest` (400). `GRAPH_HUB_BODY_TIMEOUT_MS` is the hub's own; graph-server's identical 10 000 is enforced independently on its side of the relay.
- [ ] **Step 5: tests PASS. Commit.** Then the rows:

```
hub-limits|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test limits
negctl-no-cap|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-cap scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test limits; test $? -ne 0
```

`no-cap` reads every `Limit` as `u64::MAX` and every `Gate` as 64 permits, so the 413, the 429, the 408 and both 503s all disappear. Commit.

### Task 5: the write routes

**Files:**
- Create: `src/routes/mod.rs`, `src/routes/{workspaces,plugins,batches}.rs`, `tests/write.rs`, `tests/support/fixtures.rs`
- Modify: `src/lib.rs` (wire the three route modules), `src/error.rs`, `src/breaks.rs`, `scripts/orch/rows/hub.rows` (`hub-routes`, `negctl-lax-reader`, `hub-idem`, `negctl-no-idem`)

**Interfaces:**
- Produces: `PUT /v1/workspaces/{ws}` → `Store::create_workspace(&ws, &limits)`, 201 when inserted, 200 when it existed, under a `WRITERS` permit; `PUT /v1/workspaces/{ws}/plugins/{plugin}` → `Store::put_manifest(&ManifestWrite { ws, plugin, manifest, limits })`, answering `ManifestWritten.status` (201 or 200) and `manifest_json(&manifest)` as the body, 409 `Conflict` for `HubError::Conflict`, 413 for `HubError::TooLarge`; `POST /v1/workspaces/{ws}/plugins/{plugin}/batches` → `Store::apply_batch(&BatchWrite { ws, plugin, manifest, batch, idem, if_match, limits })` answering `BatchOutcome.response` (graph-contract's `answer_json(seq, applied)`) with a `Graph-Seq: <epoch>.<seq>` header; `GET /v1/workspaces/{ws}/plugins` → every manifest, streamed one plugin at a time; `GET …/plugins/{plugin}/records?cursor=&limit=` → `records::page`, `limit` default 1 000 and max 10 000, an opaque `next`, and `plugin_seq` as `"<epoch>.<plugin_seq>"` on every page; `GET …/records/{plugin}/{collection}/{id}` → the record's `values` and `rev`, read through `read_manifest`-shaped canonical text; `Idempotency-Key` and `If-Match` are read from the headers and passed through, never interpreted by the hub.
- The manifest and the batch bodies are read with `read_manifest(text, plugin)` and `read_batch(text, &Limits::DEFAULT)`; the hub adds no wire parsing of its own and no second reader.
- Consumes: everything in Task 1's store list, plus `graph_contract::hub::{read_manifest, read_batch, manifest_json, answer_json, growth, Growth, notice_json, ChangeHead}` and `hub::batch::{Batch, Delete, Upsert}`.

- [ ] **Step 1: Failing tests in `tests/write.rs`.** One test per status §5.2 and §4 give these routes: `put_workspaces_is_201_then_200_and_takes_no_seq`; `put_manifest_is_201_then_200_and_the_same_content_takes_no_seq`; `put_manifest_refuses_a_removed_field_with_409`; `put_manifest_refuses_the_same_version_with_other_content_with_409`; `put_manifest_refuses_the_sixty_fifth_plugin_with_413`; `post_batch_answers_seq_and_applied_and_a_graph_seq_header`; `an_identical_upsert_answers_applied_zero`; `a_batch_with_one_bad_record_changes_nothing` (422 and no seq); `if_match_gives_412_on_a_stale_plugin_seq`; `idempotency_replay_returns_the_same_response_and_the_same_head_seq`; `the_same_key_with_another_body_is_422`; `an_idempotency_key_over_128_bytes_is_422`; `a_record_page_is_in_byte_order_and_carries_plugin_seq`; `a_records_page_next_cursor_terminates`; `one_record_is_200_with_its_rev` and `one_record_of_another_plugin_is_403_not_404`; `get_plugins_returns_every_manifest`; `every_write_route_takes_its_permit` (a third concurrent request on each route gets 503, which is the row's own evidence that the permit is on the route and not only on the pool).
- [ ] **Step 2: run, FAIL.**
- [ ] **Step 3: the three route modules.** Each handler is `async fn` taking `State<Arc<App>>` plus at most three extractors — a request struct when more would be needed, never a fourth parameter. The order inside every write handler is fixed and is the order §5.2's refusals require: **admit the permit (or 503), then authorize (401/403/400), then read the body (413/408/422), then call the store, then map `StoreError`.** The store's `StoreError` mapping is one function, `write_fault(&StoreError) -> HubApiError`, in `routes/mod.rs`: `Hub(HubError)` → `HubError::status()` with the message, so a 409/413/422 the store decided keeps the store's status and never the hub's guess; `Busy { retry_after }` → 503 with `Retry-After`; `Serialization { retried: true }` → 503 with `Retry-After: 1` (§5.1's one retry, then 503); `NoDatabase` → 503 with `Retry-After`; `Db(_)` → 500 logged whole; `Gone` → 410; `Eof` → 500.
- [ ] **Step 4: `src/routes/batches.rs` and the watch.** After a successful `apply_batch` with `applied > 0`, the hub publishes the workspace's new `(epoch, seq)` on its `watch` **after** the store's step 8 has returned, which is the ordering §5.1 fixes ("only then is the response sent and the watch updated"). `watch.rs` (Task 7's module, created here because this is its first writer) holds one `tokio::sync::watch::Sender<(u64, u64)>` per workspace in a `BTreeMap`, raising it only when the new pair is greater — H7's monotone rule — and never iterating the map for output order.
- [ ] **Step 5: tests PASS. Commit.** Then the rows:

```
hub-routes|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write
negctl-lax-reader|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=lax-reader scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write; test $? -ne 0
hub-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write idem
negctl-no-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-idem scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write idem; test $? -ne 0
```

`lax-reader` is graph-contract's own break, reached through a `[dev-dependencies]` edge on `graph-contract` with `features = ["negctl"]` — the trick `server/graph-store/Cargo.toml` uses — so the hub's own reader is graph-contract's and the control is graph-contract's. `no-idem` is the store's break (slice 2 declares it), forced through `GM_HUB_BREAK` in the same run, so `hub-idem`'s sweeper half runs here with the store's control. Commit.

### Task 6: the read routes — `/graph`, `/changes`, `/v1/meta`, `/v1/workspaces`

**Files:**
- Create: `src/routes/{graph,changes,meta}.rs`, `src/routes/changes.rs` (410 mapping), `src/etag.rs`, `tests/read.rs`
- Modify: `src/lib.rs`, `src/error.rs`, `scripts/orch/rows/hub.rows` (`hub-routes` gains the read half)

**Interfaces:**
- Produces: `GET /v1/graph` → a `Body::from_stream` over `materialize::open(&store, ws)`'s `Document`, with `ETag: "<epoch>.<seq>"` from `Document::cursor()` and 304 on a matching `If-None-Match`; `GET /changes` → `changes::page` with `since` from `Cursor::parse`, 410 for `CursorState::Gone` and for `StoreError::Gone`, at most `limit` changes and at most `GRAPH_HUB_CHANGES_BYTES` (never fewer than one); `GET /v1/meta` → `{"api":1,"version":…,"limits":{…}}` with the version from `graph_server::config` and every limit of §6 spelled out; `GET /v1/workspaces` → the workspaces the key may read, each with `epoch` and `head_seq`, in id order; `etag::matches(if_none_match: Option<&str>, cursor: &Cursor) -> bool` — the exact string compare, no weak comparison, no `*` wildcard beyond what `If-None-Match: *` means.
- The streamed body is the store's `Document`, one `Bytes` per `head()`/`next()`/`tail()` piece, and the hub never concatenates them. A client that stops reading is cut at `GRAPH_HUB_STREAM_DEADLINE_MS`, which releases the pool connection and the `READS` permit.
- Consumes: `materialize::open`, `Document::{head, next, tail, cursor}`, `changes::{page, cursor_state}`, `ChangesReq`, `ChangePage`, `Change`, `ChangeOp`, `CursorState`, `records::page`, `RecordsPage`, `Cursor::parse`, `Cursor::to_string`, `HubError::Cursor` (400), `max_change`.

- [ ] **Step 1: Failing tests in `tests/read.rs`.** `graph_streams_the_document_with_an_e_tag`; `graph_is_304_on_a_matching_if_none_match`; `the_same_cursor_gives_the_same_bytes` (twice over the same cursor, byte-equal — the property `hub-materialize` proves in the store and the route must not break); `graph_never_buffers_a_whole_document` (a `Document` whose `next()` counts calls, and a client that reads the first chunk and drops the response: the count stops, which is what streaming means); `graph_stops_at_the_stream_deadline` (a client that reads nothing, deadline shortened through the test hook, the pool connection is back — asserted through the store's pool, not by timing); `changes_after_a_cursor_are_in_seq_order`; `changes_returns_410_for_a_cursor_from_another_epoch`; `changes_returns_410_for_a_cursor_below_what_is_kept`; `changes_returns_410_for_a_seq_above_head`; `a_changes_page_never_exceeds_the_byte_cap_but_holds_one_change`; `meta_reports_every_limit_of_section_6`; `the_workspaces_list_holds_only_what_the_key_may_read`.
- [ ] **Step 2: run, FAIL.**
- [ ] **Step 3: the four modules.** `graph.rs` opens the document under a `READS` permit and returns the stream; the permit is held **in the body**, not the handler, because the permit must live as long as the bytes — so the handler moves the `OwnedSemaphorePermit` into the stream's guard type. Caveat on that: a `READS` permit is then held for up to `GRAPH_HUB_STREAM_DEADLINE_MS` by a slow client, which is §6's stated cost and the reason the deadline exists. `changes.rs` parses the cursor, refuses a bare seq with 400, asks `changes::cursor_state` first (410 before the page read, so a gone cursor never costs a transaction), and maps the page. `meta.rs` builds its JSON with `serde_json::json!` from the same `Settings` the start check read, so the two cannot disagree.
- [ ] **Step 4: tests PASS. Commit.** The `hub-routes` row becomes the whole route table, one row, both test targets — this is its exact new text, and it is what Task 11 ships:

```
hub-routes|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write --test read
```

Commit.

### Task 7: SSE notices

**Files:**
- Create: `src/events.rs`, `src/events/{stream,page,beat}.rs`, `tests/events.rs`, `tests/support/bus.rs`
- Modify: `src/routes/mod.rs`, `src/breaks.rs` (`skip-event`), `scripts/orch/rows/hub.rows` (`hub-events`, `negctl-skip-event`, `hub-events-seq`)

**Interfaces:**
- Produces: `GET /v1/workspaces/{ws}/events` → `axum::response::sse::Sse` over a `futures_util::stream`, built with `Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"))` — the one place axum's `tokio`-gated keep-alive is used, which is why the hub's axum edge carries `tokio` (fact 1); `events::page(&Store, &PageReq) -> Result<Page, StoreError>` with `PageReq { ws, since: Cursor, at_most: u64 }` reading **change headers only**, at most `GRAPH_HUB_SSE_PAGE` = 256 of them, in seq order; `events::notice(&ChangeHead<'_>) -> String` (graph-contract's `notice_json`); `events::cursor_of(&Request) -> Result<Cursor, HubApiError>` reading `Last-Event-ID`, else `?since=`, else the workspace's current `<epoch>.<head_seq>`.
- The per-subscriber loop is §5.3's three steps verbatim: subscribe to the `watch` **first**, page headers after the cursor and send each as `event: change` with `id: <epoch>.<seq>` and `data: <notice_json>`, then when caught up wait on the `watch` and go back. The heartbeat is a comment every 15 s and **re-reads the epoch**; an epoch that moved sends `event: resync` and closes.
- A mid-stream page read or heartbeat that waits past `GRAPH_HUB_TIMEOUT_MS` for a pool connection: free the subscriber slot, send `event: busy` with **no `id:` line**, close (Decision 12, §16(b) condition 11). An invalid cursor at any point sends `event: resync` and closes.

- [ ] **Step 1: Failing tests in `tests/events.rs`.** `a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate` (four writers, one subscriber reconnecting under the `test-hooks` pause, the seqs delivered are `n+1, n+2, …` with no repeat and no hole); `resync_past_retention`; `resync_for_a_since_above_head`; `resync_after_an_epoch_change` (an `UPDATE workspaces SET epoch = hub_next_epoch()` from a second session — the heartbeat's re-read is what catches it, with no hub write after it); `a_stream_resumed_100_000_changes_back_reads_at_most_256_headers_per_read` (the store's `Hooks::pause_after_headers` plus a counting hook; §8's `hub-limits` case); `the_per_key_subscriber_cap_is_429`; `the_total_subscriber_cap_is_429`; `busy_carries_no_id_line` (the raw stream text between `event: busy` and the close has no `id:` line); `the_busy_slot_is_free_before_the_close` (a new subscriber on the same key succeeds immediately after the `busy`, before the old connection is gone); `the_busy_reconnect_reads_no_graph` (a counter on the `/graph` handler in the same process — the reconnect must not read it); `a_cursor_pruned_during_the_backoff_gets_resync`; `a_heartbeat_is_a_comment_and_never_carries_data`; `the_first_change_after_a_reconnect_is_cursor_plus_one_or_it_resyncs`.
- [ ] **Step 2: run, FAIL.**
- [ ] **Step 3: `src/events/page.rs`.** Reads headers with the store's `changes::page` at `limit = min(GRAPH_HUB_SSE_PAGE, what is left)` and keeps only the header fields: seq, plugin, `at`. The notice body is `notice_json(&ChangeHead { seq, plugin, at })`, so the hub is not a second producer of wire text. A page that returns a seq below the cursor, or a `CursorState::Gone`, ends the stream with `resync`.
- [ ] **Step 4: `src/events/stream.rs` and `beat.rs`.** `stream` holds the `watch::Receiver`, the cursor, the `Subscriber` guard (whose `Drop` frees the slot, so a dropped connection frees it whatever the reason) and the store handle. `beat` is `tokio::time::interval(15 s)` with `MissedTickBehavior::Skip`; Caveat: a busy loop can push the heartbeat out, so the interval is a floor, not a schedule, and the SDK's own timeout is what actually bounds a dead peer. The `busy` path takes the `Subscriber` explicitly **before** yielding the `busy` event, which is what makes `the_busy_slot_is_free_before_the_close` true.
- [ ] **Step 5: tests PASS. Commit.** Then the rows:

```
hub-events|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events
negctl-skip-event|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=skip-event scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events; test $? -ne 0
hub-events-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=sequence-seq scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate; test $? -ne 0
```

`skip-event` makes the stream publish from the `watch` without reading the database, so a seq can be skipped and `a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate` fails. `hub-events-seq` is D11's requirement that the store's `sequence-seq` break also turns the **events** row red, which is the visibility-order property §5.3 rests on; it is a row of its own because it is the same test with a different break. Commit.

### Task 8: the `/layout` relay

**Files:**
- Create: `src/relay.rs`, `src/relay/{body,answer,map}.rs`, `tests/relay.rs`, `tests/motor_map.rs`, `tests/support/motor.rs`
- Modify: `src/breaks.rs` (the hub reads `drop-record` **and** the store's name, both from `GM_HUB_BREAK`), `scripts/orch/rows/hub.rows` (`hub-roundtrip`, `negctl-drop-record`, `hub-motor-map`, `negctl-layoutfailed-as-502`)

**Interfaces:**
- Produces: `relay::post(&App, &RelayReq) -> Result<RelayAnswer, HubApiError>` with `RelayReq { ws, layout: Option<String>, post: Option<String>, accept: Option<String>, cursor: Cursor }` and `RelayAnswer { status: u16, content_type: Option<String>, vary: Option<String>, body: Body }`; `relay::body::document(&Document<'_>) -> impl Stream<Item = Result<Bytes, std::io::Error>>` — the store's `Document` turned into the request's chunked body, with the store's transaction ending when the last chunk is written, before the motor's answer is awaited (§5.3); `relay::map::fault(status: u16, error: &str, message: &str) -> HubApiError` — §5.2's table, row by row, the only place a motor answer becomes a hub status; `relay::answer::read(status, headers) -> Result<RelayAnswer, HubApiError>`.
- The relay sends `POST {GRAPH_MOTOR_URL}/v1/layout?layout=…&post=…&source=contract` with the caller's `Accept` passed through and `Content-Type: application/json`, and `Authorization: Bearer` from `GRAPH_HUB_MOTOR_KEY_FILE` (Decision 6). `source=contract` is fixed: H1 says the hub asks graph-server for the contract layout and never runs motor math. The whole exchange is under `timeout(motor_timeout)`, so a motor that never answers is 502 `MotorUnavailable` (§5.2's last row), and the snapshot is already closed by then.
- The `drop-record` break skips exactly one record in the **relay's** stream and nowhere else, so `/graph` and `/layout` cannot change together (§5.3, D4). It is read from `GM_HUB_BREAK` through **both** readers: `graph_store::breaks::on` and the hub's own, so one env var switches the store's and the hub's seams, exactly as slice 2 intended when it reserved the name.

- [ ] **Step 1: Failing tests in `tests/relay.rs`** (against `support::motor::real()`): `layout_bytes_equal_motor_bytes_at_the_same_cursor`, over the three fixtures of §8's `hub-roundtrip` — a workspace with one plugin and no links, one with a cross-plugin link, one with a link to an unregistered collection so H12's pruning is in the document — each: `POST /layout` through the hub, `POST /v1/layout?source=contract` at graph-server with the same document, the two bodies byte-equal; `the_graph_seq_header_equals_the_graph_etag`; `the_snapshot_closes_before_the_motor_answer_is_awaited` (the motor's fixture delays its answer on a test hook; the hub's pool connection count is back to zero before the delay expires); `layout_never_holds_a_whole_document` (the chunk count the store reports while the answer is being produced is greater than 1 and the hub's own peak body allocation stays under one chunk plus one record — asserted through the relay's own instrumentation, not through RSS); `a_layout_failure_is_relayed_with_the_motors_error`; `a_layout_is_never_retried` (a counting motor sees exactly one request even when it answers 503).
- [ ] **Step 2: Failing tests in `tests/motor_map.rs`** (against `support::motor::stub`): one test per row of §5.2's map — `motor_400_is_relayed_as_400`; `motor_401_is_502_motor_auth_and_logged`; `motor_406_is_relayed_as_406`; `motor_408_is_502_motor_body_timeout_with_no_retry_after`; `motor_413_is_413_graph_too_large_with_the_motors_message`; `motor_422_ingest_invalid_is_502_materialize_invalid`; `motor_422_contract_invalid_is_502_materialize_invalid`; `motor_422_layout_failed_is_relayed_as_422`; `motor_422_post_failed_is_relayed_as_422`; `motor_429_is_503_with_retry_after`; `motor_503_is_relayed_as_503_with_no_retry_after`; `motor_500_is_502_motor_error`; `motor_404_is_502_motor_error_and_logged` (the default row); `a_motor_that_never_answers_is_502_motor_unavailable` (the motor's own timeout, `GRAPH_HUB_MOTOR_TIMEOUT_MS`); `a_pool_wait_past_the_timeout_is_503_before_the_motor_is_called` (the motor's request counter must be **zero**); `a_relayed_body_keeps_the_motors_error_string`.
- [ ] **Step 3: run, FAIL.**
- [ ] **Step 4: `src/relay/map.rs`.** §5.2's table as one `match` on `(status, error)`, in this order, with no wildcard before the default arm: `200` → pass through; `400` → `Relayed`; `401` → `Motor(Auth)`; `406` → `Relayed`; `408` → `Motor(BodyTimeout)`; `413` → `TooLarge` with the motor's message; `422` split on `error`: `IngestInvalid | ContractInvalid` → `Motor(MaterializeInvalid)` logged as a hub defect, `LayoutFailed | PostFailed` → `Relayed`; `429` → `Busy { retry_after: 1 }`; `503` → `Relayed` with **no** `Retry-After`; everything else (including `500`, `404`, `405`) → `Motor(Error)` logged. The 422 split is N3 and the no-`Retry-After`-on-503 is N4/§5.2's Caveat, which says in the source that the hub cannot tell an admission wait from an overrun and that a distinct code is graph-render-4f's call.
- [ ] **Step 5: `src/relay/body.rs` and `answer.rs`.** `document` maps `Document::head()` to the first chunk, each `next()` to one, `tail()` to the last, and the `Document` is dropped when the stream ends, which is what closes the store's transaction. `answer::read` takes the status, `Content-Type` and `Vary` off the motor's response, maps through `fault`, and streams the body on the success path — never buffered, because graph-server's snapshot is the largest thing the hub relays. A `GRAPH_HUB_STREAM_DEADLINE_MS` cut applies to the answer as well as to the request.
- [ ] **Step 6: tests PASS. Commit.** Then the rows:

```
hub-roundtrip|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test relay
negctl-drop-record|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=drop-record scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test relay layout_bytes_equal_motor_bytes_at_the_same_cursor; test $? -ne 0
hub-motor-map|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test motor_map
negctl-layoutfailed-as-502|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=layoutfailed-as-502 scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test motor_map; test $? -ne 0
```

`drop-record` drops one record from the relay's stream only, so the byte equality fails while `/graph` still serves the full document — that asymmetry is the point of the control, and `negctl-drop-record` asserts it by running the `/graph` half in the same command and requiring it to pass. `layoutfailed-as-502` maps the `LayoutFailed` arm to 502 `MotorError` instead of relaying the 422, so `motor_422_layout_failed_is_relayed_as_422` and `motor_422_post_failed_is_relayed_as_422` go red. Commit.

### Task 9: durability and idempotency over HTTP

**Files:**
- Create: `tests/durability.rs`, `tests/support/step.rs`, `scripts/orch/hub-run.sh`
- Modify: `src/hooks.rs` (`before_ack`), `src/breaks.rs` (`ack-before-commit`), `src/main.rs` (the shutdown drain), `scripts/orch/rows/hub.rows` (`hub-durability`, `negctl-ack-before-commit`)

**Interfaces:**
- Produces: `scripts/orch/hub-run.sh`, the slice-3 sibling of `scripts/orch/hub-pg.sh` from slice 2, with the verbs `start|stop|kill|restart|url|run|ack-file` and exit codes 0/1/2: it starts the hub **container** from the image `scripts/hub.sh image` built, on a named volume `gm-hub-data`, with the keys, grants and motor-key files bind-mounted read-only and `GRAPH_HUB_DB_URL` pointing at the PostgreSQL container's bridge IP; `kill` is `docker kill` through `scripts/orch/drun`'s sibling, and `restart` starts it again on the same volume. The handshake with the test process is a file under `target/hub-steps/`, the same trick slice 2 uses for its container-level cases, with the Caveat that it works because `scripts/orch/gr` bind-mounts the repository read-write at `/w`.
- `Hooks::before_ack(seq)` (feature `test-hooks`) is the seam `ack-before-commit` uses: it runs **after** the store's step 8 returned and **before** the response is written, and with the break on it sleeps 500 ms there. The `ack-before-commit` break therefore delays the commit that follows… nothing: the store has already committed, which is the point — the control delays the **response** past the store's high-water read, so a hub killed in that window must still show the acked seq. Reading it the other way round would be the D6 defect the row exists to catch, and the source comment says so.

- [ ] **Step 1: Failing tests in `tests/durability.rs`.** `acked_seqs_are_written_before_the_kill` (200 batches, each with its own `Idempotency-Key`, the acked seqs appended to `target/hub-steps/durability.acked`, one per line); `every_acked_seq_is_present_after_a_container_kill` (the handshake: the test writes the file, `scripts/orch/hub-run.sh kill`, `restart`, the test reads the file and asserts every line is in `/changes` after `cursor = 0`); `no_gap_after_the_kill` (the seqs are `1..=200` with no hole and no duplicate); `a_batch_answered_after_the_commit_survives_the_kill` (the `ack-before-commit` window, which is the row's own subject).
- [ ] **Step 2: run, FAIL.**
- [ ] **Step 3: `scripts/orch/hub-run.sh` and the hook.** The script never uses a bare `docker run` (`scripts/orch/drun-check.sh` is a row) and never publishes a port except on loopback. `main`'s shutdown path drains in-flight requests for `GRAPH_HUB_TIMEOUT_MS` before exiting, as `server/graph-server/src/serve.rs:61-75` does, so a `docker kill`'s SIGTERM is a drain and only a SIGKILL is a cut.
- [ ] **Step 4: tests PASS. Commit.** Then the rows:

```
hub-durability|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-run.sh reset && scripts/orch/hub-run.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability acked_seqs_are_written_before_the_kill && scripts/orch/hub-run.sh kill && scripts/orch/hub-run.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability every_acked_seq_is_present_after_a_container_kill no_gap_after_the_kill && scripts/orch/hub-run.sh stop
negctl-ack-before-commit|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=ack-before-commit scripts/orch/hub-run.sh reset && GM_HUB_BREAK=ack-before-commit scripts/orch/hub-run.sh start && GM_HUB_BREAK=ack-before-commit scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability a_batch_answered_after_the_commit_survives_the_kill; test $? -ne 0
```

### Task 10: the memory measurement

**Files:**
- Create: `tests/memory.rs`, `scripts/orch/hub-mem.sh`, `docs/measurements/hub-memory.md`
- Modify: `src/breaks.rs` (the oversize control's names), `scripts/orch/rows/hub.rows` (`hub-memory`, `negctl-hub-memory`, `hub-upload-timeout`, `negctl-throttle-upload`)

**Interfaces:**
- Produces: `docs/measurements/hub-memory.md` with, in this order: the image digests and the two container invocations verbatim; **`F_w`**, the peak of reading a JSON body into values relative to its size, measured on **this** hub's batch reader (graph-server's 18.25 is graph-server's ingest reader, §6's Caveat); **`max_header`**, one change header row as read, measured not estimated (§6's Caveat); the last-seen map's entry size, measured (§6's Caveat); the peak RSS at every cap at once; and the `/layout` upload of Decision 4 — command, input size, pass condition, the five numbers beside the median.
- `scripts/orch/hub-mem.sh`, the runner both memory rows use: `measure|control|upload`, all containers through `scripts/orch/drun` with `--memory 1g --memory-swap 1g` (the row's own cap, §8), the peak read from `docker inspect`/`/proc/<pid>/status`'s `VmHWM` inside the container, the input built by a `tests/memory.rs` case that fills one workspace to the cap with the smallest record the contract admits.

- [ ] **Step 1: Failing tests in `tests/memory.rs`.** `f_w_is_measured_on_this_reader` (100 bodies of known size, the process's own `VmHWM` delta divided by the largest body — a real measurement, and it **replaces** the planning value in `docs/measurements/hub-memory.md`); `peak_rss_at_every_cap_fits_one_gib` (every cap at once, every body held in flight behind a barrier — `tokio::sync::Barrier` plus a `test-hooks` seam, so the row is not a race); `a_changes_page_holds_at_most_sse_page_headers`; `the_last_seen_map_entry_is_the_planned_size` (65536 entries inserted, the map's own accounting asserted against `docs/measurements/hub-memory.md`'s number, so the estimate cannot rot silently).
- [ ] **Step 2: run, FAIL** (no measurement file yet).
- [ ] **Step 3: `scripts/orch/hub-mem.sh measure`**, then write `docs/measurements/hub-memory.md` from its output: the raw numbers first, the planning arithmetic of §6 beside them, and the defaults that had to shrink with the reason (`GRAPH_HUB_DB_POOL`, `GRAPH_HUB_WRITERS`, `GRAPH_HUB_READS` — §6 says the defaults shrink until the peak fits, and a shrink is a change to a §6 default, so it is recorded here **and** in `docs/contract/hub-api.md`, never silently).
- [ ] **Step 4: `scripts/orch/hub-mem.sh upload`**, the Decision 4 measurement, into the same file under its own heading.
- [ ] **Step 5: tests PASS. Commit.** Then the rows:

```
hub-memory|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh measure
negctl-hub-memory|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GRAPH_HUB_WRITERS=64 GRAPH_HUB_DB_POOL=70 scripts/orch/hub-mem.sh control; rc=$?; test $rc -ne 0 && { test $rc -eq 137 || grep -q 'over 1 GiB' target/hub-mem/control.txt; }
hub-upload-timeout|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh upload
negctl-throttle-upload|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=throttle-upload scripts/orch/hub-mem.sh upload; test $? -ne 0
```

`negctl-hub-memory` sizes `WRITERS` from the **measured** `F_w` so `WRITERS × MAX_BODY × F_w` exceeds 1 GiB, sets `DB_POOL` just above the start check so the hub still starts (N7's whole point), runs under `--memory 1g --memory-swap 1g` with every body held behind a barrier, and requires exit 137 or a recorded peak over the limit. `throttle-upload` sleeps 2 ms per chunk in the relay, which pushes a 64 MiB upload past graph-server's 10 s body timeout; the motor answers 408, the hub maps it to 502 `MotorBodyTimeout`, and the row's own pass condition (no 408 in the motor's log) fails — so the control proves the measurement can fail. Commit.

### Task 11: the docs, the rows file and the gate

**Files:**
- Create: `docs/contract/hub-api.md`, `docs/deploy/hub.md`
- Modify: `docs/decisions/graph-hub.md` (append only), `scripts/orch/rows/hub.rows` (the whole file, below)

**Interfaces:**
- Produces: `docs/contract/hub-api.md` on `docs/contract/service-api.md`'s skeleton — `Status:` paragraph, `## What it is`, `## Routes` (one table row per §5.2 route with its grant, its request and its response), `## Errors` (the hub's own `error` classes and §5.2's `Motor*` names, three columns `Status | error | When`), `## API keys` (both files, 0640, the `SIGHUP` swap, "a key with no grant is denied"), `## Limits and scheduling` (§6's table with the values the code actually reads), `## Headers` (`ETag`, `Graph-Seq`, `Idempotency-Key`, `If-Match`, `Last-Event-ID`, `Retry-After`, `WWW-Authenticate`); `docs/deploy/hub.md` on `docs/deploy/service.md`'s skeleton — the inventory table, `## Build` (`scripts/hub.sh image`), `## Run` (the `drun` line, the four files and their modes, the bridge-IP database URL), `### Start checks` (the seven refusals and what an operator does about each), `### Restore runbook` (§5.3's `UPDATE workspaces SET epoch = hub_next_epoch()`, with its Caveat that at depth 0 it draws two epochs and both are fresh), `### Rotate a key`.
- `scripts/orch/rows/hub.rows`, the file the orchestrator gates, is exactly the content below: the §8 row map in the header, every slice-3 row, every shared row, and a negative control for every row that has a subject.

- [ ] **Step 1: Write `docs/contract/hub-api.md`**, every status the spec gives each route, copied from the tests that assert it (never from the plan), so a drifted test and a drifted doc cannot both be green.
- [ ] **Step 2: Write `docs/deploy/hub.md`**, including the two secrets the operator must create (the hub's keys file, the grants file, and the motor key file of Decision 6) and the `Caveat:` on each mode.
- [ ] **Step 3: Append to `docs/decisions/graph-hub.md`** one `## Slice 3 (hub-api), 2026-10-05` section: the four decisions, the measured facts with their commands, the `pub fn bearer` commit, and the `client-legacy` grep result. Append only; the text above is not touched.
- [ ] **Step 4: Write the whole rows file** (this is the file that ships):

```
# hub.rows — slice 3 (hub-api) of docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md
# (revision 5.1). Every §8 row whose slice column names 3, plus the rows the shared columns name.
#
# §8 row map, and where each row is implemented:
#   hub-durability         -> hub-durability, negctl-ack-before-commit                  (Task 9)
#   hub-roundtrip          -> hub-roundtrip, negctl-drop-record                          (Task 8)
#   hub-authz              -> hub-authz, negctl-skip-grant                               (Task 3)
#   hub-reload             -> hub-reload, negctl-reload-keys-only                        (Task 3)
#   hub-events             -> hub-events, negctl-skip-event, hub-events-seq              (Task 7)
#   hub-motor-map          -> hub-motor-map, negctl-layoutfailed-as-502                 (Task 8)
#   hub-idem               -> hub-idem, negctl-no-idem (store's break)                   (Task 5)
#   hub-limits             -> hub-limits, negctl-no-cap, hub-start-check,
#                             negctl-no-start-check                                      (Tasks 2, 4)
#   hub-memory             -> hub-memory, negctl-hub-memory, hub-upload-timeout,
#                             negctl-throttle-upload                                     (Task 10)
#   hub-breaks-off         -> hub-breaks-off, negctl-hub-breaks-off                     (Task 1)
#   hub-floor      (2, 3)  -> hub-floor, negctl-hub-floor, clippy-hub, negctl-clippy-hub,
#                             hooks-gated-hub, negctl-hooks-gated-hub                   (Task 1)
#   hub-virtual-root(2,3,5)-> hub-virtual-root, negctl-hub-virtual-root                 (Task 1)
#   svc-supply      (2, 3) -> svc-supply, negctl-svc-supply-features,
#                             negctl-svc-supply-lock-version, negctl-svc-supply-lock-feature,
#                             svc-supply-messages                                        (Task 1)
#   cargo-deny-server, lock-parity, svc-features (2, 3) -> same names, existing controls
#   svc-image (condition (d)) -> svc-image, negctl-svc-image, negctl-svc-image-leak     (Task 1)
#   §5.2's route table      -> hub-routes, negctl-lax-reader                             (Tasks 5, 6)
#   §6's start checks       -> hub-start-check, negctl-no-start-check                   (Task 2)
#   §5.3's upload vs graph-server's 10 s body timeout -> hub-upload-timeout,
#                             negctl-throttle-upload                                     (Task 10)
#   the hub image           -> hub-image, negctl-hub-image                               (Task 1)
#   hub-wire, hub-materialize, hub-seq, hub-epoch, hub-changes-snapshot, hub-pg-durability,
#   the store's retention row, codegen --check, the wasm32 build, motor-lock
#                            -> unchanged, owned by slices 1, 2 and 5; hub-floor re-runs the
#                              svc-* half on this tree.
#
# Every `negctl-*` row passes only when the thing it breaks goes RED for its own reason. A green
# negative control is a failure (§8 preamble). A row that cannot run is a failure: no row skips.
# Every database row starts and stops its own PostgreSQL through scripts/orch/hub-pg.sh, and the
# two container rows (hub-durability, hub-memory) their own hub container; `scripts/orch/gr` has
# no network option, so the tests connect by the container's bridge IP, which
# scripts/orch/hub-pg.sh hands them as GRAPH_HUB_DB_URL. A test with no GRAPH_HUB_DB_URL panics.

drun-check|0|scripts/orch/drun-check.sh
hub-floor|0|scripts/orch/gr cargo fmt --manifest-path server/Cargo.toml --all --check && scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings && scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast
negctl-hub-floor|0|d=target/hub-floor; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-hub/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-hub/Cargo.toml $d/server/graph-hub/; cp server/graph-hub/src/lib.rs $d/server/graph-hub/src/; printf 'pub fn unformatted( ) ->u8{7}\n' >>$d/server/graph-hub/src/lib.rs; scripts/orch/gr cargo fmt --manifest-path $d/server/Cargo.toml --all --check
clippy-hub|0|scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml -p graph-hub --all-targets --features db-tests,negctl,test-hooks -- -D warnings
negctl-clippy-hub|0|d=target/hub-clippy; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-hub/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-hub/Cargo.toml $d/server/graph-hub/; cp -r server/graph-hub/src/. $d/server/graph-hub/src/; printf '/// Undocumented on purpose.\npub fn undocumented() {}\n' >>$d/server/graph-hub/src/lib.rs; rc=0; scripts/orch/gr cargo clippy --manifest-path $d/server/Cargo.toml -p graph-hub --all-targets --features db-tests,negctl,test-hooks -- -D warnings >target/hub-clippy-negctl.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'missing documentation' target/hub-clippy-negctl.log
hooks-gated-hub|0|d=target/hooks-gated-hub; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --release --bin graph-hub -- --print cfg >$d/release.cfg 2>/dev/null && ! grep -qx 'feature="test-hooks"' $d/release.cfg && ! grep -qx 'feature="negctl"' $d/release.cfg && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --profile test --test health --features test-hooks -- --print cfg >$d/test.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/test.cfg
negctl-hooks-gated-hub|0|d=target/hooks-gated-hub; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-hub --release --bin graph-hub --features test-hooks -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg
hub-breaks-off|0|d=target/hub-breaks; mkdir -p $d; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build,features -i graph-server -p graph-hub --prefix none >$d/tree.txt && grep -c 'graph-server v' $d/tree.txt && ! grep -qE 'negctl|test-hooks' $d/tree.txt
negctl-hub-breaks-off|0|d=target/hub-breaks; c=target/hub-breaks-off; rm -rf $c; mkdir -p $c/server/graph-server $c/server/graph-hub/src; ln -s $(realpath --relative-to=$c crates) $c/crates; ln -s $(realpath --relative-to=$c/server/graph-server server/graph-server/src) $c/server/graph-server/src; cp server/Cargo.lock $c/server/; cp server/graph-server/Cargo.toml $c/server/graph-server/; cp server/graph-hub/Cargo.toml $c/server/graph-hub/; cp server/graph-hub/src/lib.rs $c/server/graph-hub/src/; printf 'members = ["graph-server", "graph-hub"]\n' >$c/server/Cargo.toml; sed -i 's|default-features = false }|default-features = false, features = ["negctl"] }|' $c/server/graph-hub/Cargo.toml; grep -q 'features = \["negctl"\]' $c/server/graph-hub/Cargo.toml || { echo "negctl-hub-breaks-off: the injected feature is gone" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $c/server/Cargo.toml -e normal,build,features -i graph-server -p graph-hub --prefix none >$d/ctl.txt && grep -q 'graph-server feature "negctl"' $d/ctl.txt
hub-virtual-root|0|base=f1a23521; d=target/hub-tree; git cat-file -e $base^{commit} || { echo "hub-virtual-root: $base is not in this clone" >&2; exit 2; }; rm -rf $d; mkdir -p $d/base/server/graph-server; ln -s $(realpath --relative-to=$d/base crates) $d/base/crates; ln -s $(realpath --relative-to=$d/base/server/graph-server server/graph-server/src) $d/base/server/graph-server/src; git show $base:server/Cargo.toml >$d/base/server/Cargo.toml; git show $base:server/Cargo.lock >$d/base/server/Cargo.lock; git show $base:server/graph-server/Cargo.toml >$d/base/server/graph-server/Cargo.toml; strip() { sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/base-server.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/new-server.txt; diff -u $d/base-server.txt $d/new-server.txt || { echo "hub-virtual-root: condition (b) moved" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e features --locked --prefix none | strip >$d/base-root.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/new-root.txt; diff -u $d/base-root.txt $d/new-root.txt && scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked -p graph-hub --prefix none | strip >$d/hub-root.txt && test "$(grep -c 'client-legacy' $d/hub-root.txt)" = 1 && test "$(grep -c 'client-legacy' $d/new-root.txt)" = 0
negctl-hub-virtual-root|0|base=f1a23521; d=target/hub-tree; rm -rf $d/ctl; mkdir -p $d/ctl/server/graph-server $d/ctl/server/graph-hub/src; ln -s $(realpath --relative-to=$d/ctl crates) $d/ctl/crates; ln -s $(realpath --relative-to=$d/ctl/server/graph-server server/graph-server/src) $d/ctl/server/graph-server/src; cp server/Cargo.lock $d/ctl/server/Cargo.lock; cp server/graph-server/Cargo.toml $d/ctl/server/graph-server/; cp server/graph-hub/Cargo.toml $d/ctl/server/graph-hub/; cp server/graph-hub/src/lib.rs $d/ctl/server/graph-hub/src/; grep -v '^default-members' server/Cargo.toml | sed 's|^members = .*|members = ["graph-server", "graph-hub"]|' >$d/ctl/server/Cargo.toml; strip() { sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/real-root.txt; scripts/orch/gr cargo tree --manifest-path $d/ctl/server/Cargo.toml -e features --locked --prefix none | strip >$d/ctl-root.txt; diff -q $d/base-root.txt $d/real-root.txt || { echo "negctl-hub-virtual-root: the real tree moved, so this control proves nothing" >&2; exit 1; }; diff -u $d/base-root.txt $d/ctl-root.txt >$d/ctl.diff; test -s $d/ctl.diff && grep -q 'client-legacy' $d/ctl.diff && grep -q 'graph-hub v' $d/ctl.diff
hub-start-check|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test start
negctl-no-start-check|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-start-check scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test start; test $? -ne 0
hub-authz|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test authz
negctl-skip-grant|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=skip-grant scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test authz; test $? -ne 0
hub-reload|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test reload
negctl-reload-keys-only|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=reload-keys-only scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test reload; test $? -ne 0
hub-limits|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test limits
negctl-no-cap|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-cap scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test limits; test $? -ne 0
hub-routes|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write --test read
negctl-lax-reader|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=lax-reader scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write; test $? -ne 0
hub-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write idem
negctl-no-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-idem scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test write idem; test $? -ne 0
hub-events|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events
negctl-skip-event|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=skip-event scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events; test $? -ne 0
hub-events-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=sequence-seq scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test events a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate; test $? -ne 0
hub-roundtrip|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test relay
negctl-drop-record|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=drop-record scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test relay layout_bytes_equal_motor_bytes_at_the_same_cursor; test $? -ne 0
hub-motor-map|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test motor_map
negctl-layoutfailed-as-502|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=layoutfailed-as-502 scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test motor_map; test $? -ne 0
hub-durability|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-run.sh reset && scripts/orch/hub-run.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability acked_seqs_are_written_before_the_kill && scripts/orch/hub-run.sh kill && scripts/orch/hub-run.sh start && scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability every_acked_seq_is_present_after_a_container_kill no_gap_after_the_kill && scripts/orch/hub-run.sh stop
negctl-ack-before-commit|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=ack-before-commit scripts/orch/hub-run.sh reset && GM_HUB_BREAK=ack-before-commit scripts/orch/hub-run.sh start && GM_HUB_BREAK=ack-before-commit scripts/orch/gr -e GRAPH_HUB_DB_URL="$(scripts/orch/hub-pg.sh url)" cargo test --manifest-path server/Cargo.toml -p graph-hub --features db-tests,negctl,test-hooks --test durability a_batch_answered_after_the_commit_survives_the_kill; test $? -ne 0
hub-memory|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh measure
negctl-hub-memory|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GRAPH_HUB_WRITERS=64 GRAPH_HUB_DB_POOL=70 scripts/orch/hub-mem.sh control; rc=$?; test $rc -ne 0 && { test $rc -eq 137 || grep -q 'over 1 GiB' target/hub-mem/control.txt; }
hub-upload-timeout|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh upload
negctl-throttle-upload|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=throttle-upload scripts/orch/hub-mem.sh upload; test $? -ne 0
hub-image|0|scripts/hub.sh image && test -x target/hub-stage/bin/graph-hub
negctl-hub-image|0|HUB_IMAGE_BREAK=bin scripts/hub.sh image; rc=$?; test $rc -eq 1 && ! test -x target/hub-stage/bin/graph-hub && grep -q 'graph-server' target/hub-image/negctl-bin.log
svc-supply|0|scripts/orch/svc-digest.sh && scripts/orch/svc-digest-wasm.sh && scripts/orch/svc-features.sh && scripts/orch/lock-parity.sh
negctl-svc-supply-features|0|scripts/orch/svc-features.sh --break; test $? -eq 1
negctl-svc-supply-lock-version|0|scripts/orch/lock-parity.sh --break-version; test $? -eq 1
negctl-svc-supply-lock-feature|0|scripts/orch/lock-parity.sh --break-feature; test $? -eq 1
svc-supply-messages|0|scripts/orch/svc-features.sh --break >target/svc-features-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q 'graph-wasm is linked with probe in' target/svc-features-negctl.log && scripts/orch/lock-parity.sh --break-version >target/lock-parity-version-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after moving indexmap" target/lock-parity-version-negctl.log && scripts/orch/lock-parity.sh --break-feature >target/lock-parity-feature-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after adding probe" target/lock-parity-feature-negctl.log
cargo-deny-server|0|GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config deny.toml check advisories bans licenses sources
negctl-deny-license-server|0|sed '/^    "MIT",$/d' deny.toml >target/deny-negctl-server.toml; grep -q '"MIT"' target/deny-negctl-server.toml && exit 1; rc=0; GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config target/deny-negctl-server.toml check licenses >target/deny-negctl-server.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'MIT - MIT License:' target/deny-negctl-server.log
lock-parity|0|scripts/orch/lock-parity.sh
negctl-lock-parity-version|0|scripts/orch/lock-parity.sh --break-version; test $? -eq 1
negctl-lock-parity-feature|0|scripts/orch/lock-parity.sh --break-feature; test $? -eq 1
svc-features|0|scripts/orch/svc-features.sh
negctl-svc-features|0|scripts/orch/svc-features.sh --break; test $? -eq 1
svc-image|0|timeout 2400 scripts/service-image.sh
negctl-svc-image|0|SERVICE_IMAGE_BREAK=headers timeout 2400 scripts/service-image.sh; rc=$?; t=target/service-image/negctl-headers/table.md; test $rc -eq 1 && grep -q 'svc-healthy` .*| PASS |$' $t && grep -q 'svc-embed-graph-studio.js` .*| FAIL |$' $t
negctl-svc-image-leak|0|SERVICE_IMAGE_BREAK=leak timeout 2400 scripts/service-image.sh; rc=$?; t=target/service-image/negctl-leak/table.md; test $rc -eq 1 && grep -q 'svc-no-leak` .*| FAIL |$' $t
```

- [ ] **Step 5:** `scripts/orch/gate.sh target/gate-hub scripts/orch/rows/hub.rows` → all 53 rows exit as declared: the 25 database rows green (each starting and stopping its own PostgreSQL), the 28 static rows green, and all 27 `negctl-*` rows green **because their subject went red for its own reason** — a green control is a failure (§8 preamble). Commit.
- [ ] **Step 6: the slice report inputs** for slice 5: the seven measured facts with their commands and outputs, the `client-legacy` grep pair, the condition (b) diff (empty), the `cargo deny` output, the `pub fn bearer` commit hash, and the §6 defaults that `hub-memory` had to shrink.

## Self-review

- Spec coverage, section by section: **§2** H1 (the hub never runs motor math; the relay is the only path to graph-server — Task 8), H2 (`server/graph-hub`, `default-members` kept — Task 1), H7 (the per-workspace `watch`, notices only, monotone — Tasks 5, 7), H8 (the hub's own keys file, `bearer` plus `KeySet`, `GRAPH_HUB_GRANTS_FILE`, `hub-breaks-off` — Tasks 1, 3), H9 (no second wire definition: `manifest_json`, `answer_json`, `notice_json` — Tasks 5, 6, 7), H14 (nothing holds a whole document — Tasks 6, 8, 10). **§3** the hub's row (routes, keys and grants, semaphores and limits, SSE notices, the streaming relay; never motor code, never an edit beyond `pub fn bearer` — Tasks 1–8). **§4** the manifest caps and the 409/413 cases, `If-Match` per plugin, the NUL rules, the no-op upsert — Tasks 2, 5. **§5.1** the hub's side of the writer transaction: `Idempotency-Key`, `If-Match`, the `{seq, applied}` answer, the watch after step 8 — Task 5. **§5.2** every route with a test per status the spec gives it (Task 5 writes, Task 6 reads, Task 8 the relay), the order of refusals (Task 3), the error shape and the `Motor*` names (Decisions 9, Task 8), the grants grammar and the `SIGHUP` swap (Task 3). **§5.3** `/graph` streaming and its ETag, `/changes`'s 410, the SSE protocol with `resync` and `busy`, the `/layout` relay and the `drop-record` placement, the upload measurement (Tasks 6, 7, 8, 10). **§6** every limit with its default, every route under a permit, the seven start checks, `doc_bytes`'s share of the caps, the memory budget and its four Caveats (Tasks 2, 4, 10). **§7** what the SDK expects of the wire: the cursor format, `event: busy` with no `id:`, the backoff's contract, `/layout` never retried, the records route under `write:<plugin>` with `plugin_seq` (Tasks 5, 6, 7, 8). **§8** every row whose slice column names 3, mapped in the rows file header (Tasks 1–10). **§10 slice 3** `pub fn bearer` in the same commit as its first caller (Task 1 Step 5), routes, keys, grants, semaphores, limits, start checks, SSE, the relay, `deploy/hub.Dockerfile`, `scripts/orch/rows/hub.rows`, `docs/contract/hub-api.md`, `docs/deploy/hub.md`, `docs/measurements/hub-memory.md`, `hub-virtual-root` re-run with `client-legacy` (Tasks 1–11). **§11** the axum SSE feature and the streamed body (Measured facts 1, 2; Task 1's edge), `client-legacy` (facts 3, 4), D8 option (i) with its Caveat and reopen trigger (Global Constraints), `bearer` in slice 3 (Task 1), D13 (Task 1's row), the records that are not final and are not this spec's to change (nothing here cites them as settled). **§12** conditions 8 (memory, Task 10), 9 (byte-bounded SSE and the resume protocol, Task 7), 10 (the hub's own keys file, one swapped pair, 0640 grants, Task 3), 11 (the authz matrix, no veto across plugins, no existence leak, no auth-off, Tasks 3, 5), 12 (`source=contract` and the mapped answers, Task 8), 13 (deny, lock-parity, svc-features, the `-p graph-hub` image, breaks off — Tasks 1, 11), 14 (u64 seq read exactly; no key in a browser, which is the SDK's rule and the hub never serves a page). **§13** conditions 1 (N6, every route bounded, the connection and header limits, every M term — Tasks 4, 10), 2 (N3, N4, N15 — Task 8), 3 (N5, N13 — Tasks 4, 8), 4 (N7, the peak in `docs/measurements/hub-memory.md` — Task 10), 5 (N11 — the `hub-events-seq` row, Task 7), 6 (`hub-breaks-off` with `-e normal,build,features` — Task 1), 7 (`hub-virtual-root` re-run with `client-legacy` — Task 1), 8 (`pub fn bearer` lands with its first caller — Task 1), 9 (N1's dangling policy is the store's, but the route must not undo it — Task 6's byte-equality test), 10 (N2's cursor 410 — Task 6), 11 (If-Match, quotas, subscriber caps, authorize first — Tasks 3, 5, 7), 12 (every graph-server status, the motor timeout, 503 not 408 — Task 8), 13 (`drop-record` in the relay, the ack delay, `sequence-seq` in `hub-events` — Tasks 7, 8, 9), 14 (the reuse conditions, `hub-breaks-off` through `cargo tree`, the linkage stated — Global Constraints, Task 1), 15 (the records route under `write:<plugin>`, no-op upserts — Task 5), 16 (retry transport errors with the same key — the SDK's, and the hub's `Idempotency-Key` makes it possible — Task 5). **§14(c) hub-api** 1–8 (Tasks 4, 8, 4/8, 10, 7, 1, 1, 1). **§15(c) hub-api** 9 (`GRAPH_HUB_SSE_PAGE`, its M term measured, the create under `WRITERS`, `event: busy` — Tasks 4, 7, 10), 10 (`GRAPH_HUB_WRITERS_PER_KEY` — Task 4). **§16(b) hub-api** 11 (the `busy` deviation: no `id:`, the slot freed before the close, no `/graph` on the reconnect, `resync` for a cursor pruned in the backoff — Task 7).
- Names used across tasks, each defined once above: `App`, `Settings`, `ConfigError`, `Lookup`, `Limits`, `Connections`, `Gates`, `Subscribers`, `StoreConfig`, `HubApiError`, `MotorFault`, `Keyring`, `Grants`, `Grant`, `Mode`, `Need`, `Credential`, `Ids`, `Gate`, `KeyGate`, `Subscriber`, `body::read`, `watch` registry, `relay::{post, body::document, answer::read, map::fault}`, `RelayReq`, `RelayAnswer`, `events::{page, notice, cursor_of}`, `PageReq`, `Page`, `etag::matches`, `routes::{workspaces, plugins, batches, records, graph, changes, meta, events, layout}`, `write_fault`, `breaks::on`, `Hooks`, `router`, `healthz`, `not_found`. Hub-side break names, one owner each: `no-start-check` (Task 2), `skip-grant` and `reload-keys-only` (Task 3), `no-cap` (Task 4), `skip-event` (Task 7), `throttle-upload` (Task 10), and `drop-record`, which is the **store's** name (slice 2 reserved it) read by the relay and by nothing else (Task 8).
- The four decisions the brief left open are Decisions 1–4 above; Decisions 5–12 record what the spec never names (`GRAPH_HUB_DB_URL`, the motor key file's content, the two required credential files, the NUL check, the `error` strings, the sweeper's owner, the two hyper features, and `busy`).
- Every §5.2 route has a task and a test per status the spec gives it: `/healthz` and `/v1/meta` (Tasks 1, 6), `PUT /v1/workspaces/{ws}` and `GET /v1/workspaces` (Tasks 5, 6), `PUT`/`GET …/plugins` (Task 5), `POST …/batches` (Task 5), `GET …/records` and `GET …/records/{plugin}/{collection}/{id}` (Task 5), `GET …/graph` (Task 6), `GET …/changes` (Task 6), `GET …/events` (Task 7), `POST …/layout` (Task 8).
- Nothing in this plan reads or writes `server/graph-server/**` beyond `auth.rs:34`, `server/graph-store/**` (named as consumed, never edited), `crates/**`, `deny.toml`, `docs/contract/delta.md`, `docs/contract/service-api.md`, `push-once.sh`, `push-loop.sh` or `.claude/rules/devil/`. A store change the API turns out to need is a **decision needed** for slice 2's owner, not an edit here.
