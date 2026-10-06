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
| `upload_doc_bytes` | 67108842 | 67108842 | the `/layout` document, 22 B under `GRAPH_HUB_MAX_DOC_BYTES` |
| `upload_records` | 745633 | 745633 | records in that document |
| `upload_record_bytes` | 89 | 89 | one record's stored text, separators excluded |
| `upload_id_width` | 5 | 5 | the id's fixed width in base 36 |
| `upload_plugins` | 8 | 8 | plugins the records are spread over |
| `upload_batches` | 75 | 75 | batch `POST`s the fill took |
| `upload_ms` | 7847 8482 7773 7737 7813 | the same | the five timed uploads, in log order |
| `upload_median_ms` | 7813 | 7813 | their median |
| `upload_slowest_ms` | 8482 | 8482 | the slowest, which §5.3's condition is over |
| `upload_chunks` | 745635 | 745635 | chunks in one upload: one per record, plus head and tail |
| `upload_chunk_bytes` | 90 | 90 | mean chunk size, the knob a miss names |
| `upload_budget_ms` | 8000 | 8482 | §5.3's budget, against the slowest run — **a miss** |

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

## The /layout upload (Decision 4)

**Command**, verbatim, the whole expansion of `scripts/orch/hub-mem.sh upload` (gate row
`hub-upload-timeout` runs `scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start &&` before it):

```
scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start \
  && GRAPH_HUB_MOTOR_URL=http://<motor bridge ip>:8080 \
     GRAPH_HUB_MAX_DOC_BYTES=67108864 GRAPH_HUB_MAX_RECORD_BYTES=1048576 \
     HUB_MOTOR_KEY_FILE=target/hub-mem/motor-key HUB_RUN_MEMORY=1g \
     scripts/orch/hub-run.sh start \
  && scripts/orch/hub-mem.sh upload
```

Two containers: the hub (`scripts/orch/hub-run.sh`, `--memory 1g --memory-swap 1g` through
`HUB_RUN_MEMORY`) and a real graph-server as the motor (`scripts/service.sh run`, `SERVICE_PORT=0`
so no host port is published). The hub reaches the motor at its **bridge** address, so the path
measured is container to container.

**Input size**, read back off the store by the client case (`tests/memory/upload.rs`), not computed
in advance:

| # | Quantity | Value |
|---|---|---|
| `upload_doc_bytes` | the workspace's `doc_bytes`, the number the cap is enforced on (`writer/plan.rs:275`) | 67 108 842 |
| `upload_records` | records in the document | 745 633 |
| `upload_record_bytes` | one record's stored text, separators excluded | 89 |
| `upload_id_width` | the id's fixed width in base 36, because ids must be distinct and a document this size holds ~7.5·10⁵ of them | 5 |
| `upload_plugins` | plugins the records are spread over; `GRAPH_HUB_MAX_PLUGIN_BYTES` is 16 MiB, so one plugin cannot hold 64 MiB | 8 |
| `upload_batches` | batch `POST`s the fill took (`max_batch` is 10 000 operations) | 75 |

The record is the smallest the contract admits: one collection, one scalar cell, and the four members
`read_batch` requires. That is what makes the document hold as many records as the cap allows, so the
upload is the relay's worst case and not its best. The plan's "one-character id" is not reachable —
ids must be distinct and 36 one-character ids cannot fill 64 MiB — so the ids are the shortest
distinct ones of a **fixed** width, which is what lets one `record_bytes` stand for all of them.

**Pass condition** (§5.3, N4): the slowest of five timed uploads finishes in **under 8 000 ms** —
two seconds under graph-server's `GRAPH_BODY_TIMEOUT_MS` default of 10 000
(`server/graph-server/src/config.rs:196`) — with no 408 in the motor's log and every `Graph-Seq`
equal to the `/graph` ETag at the same cursor. One warm-up precedes the five and is excluded. A miss
is a **stop**, not a retune.

**The five numbers and the median**, from the run gate row `hub-upload-timeout` recorded
(`target/gate-hub-upload/hub-upload-timeout.log`):

| # | Quantity | Value |
|---|---|---|
| `upload_ms` | the five `upload_ms`, in log order | 7847 · 8482 · 7773 · 7737 · 7813 |
| `upload_median_ms` | their median | 7813 |
| `upload_slowest_ms` | the slowest, which is what the condition is over | 8482 |

A second run of the same row, minutes earlier, gave **7410 · 7492 · 7671 · 7708 · 8133** (median
7671, slowest 8133) on the identical input — the figures above are the gate's, and the two runs are
both recorded because the spread is part of the result: the run-to-run range here is about 800 ms,
which is the same order as the 2 s of headroom §5.3 asks for.

**Chunk size**, the knob a miss names: the walk yields one record per chunk plus a head and a tail,
so one upload is **745 635** chunks and the mean chunk is **90 bytes**. A miss is this number that
grows — the relay's chunking is one record per `Document::next()` (`relay/body.rs`), and a larger
chunk would be a change to graph-store's walk, not to the hub.

