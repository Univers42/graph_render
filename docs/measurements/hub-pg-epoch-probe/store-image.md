# hub-store image (Task 10 Step 5)

The image every `graph-store` database row runs against, and what it actually read at run time.
Every number below is copied out of the raw files beside this one, all measured in this worktree
on 2026-10-05 against instance `gm-hub-pg-hub-store-pg`.

## Tag and digest

- Base: `postgres:17`, image id `sha256:212aeeeb8faaef6c46498d86cbec6b9344d8d9492996b174664ff82c562ab685`.
- Hub image: `gm-hub-pg:17`, image id
  `sha256:a20e9b51f1e5d71178204f3fa794511bb957400058e99b25bad79b2e97270631`, built by
  `scripts/orch/hub-pg.sh image` with
  `docker build -q -f deploy/postgres.Dockerfile -t gm-hub-pg:17 deploy`. `docker build` is not a
  run, so `scripts/orch/drun-check.sh` stays exit 0.

The image id IS the build's content digest for a locally built image: `docker image ls --digests`
reports `<none>` for it because a local build has no registry manifest. The base line's id is
recorded for the same reason — `postgres:17` moves within the line, and this is what moved.

Raw: `store-image-tag.txt`.

## `postgres --version`

```
PostgreSQL 17.11 (Debian 17.11-1.pgdg13+2) on x86_64-pc-linux-gnu,
compiled by gcc (Debian 14.2.0-19) 14.2.0, 64-bit
```

Caveat: `postgres:17` moves within the line, so two runs can differ by patch level. Nothing in
this slice depends on a patch level; the tag and the id above are recorded so a future bump is
visible. Raw: `store-image-version.txt`.

## The settings, as the running server read them

`SHOW` on the live server, through `scripts/orch/hub-pg.sh sql`:

| setting | value |
| --- | --- |
| `fsync` | `on` |
| `synchronous_commit` | `on` |
| `full_page_writes` | `on` |
| `server_encoding` | `UTF8` |
| `pg_database.datcollate` (database `hub`) | `C` |
| `pg_database.datctype` (database `hub`) | `C` |
| `archive_mode` | `on` |
| `archive_command` | `test ! -f /archive/%f && cp %p /archive/%f` |
| `wal_level` | `replica` |
| `hot_standby` | `on` |
| `max_wal_senders` | `10` |

The three durability settings and `archive_mode` are written into `$PGDATA/postgresql.conf` by
`/docker-entrypoint-initdb.d/10-hub.conf.sh`, so a fresh volume and a rebuilt image agree.
`datcollate = C` comes from the entrypoint's `initdb` arguments, which is what makes
`(qcoll COLLATE "C", id COLLATE "C")` real byte order (N12).

`SHOW lc_collate` is in the plan's list and **does not exist** on PostgreSQL 17: the collation is
per database, so the row above reads `pg_database.datcollate` instead. The `ERROR` is in the raw
file rather than removed, because it is what the plan's literal command prints. Raw:
`store-image-settings.txt`.

## `GRANT SET ON PARAMETER` lines `hub-pg.sh` applies

Three, all applied by `scripts/orch/hub-pg.sh start` as `postgres` and re-applied on every start:

```sql
GRANT ALL ON SCHEMA public TO hub;
GRANT SET ON PARAMETER hub.writer TO hub;
GRANT pg_monitor TO hub;
```

The first is not optional: PostgreSQL 15 and later grant `CREATE` on schema `public` to
`pg_database_owner`, not to `PUBLIC`, so the store's own role cannot create its tables in a
database it does not own. The second is what lets `detector_refuses_a_hub_writer_default` set
that GUC as a role default.

## Which grant reaches which read

**`GRANT pg_monitor` is sufficient for every read the restore detector makes; no per-function
`GRANT EXECUTE` was needed and `hub` is never made a superuser.** Measured as `hub`:

| read | `has_function_privilege('hub', …, 'EXECUTE')` |
| --- | --- |
| `pg_control_system()` | `t` |
| `pg_control_checkpoint()` | `t` |
| `pg_control_init()` | `t` |
| `pg_control_recovery()` | `t` |
| `pg_create_restore_point(text)` | `f` |
| `pg_switch_wal()` | `f` |

`pg_control_checkpoint()` reading `t` is what makes row `negctl-checkpoint-timeline` a real
control rather than a permission error: the break substitutes that expression for §5.3's
`substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8)`, and the substituted one is reachable by the
store's own role.

The two `f` rows are why `tests/support/case.rs::admin()` exists: §5.3's own steps (naming a
restore point, forcing a WAL switch) need a superuser, and a store test must not be one. Those
statements go through an admin connection in the container-level phases; nothing in `src/` does.

