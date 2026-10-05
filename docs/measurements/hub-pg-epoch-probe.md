# graph-hub: PostgreSQL 17 epoch triggers, restore, and `/changes` isolation

Measured 2026-10-05 for the revision 3 verdict on `docs/decisions/graph-hub.md`, defects N1, N2 and
N12. The verdict listed each PostgreSQL fact as UNKNOWN because none had been run. This record runs
them. The sources and raw outputs are in `hub-pg-epoch-probe/`.

## Conditions

- Image: `hub-pg-epoch-probe/Dockerfile`, tagged locally `gm-pg-probe:17`
  (`sha256:013594e77e1c34655aa1fd8c09b19d4876253afbb7d4bcf09ec85241db3fd71c`).
  `postgres --version` prints `postgres (PostgreSQL) 17.11 (Debian 17.11-0+deb13u1)`.
  The base is unpinned: this is a probe image, not the hub's (slice 2 writes `deploy/postgres.Dockerfile`).
- Cluster: `initdb --encoding=UTF8 --locale=C`, Unix socket only (`listen_addresses=''`).
- Host dlesieur42. Every container ran through `scripts/orch/drun`.

## Commands

| # | Command | Exit | Output |
|---|---|---|---|
| 1 | `docker build -t gm-pg-probe:17 docs/measurements/hub-pg-epoch-probe` | 0 | |
| 2 | `scripts/orch/drun --memory 1g --memory-swap 1g --rm -v "$PWD/docs/measurements/hub-pg-epoch-probe:/probe:ro" gm-pg-probe:17 bash /probe/run.sh` | 0 | `out.txt` |
| 3 | the same with `run2.sh` | 0 | `out2.txt` |

## Results

`probe.sql` builds a reduced schema: `workspaces(id, epoch, head_seq)`, `records(ws, id, body)`, the
one-row `epoch_clock`, `hub_next_epoch()` and the trigger function `hub_bump()`. The guard in
`hub_bump()` is `current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1`.
Each step below writes the epoch of workspace `w1` (`w3` at step 13).

| Step | Case | Epoch change | Expected |
|---|---|---|---|
| 0 | workspace created with `hub.writer` | — | — |
| 1 | hub write: insert two records and bump `head_seq` | none | none |
| 2 | manual `INSERT` | +3 | changes |
| 3 | manual `UPDATE` | +1 | changes |
| 4 | manual `DELETE` | +1 | changes |
| 5 | sweeper-like `DELETE` with `hub.writer` | none | none |
| 6 | manual `UPDATE workspaces SET head_seq` (the trigger updates the same table) | +1, no recursion | changes once |
| 7 | `TRUNCATE records` | +2 | changes |
| 8 | manual `INSERT` | +1 | changes |
| 9 | `session_replication_role = replica`, triggers `ENABLE` | none | the bypass |
| 10 | the same with `ENABLE ALWAYS` | +1 | changes |
| 11 | manual delete and re-create of `w1` with `epoch = 1` | overwritten to a fresh, larger value | changes |
| 12 | `\copy records FROM stdin` | +1 | changes |
| 13 | `epoch_clock.last` reset to 0 (a rewind), then a new workspace | larger than every earlier epoch | larger |

Further facts from the same runs:

- **Multi-event triggers are refused.** `CREATE TRIGGER ... AFTER INSERT OR UPDATE OR DELETE ...
  REFERENCING NEW TABLE ...` fails with `transition tables cannot be specified for triggers with more
  than one event` (`out.txt`, test 1). One trigger per event is required.
- **`/changes` isolation.** A reader counted ten change rows, slept two seconds, then counted again.
  Meanwhile another session deleted some rows (`run.sh`, the section headed `== 12 REPEATABLE READ`).
  - Under `REPEATABLE READ READ ONLY`: `headers|10`, then `ops|10`, after the concurrent `pruned|5`.
  - Under `READ COMMITTED`: `rc headers|5`, then `rc ops|2`. The second statement sees the prune.
- **Byte order.** With `--locale=C`, `server_encoding` is `UTF8`, `datcollate`/`datctype` are `C`, and
  `ORDER BY x COLLATE "C"` over `é z Z a` gives `Z,a,z,é`, which is UTF-8 byte order. PostgreSQL 17 has no
  `lc_collate` setting; the start check reads `pg_database.datcollate`.
- **The restore detector inputs are readable without superuser** (`out2.txt`, as role `hub`):
  `pg_control_system().system_identifier`, `pg_control_checkpoint().timeline_id` and `pg_database.oid`.
- **`pg_restore` bypasses the triggers.**
  - `pg_restore -l` lists the table data (`3418; 0 16386 TABLE DATA public t hub`) before the trigger
    (`3272; 2620 16390 TRIGGER public t tr hub`). The trigger exists only after the rows are loaded.
  - The `NOTICE: fired` in `out2.txt` comes from the original insert, before the dump. Restoring into a
    fresh database printed no notice, and `restored rows: 1`.
  - The restored database has a new oid (16391 against 16385). That is what the restore detector keys on.

## What this does not show

- `ALTER TABLE ... DISABLE TRIGGER`, `pg_restore --disable-triggers`, and `pg_restore --clean` into the
  same database all bypass the triggers without changing the database oid. Only the runbook's manual
  bump catches them.
- A point-in-time recovery whose host clock is also stepped back can reissue an epoch. Step 13 rewound
  only `epoch_clock`.
- The probe ran the reduced schema, not the hub's. Slice 2's `hub-epoch` row reruns every case against
  the real schema.
