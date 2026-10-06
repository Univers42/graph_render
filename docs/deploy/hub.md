# graph-hub as a service

One image holds one thing: `graph-hub`, the HTTP edge that owns the graph store's writes, streams
the materialized document and relays `/layout` to `graph-server`. Contract:
`docs/contract/hub-api.md`. The scripts' headers are the manual; this page says how the parts fit
together and what an operator does when the hub refuses to start.

| file | what it is |
|---|---|
| `scripts/hub.sh` | `image`, `run`, `keygen`, `test`, `upload-measurement`. Its header (lines 2–31) is the manual |
| `deploy/hub.Dockerfile` | `debian:trixie-slim` pinned by digest, uid 10001, `EXPOSE 8080`. It copies the staged binary in and builds nothing |
| `scripts/orch/hub-run.sh` | the gate's hub: `reset`, `start`, `stop`, `kill`, `restart`, `url`, `run`, `ack-file`, `inspect`. Publishes nothing and hands the tests the container's bridge IP |
| `scripts/orch/hub-pg.sh` | the hub's PostgreSQL 17: `reset`, `start`, `stop`, `kill`, `url`, `ip`, `wait`, `sql`, `run`, plus the container-level verbs (`copy-data`, `restore-data`, `switch-wal`, `replica`, `replica-promote`, `pitr`). Every database row starts and stops its own; the store connects as `hub` on database `hub` |
| `scripts/orch/hub-mem.sh` | `measure` and `control`, the two rows of `docs/measurements/hub-memory.md`. Its third verb, `upload`, is the intended command of the not-yet-written `hub-upload-timeout` row and is **not in `hub.rows` nor in this tree**: `hub-mem.sh` prints `usage: hub-mem.sh measure\|control` and exits 2 |

## Build

```sh
scripts/hub.sh image      # prints graph-hub:<tag>; stages target/hub-stage, the whole build context
scripts/hub.sh keygen ops keys   # the key on stdout, once; `ops <sha256>` appended to ./keys at 0640
```

`image` builds `-p graph-hub --bin graph-hub` in release through `scripts/orch/gr` (in `ge-rust`,
which is itself trixie, so it links the same glibc as the base). The `-p` is not optional:
`server/Cargo.toml` has `default-members = ["graph-server"]`, so a build with only `--bin`
resolves against graph-server and reports no such bin.

Only `target/hub-stage/bin/graph-hub` is staged, and the stage **is** the build context, so no key
file, `.env`, `.git` or scratch path can reach the build. `<tag>` is the first 16 hex of the sha256
over `sha256sum` of every staged file sorted by path, and `target/hub-image/name` records it for
`run`.

`HUB_IMAGE_BREAK=bin scripts/hub.sh image` is the negative control for `negctl-hub-image`: it asks
cargo for `--bin graph-server`, stages nothing and exits 1.

`deploy/` and `server/` are not fingerprinted. Building the image voids no motor gate evidence.

## Run

```sh
HUB_KEYS_FILE=./keys HUB_GRANTS_FILE=./grants HUB_MOTOR_KEY_FILE=./motor-key \
GRAPH_HUB_DB_URL='<url>' GRAPH_HUB_MOTOR_URL='<url>' \
scripts/hub.sh run     # 127.0.0.1:8080, read-only root, no capabilities
```

`run` refuses (exit 2) if any of the three credential variables is unset or is not a file: a hub with
keys and no grants answers 403 to everything and reads as a bug rather than a deployment mistake.
On a host without this repository the same container is one `drun` line:

```sh
scripts/orch/drun --rm --read-only --cap-drop ALL --security-opt no-new-privileges \
  -v "$PWD/keys:/run/graph/keys:ro" \
  -v "$PWD/grants:/run/graph/grants:ro" \
  -v "$PWD/motor-key:/run/graph/motor-key:ro" \
  -e GRAPH_HUB_KEYS_FILE=/run/graph/keys \
  -e GRAPH_HUB_GRANTS_FILE=/run/graph/grants \
  -e GRAPH_HUB_MOTOR_KEY_FILE=/run/graph/motor-key \
  -e GRAPH_HUB_DB_URL -e GRAPH_HUB_MOTOR_URL \
  -p "127.0.0.1:${HUB_PORT:-8080}:8080" graph-hub:<tag>
```