**Result: a miss, and a STOP.** The slowest of the five was **8 482 ms**, over the 8 000 ms budget, so
`scripts/orch/hub-mem.sh upload` exits 1, gate row `hub-upload-timeout` **FAILS**, and this file
records the numbers rather than a retune (§5.3: "A miss is a stop, not a retune"). Both runs missed on
the slowest run alone; the median was inside the budget both times (7813 and 7671 ms). Nothing was
retuned to reach the number.

What the miss is **not**: the motor answered every upload **200**, its log held no 408, and every
`Graph-Seq` matched the `/graph` ETag. So the relay reached the motor, and the motor read the whole
64 MiB inside its own 10 s `GRAPH_BODY_TIMEOUT_MS` with roughly 1.5 s to spare. The 64 MiB upload
takes the hub about 7.8 s of streaming on this host and the motor about 2.1 s of parsing; it is the
hub's streaming cost that puts the slowest run 482 ms over an 8 s budget, not the motor's timeout.

**The throttled control** (`negctl-throttle-upload`, `GM_HUB_BREAK=throttle-upload`): the relay
sleeps 2 ms before yielding each record (`relay/body.rs`, `breaks.rs`), which at 745 633 records is
about 25 minutes of upload. The motor answered **408 at 10 001 ms** — its `GRAPH_BODY_TIMEOUT_MS`
doing exactly what the un-throttled run stayed inside — and the hub mapped it to **502**
`MotorBodyTimeout`. The verb exits 1, the row (which expects a non-zero exit) **PASSES**, and
`target/hub-mem/upload.txt` names every clause that failed. So the measurement can fail, and it fails
differently from the plain miss above: here the motor refuses, there it answers.

**Caveat**: this is loopback between two containers on one host, so it measures the hub's own
streaming cost and the socket between the two containers, **not a network**. A deployment whose
motor is a hop and a queue away has a different number, and the 2 s of headroom §5.3 asks for is
headroom against graph-server's timeout, not against a network.

**Caveat**: `upload_ms` runs from the first poll of the body stream to the yield of the tail
(`relay/upload.rs`), so it includes the hub's own scheduling before the first chunk and excludes the
motor's answer entirely. It is the relay's cost, not the exchange's.

### Deviation: a test-case client plus the relay's log event, not a `graph-hub upload-measurement` subcommand

Decision 4 records the expansion as `cargo run … -p graph-hub -- upload-measurement`. This runs as
`tests/memory/upload.rs`, one `#[ignore]`d case against a hub the script started, plus the relay's
own `layout-upload` log line (`relay/upload.rs`) for the timing. Two reasons, both measured:

- The timing must be of the **shipped** relay inside its **container**, reading
  `GRAPH_HUB_MAX_DOC_BYTES` from the environment the hub was started with. A subcommand in the same
  binary would have to re-read that environment, and a disagreement between the two readings would be
  a measurement of the wrong document.
- The client has to *write* ~745 000 records through the hub before it can time an upload of them, and
  `support::wire::Remote` (the container-level client `scripts/orch/hub-run.sh` already serves) is
  what writes them. The case is driven by `scripts/orch/hub-mem.sh upload`, which is what
  `scripts/hub.sh upload-measurement` now execs — so one script owns the containers, the verdict and
  `target/hub-mem/upload.txt`, and the row and the verb cannot disagree.

## Commands

| # | Command | Exit |
|---|---|---|
| 1 | `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-hub --release --test memory -- --test-threads=1 --nocapture` (runs 1–3, before the ledger existed; `~/goinfre/logs/hub-mem-{2,3.1,3.2}.out`) | 101 |
| 2 | the same, with the ledger (`~/goinfre/logs/hub-mem-4.out`: `F_w` 50.92, entry 263.8 B, 2 passed in 23.23 s) | 0 |
| 3 | the same, debug profile (`~/goinfre/logs/hub-mem-debug.out`, 56.16 s) | 0 |
| 4 | `scripts/orch/hub-mem.sh measure` (`~/goinfre/logs/hub-mem-container-measure.out`) | 0 |
| 5 | `scripts/orch/hub-mem.sh control` (`~/goinfre/logs/hub-mem-container-control.out`) | 137 |
| 6 | `scripts/orch/hub-mem.sh upload` (Decision 4; `target/gate-hub-upload/hub-upload-timeout.log`) | 1 |
| 7 | the same with `GM_HUB_BREAK=throttle-upload` (the control) | 1 |

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
- The `/layout` upload runs as a test-case client plus the relay's own `layout-upload` log event
  rather than a `graph-hub upload-measurement` subcommand; see the Decision 4 section above.
- The upload's records are spread over 8 plugins. Decision 4 asks for one document at
  `GRAPH_HUB_MAX_DOC_BYTES`, which one plugin cannot hold: `GRAPH_HUB_MAX_PLUGIN_BYTES` is 16 MiB
  (§6's default) and the store refuses a batch past it with a 413, which the fill hit at batch 18 of
  one-plugin version before this change.
- `scripts/orch/hub-mem.sh upload` writes the motor's plaintext key into `target/hub-mem/motor-key`
  **from a container running as uid 10001**, at mode 0600. The hub container reads that file as
  10001, and a 0600 file owned by the host user is unreadable to it: the relay's `bearer()` turned
  the unreadable file into `MotorUnavailable`, so every `/layout` answered 502 in 6 ms. The host user
  cannot `chown` to 10001, which is why a container creates it.
