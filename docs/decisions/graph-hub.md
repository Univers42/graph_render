# ADR — graph-hub: a stateful service that plugins feed

- Status: **accepted with conditions**, revision 5: PROCEED-WITH-CONDITIONS from the `devil` on
  2026-10-05. Hub code may start; each slice meets its conditions (spec §12–§16) before it lands.
- Date: 2026-10-05
- Design: `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md`
- Continues: `docs/decisions/server-and-write-path.md`, phase D1 (the store and the write path).

## Context

The user asked on 2026-10-05 for a complete SDK and API: a micro-service plugin system
that tracks any kind of data and shows it as a graph. Service v1 (`server/graph-server`) is
stateless compute: a document in, a snapshot out. Nothing keeps data between requests,
numbers changes or streams them.

The spec was approved under the user's standing full-autonomy instruction, not by an
interactive review. That is a deviation, recorded here.

## Decision

A second service, **graph-hub**, in front of PostgreSQL. Plugins are out-of-process
clients that push records into their own namespace of a workspace. The hub numbers each
change, streams changes over SSE, serves the workspace as an ingest document, and asks
graph-server for layouts over HTTP. The stored model is the ingest contract. The decisions
are H1–H15 in the spec, §2.

## Verdicts

**Revision 1: BLOCK (2026-10-05).** It had already landed on develop (7a3416ac) before the
verdict; that order is a deviation. The 15 conditions and where revision 2 meets each are
in spec §12. Ten statements were found false against the tree:

| # | Revision 1 said | The tree |
|---|---|---|
| 1 | a dangling link is legal | `check_link_cells` (`crates/graph-contract/src/ingest/validate/cells.rs:28`) refuses it |
| 2 | deleting a linked record is harmless | `/layout` would answer 422 |
| 3 | `/layout` calls graph-server plainly | graph-server's `source` defaults to `studio`; the hub must send `contract` |
| 4 | the hub never sends a body graph-server refuses | per-id work caps (413), 422 and 429 remain |
| 5 | the `motor-alone` row stays green | no rows file defines it (`docs/reviews/review-studio.md:201-202`) |
| 6 | the hub reuses graph-server's auth | `auth::check` needs graph-server's `App`; `bearer` is private (`auth.rs:34`) |
| 7 | keys and grants reload atomically through `KeyStore` | `KeyStore` swaps keys alone |
| 8 | `at` is the commit time | it is read before commit; `seq` is the order |
| 9 | tags stay inside a plugin | `tag:<value>` is workspace-wide (`crates/graph-core/src/ingest/build/builder.rs:254-255`, `check_tag`) |
| 10 | the records it leans on are final | `server-dependencies.md` is "proposed"; `service-api.md:3` still reads "blocked" |

**Revision 2: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 4,
confidence 4; worst: cost and confidence. 18 conditions per slice (nine lift the block) and 13
defects. The two that carried the BLOCK: the memory bound contradicted the measured ingest peak
(18.25× the body, `server/graph-server/src/config/slots.rs:22`), and `sync` diffed against the
lossy `/graph`, so it churned forever. The others: resume holes (no epoch, `since > head`
undefined, a watch that could step back), negative controls that could not fire (`drop-record`,
`ack-before-commit`), vetoes across plugins, lock-parity and svc-features scratch copies that break
on new members, the motor linked into the hub, and features of the hub unifying into the shipped
graph-server at the `server/` virtual root (D13). Auth reuse was accepted on four conditions:
`default-features = false` with no forwarded `negctl` or `test-hooks`; the hub rebuilds the 400 for
a second header and the uniform 401; `pub fn bearer` with its first caller, slice confirmed by 4f;
D13 resolved. Spec §13 maps every condition and defect to where revision 3 meets it.