Role and database, both created by the image's own entrypoint on first init
(`POSTGRES_DB=hub`, `POSTGRES_PASSWORD=hub`) and by the role bootstrap in `hub-pg.sh start`:

- database `hub`, role `hub` with password `hub`, `CREATEDB`, **`rolsuper = f`**.

Raw: `store-image-grants.txt`.

## The exact `hub-pg.sh` invocation each container-level case used

Each container-level case is split into `#[ignore]` phases and sequenced by its gate row, because
only `hub-pg.sh` can promote a standby, kill a container, copy a volume or replay an archive. The
four commands are in `store-image-container-cases.txt`, one per row, with the real exit code and
the observed output; each ends `exit=0`.

| row | what it does |
| --- | --- |
| `hub-promotion` | `reset`, `start`, write phase, `replica`, `replica-promote`, assert phase |
| `hub-pitr` | `reset`, `start`, `copy-data`, `start`, write phase, `switch-wal`, `restore-data`, `pitr hub_pitr`, assert phase |
| `hub-snapshot` | `reset`, `start`, write phase, `copy-data`, `start`, after-copy phase, `restore-data`, `start`, assert phase |
| `hub-crash-copy` | `reset`, `start`, write phase, `kill`, `copy-data`, `start`, commit phase, `restore-data`, `start`, `switch-wal`, assert phase (`--nocapture`, so the leg that fired is in the log) |

Every row begins `reset && start`: `reset` removes the container `gm-hub-pg-hub-store-pg` and its
three named volumes (`-data`, `-archive`, `-snapshot`), `start` builds the image, runs it through
`scripts/orch/drun`, polls `pg_isready`, applies the three grants and writes
`postgres://hub:hub@<bridge-ip>:5432/hub` to `target/hub-pg-url`.

Caveat: `gr` has no network option and its containers sit on Docker's default bridge, so the tests
reach PostgreSQL by the container's **bridge IP**, never by `127.0.0.1` and never by a published
host port. Every verb above that replaces the container (`replica`, `pitr`, `restore-data` +
`start`) hands the replacement a bridge IP that Docker may renumber, and the assert phases re-read
`target/hub-pg-url` for exactly that reason. Observed on this instance: the promotion row dialled
`172.17.0.21` before `replica` and `172.17.0.8` after it in one run, and the same address twice
in another — which is exactly why the URL file, not the value `GM_HUB_PG_URL` held at the first
phase, is what a later phase reads. `hub-pg.sh sql` shares the network namespace with
`--network container:`, but not the unix socket, so its `psql` needs `-h 127.0.0.1`.

### What the crash-consistent copy measured

`hub-crash-copy` prints which of §5.3's two non-identity signals caught the restore:

```
crash-copy: restored flush LSN 0/3000000 against the hub's high-water 0/2001568
crash-copy: the last-seen map leg fired; the map leg is exact whenever the LSN leg does not
```

That is the whole point of the case and it is not a guess: `switch-wal` before the reconnect moved
the restored server's WAL a full 16 MiB segment forward, so the high-water leg reads a MATCH
(`0/3000000` is above `0/2001568`) and the last-seen map is the only thing left. The spec's
measured threshold — a gap under 10.9 MiB being invisible to the LSN — is reproduced here by
construction. The assert phase still asserts the bump, and separately asserts that `head_seq` is
below the hub's map entry, so the case does not depend on the printed line being favourable.

## Two fixes `hub-pg.sh` needed for these rows

Both were found by running the four rows, and both are one line each.

1. **`start_recovery` never rewrote `target/hub-pg-url`.** `replica`, `replica-promote` and
   `pitr` all land in it, and every one of them replaces the container — so any test that read the
   URL between a verb and the next `run` dialled a container that no longer existed. It now calls
   `url` after `wait_ready`, exactly as `start` does.
2. **`run` passed `GM_HUB_STEP_DIR=target/hub-steps`,** which resolves inside the test container
   to `server/graph-store/target/hub-steps` (cargo runs a test binary with its CWD at the *package*
   root) and, made absolute on the host, would not exist inside the container at all. It now
   passes `../../target/hub-steps`, which is `/w/target/hub-steps` — the repository's own
   `target/`, host-visible, and the same string for `tests/support/step.rs` and
   `src/hooks.rs`. `tests/support/step.rs` and `tests/support/db.rs` anchor their defaults at
   `CARGO_MANIFEST_DIR` for the same reason.

## `negctl-checkpoint-timeline` over the promotion case

