# graph-hub slice 2 (hub-store) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `server/` gains `graph-store`, a library crate that owns the PostgreSQL schema, the migrations, the epoch clock and its triggers, the restore detector, the writer transaction, the streamed materializer, `/changes` after a cursor, a plugin's own records, retention and the idempotency sweeper — and nothing else. No route, no key, no HTTP.

**Architecture:** One lib crate, `tokio-postgres` over plain TCP (`NoTls`), no HTTP. The writer is one explicit `READ COMMITTED` transaction per batch in §5.1's lock order; the readers are one `REPEATABLE READ, READ ONLY` transaction each. Every write path opens with `set_config('hub.writer', '1', true)`, so the four per-event and one `TRUNCATE` `ENABLE ALWAYS` triggers on each workspace-keyed table stay quiet for the hub's own writes and fire for everybody else's. The document is streamed from an ordered scan and graph-contract stays the only producer of canonical bytes.

**Tech Stack:** Rust 2024 (`server/graph-server/Cargo.toml:6` sets `edition = "2024"`, `rust-version = "1.98"`), `tokio-postgres 0.7.18` (`default-features = false`, features `["runtime"]`), `tokio 1.53.2`, `sha2 0.10.9` (the version graph-server already pins), `graph-contract` by path. Tests reach PostgreSQL through `scripts/orch/hub-pg.sh`; every command through `scripts/orch/gr`, every container through `scripts/orch/drun`.