**Revision 3: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 4,
confidence 3; the worst is cost on failure. Three defects carry the block: N1, the multi-event
trigger DDL that PostgreSQL refuses, with `TRUNCATE`, the replica role and a restore bypassing the
epoch, and a sweeper that would move it; N2, `/changes` reading headers and operations in two
`READ COMMITTED` statements, so a prune between them loses operations silently; N6, a memory bound
that does not cover every route. Thirteen more: N3–N5, N7–N16. Revision 2 conditions met: 1, 2, 3,
6, 7, 11, 14, 16, 17, 18; partly met: 4, 5, 8, 9, 10, 12, 13, 15. The claim that H12 guarantees a
readable document was refuted by `motor.rs:18-19`: a 422 also comes from the caller's layout. The
PostgreSQL facts are measured in `docs/measurements/hub-pg-epoch-probe.md`; spec §14 maps every fix.

**Revision 4: BLOCK (2026-10-05).** Axes: blast radius 3, reversibility 3, cost on failure 3,
confidence 3; the worst is confidence. Two defects carry the block: R3, a restore detector that ran
only at start and whose keys no physical restore changes, with point-in-time recovery and promotion
unmeasured; R1, a trigger set that named no tables and a detector bump outside `hub.writer`. Nine
more: R2 (an empty `hub_meta`, a bump outside one transaction), R4 (SSE pages and `PUT /workspaces`
outside the memory bound), R5 (a negative control that could not fail), R6 (millisecond epochs),
R7 (one key holding both writer permits), R8 (a manual-SQL deadlock as a 500), R9 and R11 (two
wrong citations), R10 (`SET LOCAL` outside a transaction). The physical restores are measured in
`docs/measurements/hub-pg-epoch-probe.md` ("Physical restores"); spec §15 maps every fix and the one
deviation (`event: busy` instead of `event: resync`).

**Revision 5: PROCEED-WITH-CONDITIONS (2026-10-05).** Axes: blast radius 3, reversibility 3, cost
on failure 4, confidence 3; the worst is cost on failure. R1–R11 are met, and the `event: busy`
deviation is accepted on its merits. Seven new defects become conditions: S1, a small-gap restore
that passes the LSN compare (fixed by a per-workspace last-seen map checked on every new
connection); S2, an order of high-water reads that bumps a live database under write load (the
high-water snapshotted before the LSN read and lowered only after COMMIT, under a mutex); S3, a
"measured" claim for triggers the probe never set to `ALWAYS` (reworded; a catalog assertion); S4,
a durability premise checked only on the gate's image (`synchronous_commit` on per writer
transaction, `fsync` and `full_page_writes` checked per connection, the flush LSN); S5, stale status
lines; S6, two guard bypasses missing from the Caveat; S7, a deadlock case that could not fail.
Spec revision 5.1 folds the fixes into the body, and its §16 lists the conditions: hub-store 12–18,
hub-api 11, hub-sdk 6, hub-report 5.

## Agreements before slice 2

graph-render-4f owns `server/` and `server/graph-server`. Asked on 2026-10-05 for:

- one line added to the `members` list of `server/Cargo.toml`, and the shared
  `server/Cargo.lock`;
- `auth::bearer` made `pub`, so the hub carries no second Bearer parser.

Their reply, 2026-10-05, verbatim:

> 1. YES to adding server/graph-hub and server/graph-store to `members` and sharing
> server/Cargo.lock, on these conditions:
> (a) Edits to server/Cargo.toml are additive. The [profile.*] panic = "unwind" applies to your
> members too; leave it as is.
> (b) What ships stays the same. `cargo tree --manifest-path server/Cargo.toml -p graph-server -e
> normal,build --locked` must be byte-identical before and after (diff it in the slice report). In
> Cargo.lock, graph-server's existing entries must not change version.
> (c) My floor stays database-free. svc-clippy and svc-test run `--workspace`
> (svc-floor.rows:3-4), so your crates join them. They must pass there without a live Postgres.
> Put any DB test behind its own row and negctl, not in plain `cargo test`.
> (d) The image ships graph-server only. scripts/service.sh:73 builds `--bin graph-server`; keep
> it that way, and keep svc-image (service-image.rows) and its negctls green on the merged tree.
> (e) cargo-deny-server, lock-parity and svc-features stay green with their negctls, as you
> offered.
> 2. YES to `pub fn bearer` (server/graph-server/src/auth.rs:34). Make the one-word change in your
> slice-2 branch, in the same commit as its first caller: no pub without a caller. Keep its doc
> comment.
> Reuse bearer and keys::KeySet::{load,parse,name_of} only. Never reuse auth::check: it carries the
> `any-key` break knob (auth.rs:24) and the compute App.
> This is an auth surface, so the devil's verdict on revision 2 must cover the reuse before it
> lands.