`drun`'s default cap is `DRUN_MEM`, 4 GiB, and the hub fits in far less: the container peak at the
defaults is 404 580 KiB (`docs/measurements/hub-memory.md`). The gate runs it at
`--memory 1g --memory-swap 1g` and that is the figure the budget is built to.
`scripts/orch/hub-run.sh` adds `--memory` only when `HUB_RUN_MEMORY` is set, which is how
`hub-memory` reaches its cap. `scripts/service.sh run` is the one script that raises its own to 8 GiB,
because one graph-server worker slot needs 4.32 GiB; `hub.sh run` does not.

`GRAPH_HUB_DB_URL` and `GRAPH_HUB_MOTOR_URL` carry passwords, so they pass **by name** (`-e
GRAPH_HUB_DB_URL`, no value on the command line) and are never printed by any script. Inside the
gate's own network the hub publishes no port at all: `hub-run.sh` reads the container's bridge IP
with `docker inspect` and writes `http://<ip>:8080` to `target/hub-run/url`, and the tests connect
there. `scripts/orch/gr` has no network option, so that bridge IP is how the test process reaches
the hub it just started. On a real deployment the URL is the ordinary PostgreSQL URL of the hub's own
database, created by `initdb --encoding=UTF8 --locale=C` or the start check refuses it.

### The three secret files

An operator creates these three, in this order. No value below is real: `<key>` is a placeholder.

| file | mounted at | mode | content | Caveat |
|---|---|---|---|---|
| the hub's keys file | `/run/graph/keys` | `0640` | one `<name> <sha256-hex>` line per key, `#` for a comment | Caveat: `0640` means *stricter is refused too* — the code's mask is `mode & 0o037`, so `0600` is accepted but `0440` is not, and a group-readable file is what makes `--group-add` work for uid 10001. The file holds **hashes**, not keys, so the group bit is not the risk the mode is about |
| the grants file | `/run/graph/grants` | `0640` | one `<key-name> <ws\|*> <read\|write:<plugin>\|admin>` line per grant | Caveat: an empty file is a load **failure** (`holds no grant`), not "no grants yet". A malformed line fails the whole load; it is never skipped, so one typo refuses every request after the next `SIGHUP` while the old pair stays in force |
| the motor key file | `/run/graph/motor-key` | `0600` | **one line: the plaintext motor key** (`<key>`), nothing else | Caveat: this is the only one of the three that holds a secret rather than a hash, and it is the only one whose mode `0640` would be wrong. It is `GRAPH_HUB_MOTOR_KEY_FILE`, the key the hub presents to `graph-server` on `/layout`; a wrong one is `502 MotorAuth` on every relay, logged `motor-fault`, never `401` to the caller |

`scripts/hub.sh keygen NAME KEYFILE` appends to the keys file at mode `0640` and prints the key on
stdout exactly once. The motor key file has no generator: the same generator is used, and the result
is written by hand to `motor-key` with `install -m 0600 /dev/null motor-key` first.

A browser never holds any of these. Every `/v1/` route needs the keys file's key, and the hub serves
no page.

### Start checks

Ten refusals come out of `Settings::check` (`src/config/check.rs`) and three more come out of the
environment reader before that. All thirteen print `graph-hub: <NAME>: <reason>` and exit 2; none of
them ever prints the value, the path or the URL. `tests/start/refusals.rs` asserts nine of the ten,
and `tests/start/settings.rs` `the_three_refusal_messages_are_graph_servers_own` asserts the three
environment messages. The one check with no asserting test is marked below.

