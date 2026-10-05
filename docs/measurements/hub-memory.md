# graph-hub memory: F_w, the last-seen entry, and the peak at every cap

Measured 2026-10-06 for the `hub-memory` row of spec §6 and §8
(`docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md`). The spec planned two terms of
the memory budget with borrowed or estimated numbers: `F_w` (graph-server's 18.25, measured on another
reader) and the last-seen entry (256 B, estimated). Both are measured here on the hub's own code, and
the cases in `server/graph-hub/tests/memory.rs` assert the ceilings below.

## Ledger

The cases read these rows by name (`tests/memory/ledger.rs`). Change a number only together with the
run that justifies it.

| Name | Value | Measured | Unit |
|---|---|---|---|
| `f_w_ceiling` | 52 | 50.92 worst of three runs | peak bytes per byte of `MAX_BODY` |
| `last_seen_entry_ceiling_bytes` | 272 | 263.8 | bytes per entry at `GRAPH_HUB_LAST_SEEN` = 65 536 |

## Conditions

- Host: nproc 20, `13th Gen Intel(R) Core(TM) i5-13600KF`; image `ge-rust:latest` (`40153c50e6a6`)
  through `scripts/orch/gr` (8 GiB cap). Tree: commit 78171669 plus the uncommitted test files of this
  change.
- Release build: `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-hub --release
  --test memory -- --test-threads=1 --nocapture`. Three runs, each exit 101 because this file did not
  exist yet (the numbers print before the ceiling is read), logs
  `~/goinfre/logs/hub-mem-{2,3.1,3.2}.out`; the green run with the ledger is listed under Commands.
- Each body is measured in a fresh child process (the test binary re-run on the ignored case
  `one_body_peak`): glibc keeps freed pages, so a second body in one process would start above the
  first one's peak. The child writes `5` to `/proc/self/clear_refs` and reads `VmHWM` before and after.
- Caveat: `VmHWM` is resident memory in pages, not bytes allocated. It counts allocator slack and
  fragmentation (which is what the container limit sees) but rounds to 4 KiB and depends on the
  glibc version. The `gr` image and the hub image (`deploy/hub.Dockerfile`, Debian trixie) both use
  glibc; a musl or jemalloc build would need its own run.

## F_w

What one child holds at its peak, which is the hub's writer path minus the database:

1. the raw body (`routes/batches.rs`, `body::read`);
2. the lossy UTF-8 copy `read_batch` parses (a temporary, dropped when it returns);
3. `read_batch`'s parse tree (`canonical_json::parse::Value`) and the `Batch` built from it, both alive
   until `read_batch` returns;
4. `Batch::check` against the fixture manifest;
5. the store's plan (`server/graph-store/src/writer/plan.rs` `upserts_of`): per upsert, a `Record`
   (a clone of the values) and its canonical text.

Five body shapes, 20 sizes each from `MAX_BODY / 20` to `MAX_BODY` (4 MiB), 100 bodies per run
(`tests/memory/bodies.rs`). `F_w` of a shape is its highest peak divided by its largest body, the
quantity the budget multiplies `MAX_BODY` by.

| Shape | Payload | F_w run 1 | run 2 | run 3 |
|---|---|---|---|---|
| `zeros` | `[0,0,0,…]` | 50.81 | 50.92 | 50.81 |
| `records` | 10 000 records of about 256 B | 50.78 | 50.68 | 50.82 |
| `strings` | `["","",…]` | 25.77 | 25.77 | 25.80 |
| `nested` | `[[],[],…]` | 25.79 | 25.67 | 25.69 |
| `keys` | `{"k0":0,"k1":0,…}` | 23.09 | 23.09 | 23.10 |

The `zeros` ladder of run 3, peak against body size, is close to linear:

| Body bytes | Peak bytes | Ratio |
|---|---|---|
| 209 714 | 10 272 768 | 48.98 |
| 1 048 574 | 51 990 528 | 49.58 |
| 2 097 148 | 110 612 480 | 52.74 |
| 3 145 720 | 160 862 208 | 51.14 |
| 4 194 298 | 213 098 496 | 50.81 |

A 2 MiB body peaks at 52.7 times its own size, but the budget term is `MAX_BODY × F_w`, and
110.6 MB is under 4 MiB × 50.92. The ceiling 52 covers every peak of the three runs.

Why 50 and not 18.25: an item `0,` is 2 bytes of body. The parse tree stores it as
`Value::Number(String)`, a 32 B slot plus a heap string (one 32 B malloc chunk), with up to 2×
capacity slack in the array's `Vec`; the `Batch` stores it again as `JsonValue::Number(f64)`, 32 B.
Both trees are alive inside `read_batch`, and the plan's `Record` clone (another 32 B) comes after
the parse tree is freed. Strings and empty arrays cost about half as much per body byte because
their items are at least 2 bytes of payload plus the separator.

Ponytail: `read_batch` converts the parse tree by reference, so the tree and the `Batch` coexist. A
by-value conversion would free each subtree as it is converted; whether glibc then reuses those
chunks for the `Batch`'s arrays is not measured. The budget fits at the measured value (below), so
this is left as the upgrade path if a smaller container is ever needed.