So the `pub` is the hub's edit, made in slice 3 (hub-api, the first caller), not 4f's.
Conditions (a)–(e) and the reuse limit are acceptance criteria of slices 2 and 3.

After the revision 2 verdict, asked about D13, D7, D8 and the slice of `bearer`. Their second
reply, 2026-10-05, verbatim:

> yes to D13, you make the D7 fix, D8 option (i), and bearer goes in slice 3.
> 1. D13: YES, add `default-members = ["graph-server"]` in hub-store. That's a real catch; my (b)
> was blind to virtual-root unification. Make the virtual-root diff the binding check:
> `cargo tree --manifest-path server/Cargo.toml -e features --locked`, with no -p, byte-identical
> before and after. Put it in the slice report next to (b).
> 2. D7: make it in hub-store yourself. Give lock-parity.sh `scratch_setup` and svc-features.sh the
> same shape: loop over the workspace members, copy each one's Cargo.toml and link each one's src
> with the computed relative `..`. Change nothing else. Run all of service-supply.rows on the merged
> tree. The three negctls (`--break`, `--break-version`, `--break-feature`) must still exit 1 for
> their original reason; grep their message, don't trust the exit code alone. Send me the diff
> before it lands.
> 3. D8: (i), with default-features = false.
> Why not (ii): keys.rs:74 calls breaks::on("accept-group-writable"). Without `negctl` it compiles
> to a const false (breaks.rs:11-14), so nothing reaches a hub release build. (ii) would add a
> forwarded negctl feature to a new crate and re-prove service.rows' group-writable control, which
> means more edits on a gated auth surface to save link size.
> Conditions: hub-breaks-off shows neither negctl nor test-hooks on graph-server's edge in the
> release build. The spec states the graph-core and graph-wasm linkage, with a Caveat.
> Reopen trigger: extract graph-keys when a third consumer appears, or when a hub size or
> attack-surface budget refuses graph-core.
> 4. Confirmed: `pub fn bearer` lands in the slice whose commit adds its first caller, i.e. slice 3
> (hub-api). My "slice 2" was wrong.

Revision 3 takes each answer as written: spec H2 and row `hub-virtual-root` (D13), slice 2 step 2
and row `svc-supply` (D7), H1, H8 and row `hub-breaks-off` (D8), §10 slice 3 (`bearer`).

## Consequences

- `crates/` gains a `hub` module in graph-contract. Every later change to the hub wire voids
  the motor's gate evidence, so the wire freezes once slice 1 lands.
- `server/` gains two members and a database driver. `cargo-deny-server`, `lock-parity` and
  `svc-features` re-run in slices 2 and 3.
- `server-and-write-path.md` deferred "streams". This record lifts that only for an SSE
  change feed. Remote access and TLS stay stop-and-ask: the hub binds loopback by default.

## Round 2b (2026-10-06), Tasks 6 and 7 — decisions taken on the way

- **`GET /v1/meta` and `GET /v1/workspaces` needed a new `Need::Any`.** §5.2 gives both rows the
  grant "any key". The layer had been answering `Need::Read` on the *empty* workspace, which only a
  `*` grant covers, so a deployment whose grants name workspaces (the ordinary case) was refused 403
  on both rows. `Need::Any` is satisfied by having any grant line at all, and the listing still
  filters per workspace. Changed: `src/grants.rs`, `src/auth/need.rs`, `src/auth/grant.rs`.