Row `negctl-checkpoint-timeline` is the promotion row with `GM_HUB_BREAK=checkpoint-timeline` on
the **assert phase only**, so the priming phase recorded the spec's WAL-file key and the detector
then compared a different key against it. Full run in
`store-image-checkpoint-timeline-control.txt`; the red is:

```
---- promotion_phase_assert stdout ----

thread 'promotion_phase_assert' (18) panicked at graph-store/tests/promotion.rs:73:5:
assertion `left == right` failed: hub_meta.timeline must be the WAL-file timeline key: the break `checkpoint-timeline` reads pg_control_checkpoint(), which lags a promotion and prints the id unpadded, so the key the detector compares with is not the key it stores
  left: "1"
 right: "00000002"

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out
```

`left: "1"` is the whole finding: **after a promotion, `pg_control_checkpoint().timeline_id` still
reads `1`** — the checkpoint has not been redone on the new timeline yet, which is precisely §5.3's
reason for reading `pg_walfile_name` instead. The break therefore stores a key that is both the
wrong source and the wrong format, and the promotion case turns red on it.

## Durability (`hub-pg-durability`, `negctl-sync-commit-unset`)

Both rows PASS in `target/gate-t10-dur`; full logs in `store-image-durability.txt`.

- **Positive row.** `fsync`, `full_page_writes` and `synchronous_commit` read `on`. Two rounds
  then each write 200 batches and `kill -9` the server: the first round with the server at `on`,
  the second at `ALTER SYSTEM SET synchronous_commit = off`. After each restart every acknowledged
  seq is in `change_headers`. One write phase takes 7.6–8.1 s, about 38 ms a batch.
- **Negative control.** `GM_HUB_BREAK=sync-commit-unset` drops the writer's own
  `set_config('synchronous_commit','on',true)`. The server runs at `off` with
  `wal_writer_delay = 10s` and `wal_writer_flush_after = 1GB`, so the WAL writer flushes nothing
  inside one write phase. After the kill, **200 of the 200 acknowledged batch seqs (2..201) are
  gone**. Seq 1 is the manifest PUT, which the setup fence (`SELECT txid_current()` on a session
  at `on`) made durable.
- **Why the fence.** Without it the schema went down with the batches, and the assert failed on
  `relation "change_headers" does not exist` instead of naming the lost seqs (measured
  2026-10-05). The fence makes the control fail for the reason it exists to show.

Caveat: the control relies on the WAL writer not flushing inside about 8 s. A host that flushes
anyway (a checkpoint, or another session's synchronous commit) makes the assert pass. The row
then retries up to 3 times, and a pass on all three attempts turns it red: no false green, but a
possible false red under load.

## Slice 2

Raw output: `store-image-slice2.txt`.

| Item | Result |
|---|---|
| condition 11 / §16 condition 18: `tokio feature "fs"` in the baseline trees | 0 in `base-root.txt`, 0 in `base-server.txt` |
| the same feature in the control's tree | `ctl.diff` line 395 `+tokio feature "fs"`, under `+graph-store v0.1.0` (line 177); 233 changed lines |
| `hub-virtual-root`: `diff base-root.txt new-root.txt` | empty |
| condition (b): `diff base-server.txt new-server.txt` | empty |
| `cargo deny --manifest-path server/Cargo.toml check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `git diff f1a23521 HEAD -- scripts/orch/lock-parity.sh scripts/orch/svc-features.sh` | 2 files, 6 insertions, 32 deletions, as sent to graph-render-4f |

`negctl-hub-virtual-root` resolves its scratch control with `--offline`, not `--locked`. Since
graph-hub became a workspace member, the control's member list leaves it out. Under `--locked`,
cargo would then refuse to drop graph-hub's lock entries. The row now checks instead that the
scratch lock gained or moved no entry (`diff … | grep -q '^>'` fails the row), so every package
left keeps its locked version.

## Raw outputs

Beside this file, as the plan requires:

- `store-image-tag.txt` — the two image ids and the `docker image ls --digests` line
- `store-image-version.txt` — `SELECT version()` and `SHOW server_version`
- `store-image-settings.txt` — the `SHOW` lines above, including the `SHOW lc_collate` error
- `store-image-grants.txt` — the privilege reads and the three detector reads as `hub`
- `store-image-container-cases.txt` — the four rows' commands, outputs and exit codes
- `store-image-checkpoint-timeline-control.txt` — the control's full red output
- `store-image-durability.txt` — both durability rows' logs and the control's assert, the lost list trimmed to its ends
- `store-image-slice2.txt` — the two tree diffs, the `tokio/fs` lines, `cargo deny`, the two scripts' diff