| Refusal | Message the operator sees | What to do |
|---|---|---|
| `GRAPH_HUB_DB_URL` unset or empty | `GRAPH_HUB_DB_URL: is unset or empty` | set it. There is no default and no in-memory mode; the hub has no store to serve without one. Test: `start_check_refuses_an_empty_db_url` |
| `GRAPH_HUB_KEYS_FILE` or `GRAPH_HUB_GRANTS_FILE` unset | `<NAME>: is unset or empty` | set both. There is no auth-off mode. Test: `start_check_refuses_a_missing_keys_or_grants_file` |
| `GRAPH_HUB_DB_POOL` at or below `WRITERS + READS + LAYOUTS` | `GRAPH_HUB_DB_POOL: must exceed GRAPH_HUB_WRITERS + GRAPH_HUB_READS + GRAPH_HUB_LAYOUTS` | raise `GRAPH_HUB_DB_POOL` above the sum. At the defaults the sum is 5 and the pool is 8. Lowering a permit is the wrong fix: a hub whose permits cannot all be admitted deadlocks its writes against its own reads. Test: `start_check_refuses_a_pool_at_or_below_the_permits` |
| `GRAPH_HUB_RETAIN_BYTES` below one maximum change | `GRAPH_HUB_RETAIN_BYTES: is below one maximum change` | raise it. A bound that cannot hold one change is not a bound. Test: `start_check_refuses_retain_bytes_below_max_change` |
| `GRAPH_HUB_CHANGES_BYTES` below one maximum change | `GRAPH_HUB_CHANGES_BYTES: is below one maximum change` | raise it. The maximum is `StoreConfig::max_change` over `Limits::DEFAULT`, so this fires as soon as `MAX_BODY`, `MAX_BATCH` or `MAX_RECORD_BYTES` grows past the pages. Test: `start_check_refuses_changes_bytes_below_max_change` |
| `GRAPH_HUB_MOTOR_TIMEOUT_MS` at or below 40 000 | `GRAPH_HUB_MOTOR_TIMEOUT_MS: must exceed 40000, the motor's own timeout plus its body timeout` | leave it at 45 000, or raise it. Below graph-server's own `30000 + 10000` the hub would give up on a motor that is still working, and a request that was about to succeed would be a `502`. Test: `start_check_refuses_a_motor_timeout_at_or_below_forty_thousand` |
| database unreachable | `GRAPH_HUB_DB_URL: cannot be reached` | check the URL and the network. The refusal never echoes the URL, so a wrong password is indistinguishable from a wrong host by design. Test: `start_check_refuses_an_unreachable_database` |
| database unreadable by the hub's role | `GRAPH_HUB_DB_URL: cannot be read` | grant the role what the store's migrations and queries need, or run the migration as that role. **Not asserted by a test** |
| `server_encoding` not `UTF8` | `GRAPH_HUB_DB_URL: the database's encoding is not UTF8` | recreate with `initdb --encoding=UTF8`. There is no override. Test: `start_check_refuses_a_wrong_encoding` |
| `pg_database.datcollate` not `C` | `GRAPH_HUB_DB_URL: the database's collation is not C, so byte order is not its order` | recreate with `initdb --locale=C`. Without it `/graph` and the records pages order their output by the database's rules, and every byte-equality claim in the contract fails. Test: `start_check_refuses_a_wrong_collation` |
| an environment value not UTF-8 | `<NAME>: is not UTF-8` | fix the variable. The value is never printed |
| an environment value malformed | `<NAME>: is malformed` | fix the variable |
| an environment value out of range | `<NAME>: is out of range` | fix the variable; the range is in `docs/contract/hub-api.md` "Limits and scheduling" |

Two refusals have no message because they are hyper's own: a head over
`GRAPH_HUB_MAX_HEADER_BYTES` is `431` and a head that stalls past `GRAPH_HUB_HEADER_TIMEOUT_MS` is a
closed connection, both with no JSON body.

The image's `HEALTHCHECK` runs `graph-hub healthcheck`, which is true only on a `200` from
`/healthz` inside its own budget, and `/healthz` needs no key and no database — so a hub whose
database is refusing it still reports liveness while the process refuses to serve.