- **`routes/scan.rs`'s `oldest()` was returning a cursor that skipped a change.** It probed forward
  for the first *servable* seq and returned it, but a page returns the changes **after** its cursor,
  so the change sitting at that seq was never seen. Invisible while a workspace's log starts at
  seq 1; a workspace whose earliest retained change is above 0 (anything retention has pruned) read
  its manifest as absent, and `POST …/batches` answered 404. It is now a binary search for the low
  bound, returning one below it. `log2(head_seq)` probes, about 17 at §6's `RETAIN`; a store
  accessor for the low bound would replace the search with one read.
- **`Subscribers::admit` now takes `&self`.** It took `self: &Arc<Self>`, so the events route had to
  own an `Arc<Subscribers>` to take a slot. `Subscriber` now holds the shared counters directly, which
  is the only state it needs to release on drop.
- **The `AsyncHook` seam is handed the route's name.** `the_busy_reconnect_reads_no_graph` needs a
  counter on `/graph` and not on "some route", and every test hook had no way to tell two routes
  apart. No existing test installed `pause_after_admit`, so the signature change cost nothing.
- **A fourth test seam, `page_fault`.** A stream's two early closes are decided by *which* store
  fault a read meets — `Gone` is a `resync`, `Busy` is a `busy` — and neither can be produced on
  demand: a pool cannot be made short and a log cannot be pruned. The seam is a function from the
  cursor to an optional fault, so one fixture forces either close.
- **The epoch re-read rides on the page read, not on the heartbeat.** The stream's next read answers
  with the epoch it read, so the check costs nothing, and a page answered from another epoch is the
  same `resync` as a `Gone` cursor. Cost, stated as a Caveat in the code: an idle subscriber of a
  promoted workspace is not told until its client times out or a change arrives.
- **`subscribers.admit` is keyed by key *name*, never by the token.** Nothing else was available:
  the hub only ever learns the name (`KeySet::name_of`).

## Round 4 (slice 3, Task 8): the `/layout` relay

1. **A motor 429 becomes the hub's own 503 with `Retry-After: 1`**, which is
   `HubApiError::busy_wait()`. §5.2's table says "503 + `Retry-After`" for the motor's
   queue-full row and the plan's test name says the same, while `HubApiError::Busy`'s status is
   429 for a non-zero `retry_after` and 503 only for `busy_wait()`. So the 429 row uses
   `busy_wait()`: the caller waits in the **hub's** queue, which is what a 503 with a
   `Retry-After` already says.
   Caveat: the hub cannot distinguish its own queue-full from a motor admission wait, and
   does not try.