## The last-seen entry

`last_seen_peak` fills a `graph_store::pool::LastSeen` of capacity 65 536 with 65 536 distinct
63-byte workspace ids (the longest `check_workspace_id` accepts), the ids made before the baseline.

| Entries | Peak bytes | Bytes per entry |
|---|---|---|
| 65 536 | 17 289 216 | 263.8 |

The spec planned 256 B. At the default the map is 16.5 MiB rather than 16 MiB.

## The budget at the defaults, with the measured numbers

Spec §6's formula with `F_w` = 52 and the entry at 272 B; every other term is the spec's.

| Term | Formula at the defaults | MiB |
|---|---|---|
| writers | 2 × 4 MiB × 52 | 416 |
| reads and layouts | (2 + 1) × max(2 × 32 × 1 MiB + 1 MiB × 52, 2 × 8 MiB, 64 × 256 KiB, 1 MiB) | 348 |
| headers | 256 × 16 KiB | 4 |
| subscribers | 64 × (256 B + 256 × 256 B) | 4.02 |
| last-seen map | 65 536 × 272 B | 17 |
| pool | 8 × 1 MiB (planned `conn_buf`) | 8 |
| **total** | plus `base` and three `IO_BUF` | **797** |

That leaves 227 MiB of the container's 1 GiB for `base` and the I/O buffers, so no default shrinks on
this arithmetic. Caveat: this is arithmetic over a measured `F_w`, not a measurement of the whole
process; the container run below is.

## The container run

`scripts/orch/hub-mem.sh` starts the hub image (`deploy/hub.Dockerfile`) through `hub-run.sh` with
`--memory 1g --memory-swap 1g`, `GM_HUB_HOLD_BODIES` and `GRAPH_HUB_WRITERS_PER_KEY` equal to the
writer count, and runs `container::peak_rss_at_every_cap_fits_one_gib`
(`tests/memory/container.rs`). That case posts one `zeros` body of `MAX_BODY` per writer, each to its
own workspace. The hook in `src/hooks.rs` parks every write after `body::read` until all of them have
read their body, so every parse starts together. The script reads `VmHWM` of the hub's process and
`memory.peak` of its cgroup from the host, and the OOM flag from `docker inspect`.

| Verb | Writers | Pool | Idle VmHWM | Peak VmHWM | cgroup peak | OOM-killed | Exit |
|---|---|---|---|---|---|---|---|
| `measure` (the defaults) | 2 | 8 | 5 656 KiB | 404 580 KiB (395 MiB) | 413 130 752 B | false | 0 |
| `control` | 6 | 10 | 5 836 KiB | gone | unknown (cgroup removed) | true | 137 |

At the defaults the peak is 2 × 213.5 MB (the single-body peak above) less the slack of two
staggered parses: 38 % of the cap. Six writers need about 1.28 GB, and the kernel kills the hub at
the cap, which is what the negative control asserts. Both rows are in `scripts/orch/rows/hub.rows`
(`hub-memory`, `negctl-hub-memory`).

Caveat: the barrier aligns the start of every parse, not the peaks, so the measured peak can sit
below writers × the single-body peak. The control is sized to pass the cap even so.

## Commands

| # | Command | Exit |
|---|---|---|
| 1 | `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-hub --release --test memory -- --test-threads=1 --nocapture` (runs 1–3, before the ledger existed; `~/goinfre/logs/hub-mem-{2,3.1,3.2}.out`) | 101 |
| 2 | the same, with the ledger (`~/goinfre/logs/hub-mem-4.out`: `F_w` 50.92, entry 263.8 B, 2 passed in 23.23 s) | 0 |
| 3 | the same, debug profile (`~/goinfre/logs/hub-mem-debug.out`, 56.16 s) | 0 |
| 4 | `scripts/orch/hub-mem.sh measure` (`~/goinfre/logs/hub-mem-container-measure.out`) | 0 |
| 5 | `scripts/orch/hub-mem.sh control` (`~/goinfre/logs/hub-mem-container-control.out`) | 137 |

## Deviations from the plan

- `F_w` is the highest peak of a shape divided by its largest body, not a fit through the ladder:
  the budget multiplies `MAX_BODY`, so this is the quantity it needs.
- Each body runs in its own child process (see Conditions). The plan measured them in one process.
- The writer path is emulated in-process (body, lossy copy, parse, check, the store's plan). The
  database and the motor are not in the in-process measurement; the container run covers the whole
  hub at the writer term only.
- `rss.rs` is copied from graph-cli's reader of `/proc/self/status` rather than shared: graph-hub's
  tests do not depend on graph-cli.
- The container run sets `GRAPH_HUB_WRITERS_PER_KEY` to the writer count so one key can hold every
  writer permit at once; the default is 1.
- The reads, layouts and subscribers terms are arithmetic only. The records page carries no record
  values and the hub calls no motor yet, so no route reaches those peaks. The SSE-page case waits on
  Task 7 and the upload and throttled-upload rows on Task 8.
- The hub does not migrate its database on start; the container case migrates it
  (`support::db::migrated`) before the first write.