**Caveat:** these are start checks, not a runtime guard. Nothing re-reads the variables, so a value
that is legal here stays legal for the life of the process, and `SIGHUP` re-reads only the two
credential files. The escape hatch for a wrong deployment is the variable itself, and a restart.

### Restore runbook

The hub refuses to serve a database it did not write: on **every** new connection, before it joins
the pool, the restore detector compares `hub_meta` (system identifier, timeline, `pg_database.oid`)
against the live database, and refuses when `pg_is_in_recovery()` is true, when `fsync` or
`full_page_writes` reads `off`, or when `current_setting('hub.writer', true)` is neither null nor
empty. After any restore — logical, a volume snapshot or a base backup — the detector refuses, and
the operator runs, by hand, against the hub's database:

```sql
UPDATE workspaces SET epoch = hub_next_epoch();
```

In this repository: `scripts/orch/hub-pg.sh sql "UPDATE workspaces SET epoch =
hub_next_epoch();"`, against the database `hub-pg.sh start` brought up.

`hub_next_epoch()` is `UPDATE epoch_clock SET last = greatest(last + 1,
(extract(epoch FROM clock_timestamp()) * 1000000)::bigint) RETURNING last`, so each call draws one
fresh epoch.

Caveat: run by hand at depth 0 it draws **two** epochs per workspace — the one the statement sets,
then the trigger's own at depth 1 — and both are fresh, so that is harmless. The statement is the
whole runbook because no key and no trigger catches a restore that happened while the hub was also
restarted: every key is the same and the high-water and the last-seen map were in memory.

Caveat: a workspace the last-seen map does not hold (evicted past `GRAPH_HUB_LAST_SEEN`, or neither
committed to nor served since this hub started) is not caught either; the SDK sees a gap and resyncs.
The upgrade path is persisting both outside the database, and it is not built.

### Rotate a key

1. Run `scripts/hub.sh keygen ops-2 keys`. It appends the new line at `0640`; the old key keeps
   working.
2. `docker kill -s HUP <container>`. The hub reads **both** files and swaps the key set and the
   grants as one pair. If either file fails to load, the old pair stays in force — no window, no
   half-swap.
3. Move the clients to `ops-2`, delete the `ops` line, then send `SIGHUP` again.

`tests/reload.rs` covers all four steps, including the refusal cases. There is no restart in this
loop: rotation is a `SIGHUP`, and a key file that is malformed or empty keeps the old set rather
than emptying it.

## The gate

```sh
scripts/orch/gate.sh target/gate-hub scripts/orch/rows/hub.rows
```

`scripts/orch/hub.rows` holds every spec §8 row whose slice column names 3, plus the shared rows.
Every row that boots the hub starts and stops its own PostgreSQL through `scripts/orch/hub-pg.sh`,
and the two container rows (`hub-durability`, `hub-memory`) their own hub container. A `negctl-*`
row passes only when the thing it breaks goes red **for its own reason**; a green control is a
failure.

The docs row is separate and cheap:

```sh
scripts/orch/gate.sh target/gate-hub-docs scripts/orch/rows/docs.rows
```

## What it does not do

- **No TLS.** The hub sits behind the host's proxy, like the service.
- **No migrations on start.** The hub does not migrate its database; the operator runs the store's
  migrations with the role that owns them. A hub against an unmigrated database refuses at the first
  query, not at start.
- **No embed, no studio, no `/metrics`.** The hub serves no page and no asset.
- **No second motor path.** `/layout` is the only route to graph-server, and the hub computes no
  layout itself.
- **No sharding, no replica read, no HA.** One database, one process, one store.

## Supply chain

`cargo-deny-server` in `scripts/orch/rows/hub.rows` runs `cargo deny` over `server/Cargo.lock` with
`deny.toml` passed with `--config`, checking RustSec advisories, the SPDX `allow` list, duplicate
versions and crate provenance. `hub-virtual-root` re-runs the condition (b) diff and requires
`client-legacy` to appear exactly once under `-p graph-hub` and zero times without it, so the hub's
axum edge cannot pull graph-server's HTTP client into the virtual root.