2. **`relay::post` takes the already-open `Document` and an optional `Probe`** rather than
   opening the snapshot itself. The handler admits the `LAYOUTS` permit and opens the snapshot
   before the exchange (§5.2's order: permit, then work), and `layout_never_holds_a_whole_
   document` needs to hand the relay its own counters. `RelayReq.cursor` is read off that same
   `Document`, so it is the snapshot's own position and not a second read.
3. **The `LAYOUTS` permit travels with the response body** (`relay::body::held`). §6's default
   of 1 is what bounds a waiting `/layout` to one snapshot, one xmin horizon and one pool
   connection; a permit freed when the response head was written would let a second upload start
   while the first answer was still streaming. Caveat: a caller that keeps a connection open and
   never reads holds the permit until `GRAPH_HUB_STREAM_DEADLINE_MS` cuts the stream.
4. **`GRAPH_HUB_STREAM_DEADLINE_MS` cuts the answer as well as the request**, and a cut ends the
   stream rather than failing it: the caller sees a short body and re-reads at its cursor, which
   is §6's rule for every streamed route.
5. **The relay builds a `hyper-util` client per call.** Caveat: a fresh TCP connection per
   `/layout`, which §6's table treats as free because `GRAPH_HUB_LAYOUTS` defaults to 1.
6. **`negctl-drop-record` runs two commands**: the byte equality must fail and
   `graph_is_not_affected_by_drop_record` must pass. One command alone would only prove that some
   relay exists, not that `drop-record` is in the relay alone (D4, §5.3).
7. **`GM_HUB_BREAK` is forwarded into the container with `-e GM_HUB_BREAK`** on both control
   rows, like every other control in `scripts/orch/rows/hub.rows`. Without it the variable is set
   on the host only, the break never reaches the test, and the control passes for the wrong
   reason.

## Slice 3 (hub-api), 2026-10-05

The four decisions the plan left open, and the seven facts it measured before any code was written.

### Decisions as built

1. **The hub's tests reach graph-server through an in-process router on an ephemeral port, never
   the built binary.** `tests/support/motor.rs` provides `real()` (a `graph_server::router` served
   by `axum::serve`) and `stub()` (a hand-written router answering one scripted
   `(status, error, message, body)` per request, which is what `hub-motor-map` needs), reusing
   `server/graph-server/tests/common/child.rs`'s conventions verbatim — loopback only, the port
   learned from the listener, a bounded wait, and a `Drop` that stops the server. Cargo defines
   `CARGO_BIN_EXE_graph-server` only for test targets of the crate that owns the binary, so a
   different crate cannot spawn it. Caveat: the in-process motor shares the test binary's event loop
   and its 8 GiB container, so a `/layout` timing measured here is a floor, not the production
   figure. As built, `hub-roundtrip` (`tests/relay/roundtrip.rs`
   `layout_bytes_equal_motor_bytes_at_the_same_cursor`) compares bytes against that router, and
   `hub-motor-map` (`tests/motor_map/rows.rs`) drives the stub.
2. **The key and grant model lives in four files, none of them about HTTP.** `src/auth.rs` holds
   `credential()` and `authorize()` only; `src/keys.rs` holds `Keyring::{load, current, reload}`;
   `src/grants.rs` holds `Grants::parse` and `Grants::allows`; `src/gate.rs` holds the four
   semaphores and the two subscriber counters; `src/app.rs` holds the state every handler takes.
   Every file is under 300 lines. As built, the pair is one `RwLock<Arc<(KeySet, Grants)>>`, so
   `SIGHUP` swaps both or neither (`tests/reload.rs` `a_good_pair_swaps_both`,
   `a_sighup_with_a_bad_grants_file_keeps_the_old_pair`, `a_sighup_with_a_bad_keys_file_keeps_the_old_pair`).
3. **`scripts/orch/rows/hub.rows` holds every §8 row whose slice column names 3, plus the rows the
   shared columns name**, and **every row that boots the hub needs PostgreSQL**. Each database row
   starts and stops its own with `scripts/orch/hub-pg.sh reset && … start`, because the first
   database connection runs the restore detector and the start check reads `server_encoding` (§5.3,
   §6). `scripts/orch/gr` has no network option, so the container rows are reached at the container's
   bridge IP, which `scripts/orch/hub-run.sh` writes to `target/hub-run/url` on every start. A test
   with no `GRAPH_HUB_DB_URL` panics, so every database row passes one explicitly.
4. **`docs/measurements/hub-memory.md` measures the `/layout` upload against graph-server's body
   timeout by filling one workspace to exactly `GRAPH_HUB_MAX_DOC_BYTES` with the smallest record
   the contract admits, then timing the relay's upload alone over 5 runs after one warm-up.** The
   pass condition is the slowest of the 5 under **8 000 ms**, two seconds under graph-server's
   `GRAPH_BODY_TIMEOUT_MS` default of 10 000, because the motor's answer must arrive before its own
   body timeout. A miss is a stop (§5.3), not a retune. The intended command is
   `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-mem.sh upload`,
   and its control `GM_HUB_BREAK=throttle-upload`. **This decision is not yet built.** `hub.rows` has
   neither `hub-upload-timeout` nor `negctl-throttle-upload`, `scripts/orch/hub-mem.sh` refuses a
   third verb with `usage: hub-mem.sh measure|control` and exit 2, and `scripts/hub.sh
   upload-measurement` says so itself. They are Task 10's, and Task 11 Step 4's rows file is where
   they land.

### Measured facts

1. **axum 0.8.9's SSE needs no feature of its own; the heartbeat needs `tokio`.** Probe with
   `axum = { default-features = false, features = ["http1"] }`:
   `error[E0599]: no method named 'keep_alive' found for struct 'Sse<S>' in the current scope`.
   With `features = ["http1", "tokio"]`: `Finished dev profile`.
2. **A streamed request body and a streamed response body need no feature beyond `http1`.**
   `scripts/orch/gr cargo check --manifest-path server/Cargo.toml -p graph-hub` → `Finished dev profile`.
3. **`client-legacy` adds exactly eleven crates to graph-hub's closure**: `want v0.3.1`,
   `try-lock v0.2.5`, `futures-channel v0.3.34`, `futures-util v0.3.34`, `httparse v1.10.1`,
   `libc v0.2.190`, `socket2 v0.6.5`, `tracing v0.1.44`, `pin-project-lite v0.2.17`,
   `tracing-core v0.1.36`, `once_cell v1.21.4`. Caveat: `tracing` and `tracing-core` are the two
   that matter — graph-server's edge already unifies them, so the pair is not a second copy in
   practice, but the eleven is the honest closure delta.
4. **The `tokio` feature on the hub's axum edge adds no crate**, because graph-server's edge already
   unifies it: the two trees are byte-identical after paths are stripped.
5. **Condition (b) holds with the hub edge**: `scripts/orch/gr cargo tree --manifest-path
   server/Cargo.toml -e normal,build --locked -p graph-server --prefix none` piped through
   `sed -e 's| (/[^)]*)||g' -e 's| (command-line)$||' | LC_ALL=C sort` produces no difference
   against `f1a23521`, 130 lines each.
6. **`cargo deny` passes on the tree with the hub edge**:
   `GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config deny.toml
   check advisories bans licenses sources` → exit 0, last line
   `advisories ok, bans ok, licenses ok, sources ok`. Caveat: the pre-existing `hashbrown` duplicate
   `0.15.5`/`0.17.1` is still there and is not this slice's to close.
7. **`hub-breaks-off`'s command works and its control bites**:
   `cargo tree --manifest-path server/Cargo.toml -e normal,build,features -i graph-server -p
   graph-hub --prefix none` prints one line `graph-server v0.1.0` and zero matching
   `negctl|test-hooks`; the control, with `features = ["negctl"]` injected into a scratch copy,
   prints `graph-server feature "negctl"`.

### The `pub fn bearer` commit

```
$ git log --format='%h %s' -S 'pub fn bearer' -- server/graph-server
e8140af7 updated
afbd86f6 updated
```

`e8140af7` is the one that matters: it is the only commit in this history that changed
`server/graph-server/src/auth.rs`, and its diff is `-fn bearer` → `+pub fn bearer` at
`server/graph-server/src/auth.rs:34`, in the same commit as its first caller
`App::credential_of` in `server/graph-hub/src/app.rs` (§10's condition 8).

`afbd86f6` is a false positive of the same grep and is named here so nobody chases it: it adds
`pub fn bearer(key: &str) -> String` to `server/graph-server/tests/common/child.rs`, a **test
helper** of the same name in graph-server's own test tree. It is not the hub's seam. The reliable
form of the query is `git log --format='%h %ad %s' --date=short -S 'pub fn bearer' --
server/graph-server/src/auth.rs`, which prints `e8140af7 2026-10-05` and nothing else.

Caveat: the query is a substring search, so any future commit adding a function of that name
anywhere under `server/graph-server` widens the answer. Pin the path, not the string.

### The `client-legacy` grep pair

```
$ scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked -p graph-hub --prefix none | grep -c client-legacy
1
$ scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | grep -c client-legacy
0
```

The second command exits 1, because `grep -c` prints `0` and returns 1 when nothing matched; the
count is what matters. This pair is the `client-legacy` clause of row `hub-virtual-root`
(§13 condition 7): the feature appears exactly once in graph-hub's own closure and zero times in
the workspace's, so the hub's axum edge has not pulled graph-server's HTTP client into the virtual
root.
