# hub-store image (Task 10 Step 5)

The image every `graph-store` database row runs against, and what it actually read at run time.

## Tag and build

- Base: `postgres:17`, image id `212aeeeb8faa` (pulled 2026-10-05).
- Hub image: `gm-hub-pg:17`, id `a20e9b51f1e5`, built by `scripts/orch/hub-pg.sh image` with
  `docker build -q -f deploy/postgres.Dockerfile -t gm-hub-pg:17 deploy`. `docker build` is not a
  run, so `scripts/orch/drun-check.sh` stays exit 0.

## `postgres --version`

```
PostgreSQL 17.11 (Debian 17.11-1.pgdg13+2) on x86_64-pc-linux-gnu,
compiled by gcc (Debian 14.2.0-19) 14.2.0, 64-bit
```

Caveat: `postgres:17` moves within the line, so two runs can differ by patch level. Nothing in
this slice depends on a patch level; the tag is recorded here so a future bump is visible.

## Durability and collation, as the server read them

| setting | value |
| --- | --- |
| `fsync` | `on` |
| `synchronous_commit` | `on` |
| `full_page_writes` | `on` |
| `server_encoding` | `UTF8` |
| `pg_database.datcollate` (database `hub`) | `C` |

`fsync`, `synchronous_commit` and `full_page_writes` are written into
`$PGDATA/postgresql.conf` by `/docker-entrypoint-initdb.d/10-hub.conf.sh`, so a fresh volume and a
rebuilt image agree. `datcollate = C` comes from `POSTGRES_INITDB_ARGS="--encoding=UTF8
--locale=C"`, which is what makes `(qcoll COLLATE "C", id COLLATE "C")` real byte order (N12).

## `GRANT SET ON PARAMETER` lines `hub-pg.sh` applies

Three, all applied by `scripts/orch/hub-pg.sh start` as `postgres` and re-applied on every start:

```sql
GRANT ALL ON SCHEMA public TO hub;
GRANT SET ON PARAMETER hub.writer TO hub;
GRANT pg_monitor TO hub;
```

The first is not optional: PostgreSQL 15 and later grant `CREATE` on schema `public` to
`pg_database_owner`, not to `PUBLIC`, so the store's own role cannot create its tables in a
database it does not own. The second is what lets `detector_refuses_a_hub_writer_default`
set that GUC as a role default; `SELECT has_parameter_privilege('hub','hub.writer','SET')` reads
`t` on this image.

## Which grant reaches the detector's three reads

**`GRANT pg_monitor` is sufficient on its own; no per-function `GRANT EXECUTE` was needed.**
Measured on this image as the `hub` role, which is not a superuser:

| read | result |
| --- | --- |
| `has_function_privilege('hub','pg_control_system()','EXECUTE')` | `t` |
| `pg_has_role('hub','pg_monitor','MEMBER')` | `t` |
| `SELECT (pg_control_system()).system_identifier` | `7693188589578006572` |
| `SELECT pg_current_wal_flush_lsn() IS NOT NULL` | `t` |
| `SELECT substr(pg_walfile_name(pg_current_wal_lsn()),1,8)` | `00000001` |

So the detector's `hub_meta` read, its flush-LSN read and its timeline read all work as `hub`
under `pg_monitor`, and the role is never made a superuser. Caveat: `pg_monitor` is a
predefined role whose membership PostgreSQL may extend in a future major; if a later image stops
granting one of these three to it, the start step needs an explicit
`GRANT EXECUTE ON FUNCTION <fn> TO hub` rather than a superuser.

Role and database, both created by the image's own entrypoint on first init
(`POSTGRES_DB=hub`, `POSTGRES_PASSWORD=hub`) and the role bootstrap in `hub-pg.sh start`:

- database `hub`, role `hub` with password `hub`, `CREATEDB`, **not** a superuser.

## The exact `hub-pg.sh` invocation each container-level case used

Every database row in `scripts/orch/rows/hub-store.rows` begins the same way:

```
scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start
```

`reset` removes the container `gm-hub-pg` and the named volumes `gm-hub-pg-data` and
`gm-hub-pg-archive`; `start` builds the image, runs it through `scripts/orch/drun`, polls
`pg_isready`, applies the two grants and prints `postgres://hub:hub@<bridge-ip>:5432/hub`.

The tests then run as:

```
scripts/orch/gr -e GM_HUB_PG_URL="$url" -e GM_HUB_BREAK="" -e GM_HUB_STEP_DIR=target/hub-steps \
  cargo test --manifest-path server/Cargo.toml -p graph-store --features db-tests,negctl --test <name>
```

which is exactly `scripts/orch/hub-pg.sh run --test <name>`.

Caveat: `gr` has no network option and its containers sit on Docker's default bridge, so the
tests reach PostgreSQL by the container's **bridge IP**, never by `127.0.0.1` and never by a
published host port. `hub-pg.sh sql` shares the network namespace with `--network container:`, but
not the unix socket, so its `psql` needs `-h 127.0.0.1`.

## Raw outputs

Beside this file, as the plan requires:

- `store-image-version.txt` — `SELECT version()`
- `store-image-settings.txt` — the five settings above
- `store-image-grants.txt` — `has_parameter_privilege('hub','hub.writer','SET')`