**Spec:** `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1; §2 H3/H6/H11/H12/H14/H15, §3, §4, §5.1, §5.3, §6, §8 rows, §10 slice 2 steps 1–3). Verdict and the conditions of `docs/decisions/graph-hub.md:101-152`.

## Global Constraints

- **Owner of `server/` is graph-render-4f, and these are binding** (`docs/decisions/graph-hub.md:105-116`, their second reply `:131-140`):
  - (a) Edits to `server/Cargo.toml` and to graph-server are **additive**; `panic = "unwind"` stays in `[profile.dev]` (`:11-12`) and `[profile.release]` (`:14-15`) untouched.
  - (b) `scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -p graph-server -e normal,build --locked` is **byte-identical** before and after (paths stripped, sorted), and no graph-server entry in `server/Cargo.lock` changes version. Recorded as row `hub-virtual-root`, next to the virtual-root diff.
  - (c) `svc-clippy` and `svc-test` (`scripts/orch/rows/svc-floor.rows:3-4`) run `--workspace` and must pass **with no PostgreSQL running**. Every database test therefore sits behind its own row, its own feature and its own negative control; nothing in `tests/` may run under a plain `cargo test`.
  - (d) `scripts/service.sh:73` still builds `--bin graph-server` only; `svc-image` (`scripts/orch/rows/service-image.rows`) and its negative controls stay green. No `deploy/hub.Dockerfile` in this slice.
  - (e) `cargo-deny-server`, `lock-parity` and `svc-features` stay green, and their three negative controls still exit 1 **for their original reason** (grepped, not trusted by exit code alone).
- The D7 change (Task 2) is diffed and sent to graph-render-4f **before it lands**, and it changes nothing else in the two scripts.
- The 38 crates `graph-store` adds beyond graph-server's closure (measured 2026-10-05 on develop `f1a23521`): `async-trait 0.1.92`, `base64 0.22.1`, `block-buffer 0.12.1`, `byteorder 1.5.0`, `chacha20 0.10.2`, `cmov 0.5.4`, `const-oid 0.10.2`, `cpufeatures 0.3.1`, `crypto-common 0.2.2`, `ctutils 0.4.2`, `digest 0.11.3`, `fallible-iterator 0.2.0`, `futures-sink 0.3.34`, `getrandom 0.4.3`, `hmac 0.13.0`, `hybrid-array 0.4.15`, `lock_api 0.4.14`, `log 0.4.34`, `md-5 0.11.0`, `parking_lot 0.12.5`, `parking_lot_core 0.9.12`, `phf 0.13.1`, `phf_shared 0.13.1`, `postgres-protocol 0.6.12`, `postgres-types 0.2.14`, `rand 0.10.3`, `rand_core 0.10.1`, `scopeguard 1.2.0`, `sha2 0.11.0`, `siphasher 1.0.4`, `stringprep 0.1.5`, `tinyvec 1.13.3`, `tokio-postgres 0.7.18`, `tokio-util 0.7.19`, `unicode-bidi 0.3.18`, `unicode-normalization 0.1.25`, `unicode-properties 0.1.4`, `whoami 2.1.3` (plus `graph-store` itself). Every external dependency is pinned with `=` (`server/graph-server/Cargo.toml:13`).
- Measured on the same day: `GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path <scratch>/server/Cargo.toml --config deny.toml check advisories bans licenses sources` exits 0 (`advisories ok, bans ok, licenses ok, sources ok`) with this dependency set. It re-reports `multiple-versions = "warn"` for the new duplicates `sha2 0.11` / `digest 0.11` / `block-buffer` / `crypto-common` / `cpufeatures` and for `wasi` (named by `postgres-protocol` and `whoami`); `hashbrown`'s duplicate predates the store. **A `cargo deny` failure on any added crate is a stop**, not a warning to waive.
- `scripts/orch/gr` has no network option and its containers sit on Docker's default bridge, so a test container reaches PostgreSQL by the container's **bridge IP**, never by `127.0.0.1` and never by a host port. Every container is created through `scripts/orch/drun`; `scripts/orch/drun-check.sh:14` fails the tree on a bare `docker run`.
- Workspace and plugin ids `[a-z0-9][a-z0-9-]{0,62}`; collection ids `[A-Za-z0-9_-]{1,64}`, qualified `<plugin>.<collection>`; record ids non-empty and without `:`. `seq`, `rev` and `epoch` stop at 2^53 − 1.
- Values are stored as the qualified **canonical text** graph-contract's writer produces, never as `jsonb`: `-0`, 2^53 − 1 and key order come back byte-identical. Equality of a batch's write is decided by comparing SHA-256 of that text, so an identical upsert is a no-op: no rev, no seq, no notice.
- `seq` comes from `UPDATE workspaces SET head_seq = head_seq + 1 ... RETURNING head_seq` under the workspace row lock taken first (§5.1 step 1). No `SEQUENCE`, anywhere, ever.
- House limits: ≤ 40 lines per function, ≤ 4 parameters (a request struct when more would be needed), ≤ 300 lines per file; `BTreeMap` never `HashMap` in anything hashed or ordered; a doc comment on every `pub` item; clippy `-D warnings`; a `Caveat:` line on every heuristic, timeout and retry (there is exactly one retry, the 40P01/40001 one).
- No route, no key file, no semaphore, no SSE, no `event: busy`, no `/layout`: those are slice 3 (`server/graph-hub`). graph-store exports types and functions, never an HTTP status.
- Never touch `server/graph-server/**`, `crates/**`, `docs/contract/delta.md`, `docs/contract/service-api.md`, `push-once.sh`, `push-loop.sh`, `.claude/rules/devil/`. Never open `/goinfre/dlesieur/refs/igraph-*`.

## Decisions recorded here (spec silent, illustrative, or impossible as written)

1. **How the tests reach PostgreSQL.** `scripts/orch/hub-pg.sh` builds the image from `deploy/postgres.Dockerfile` and runs it through `drun` with a named data volume and a named archive volume, prints `postgres://hub:…@<bridge-ip>:5432/hub` for `url`, and every `run` verb passes it as `GM_HUB_PG_URL`. A test reads that one variable and **panics** when it is unset — never skips; a skipped test is not a pass, and condition (c) is what forces the feature gate instead.
2. **Where the tests live.** `server/graph-store/tests/`, one file per concern, every file starting `#![cfg(feature = "db-tests")]`. `db-tests` is a plain feature, never a dev-dependency edge, so `svc-test`'s `--workspace` compiles empty test binaries and passes with no database. The rows pass `--features db-tests`; the `negctl` breaks come from a dev-dependency on graph-contract with its `negctl` feature, the trick `server/graph-server/Cargo.toml:41` already uses.
3. **`tokio-postgres` features: `default-features = false, features = ["runtime"]`**, the probe's set. No `with-serde_json-1` (the store reads and writes canonical text, never JSON through the driver), no `with-chrono-*`, no TLS backend. `runtime` is the only feature the store needs, and it pulls `tokio/net`, which graph-server already enables, which is why condition (b) held in the probe. Any change to this set re-runs condition (b) and `cargo deny` before it lands, and the plan says so in the task that makes it.
4. **Migrations: SQL files under `server/graph-store/sql/`, embedded with `include_str!`, applied in file-name order, each in its own transaction, each recorded in `hub_migrations(name, sha256, applied_at)` with its SHA-256.** No migration crate. An applied file whose bytes changed is refused, which is how the schema is pinned to the code.
5. **Keyset pages, not portals.** §5.3 says the fetch size goes "through `client.bind` and `query_portal`". `tokio_postgres::Client` at the pinned 0.7.18 has no `bind` and no `query_portal` (`docs.rs/tokio-postgres/0.7.18/tokio_postgres/struct.Client.html`: `query_raw`, `query_typed_raw`, `execute*`, `batch_execute`, `build_transaction`, `cancel_token`, `is_closed`). The materializer therefore pages with `(qcoll, id) > ($last_qcoll, $last_id) ORDER BY qcoll COLLATE "C", id COLLATE "C" LIMIT $fetch_rows` and reads the page with `query_raw`, and the anti-join with the same shape on `(src_qcoll, src_id, field)`. The memory bound §6 names is the same: one page of `GRAPH_HUB_FETCH_ROWS` rows is what the server materialises and what the driver buffers. Caveat: a page re-plans its query, so a 64 MiB workspace at 32 rows a page is many plans; `hub-memory` (slice 3) measures it and `GRAPH_HUB_FETCH_ROWS` is the knob.
6. **Reading one stored record back.** Slice 1's readers are `pub(crate)` and this slice may not edit `crates/**`, so graph-store reads a stored canonical text back with `canonical_json::parse` (a public module of graph-contract) into the `Upsert`/`Batch` slice 1 exposes, and prunes with `hub::prune::{kept_collection, prune_record}` plus `record_piece`. Caveat: this mirrors the record member names of the wire; a hub-wire change that renames one breaks it, and `hub-materialize` (Task 9) is what catches that.
7. **A restored record's `Record` comes from the upsert path, not from SQL.** The materializer needs `rev` and `updatedAt`, which only the stored text carries, so the text is the input. `doc_bytes` is kept by the writer in the same transaction (§6), never recomputed by a scan.
8. **The image pins the major line, and the row records the exact one.** `postgres:17`, because the probe's digest is unpinned and this slice needs the same major; the row `hub-pg-durability` reads `server_version` and records it in `docs/measurements/hub-pg-epoch-probe/store-image.md`, so a future bump is visible. Caveat: `postgres:17` moves within the line, so two runs can differ by patch level; nothing in this slice depends on a patch level.
9. **Retention is a count and a byte sum, pruned in the committing transaction** (§5.1 step 6), never by a background job. A cursor below what is kept is 410 (`CursorState::Gone`), which `changes()` returns as a value, not an error.

## Review Focus

1. **Epochs are microseconds, and the run-ahead is microseconds too.** `hub_next_epoch()` floors at `clock_timestamp() * 1e6`, so N epochs drawn inside one microsecond leave `last` N µs ahead; a millisecond reading would put 2026 near 1.8e12 and a ms run-ahead would let two hubs draw the same epoch after a restore (R6). Pinned by `epoch_is_microseconds` and `epoch_run_ahead_is_microseconds` (Task 5).
2. **The trigger guard is `hub.writer = '1' OR pg_trigger_depth() > 1`**, so the batch, the manifest PUT, the workspace create, the sweeper and the detector move no epoch, and the trigger on `workspaces` does not fire on its own update. Pinned by `hub_write_paths_move_no_epoch` and `workspaces_update_does_not_bump_itself` (Task 5).
3. **The detector must not bump a healthy database under write load.** The high-water is snapshotted *before* the LSN read and lowered only after `COMMIT`, under a mutex held across the detector (S2). Pinned by `detector_match_under_four_writers_bumps_nothing`, and its negative control `hw-after-lsn` (Task 6).
4. **`doc_bytes` bounds the streamed document from above and equals it exactly when nothing was pruned** — head, one separator per collection and per record, tail (N8). Pinned by `doc_bytes_bounds_the_document` and `doc_bytes_equals_the_streamed_length_when_nothing_is_pruned` (Task 9).
5. **An identical upsert takes no seq.** The decision is a SHA-256 of the canonical text against `text_sha256`, so `-0` vs `0` or a reordered map is a real change and a byte-identical resend is not; a gap in `1..N` under 100 writers is a bug either way. Pinned by `identical_upsert_takes_no_seq` and `hundred_writers_have_no_gap` (Task 7).

---

### Task 1: `default-members` and the two tree baselines

**Files:**
- Modify: `server/Cargo.toml` (one line after `members` on line 7)
- Create: `scripts/orch/rows/hub-store.rows`

**Interfaces:**
- Produces: `default-members = ["graph-server"]` in `server/Cargo.toml`, so a build at the virtual root selects graph-server alone and graph-store's `tokio` features never unify into the shipped binary (H2, D13).
- Consumes: the pre-slice-2 baseline, reconstructed in a scratch copy from `git show f1a23521:server/…` (`f1a23521` = develop on 2026-10-05). Nothing is stored in the tree: the row rebuilds the baseline from git, so it cannot rot.

- [ ] **Step 1: Write the rows file with the two tree rows and their controls, then run the gate RED.**

```
# hub-store.rows — slice 2 (hub-store). §8 row map is at the top of the file (Task 10 writes the final copy).
hub-virtual-root|0|base=f1a23521; d=target/hub-tree; git cat-file -e $base^{commit} || { echo "hub-virtual-root: $base is not in this clone" >&2; exit 2; }; rm -rf $d; mkdir -p $d/base/server/graph-server; ln -s $(realpath --relative-to=$d/base crates) $d/base/crates; ln -s $(realpath --relative-to=$d/base/server/graph-server server/graph-server/src) $d/base/server/graph-server/src; git show $base:server/Cargo.toml >$d/base/server/Cargo.toml; git show $base:server/Cargo.lock >$d/base/server/Cargo.lock; git show $base:server/graph-server/Cargo.toml >$d/base/server/graph-server/Cargo.toml; strip() { sed -e 's| (/[^)]*)$||' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/base-server.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/new-server.txt; diff -u $d/base-server.txt $d/new-server.txt || { echo "hub-virtual-root: condition (b) moved" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e features --locked --prefix none | strip >$d/base-root.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/new-root.txt; diff -u $d/base-root.txt $d/new-root.txt
negctl-hub-virtual-root|0|base=f1a23521; d=target/hub-tree; rm -rf $d/ctl; mkdir -p $d/ctl/server/graph-server; ln -s $(realpath --relative-to=$d/ctl crates) $d/ctl/crates; ln -s $(realpath --relative-to=$d/ctl/server/graph-server server/graph-server/src) $d/ctl/server/graph-server/src; cp server/Cargo.lock $d/ctl/server/Cargo.lock; cp server/graph-server/Cargo.toml $d/ctl/server/graph-server/Cargo.toml; grep -v '^default-members' server/Cargo.toml >$d/ctl/server/Cargo.toml; sed -i 's|^tokio = { version = "=1.53.2", features = |tokio = { version = "=1.53.2", features = ["fs", |' $d/ctl/server/graph-server/Cargo.toml; grep -q '"fs"' $d/ctl/server/graph-server/Cargo.toml || { echo "negctl-hub-virtual-root: the injected feature is gone" >&2; exit 1; }; strip() { sed -e 's| (/[^)]*)$||' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/ctl/server/Cargo.toml -e features --locked --prefix none | strip >$d/ctl-root.txt; diff -q $d/base-root.txt $d/new-root.txt || { echo "negctl-hub-virtual-root: the real tree moved, so this control proves nothing" >&2; exit 1; }; diff -u $d/base-root.txt $d/ctl-root.txt >$d/ctl.diff; test -s $d/ctl.diff && grep -q 'tokio feature "fs"' $d/ctl.diff
```

`scripts/orch/gate.sh target/gate-hub-store scripts/orch/rows/hub-store.rows` → `hub-virtual-root` **FAIL** (no `default-members`, so the virtual root is not pinned and nothing in the row's contract is met) and `negctl-hub-virtual-root` **FAIL** (the two trees are identical, so the control is vacuous).

- [ ] **Step 2: Add the line.** In `server/Cargo.toml`, directly under `members`:

```toml
# Only graph-server is built at the virtual root: graph-store's `tokio` features must never
# unify into the shipped binary (spec H2, D13). Row `hub-virtual-root` diffs both trees.
default-members = ["graph-server"]
```

- [ ] **Step 3:** rerun the gate → both rows PASS. Commit `git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -qm updated`.
- [ ] **Step 4: Upgrade the control to the §8 form.** Task 3 replaces the `negctl-hub-virtual-root` body with the R5 version: the scratch copy has no `default-members` and **graph-store's own** `tokio` edge carries `features = [… , "fs"]` (`tokio/fs` is absent from graph-server's edge list at `server/graph-server/Cargo.toml:29`, which is what makes the injection bite), and the row asserts the real tree still equals the baseline, the scratch tree does not, and the differing lines name both `graph-store` and `tokio feature "fs"`. The slice report names the injected feature and shows it absent from the baseline's graph-server subtree (condition 11, §16 condition 18).

### Task 2: the D7 scratch copies, over the workspace members

**Files:**
- Modify: `scripts/orch/lock-parity.sh` (`scratch_setup`, `:58-74`), `scripts/orch/svc-features.sh` (the inline copy/link block, `:66-73`)
- Modify: `scripts/orch/rows/hub-store.rows` (add `scratch-members` and its control)

**Interfaces:**
- Produces: both scripts copy every member's `Cargo.toml` and link every member's `src` at a computed relative path, read from `server/Cargo.toml`'s `members` list. Nothing else in either script changes; `--break-version`, `--break-feature` and `--break` keep their behaviour and their messages.

- [ ] **Step 1: The failing test — a two-member scratch copy.** Build `target/scratch-members/server/` exactly as `scratch_setup` does — manifests copied, `crates/` and each member's `src` linked with `realpath --relative-to` — but with `members = ["graph-server", "graph-store"]` and a stub `graph-store/Cargo.toml` + `src/lib.rs`. Run `scripts/orch/lock-parity.sh --break-version` with that copy in place of `server/`: it exits 2 with cargo's `failed to read …/graph-store/Cargo.toml`, because `scratch_setup` copies only graph-server. Same for `svc-features.sh --break`.
- [ ] **Step 2: Make `scratch_setup` loop.** In `lock-parity.sh`, replace the body of `scratch_setup` (`:58-74`) with:

```bash
scratch_setup() {
  local scratch=$1 link member
  rm -rf "$scratch" || return 2
  mkdir -p "$scratch/server" || return 2
  # WHY the member list is read from the manifest and not written out: a second member
  # (server/graph-store) joined `server/Cargo.toml` and this copy then named one member, so
  # the copy was a workspace cargo cannot resolve -- and the error reads like a broken
  # workspace, not a stale list.
  for member in $(sed -n 's/^members = \[\(.*\)\]$/\1/p' server/Cargo.toml | tr -d '"' | tr ',' ' '); do
    mkdir -p "$scratch/server/$member" || return 2
    cp "server/$member/Cargo.toml" "$scratch/server/$member/" || return 2
    link=$(realpath --relative-to="$scratch/server/$member" "$root/server/$member/src") || return 2
    ln -s "$link" "$scratch/server/$member/src" || return 2
  done
  link=$(realpath --relative-to="$scratch" "$root/crates") || return 2
  ln -s "$link" "$scratch/crates" || return 2
  cp server/Cargo.toml server/Cargo.lock "$scratch/server/" || return 2
}
```

Each function stays under 40 lines; the WHY block above the loop replaces the two per-link WHY blocks it subsumes and keeps the mount reason on record.

- [ ] **Step 3: The same loop in `svc-features.sh`.** Factor its inline block (`:66-73`) into a `scratch_setup` function with the body above, call it where the `rm -rf`/`mkdir`/`ln`/`cp` lines stood, and leave the `--break` edge edit (`:77-82`) and the `graph-wasm` greps (`:92-97`) exactly as they are.
- [ ] **Step 4: Green again, with the controls still red for their own reason.** `scripts/orch/lock-parity.sh --break-version` and `--break-feature` and `scripts/orch/svc-features.sh --break` each exit 1 and print their original messages (`graph-wasm's normal closure after moving indexmap`, `graph-wasm's normal closure after adding probe`, `graph-wasm is linked with probe in …`); plain `lock-parity.sh` and `svc-features.sh` exit 0. Verify by grep, not by exit code alone.
- [ ] **Step 5: Rows.** Add to `scripts/orch/rows/hub-store.rows`:

```
scratch-members|0|d=target/scratch-members; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; printf '[package]\nname = "graph-store"\nversion = "0.1.0"\nedition = "2024"\n\n[dependencies]\n' >$d/server/graph-store/Cargo.toml; printf '' >$d/server/graph-store/src/lib.rs; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; sed -i 's|^members = .*|members = ["graph-server", "graph-store"]|' $d/server/Cargo.toml; scripts/orch/gr cargo tree --manifest-path $d/server/Cargo.toml -e features --prefix none >/dev/null && scripts/orch/lock-parity.sh --help >/dev/null && scripts/orch/svc-features.sh --help >/dev/null
negctl-scratch-members|0|d=target/scratch-members; test -d $d/server/graph-store || { echo "negctl-scratch-members: no two-member fixture" >&2; exit 2; }; rm -f $d/server/graph-store/Cargo.toml; scripts/orch/gr cargo tree --manifest-path $d/server/Cargo.toml -e features --prefix none >target/scratch-members-negctl.log 2>&1 && exit 1; grep -q 'graph-store/Cargo.toml' target/scratch-members-negctl.log && echo "negctl-scratch-members: the member loop left a listed member uncopied"
```

The control removes one member's manifest from a two-member copy and requires cargo to fail naming that file: it proves the copy step is load-bearing, so a green `scratch-members` is not a vacuous green. Commit.
- [ ] **Step 6: Send the diff.** `git diff -- scripts/orch/lock-parity.sh scripts/orch/svc-features.sh` goes to graph-render-4f before the slice lands, with the three controls' exit codes and grepped messages attached.

### Task 3: the crate skeleton, the image and `hub-pg.sh`

**Files:**
- Create: `deploy/postgres.Dockerfile`, `scripts/orch/hub-pg.sh`, `server/graph-store/Cargo.toml`, `src/lib.rs`, `src/config.rs`, `src/error.rs`, `src/breaks.rs`, `src/hooks.rs`, `src/store.rs`, `src/pool.rs`, `src/pool/{connect,last_seen}.rs`, `tests/support/mod.rs`, `tests/support/{db,step}.rs`, `tests/connect.rs`
- Modify: `server/Cargo.toml` (`members`, line 7), `server/Cargo.lock` (regenerated), `scripts/orch/rows/hub-store.rows` (Task 1's control upgrade, plus `hub-floor`, `clippy-store`, `hooks-gated-store` and their controls)

**Interfaces:**
- Produces: `graph_store::{Store, StoreError, StoreConfig, Detector, Hooks}`; `Store::connect(&StoreConfig) -> Result<Store, StoreError>`; `Store::ping(&self) -> Result<(), StoreError>`; `StoreConfig { url, pool, retain, retain_bytes, changes_bytes, fetch_rows, max_batch, max_body, max_record_bytes, max_plugin_bytes, max_doc_bytes, last_seen, timeout_ms, stream_deadline_ms, sweeper_interval_ms, idem_ttl_ms }` with `defaults()` (every §6 default, `url` empty) and `check()`; `StoreError::{Hub(HubError), Db(DbError), NoDatabase, Serialization{retried}, Busy{retry_after}, Gone, Eof}` with `retry_after() -> Option<u64>` and `code() -> &'static str`. The test support module: `tests/support/db.rs` (`url()` panics without `GM_HUB_PG_URL`, `open()`, `migrated()`, `reset_schema()`) and `tests/support/step.rs` (`signal(name)`, `await_done(name)`, `write(name, lines)` over `$GM_HUB_STEP_DIR`, the file handshake the container-level cases need — Caveat: it works because `scripts/orch/gr` bind-mounts the repository read-write at `/w`, so a path under `target/` is the same directory on the host and in the test container).
- `Hooks` (feature `test-hooks`, empty without it): `pause_after_lock()`, `pause_before_commit(seq)`, `pause_after_headers(seq)` — the three seams §5.1 and §5.3 need to force an interleaving. `breaks::on(name)` reads `GM_HUB_BREAK` (feature `negctl`), names: `sequence-seq`, `no-idem`, `changes-read-committed`, `sync-commit-unset`, `prune-own-transaction`, `no-trigger`, `trigger-enable-origin`, `one-trigger-origin`, `detector-at-start`, `checkpoint-timeline`, `lsn-only`, `hw-after-lsn`, `no-deadlock-retry`, `drop-record` (reserved for slice 3's relay, never read here).

```toml
# server/graph-store/Cargo.toml
[package]
name = "graph-store"
description = "graph-hub's PostgreSQL store: schema, epochs, the writer transaction, the streamed document."
version = "0.1.0"
edition = "2024"
rust-version = "1.98"
publish = false

[dependencies]
graph-contract = { path = "../../crates/graph-contract" }
# The version graph-server already pins (server/graph-server/Cargo.toml:27): no third copy of sha2.
sha2 = { version = "=0.10.9", default-features = false, features = ["std"] }
# `rt`, `sync` and `time` are already enabled by graph-server's edge (:29), so this edge adds
# no feature the virtual-root diff could see; `macros` is a dev-dependency only.
tokio = { version = "=1.53.2", features = ["rt", "sync", "time"] }
tokio-postgres = { version = "=0.7.18", default-features = false, features = ["runtime"] }
# `TryStreamExt` for `Client::query_raw`'s RowStream; already in the lock through graph-server.
futures-util = { version = "=0.3.34", default-features = false }

[dev-dependencies]
# graph-contract's own break names (`keep-dangling`, `keep-cells`) are behind ITS `negctl`
# feature. A dev-dependency turns it on for test targets only; no normal build sees it.
graph-contract = { path = "../../crates/graph-contract", features = ["negctl"] }
tokio = { version = "=1.53.2", features = ["rt-multi-thread", "macros", "time", "sync"] }

[features]
# The negative controls' server-side breaks (`src/breaks.rs`). Off in every shipped build.
negctl = []
# The test-only seams (`src/hooks.rs`): enabled by the rows, never by a build that needs no
# database. `#![cfg(feature = "db-tests")]` in every `tests/*.rs` is what keeps condition (c).
db-tests = []
test-hooks = []

[lints.rust]
missing_docs = "warn"
unsafe_code = "forbid"
```

- [ ] **Step 1: Failing tests, `tests/connect.rs`.** `migrations_run_and_are_recorded` (the two files are in `hub_migrations` with their hashes), `connect_pings`, `the_database_is_utf8_with_a_c_collation` (the §6 start check the store owns: `server_encoding = 'UTF8'`, `datcollate = 'C'` — `initdb --encoding=UTF8 --locale=C`, N12), `the_catalog_is_only_what_migration_one_names` (no table outside `sql/0001_schema.sql`), and `config_defaults_match_section_6` (every default in §6 spelled out as a literal, so a spec change that renames a default is a failing test here). Each begins with `let url = support::db::url();` — `support::db::url()` **panics** with `set GM_HUB_PG_URL (scripts/orch/hub-pg.sh url)` when the variable is unset.
- [ ] **Step 2:** `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests --test connect` → FAIL (no `lib` target, no `GM_HUB_PG_URL`).
- [ ] **Step 3: `deploy/postgres.Dockerfile`.** One file, no COPY (the build context is `deploy/` and nothing else there is ours to add):

```dockerfile
FROM postgres:17
# The hub's settings, appended to $PGDATA/postgresql.conf by the entrypoint on first init, so a
# fresh volume and a rebuilt image agree. `archive_mode` is what makes hub-epoch's promotion and
# point-in-time-recovery cases possible at all; /archive is a named volume hub-pg.sh mounts.
RUN set -eu; printf '%s\n' \
  'fsync = on' \
  'synchronous_commit = on' \
  'full_page_writes = on' \
  "listen_addresses = '*'" \
  'wal_level = replica' \
  'max_wal_senders = 10' \
  'hot_standby = on' \
  'archive_mode = on' \
  "archive_command = 'test ! -f /archive/%f && cp %p /archive/%f'" \
  > /docker-entrypoint-initdb.d/10-hub.conf.sh \
  && printf '#!/bin/sh\ncat <<EOF >> "$PGDATA/postgresql.conf"\n%s\nEOF\n' "$(cat /docker-entrypoint-initdb.d/10-hub.conf.sh | tail -n +4)" > /docker-entrypoint-initdb.d/10-hub.conf.sh \
  && chmod +x /docker-entrypoint-initdb.d/10-hub.conf.sh
```

- [ ] **Step 4: `scripts/orch/hub-pg.sh`** (`chmod 0755`). Header ≤ 30 lines (what it is, `start|stop|kill|reset|url|ip|wait|sql|copy-data|restore-data|switch-wal|replica|replica-promote|pitr|run`, the volume names, exit codes 0/1/2, and the Caveat that `run` needs the repo bind-mounted for the step directory). Harness ≤ 90 lines: `image` (build `gm-hub-pg:17` with `docker build -q -f deploy/postgres.Dockerfile`, `docker build` is not banned by `drun-check.sh:14`), `start` (`scripts/orch/drun --name gm-hub-pg -d -e POSTGRES_PASSWORD=hub -v gm-hub-pg-data:/var/lib/postgresql/data -v gm-hub-pg-archive:/archive gm-hub-pg:17`, or `docker start gm-hub-pg` when it exists), `wait` (`scripts/orch/gr cargo` is not needed: a `docker exec gm-hub-pg pg_isready -U postgres` poll, bounded, Caveat on the bound), `ip` (`docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}'`), `url` (`postgres://hub:hub@$(ip):5432/hub`), `sql` (`scripts/orch/drun --rm --network container:gm-hub-pg gm-hub-pg:17 psql -v ON_ERROR_STOP=1 -U postgres -d hub -c "$1"`), `copy-data`/`restore-data` (a `tar` through `scripts/orch/drun -v …:/from -v …:/to`), `switch-wal`, `replica`/`replica-promote`/`pitr` (write `standby.signal`/`recovery.signal` and the recovery lines into the volume through `scripts/orch/drun -v …:/data`, then start or promote), `reset` (remove container and volumes), `run` (`scripts/orch/gr -e GM_HUB_PG_URL="$(url)" -e GM_HUB_BREAK="${GM_HUB_BREAK-}" -e GM_HUB_STEP_DIR=target/hub-steps cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl "$@"`). Total under 300 lines; `scripts/orch/drun-check.sh` exits 0.
- [ ] **Step 5: Implement `src/` skeleton.** `lib.rs` (module list + `pub use`), `breaks.rs` (the table above, `#[cfg(feature = "negctl")] pub fn on` / `#[cfg(not)] pub const fn on(_) -> bool { false }`, the copy of `crates/graph-contract/src/hub/breaks.rs` shape), `hooks.rs` (the three no-op seams), `error.rs` (`StoreError` mapping `DbError` by `code()`: `40001` → `Serialization { retried: false }`, `40P01` likewise, `23505` → `Hub(Conflict)`, everything else `Db`), `config.rs`, `pool.rs` (`Pool::connect` + the `Detector` handle, one `tokio::sync::Mutex<Guard>` shared by every connection), `store.rs` (`Store { pool: Pool, detector: Arc<Detector> }` + `ping`). Each file under 300 lines, each function under 40, every `pub` item documented.
- [ ] **Step 6:** `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests --test connect` → PASS. Then `scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast` (condition (c)) → PASS with PostgreSQL **stopped** (`scripts/orch/hub-pg.sh stop` first). Commit.
- [ ] **Step 7: Rows** (the Task 1 control upgrade, plus the floor rows):

```
hub-floor|0|scripts/orch/gr cargo fmt --manifest-path server/Cargo.toml --all --check && scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings && scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast
negctl-hub-floor|0|d=target/hub-floor; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-store/Cargo.toml $d/server/graph-store/; cp server/graph-store/src/lib.rs $d/server/graph-store/src/; printf 'pub fn unformatted( ) ->u8{7}\n' >>$d/server/graph-store/src/lib.rs; scripts/orch/gr cargo fmt --manifest-path $d/server/Cargo.toml --all --check
clippy-store|0|scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml -p graph-store --all-targets --features db-tests,negctl,test-hooks -- -D warnings
negctl-clippy-store|0|d=target/hub-clippy; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-store/Cargo.toml $d/server/graph-store/; cp -r server/graph-store/src/. $d/server/graph-store/src/; printf '/// Undocumented on purpose.\npub fn undocumented() {}\n' >>$d/server/graph-store/src/lib.rs; rc=0; scripts/orch/gr cargo clippy --manifest-path $d/server/Cargo.toml -p graph-store --all-targets --features db-tests,negctl,test-hooks -- -D warnings >target/hub-clippy-negctl.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'missing documentation' target/hub-clippy-negctl.log
hooks-gated-store|0|d=target/hooks-gated; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests,test-hooks --lib -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests --lib -- --print cfg >$d/plain.cfg 2>/dev/null && ! grep -qx 'feature="test-hooks"' $d/plain.cfg
negctl-hooks-gated-store|0|d=target/hooks-gated; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests --features test-hooks --lib -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg
```

Commit.

### Task 4: the schema and the migration runner

**Files:**
- Create: `server/graph-store/sql/0001_schema.sql`, `sql/0002_epoch.sql`, `src/migrate.rs`, `tests/migrate.rs`, `tests/support/schema.rs`

**Interfaces:**
- Produces: `migrate::{FILES, manifest, apply}` where `FILES: [(&str, &str); 2]` is `[(name, include_str!)]` in apply order and `manifest() -> Vec<(String, String)>` returns `(name, sha256)`; `apply(&mut Client) -> Result<u32, StoreError>` applies every unrecorded file in order, each in its own transaction, and records `(name, sha256, applied_at)` in `hub_migrations`.
- The tables (§4, §5.3): `workspaces`, `manifests`, `records`, `links`, `change_headers`, `change_ops`, `idempotency`, `epoch_clock`, `hub_meta`, `hub_migrations`. Only the first six carry a workspace column and therefore triggers (Task 5).

- [ ] **Step 1: Failing tests in `tests/migrate.rs`.** `migrations_are_recorded_with_their_hashes` (every row of `hub_migrations` matches `manifest()`, `applied_at` set), `apply_is_idempotent` (a second `apply` applies 0 and changes no row), `a_changed_migration_file_is_refused` (the stored sha differs → `StoreError::Db` with `MigrationHash`), `schema_has_exactly_the_named_tables` (the ten above, no others), `record_columns_are_exactly` (`ws, plugin, qcoll, id, rev, updated_at, text, text_sha256, text_bytes`), `records_scan_index_is_in_byte_order` (`pg_indexdef` of the primary key names `qcoll COLLATE "C"` and `id COLLATE "C"`), `doc_bytes_columns_are_bigint`, `no_table_stores_json` (no `jsonb` column anywhere in the ten tables), `hub_migrations_has_no_trigger`.
- [ ] **Step 2:** `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests --test migrate` → FAIL.
- [ ] **Step 3: `sql/0001_schema.sql`.** `workspaces(id text PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9-]{0,62}$'), epoch bigint NOT NULL CHECK (epoch >= 0), head_seq bigint NOT NULL DEFAULT 0, doc_bytes bigint NOT NULL DEFAULT 0, created_at timestamptz NOT NULL DEFAULT clock_timestamp())`; `manifests(ws text NOT NULL REFERENCES workspaces(id), plugin text NOT NULL, version int NOT NULL, text text NOT NULL, text_bytes bigint NOT NULL, decl_bytes bigint NOT NULL, plugin_bytes bigint NOT NULL DEFAULT 0, plugin_seq bigint NOT NULL DEFAULT 0, PRIMARY KEY (ws, plugin))`; `records(ws text NOT NULL REFERENCES workspaces(id), plugin text NOT NULL, qcoll text NOT NULL, id text NOT NULL, rev bigint NOT NULL CHECK (rev >= 1), updated_at int NOT NULL, text text NOT NULL, text_sha256 bytea NOT NULL, text_bytes bigint NOT NULL, PRIMARY KEY (ws, qcoll COLLATE "C", id COLLATE "C"))`; `links(ws text NOT NULL, src_qcoll text NOT NULL, src_id text NOT NULL, field text NOT NULL, target_qcoll text NOT NULL, target_id text NOT NULL, PRIMARY KEY (ws, src_qcoll, src_id, field, target_qcoll, target_id))`; `change_headers(ws text NOT NULL, seq bigint NOT NULL, plugin text NOT NULL, at timestamptz NOT NULL, kind text NOT NULL CHECK (kind IN ('batch','manifest')), bytes bigint NOT NULL, ops int NOT NULL, PRIMARY KEY (ws, seq))`; `change_ops(ws text NOT NULL, seq bigint NOT NULL, ord int NOT NULL, op text NOT NULL CHECK (op IN ('upsert','delete')), qcoll text NOT NULL, id text NOT NULL, rev bigint NOT NULL, text text NOT NULL, PRIMARY KEY (ws, seq, ord))`; `idempotency(ws text NOT NULL, plugin text NOT NULL, key text NOT NULL, body_sha256 bytea NOT NULL, response text NOT NULL, seq bigint NOT NULL, created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY (ws, plugin, key))`; `hub_migrations(name text PRIMARY KEY, sha256 text NOT NULL, applied_at timestamptz NOT NULL DEFAULT clock_timestamp())`; plus `CREATE INDEX links_target ON links (ws, target_qcoll, target_id)` and `CREATE INDEX idempotency_created ON idempotency (created_at)`. Every `id`/`plugin` carries the §4 grammar as a `CHECK`, and every `seq`/`rev`/`epoch` a `CHECK (<= 9007199254740991)`.
- [ ] **Step 4: `sql/0002_epoch.sql`.** `epoch_clock(one bool PRIMARY KEY CHECK (one), last bigint NOT NULL)`, one row; `hub_meta(one bool PRIMARY KEY CHECK (one), system_identifier bigint NOT NULL, timeline text NOT NULL, datoid oid NOT NULL)`; and the function verbatim from §5.3:

```sql
CREATE OR REPLACE FUNCTION hub_next_epoch() RETURNS bigint LANGUAGE sql AS $$
  UPDATE epoch_clock SET last = greatest(last + 1,
    (extract(epoch FROM clock_timestamp()) * 1000000)::bigint)
  RETURNING last
$$;
```

with `INSERT INTO epoch_clock(one, last) VALUES (true, 0)` and the epoch floor documented above it as microseconds (R6).

- [ ] **Step 5: `src/migrate.rs`.** `apply`: `BEGIN`, `CREATE TABLE IF NOT EXISTS hub_migrations …` on a connection with no transaction, then for each `FILES` entry in order `BEGIN` → `execute(sql)` → `INSERT INTO hub_migrations … ON CONFLICT (name) DO NOTHING` when the recorded sha equals the file's, or `Err(StoreError::Db(DbError::new("migration hash")))` when it differs → `COMMIT`; the count of files applied is the return value. `sha256` is `sha2::Sha256` over the file's bytes (`std::fmt` hex by hand — no hex crate).
- [ ] **Step 6:** `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start` then the `migrate` test → PASS. Commit.

### Task 5: the epoch triggers

**Files:**
- Modify: `server/graph-store/sql/0002_epoch.sql` (four trigger functions, 24 triggers)
- Create: `src/epoch.rs`, `tests/epoch.rs`
- Modify: `scripts/orch/rows/hub-store.rows` (`hub-epoch-trigger` + three controls)

**Interfaces:**
- Produces: `epoch::{bump_now, head_of, workspace_ids}` (`bump_now(&mut Client) -> Result<u64, StoreError>` calls `SELECT hub_next_epoch()`); the trigger functions `hub_bump_insert/update/delete/truncate()` and 24 triggers named `hub_<table>_<ins|upd|del|trunc>`, all `ENABLE ALWAYS`, one per event per trigger table.
- Trigger tables and their workspace column (the name is `TG_ARGV[0]`): `workspaces` → `id`; `manifests`, `records`, `links`, `change_headers`, `change_ops` → `ws`. **No trigger** on `epoch_clock`, `hub_meta`, `idempotency` or `hub_migrations`.

- [ ] **Step 1: Failing tests in `tests/epoch.rs`.** `epoch_is_microseconds` (an epoch is above 1.7e15 and below 2^53 on a live database), `epoch_run_ahead_is_microseconds` (ten `hub_next_epoch()` calls inside one transaction leave `last` 10 µs ahead, never 10 ms), `manual_insert_update_delete_truncate_copy_move_the_epoch` (one test, one statement per table group, plus `COPY records FROM STDIN` through `copy_in`), `replica_role_write_moves_the_epoch_for_every_event` (four statements, each with `SET session_replication_role = replica`), `trigger_catalog_is_exactly_four_per_table_all_always` (`pg_trigger` holds 24 rows named `hub_%`, four per table, every `tgenabled = 'A'`; zero on the other four tables), `workspace_delete_and_recreate_draws_a_larger_epoch`, `hub_write_paths_move_no_epoch` (a workspace create, a manifest PUT and an all-no-op batch each leave `epoch` alone), `workspaces_update_does_not_bump_itself` (an `UPDATE workspaces SET head_seq = …` outside the guard leaves `epoch` alone).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: The functions.** `hub_bump_insert()` uses `REFERENCING NEW TABLE AS n`, `hub_bump_update()` uses `OLD TABLE o NEW TABLE n`, `hub_bump_delete()` uses `OLD TABLE o`, and `hub_bump_truncate()` bumps every workspace. All four begin with the same guard:

```sql
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN
    RETURN NULL;
  END IF;
```

and then call `PERFORM hub_next_epoch()` once per distinct workspace id. One function per event, because PostgreSQL 17 refuses transition tables on a multi-event trigger (§5.3). Every trigger is `CREATE TRIGGER … AFTER <event> ON <table> REFERENCING … FOR EACH STATEMENT EXECUTE FUNCTION …` with `ENABLE ALWAYS` written as `ALTER TRIGGER … ENABLE ALWAYS` after creation, since `CREATE TRIGGER` has no such clause.
- [ ] **Step 4:** tests PASS; the `migrate` test still passes on a reset volume (the new DDL is in the file, and its hash changes, so the row resets the volume first — `hub-pg.sh reset`). Commit.
- [ ] **Step 5: Rows** (`hub-epoch-trigger` runs the trigger test set; the three controls break exactly the three catalog claims):

```
hub-epoch-trigger|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch
negctl-no-trigger|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
negctl-trigger-enable-origin|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=trigger-enable-origin scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
negctl-one-trigger-origin|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=one-trigger-origin scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
```

`trigger-enable-origin` writes `ENABLE ORIGIN` instead of `ENABLE ALWAYS` on all 24; `one-trigger-origin` leaves exactly one on `ENABLE ORIGIN`, so the catalog assertion and the replica-role `UPDATE` case both fail. Commit.

### Task 6: the restore detector, the high-water and the last-seen map

**Files:**
- Create: `src/pool/detector.rs`, `src/pool/last_seen.rs`, `tests/detector.rs`
- Modify: `src/pool.rs` (call the detector before a connection joins the pool)
- Modify: `scripts/orch/rows/hub-store.rows` (`hub-epoch-detector` + four controls)

**Interfaces:**
- Produces: `Detector::run(&self, &mut Client) -> Result<DetectorOutcome, StoreError>` with `DetectorOutcome::{Match, Bumped { workspaces: u64 }}`; `Detector::commit_watermark(&self, &mut Client, ws: &str, epoch: u64, seq: u64)` — step 8's `pg_current_wal_flush_lsn()` read and the map raise, called on the **same connection** after `COMMIT`; `LastSeen` (a `BTreeMap<String, (u64, u64)>` plus a `VecDeque` recency list, `GRAPH_HUB_LAST_SEEN` entries, least recently used evicted first).
- Breaks read here: `detector-at-start`, `checkpoint-timeline`, `lsn-only`, `hw-after-lsn`.

- [ ] **Step 1: Failing tests in `tests/detector.rs`.** `detector_refuses_in_recovery`, `detector_refuses_fsync_off`, `detector_refuses_full_page_writes_off`, `detector_refuses_a_hub_writer_default` (`ALTER ROLE hub SET hub.writer = '1'`), `two_connections_opened_at_once_bump_once`, `detector_bumps_on_empty_hub_meta`, `detector_bumps_on_a_new_database_oid` (`CREATE DATABASE` as `editor`, then connect there), `detector_bumps_on_a_promotion` (handshake: the test signals `target/hub-steps/promotion.ready`, `hub-pg.sh replica` starts a standby on a copy of the data volume, `hub-pg.sh replica-promote` promotes it, the test resumes on the promoted server), `detector_bumps_on_a_point_in_time_recovery` (`hub-pg.sh pitr <target time>` on a copy, handshake around it), `detector_bumps_on_a_volume_snapshot_under_a_running_hub` (handshake; `copy-data`, `restore-data`, `start`), `detector_bumps_on_a_small_gap_crash_consistent_copy` (handshake; `kill -9` via `hub-pg.sh kill`, `copy-data`, `restore-data`, `start`, `hub-pg.sh switch-wal` **before** the reconnect, then the last-seen map is what catches it), `detector_match_under_four_writers_bumps_nothing` (4 writers committing while 8 connections open; the epoch must not move), `detector_match_alone_bumps_nothing`, `a_cursor_from_before_a_bump_is_gone` (`CursorState::Gone`), `last_seen_evicts_least_recently_used_first` (insert `GRAPH_HUB_LAST_SEEN + 1` ids and check the oldest is gone).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: `src/pool/detector.rs`, in §5.3's order.** Refusals first, read **outside** a transaction: `pg_is_in_recovery()`, `current_setting('fsync')`, `current_setting('full_page_writes')`, `current_setting('hub.writer', true)` (refused when neither null nor empty). Then, under the in-process mutex held until the high-water is set after `COMMIT`: `BEGIN` → `SELECT set_config('hub.writer','1',true), pg_advisory_xact_lock(HUB_LOCK)` → **snapshot** the high-water and the last-seen ids → read `hub_meta`, `pg_current_wal_flush_lsn()` **with the snapshotted high-water passed as a bound parameter**, and `SELECT id, epoch, head_seq FROM workspaces WHERE id = ANY($1)` over the snapshot's ids → mismatch (`no hub_meta row`, `a different system identifier / timeline / datoid`, `flush_lsn < high_water`, `a row below its map entry`, `a row missing while the map holds it`) → `UPDATE workspaces SET epoch = hub_next_epoch()` and upsert `hub_meta` → `COMMIT` → then set the high-water to the flush LSN read on that connection and clear the map. A failed commit leaves both, so the next connection bumps. `hw-after-lsn` moves the snapshot to after the LSN read; `checkpoint-timeline` reads `pg_control_checkpoint().timeline_id` instead of `substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8)`; `lsn-only` skips the map comparison; `detector-at-start` runs the detector once in `Pool::connect` instead of on every connection.
- [ ] **Step 4: `src/pool/last_seen.rs`.** `LastSeen { map: BTreeMap<String, (u64, u64)>, order: VecDeque<String>, cap: usize }` with `snapshot(&mut self) -> Vec<String>` (the ids, taken before any read), `raise(&mut self, ws, epoch, seq)` (entries only rise), `evict(&mut self)` (pop the front of `order` until `len <= cap`). Doc comment on each; the recency list is a `VecDeque`, never a `HashMap`.
- [ ] **Step 5:** tests PASS. Commit.
- [ ] **Step 6: Rows** (each control turns exactly one claim red):

```
hub-epoch-detector|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector
negctl-detector-at-start|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=detector-at-start scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-checkpoint-timeline|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=checkpoint-timeline scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-lsn-only|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=lsn-only scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-hw-after-lsn|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=hw-after-lsn scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
```

Commit.

### Task 7: the writer transaction

**Files:**
- Create: `src/writer.rs`, `src/writer/{apply,idempotency,bytes,retry}.rs`, `tests/writer.rs`
- Modify: `src/store.rs` (the three write entry points)
- Modify: `scripts/orch/rows/hub-store.rows` (`hub-seq`, `negctl-sequence-seq`, `hub-idem`, `negctl-no-idem`, `hub-epoch-deadlock`, `negctl-no-deadlock-retry`)

**Interfaces:**
- Produces: `Store::create_workspace(&self, ws: &str, limits: &Limits) -> Result<bool, StoreError>` (true = inserted, 201; false = existed, 200; no seq); `Store::put_manifest(&self, req: &ManifestWrite) -> Result<ManifestWritten, StoreError>` with `ManifestWrite { ws, plugin, manifest, limits }`; `Store::apply_batch(&self, req: &BatchWrite) -> Result<BatchOutcome, StoreError>` with `BatchWrite { ws, plugin, manifest, batch, idem, if_match, limits }`; `BatchOutcome { seq, applied, response }`; `ManifestWritten { status: u16, seq, growth: Growth }`.
- Consumes: graph-contract `Manifest`, `Batch`, `Upsert`, `Growth`, `growth()`, `Limits`, `check_collection_id`, `qualify`, `record_piece`, `change_json`, `manifest_change_json`, `answer_json`, `max_change`, `MAX_PLUGINS`, `MAX_SEQ`.
- Breaks: `sequence-seq`, `no-idem`, `sync-commit-unset`, `no-deadlock-retry`.

- [ ] **Step 1: Failing tests in `tests/writer.rs`.** `workspace_create_is_idempotent_and_draws_an_epoch`; `manifest_put_grows_and_refuses` (`Same` → 200 no seq; `Grown` → a seq and a `manifest` change; a removed field → 409; same version other content → 409; the 65th plugin → 413); `manifest_put_takes_the_workspace_lock_first` (a manual `SELECT … FOR UPDATE` on `workspaces` from another session makes the PUT wait, not deadlock); `batch_applies_and_answers` (`{"applied":2,"seq":118}`); `identical_upsert_takes_no_seq` (a resend of the same canonical text answers `applied: 0` with the current `head_seq`, and `rev` is unchanged); `a_changed_upsert_bumps_rev` (`-0` vs `0` and a reordered map both count as changes); `deleting_an_absent_record_is_a_noop`; `a_batch_with_one_bad_record_changes_nothing` (atomicity, read back through the materializer in Task 9); `hundred_writers_have_no_gap` (100 concurrent batches on one workspace: `1..=100`, no duplicate, `idempotency` rows present); `an_all_noop_batch_takes_no_seq`; `if_match_refuses_a_stale_plugin_seq` (412) `and_ignores_another_plugins_writes`; `idem_replay_returns_the_stored_response_and_no_seq`, `idem_same_key_other_body_is_422`, `idem_key_over_128_bytes_is_422`; `plugin_bytes_and_doc_bytes_caps_are_413`; `change_over_max_change_is_413`; `a_deadlock_ends_in_a_commit_or_a_503_never_a_500` (the pause hook after step 1, `ALTER ROLE hub SET deadlock_timeout = '200ms'` against the manual session's default, then assert `store.retry_count()` is above zero — 40P01, §16 condition 17).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: `src/writer.rs`, §5.1's eight steps, in order and nowhere else.** One explicit transaction per call: `BEGIN` → `SELECT set_config('hub.writer','1',true), set_config('synchronous_commit','on',true)` → `SELECT epoch, head_seq FROM workspaces WHERE id = $1 FOR UPDATE` → idempotency lookup (a hit returns the stored response and adds no seq) → `If-Match` against `plugin_seq` → apply (`SELECT qcoll, id, text_sha256 FROM records WHERE ws = $1 AND (qcoll, id) = ANY($2)`, then for each upsert whose SHA-256 differs: `INSERT … ON CONFLICT (ws, qcoll, id) DO UPDATE SET rev = records.rev + 1, …`, its `links` rows deleted and rewritten, `plugin_bytes`/`doc_bytes` updated and refused past the caps with a rollback) → when anything applied: `UPDATE workspaces SET head_seq = head_seq + 1 … RETURNING head_seq`, insert the change header and its operations, `UPDATE manifests SET plugin_seq = …` → prune (Task 10's `retention::prune`, called in the same transaction) → insert the idempotency row → `COMMIT` → step 8 on the same connection: `SELECT pg_current_wal_flush_lsn()` and `detector.commit_watermark(...)`. `synchronous_commit` is set unconditionally, not only when it reads `off` (S4). `create_workspace` and `put_manifest` run the same shape with `INSERT … ON CONFLICT (id) DO NOTHING` and the workspace row lock first (N9).
- [ ] **Step 4: `src/writer/retry.rs`, the one retry.** On `40P01` or `40001`, roll back and run the whole transaction **once** more; a second one is `StoreError::Serialization { retried: true }` → the hub's 503 with `Retry-After: 1`. A unique violation on the idempotency key also rolls back and retries once, and that retry finds the stored response at step 2. `retry_count` is an `AtomicU64` on `Store` so the row can assert it moved; `no-deadlock-retry` drops the retry and lets 40P01 reach the caller, so `a_deadlock_ends_in_a_commit_or_a_503_never_a_500` fails with a 500-class error and a zero counter.
- [ ] **Step 5: `src/writer/bytes.rs`.** `doc_bytes` for a workspace = Σ `text_bytes` over its records + Σ `decl_bytes` over its manifests + one separator per record and per collection + `frame_bytes(ws, collections, records)` (graph-contract). `create_workspace` seeds it with the head and tail lengths alone; `put_manifest` adds the declarations' bytes; `apply_batch` adds and subtracts the changed records'. `max_change` is checked against the change's `bytes` before the header row goes in.
- [ ] **Step 6:** tests PASS; `epoch` and `detector` still PASS. Commit.
- [ ] **Step 7: Rows.**

```
hub-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer
negctl-sequence-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=sequence-seq scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer; test $? -ne 0
hub-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer idem
negctl-no-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-idem scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer idem; test $? -ne 0
hub-epoch-deadlock|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && for i in 1 2 3 4 5 6 7 8 9 10; do scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer a_deadlock_ends_in_a_commit_or_a_503_never_a_500 || exit 1; done
negctl-no-deadlock-retry|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && ok=0; for i in 1 2 3 4 5 6 7 8 9 10; do GM_HUB_BREAK=no-deadlock-retry scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer a_deadlock_ends_in_a_commit_or_a_503_never_a_500 >/dev/null 2>&1 && ok=$((ok+1)); done; test $ok -eq 0
```

`sequence-seq` creates `change_seq_seq SEQUENCE`, draws the batch's seq from it instead of the row-locked `head_seq`, drops step 1's `FOR UPDATE`, and delays the commit of odd seqs through the `test-hooks` seam — `hundred_writers_have_no_gap` then fails on the gap a rolled-back batch leaves. Commit.

### Task 8: changes after a cursor, and the plugin's own records

**Files:**
- Create: `src/changes.rs`, `src/changes/{page,cursor}.rs`, `src/records.rs`, `tests/changes.rs`
- Modify: `scripts/orch/rows/hub-store.rows` (`hub-changes-snapshot`, `negctl-changes-read-committed`)

**Interfaces:**
- Produces: `changes::page(&Store, &ChangesReq) -> Result<ChangePage, StoreError>` with `ChangesReq { ws, since: Cursor, limit, max_bytes }` and `ChangePage { epoch, head_seq, next: Cursor, changes: Vec<Change>, bytes }`; `changes::cursor_state(&Store, ws, cursor) -> Result<CursorState, StoreError>` with `CursorState::{Valid, Gone}`; `Change { seq, plugin, at, kind: ChangeKind, upserts: Vec<ChangeOp>, deletes: Vec<ChangeOp> }`; `records::page(&Store, &RecordsReq) -> Result<RecordsPage, StoreError>` with `RecordsReq { ws, plugin, after: Option<(String, String)>, limit }` and `RecordsPage { plugin_seq: Cursor, rows: Vec<(String, String, u64)>, next: Option<(String, String)> }`.
- Breaks: `changes-read-committed`.

- [ ] **Step 1: Failing tests in `tests/changes.rs`.** `changes_after_a_cursor_are_in_seq_order`; `a_page_never_exceeds_max_bytes_but_returns_at_least_one_change`; `a_prune_between_the_header_and_operation_reads_still_yields_every_operation` (the `test-hooks` seam `pause_after_headers(seq)` makes a second session commit a prune, then the page is read and every header's operation count equals its stored `ops`); `a_header_whose_operation_count_differs_is_never_sent` (corrupt a row, expect `StoreError::Db` and no partial page); `cursor_below_low_is_gone`, `cursor_above_head_is_gone`, `cursor_from_another_epoch_is_gone`, `cursor_at_low_minus_one_is_valid`; `records_page_is_in_byte_order_and_carries_plugin_seq`; `records_page_next_cursor_terminates`.
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: `src/changes/page.rs`.** One `build_transaction().isolation_level(IsolationLevel::RepeatableRead).read_only(true).start()`; inside it, read the headers after `since` in seq order summing `bytes` until `max_bytes` (at least one), then `SELECT … FROM change_ops WHERE ws = $1 AND seq = ANY($2) ORDER BY seq, ord`, then check each header's row count against its stored `ops` and answer 500-class on a mismatch. The body of each change is built with graph-contract's `change_json(&ChangeHead { seq, plugin, at }, &upserts, &deletes)` and `manifest_change_json`, so the store never writes canonical change text itself. `changes-read-committed` opens the two reads at `READ COMMITTED` instead.
- [ ] **Step 4: `src/records.rs`.** `SELECT qcoll, id, rev FROM records WHERE ws = $1 AND plugin = $2 AND (qcoll COLLATE "C", id COLLATE "C") > ($3, $4) ORDER BY … LIMIT $5`, plus the plugin's `plugin_seq` and the workspace's `epoch` for the per-page cursor.
- [ ] **Step 5:** tests PASS. Commit.
- [ ] **Step 6: Rows.**

```
hub-changes-snapshot|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test changes
negctl-changes-read-committed|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=changes-read-committed scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test changes; test $? -ne 0
```

Commit.

### Task 9: the streamed materializer

**Files:**
- Create: `src/materialize.rs`, `src/materialize/{document,record,anti_join}.rs`, `tests/materialize.rs`, `tests/support/rng.rs` (splitmix64, the same generator slice 1 uses)
- Modify: `scripts/orch/rows/hub-store.rows` (`hub-materialize`, `negctl-keep-dangling`, `negctl-keep-cells`)

**Interfaces:**
- Produces: `materialize::open(&Store, ws) -> Result<Document<'_>, StoreError>`; `Document<'_> { head(), next(), tail() }` where `head()` returns `DOC_HEAD` plus the collection declaration pieces streamed from `manifests` in qualified-id order, `next()` returns `Option<String>` — one record piece per call — and `tail()` returns `doc_tail(ws)`; `Document::cursor() -> Cursor`. Every read is inside one `REPEATABLE READ, READ ONLY` transaction opened by `open`, which is what makes the document exactly the state at its ETag.
- Consumes: graph-contract `DOC_HEAD`, `DOC_MIDDLE`, `DOC_SEPARATOR`, `collection_piece`, `record_piece`, `doc_tail`, `frame_bytes`, `hub::prune::{kept_collection, prune_record}`, `Model`, `canonical_json`, `ingest::read`, `ingest::to_json`.

- [ ] **Step 1: Failing tests in `tests/materialize.rs`.** `document_bytes_equal_to_json_over_the_model` (the same records pushed through the store and through `graph_contract::hub::Model`, byte-identical); `document_is_byte_identical_at_the_same_cursor` (read twice at one `<epoch>.<seq>`, equal); `permuted_insertion_order_gives_the_same_bytes` (insert 40 records in 5 different orders, same bytes); `ingest_read_accepts_the_document` (`ingest::read(&doc)` is `Ok` and `to_json(read(doc)) == doc`); `doc_bytes_bounds_the_document` and `doc_bytes_equals_the_streamed_length_when_nothing_is_pruned`; `unresolved_links_are_pruned_and_the_edge_returns_when_the_target_arrives`; `a_link_to_an_unregistered_collection_drops_the_field_and_its_cells`; `an_originally_empty_list_stays_and_a_list_emptied_by_pruning_is_removed`; `a_record_with_nothing_to_prune_is_copied_verbatim` (the stored text appears byte-identical in the stream, which is what makes the fast path observable); `random_ops_materialize_canonically` (splitmix64 seeds 0..32, 30 register/upsert/delete steps each, cross-plugin links and deletes of linked records, asserting `ingest::read` is `Ok` and the document equals `to_json` over the model, after **every** step).
- [ ] **Step 2:** run, FAIL.
- [ ] **Step 3: `src/materialize/document.rs`.** `open` takes the snapshot, reads `epoch, head_seq, doc_bytes` and holds one plugin's kept-collection set at a time while streaming the declarations in qualified-id order. `next()` runs the records query of Decision 5, and for each row consults the merged unresolved tuple for that `(qcoll, id)`: no unresolved cell and no dropped field → `row["text"]` verbatim; otherwise `anti_join`'s `prune_text` rewrites it. The head, the separators and the tail come from graph-contract's constants, so the store is never a second producer.
- [ ] **Step 4: `src/materialize/anti_join.rs`.** The unresolved query of Decision 5, keyset-paged on `(src_qcoll, src_id, field)` in the same `COLLATE "C"` order, merged into the record scan. `prune_text(text, kept, exists) -> Result<Option<String>, StoreError>` returns `None` when nothing is pruned; otherwise it reads the record back (Decision 6), calls `prune_record`, and returns `record_piece`. The head comment carries the Caveat Decision 6 states.
- [ ] **Step 5:** tests PASS. Commit.
- [ ] **Step 6: Rows** (the two controls are slice 1's break names, forced on through the dev-dependency's `negctl`):

```
hub-materialize|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize
negctl-keep-dangling|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=keep-dangling scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize; test $? -ne 0
negctl-keep-cells|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=keep-cells scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize; test $? -ne 0
```

Commit.

### Task 10: retention, the sweeper, durability, the docs and the final rows

**Files:**
- Create: `src/retention.rs`, `src/sweeper.rs`, `tests/retention.rs`, `tests/durability.rs`, `docs/measurements/hub-pg-epoch-probe/store-image.md`
- Modify: `docs/decisions/server-dependencies.md` (the amendment), `scripts/orch/rows/hub-store.rows` (the whole file, below). `scripts/orch/drun-check.sh` is not touched — it is a row.

**Interfaces:**
- Produces: `retention::prune(&mut Transaction<'_>, ws, retain, retain_bytes) -> Result<u64, StoreError>` (headers and their operations deleted in one statement pair, inside the committing transaction); `sweeper::run(&Store, older_than_ms, batch: u64) -> Result<u64, StoreError>` and `sweeper::interval_ms(&StoreConfig) -> u64` (default 600 000).
- Breaks read in this task: `prune-own-transaction`, `sync-commit-unset` (the latter is honoured in `src/writer.rs` from Task 7 — this task only writes the test that catches it).

- [ ] **Step 1: Failing tests in `tests/retention.rs`.** `prune_by_count_keeps_the_newest`; `prune_by_bytes_keeps_the_newest` (whichever bound bites first); `prune_never_runs_in_its_own_transaction` (the caller's transaction is still open when `prune` returns — read `txid_current()`'s age through a second session); `a_pruned_cursor_is_gone`; `the_sweeper_deletes_rows_past_24h_in_bounded_batches` (5 000 rows seeded, `batch = 1000`, five calls, count and bound asserted); `the_sweeper_moves_no_epoch` (it opens with `set_config('hub.writer','1',true)`, Task 5's reverse check); `the_sweeper_never_deletes_a_fresh_row`.
- [ ] **Step 1b: Failing tests in `tests/durability.rs`.** `durability_settings_read_on` (all three of `fsync`, `synchronous_commit`, `full_page_writes`); `acked_seqs_are_written_before_the_kill` (200 batches on one workspace, then the acked seqs are written to `$GM_HUB_STEP_DIR/durability.acked`, one per line, through `support::step::write`); `every_acked_seq_is_present_after_a_restart` (reads that file and asserts every line is in `change_headers`); `every_acked_seq_is_present_with_synchronous_commit_off` (the same list, after `ALTER SYSTEM SET synchronous_commit = off` and a reload — the store's own step 1 still sets it on, so the acks stay). The handshake is the file: one test writes it, the next reads it, because only `hub-pg.sh` can kill the container.
- [ ] **Step 2:** `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start` then `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests --test retention --test durability` → FAIL.
- [ ] **Step 3: `src/retention.rs` and `src/sweeper.rs`.** `prune` deletes `change_ops` for the seqs it removes, then the headers, oldest first, until both bounds hold. `sweeper::run` is one explicit transaction per batch of `GRAPH_HUB_SWEEP_ROWS` rows: `BEGIN` → `set_config('hub.writer','1',true)` → `DELETE FROM idempotency WHERE (ws, plugin, key) IN (SELECT ws, plugin, key FROM idempotency WHERE created_at < clock_timestamp() - $1 * interval '1 millisecond' ORDER BY created_at LIMIT $2)` → `COMMIT`. Caveat, in the source: PostgreSQL has no `DELETE … LIMIT`, so the bounded delete names the rows in a subquery; a concurrent insert with an older `created_at` (an operator's clock) waits for the next batch. The 10-minute interval carries its own Caveat.
- [ ] **Step 4: `docs/decisions/server-dependencies.md` amendment.** Add to the table (after the `tokio-postgres` row at line 14), as a second block under a new `## Amendment: graph-store (slice 2, 2026-10-05)` heading so the 2026-09-29 text above it stays untouched: one row per crate group with its exact pinned version, what it is for, why the lower rung fails and its removal path — `tokio-postgres` 0.7.18 (the driver; `sqlx` needs a macro layer and compile-time database access), `postgres-protocol`/`postgres-types`/`fallible-iterator`/`byteorder` (its wire layer), `md-5`/`hmac`/`sha2 0.11`/`digest 0.11`/`crypto-common`/`block-buffer`/`hybrid-array`/`cpufeatures` (SCRAM and MD5 authentication inside `postgres-protocol`, which is why a second `sha2` major appears; removal: the driver, when it drops SCRAM), `base64`/`stringprep`/`unicode-normalization`/`unicode-bidi`/`unicode-properties` (SASLprep for SCRAM), `rand`/`rand_core`/`getrandom`/`chacha20`/`cmov` (its SCRAM nonce), `whoami` (its default `application_name`), `tokio-util`/`futures-sink` (its codec adapter), `phf`/`phf_shared`/`siphasher` (its query parsing), `parking_lot`/`lock_api`/`scopeguard` (its connection state), `log` (its tracing), `async-trait` (its `GenericClient`), `const-oid`/`ctutils`/`tinyvec` (its OID table), plus the two sentences that matter for a reviewer: the new `multiple-versions = "warn"` duplicates and that `cargo deny` exits 0 on this set. Keep the `## Rules` section as it stands.
- [ ] **Step 5: `docs/measurements/hub-pg-epoch-probe/store-image.md`.** The image tag, its `sha256`, `postgres --version`, `SHOW fsync / synchronous_commit / full_page_writes / server_encoding / lc_collate`, the `GRANT SET ON PARAMETER` lines `hub-pg.sh` applies, and the exact `hub-pg.sh` invocation each container-level case used. Raw outputs beside it under `docs/measurements/hub-pg-epoch-probe/`.
- [ ] **Step 6: Write the whole rows file** (this is the file the orchestrator gates; the content is exactly what ships):

```
# hub-store.rows — slice 2 (hub-store) of docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md (revision 5.1).
#
# §8 rows this slice owns, and where each is implemented here:
#   hub-virtual-root      -> hub-virtual-root, negctl-hub-virtual-root                    (Task 1, control upgraded in Task 3)
#   svc-supply            -> svc-supply, negctl-svc-supply-features, negctl-svc-supply-lock-version, negctl-svc-supply-lock-feature  (Task 2, Task 10)
#   hub-floor             -> hub-floor, negctl-hub-floor, clippy-store, negctl-clippy-store, hooks-gated-store, negctl-hooks-gated-store  (Task 3)
#   hub-epoch             -> hub-epoch-trigger (3 controls), hub-epoch-detector (4 controls), hub-epoch-deadlock (1 control)  (Tasks 5, 6, 7)
#   hub-seq               -> hub-seq, negctl-sequence-seq                                    (Task 7)
#   hub-changes-snapshot  -> hub-changes-snapshot, negctl-changes-read-committed            (Task 8)
#   hub-materialize       -> hub-materialize, negctl-keep-dangling, negctl-keep-cells       (Task 9)
#   hub-pg-durability     -> hub-pg-durability, negctl-sync-commit-unset                    (Task 10)
#   (the store's own retention and sweeper rows, which §8 folds into hub-idem's slice-3 half)
#                        -> hub-retention, negctl-prune-in-own-transaction                   (Task 10)
#   cargo-deny-server     -> cargo-deny-server, negctl-deny-license-server                  (Task 10)
#   D7 (svc-supply's diff) -> scratch-members, negctl-scratch-members                       (Task 2)
#   motor-lock, codegen --check, graph-core wasm32, the svc-* rows -> unchanged, owned by the
#   other slices; hub-floor re-runs the svc-* half on this tree.
#
# Every `negctl-*` row passes only when the thing it breaks goes RED for its own reason. The
# `; test $? -ne 0` idiom is scripts/orch/rows/service-supply.rows's. No row needs a hand: a
# database row starts its own PostgreSQL through scripts/orch/hub-pg.sh and stops it after.
# `scripts/orch/gr` has no network option, so the tests connect to the container's bridge IP,
# which scripts/orch/hub-pg.sh hands them as GM_HUB_PG_URL. A test with no GM_HUB_PG_URL fails.

drun-check|0|scripts/orch/drun-check.sh
hub-virtual-root|0|base=f1a23521; d=target/hub-tree; git cat-file -e $base^{commit} || { echo "hub-virtual-root: $base is not in this clone" >&2; exit 2; }; rm -rf $d; mkdir -p $d/base/server/graph-server; ln -s $(realpath --relative-to=$d/base crates) $d/base/crates; ln -s $(realpath --relative-to=$d/base/server/graph-server server/graph-server/src) $d/base/server/graph-server/src; git show $base:server/Cargo.toml >$d/base/server/Cargo.toml; git show $base:server/Cargo.lock >$d/base/server/Cargo.lock; git show $base:server/graph-server/Cargo.toml >$d/base/server/graph-server/Cargo.toml; strip() { sed -e 's| (/[^)]*)$||' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/base-server.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e normal,build --locked -p graph-server --prefix none | strip >$d/new-server.txt; diff -u $d/base-server.txt $d/new-server.txt || { echo "hub-virtual-root: condition (b) moved" >&2; exit 1; }; scripts/orch/gr cargo tree --manifest-path $d/base/server/Cargo.toml -e features --locked --prefix none | strip >$d/base-root.txt; scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked --prefix none | strip >$d/new-root.txt; diff -u $d/base-root.txt $d/new-root.txt
negctl-hub-virtual-root|0|base=f1a23521; d=target/hub-tree; rm -rf $d/ctl; mkdir -p $d/ctl/server/graph-server $d/ctl/server/graph-store/src; ln -s $(realpath --relative-to=$d/ctl crates) $d/ctl/crates; ln -s $(realpath --relative-to=$d/ctl/server/graph-server server/graph-server/src) $d/ctl/server/graph-server/src; cp server/Cargo.lock $d/ctl/server/Cargo.lock; cp server/graph-server/Cargo.toml $d/ctl/server/graph-server/Cargo.toml; printf '' >$d/ctl/server/graph-store/src/lib.rs; grep -v '^default-members' server/Cargo.toml | sed 's|^members = .*|members = ["graph-server", "graph-store"]|' >$d/ctl/server/Cargo.toml; cp server/graph-store/Cargo.toml $d/ctl/server/graph-store/; sed -i 's|features = \["rt", "sync", "time"\]|features = ["rt", "sync", "time", "fs"]|' $d/ctl/server/graph-store/Cargo.toml; grep -q '"fs"' $d/ctl/server/graph-store/Cargo.toml || { echo "negctl-hub-virtual-root: the injected feature is gone" >&2; exit 1; }; strip() { sed -e 's| (/[^)]*)$||' -e 's| (command-line)$||' | LC_ALL=C sort; }; scripts/orch/gr cargo tree --manifest-path $d/ctl/server/Cargo.toml -e features --locked --prefix none | strip >$d/ctl-root.txt; diff -q $d/base-root.txt $d/new-root.txt || { echo "negctl-hub-virtual-root: the real tree moved, so this control proves nothing" >&2; exit 1; }; diff -u $d/base-root.txt $d/ctl-root.txt >$d/ctl.diff; test -s $d/ctl.diff && grep -q 'graph-store v' $d/ctl.diff && grep -q 'tokio feature "fs"' $d/ctl.diff
scratch-members|0|d=target/scratch-members; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-store/Cargo.toml $d/server/graph-store/; ln -s $(realpath --relative-to=$d/server/graph-store/src) $d/server/graph-store/src; sed -i 's|^members = .*|members = ["graph-server", "graph-store"]|' $d/server/Cargo.toml; scripts/orch/gr cargo tree --manifest-path $d/server/Cargo.toml --locked -e features --prefix none >/dev/null && scripts/orch/lock-parity.sh --help >/dev/null && scripts/orch/svc-features.sh --help >/dev/null
negctl-scratch-members|0|d=target/scratch-members; test -d $d/server/graph-store || { echo "negctl-scratch-members: no two-member fixture" >&2; exit 2; }; rm -f $d/server/graph-store/Cargo.toml; scripts/orch/gr cargo tree --manifest-path $d/server/Cargo.toml -e features --prefix none >target/scratch-members-negctl.log 2>&1 && exit 1; grep -q 'graph-store/Cargo.toml' target/scratch-members-negctl.log && echo "negctl-scratch-members: a listed member was left uncopied"
hub-floor|0|scripts/orch/gr cargo fmt --manifest-path server/Cargo.toml --all --check && scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml --workspace --all-targets -- -D warnings && scripts/orch/gr cargo test --manifest-path server/Cargo.toml --workspace --no-fail-fast
negctl-hub-floor|0|d=target/hub-floor; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-store/Cargo.toml $d/server/graph-store/; cp server/graph-store/src/lib.rs $d/server/graph-store/src/; printf 'pub fn unformatted( ) ->u8{7}\n' >>$d/server/graph-store/src/lib.rs; scripts/orch/gr cargo fmt --manifest-path $d/server/Cargo.toml --all --check
clippy-store|0|scripts/orch/gr cargo clippy --manifest-path server/Cargo.toml -p graph-store --all-targets --features db-tests,negctl,test-hooks -- -D warnings
negctl-clippy-store|0|d=target/hub-clippy; rm -rf $d; mkdir -p $d/server/graph-server $d/server/graph-store/src; ln -s $(realpath --relative-to=$d crates) $d/crates; ln -s $(realpath --relative-to=$d/server/graph-server server/graph-server/src) $d/server/graph-server/src; cp server/Cargo.toml server/Cargo.lock $d/server/; cp server/graph-server/Cargo.toml $d/server/graph-server/; cp server/graph-store/Cargo.toml $d/server/graph-store/; cp server/graph-store/src/lib.rs $d/server/graph-store/src/; printf '/// Undocumented on purpose.\npub fn undocumented() {}\n' >>$d/server/graph-store/src/lib.rs; rc=0; scripts/orch/gr cargo clippy --manifest-path $d/server/Cargo.toml -p graph-store --all-targets --features db-tests,negctl,test-hooks -- -D warnings >target/hub-clippy-negctl.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'missing documentation' target/hub-clippy-negctl.log
hooks-gated-store|0|d=target/hooks-gated; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests,test-hooks --lib -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests --lib -- --print cfg >$d/plain.cfg 2>/dev/null && ! grep -qx 'feature="test-hooks"' $d/plain.cfg
negctl-hooks-gated-store|0|d=target/hooks-gated; mkdir -p $d && scripts/orch/gr cargo rustc --manifest-path server/Cargo.toml -p graph-store --profile test --features db-tests --features test-hooks --lib -- --print cfg >$d/forced.cfg 2>/dev/null && grep -qx 'feature="test-hooks"' $d/forced.cfg
hub-epoch-trigger|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch
negctl-no-trigger|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-trigger scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
negctl-trigger-enable-origin|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=trigger-enable-origin scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
negctl-one-trigger-origin|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=one-trigger-origin scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test epoch; test $? -ne 0
hub-epoch-detector|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector
negctl-detector-at-start|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=detector-at-start scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-checkpoint-timeline|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=checkpoint-timeline scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-lsn-only|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=lsn-only scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
negctl-hw-after-lsn|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=hw-after-lsn scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test detector; test $? -ne 0
hub-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer
negctl-sequence-seq|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=sequence-seq scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer; test $? -ne 0
hub-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer idem
negctl-no-idem|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=no-idem scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer idem; test $? -ne 0
hub-epoch-deadlock|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && for i in 1 2 3 4 5 6 7 8 9 10; do scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer a_deadlock_ends_in_a_commit_or_a_503_never_a_500 || exit 1; done
negctl-no-deadlock-retry|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && ok=0; for i in 1 2 3 4 5 6 7 8 9 10; do GM_HUB_BREAK=no-deadlock-retry scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test writer a_deadlock_ends_in_a_commit_or_a_503_never_a_500 >/dev/null 2>&1 && ok=$((ok+1)); done; test $ok -eq 0
hub-changes-snapshot|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test changes
negctl-changes-read-committed|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=changes-read-committed scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test changes; test $? -ne 0
hub-materialize|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize
negctl-keep-dangling|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=keep-dangling scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize; test $? -ne 0
negctl-keep-cells|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=keep-cells scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test materialize; test $? -ne 0
hub-retention|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test retention
negctl-prune-in-own-transaction|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GM_HUB_BREAK=prune-own-transaction scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test retention; test $? -ne 0
hub-pg-durability|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-pg.sh sql "SELECT string_agg(name || '=' || setting, ' ' ORDER BY name) FROM pg_settings WHERE name IN ('fsync','full_page_writes','synchronous_commit')" | grep -q 'fsync=on.*full_page_writes=on.*synchronous_commit=on' && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test durability durability_settings_read_on acked_seqs_are_written_before_the_kill && scripts/orch/hub-pg.sh kill && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test durability every_acked_seq_is_present_after_a_restart && scripts/orch/hub-pg.sh sql "ALTER SYSTEM SET synchronous_commit = off" && scripts/orch/hub-pg.sh sql "SELECT pg_reload_conf()" && scripts/orch/hub-pg.sh sql "SHOW synchronous_commit" | grep -qw off && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test durability every_acked_seq_is_present_with_synchronous_commit_off
negctl-sync-commit-unset|0|scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && scripts/orch/hub-pg.sh sql "ALTER SYSTEM SET synchronous_commit = off" && scripts/orch/hub-pg.sh sql "ALTER SYSTEM SET wal_writer_delay = 10s" && scripts/orch/hub-pg.sh sql "SELECT pg_reload_conf()" && scripts/orch/hub-pg.sh sql "SHOW synchronous_commit" | grep -qw off && GM_HUB_BREAK=sync-commit-unset scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test durability acked_seqs_are_written_before_the_kill && scripts/orch/hub-pg.sh kill && scripts/orch/hub-pg.sh start && scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test durability every_acked_seq_is_present_after_a_restart; test $? -ne 0
cargo-deny-server|0|GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config deny.toml check advisories bans licenses sources
negctl-deny-license-server|0|sed '/^    "MIT",$/d' deny.toml >target/deny-negctl-server.toml; grep -q '"MIT"' target/deny-negctl-server.toml && exit 1; rc=0; GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path server/Cargo.toml --config target/deny-negctl-server.toml check licenses >target/deny-negctl-server.log 2>&1 || rc=$?; test $rc -ne 0 && grep -q 'MIT - MIT License:' target/deny-negctl-server.log
svc-supply|0|scripts/orch/svc-digest.sh && scripts/orch/svc-digest-wasm.sh && scripts/orch/svc-features.sh && scripts/orch/lock-parity.sh
negctl-svc-supply-features|0|scripts/orch/svc-features.sh --break; test $? -eq 1
negctl-svc-supply-lock-version|0|scripts/orch/lock-parity.sh --break-version; test $? -eq 1
negctl-svc-supply-lock-feature|0|scripts/orch/lock-parity.sh --break-feature; test $? -eq 1
svc-supply-messages|0|scripts/orch/svc-features.sh --break >target/svc-features-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q 'graph-wasm is linked with probe in' target/svc-features-negctl.log && scripts/orch/lock-parity.sh --break-version >target/lock-parity-version-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after moving indexmap" target/lock-parity-version-negctl.log && scripts/orch/lock-parity.sh --break-feature >target/lock-parity-feature-negctl.log 2>&1; rc=$?; test $rc -eq 1 && grep -q "graph-wasm's normal closure after adding probe" target/lock-parity-feature-negctl.log
```

- [ ] **Step 7:** `scripts/orch/gate.sh target/gate-hub-store scripts/orch/rows/hub-store.rows` → all 42 rows exit as declared: the 9 positive database rows green (with PostgreSQL started and stopped by the row itself), the 9 non-database positive rows green, and all 24 `negctl-*` rows green **because their subject went red for its own reason** — a green control is a failure (§8 preamble). Commit.
- [ ] **Step 8: the slice report inputs.** `docs/measurements/hub-pg-epoch-probe/store-image.md` plus a `## Slice 2` section there carrying: the injected `tokio/fs` feature and its absence from the baseline's graph-server subtree (condition 11, §16 condition 18), the `hub-virtual-root` and condition (b) diffs (both empty), the `cargo deny` output, and the `git diff` of `scripts/orch/lock-parity.sh` and `scripts/orch/svc-features.sh` as sent to graph-render-4f.

## Self-review

- Spec coverage, section by section: **§2** H3 (image, durability settings — Tasks 3, 10), H6 (row-locked `seq`, Task 7), H11 (`GRAPH_HUB_RETAIN`/`RETAIN_BYTES` and 410 — Task 10, Task 8), H12 (pruning in the materializer — Task 9), H14 (graph-contract is the only producer; the store streams — Tasks 4, 9), H15 (`epoch`, triggers, `hub_meta`, the detector — Tasks 5, 6). **§3** the store's row (schema, migrations, epoch clock, triggers, restore detector, one transaction per batch, streamed materializer, changes, records, retention, sweeper; no HTTP — Tasks 4–10). **§4** the data model (records/manifests/links/`text_sha256`, no-op upserts, caps, lock order — Tasks 4, 7). **§5.1** the writer transaction step by step, the 40P01/40001 retry, the unique-violation retry, the sweeper's 10-minute bounded batches (Tasks 7, 10). **§5.3** cursors, `/graph` order and the two portals, `/changes` in one snapshot, the epoch triggers, the restore detector with the high-water and the last-seen map, the Caveats (Tasks 5, 6, 8, 9). **§6** every limit in `StoreConfig::defaults()` and `check()`, `doc_bytes`, the memory-relevant streaming decisions (Tasks 3, 4, 9). **§8** every row this slice owns, mapped in the rows file header (Tasks 1–10). **§10 slice 2** step 1 `default-members` (Task 1), step 2 the D7 scratch copies (Task 2), step 3 everything else (Tasks 3–10), in that order, each its own commit. **§11** the tokio-postgres closure and `cargo deny` (Global Constraints, Task 10), D13 (Task 1), D7 (Task 2). **§12** condition 4 (`hub-seq`), 7 (`hub-pg-durability`), 12/13 (`hub-limits` is slice 3; the caps it needs are here). **§13** conditions 4, 5, 6, 8, N2 (`hub-changes-snapshot`), N8 (`doc_bytes`), N9 (lock order), N11 (`sequence-seq`), N12 (the `C` collation check in `check()`), N16 (`plugin_seq` per records page). **§14(c) hub-store** 1–6 (Tasks 5, 6, 8, 1, 2). **§15(c) hub-store** 7–11 (Tasks 5, 6, 7). **§16(b) hub-store** 12–18 (Tasks 6, 7, 5, 10, 1).
- Names used across tasks, each defined once above: `Store`, `StoreError`, `StoreConfig`, `Detector`, `DetectorOutcome`, `LastSeen`, `Hooks`, `breaks::on`, `migrate::{FILES, apply, manifest}`, `epoch::bump_now`, `Store::{create_workspace, put_manifest, apply_batch}`, `ManifestWrite`, `ManifestWritten`, `BatchWrite`, `BatchOutcome`, `retention::prune`, `sweeper::{run, interval_ms}`, `changes::{page, cursor_state}`, `ChangePage`, `Change`, `ChangeOp`, `CursorState`, `records::page`, `RecordsPage`, `materialize::open`, `Document`, `Document::{head, next, tail}`, `anti_join::prune_text`, `support::{db::url, step::signal, step::await_done, step::write}`.
- The four decisions the brief left open are Decisions 1–4 above; Decision 5 records the one place the spec names an API the pinned driver does not have (`client.bind` / `query_portal`), with the substitute and its Caveat.
- Nothing in this plan reads or writes `server/graph-server/**`, `crates/**`, `deny.toml`, `push-once.sh`, `push-loop.sh` or `.claude/rules/devil/`; the two scratch-gate scripts are touched only as D7 describes.