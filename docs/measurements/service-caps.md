# Service caps and the per-slot memory budget

Measured 2026-10-03 for Verdict conditions 2 and 3 of `docs/contract/service-api.md`:

- the work cap per registry id, which the server checks after ingest and before the run;
- the memory one worker slot can hold, from which `GRAPH_WORKERS` and the container limit follow.

The server reads the caps from `service-caps.tsv` (`id\tcap_n\tcap_m`, one row per layout and post id).

## Conditions of the measurement

- Host: nproc 20, `13th Gen Intel(R) Core(TM) i5-13600KF`. Every process ran in the `ge-rust` image through
  `scripts/orch/gr` (8 GiB memory cap), on the host CPU. This is not the service image's reference CPU
  named in condition 2. The image is not built yet.
- **The host was loaded.** The develop full gate (`gate-develop-e124`, which holds `timed.lock`) and the
  land gates ran during the ladder. load1 ranged from 3.76 to 38.76, logged per rung. Load only makes a
  rung slower, so a loaded rung can only lower a cap. The time-bound rows below could each rise by about
  one rung on an idle host. They were not re-run, because the develop gate was still running.
- The binary was a release build of `graph-cli` at the tree of commit fd3ef725, built at 22:52. The ladder
  ran 22:52–23:13 and logged 568 rungs, 4 of them killed.
- Graphs come from `graph_core::seeded_model(seed, n, REFERENCE_DEGREE)`, with m/n between 1.52 and 1.6 at
  every rung. After the sparse ladder, one dense rung (`--edges-per-node 4`, so m = 4n, with uniform
  extra edges) runs at the largest sparse n that finished within 15 s.

## Commands

| # | Command | Exit |
|---|---|---|
| 1 | `scripts/orch/gr cargo build --release -p graph-cli` | 0 |
| 2 | `scripts/orch/gr sh -c 'target/release/graph-cli capabilities --json > target/caps/caps.json'` | 0 |
| 3 | `timeout 18000 scripts/orch/gr bash scripts/caps-ladder.sh target/caps/caps.json target/caps/ladder.log` | 0 |
| 4 | `scripts/orch/gr bash scripts/caps-table.sh target/caps/caps.json table target/caps/ladder.log` (the table below) | 0 |
| 5 | `scripts/orch/gr bash scripts/caps-table.sh target/caps/caps.json tsv target/caps/ladder.log > docs/measurements/service-caps.tsv` | 0 |
| 6 | `scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture ingest_peak` | stopped; see "Ingest term" |

- `graph-cli cap-probe --id <id> --n <n> [--edges-per-node k] [--input <layout>]` runs one rung in its own
  process (`crates/graph-cli/src/bench/cap_probe.rs`).
  - `ms` is the timed window: the run plus both output faces, binary and JSON. For a post, the input
    layout runs first and is not timed.
  - `peak_mib` is the process's VmHWM, read from `/proc/self/status`, after a `clear_refs` reset. It is
    whole-process RSS, including the records and the topology.
- `scripts/caps-ladder.sh` doubles n from 256 up to min(scale_ceiling, 2^20), with each rung under
  `timeout 40`. A post's ceiling counts edges, so its top n is about ceiling / 1.55. A post runs over
  `layout.grid` and `layout.circular.radial`. `post.separate.grid` moves circles and boxes, so it runs over
  `layout.packing.circle` and `layout.treemap.squarified`.

## The cap rule

- **cap n** is the largest sparse rung that finished within 15 s and lies below the first rung that went
  over 15 s, was killed or failed. It is also at most the id's scale_ceiling.
- **cap m** is the dense rung's m when that rung also finished within 15 s. Otherwise it is the m of the
  sparse rung at cap n. A post's cap m is also at most its ceiling.
- A post's cap is the smallest over its inputs. The table names the input that binds.
- Why 15 s: it is half of `GRAPH_TIMEOUT_MS` = 30000, so one layout and one post at their caps fit in
  one request. A chain of several posts at cap can still pass 30 s, and the server's timeout answers that
  with a 503.
- The server checks n and m against the layout's cap and against each post's cap. The cap that applies is
  the smallest of these.

## Caps

Columns 5–7 describe the largest sparse rung that finished within 30 s, on the input that binds. The
dense column shows "m / ms" of the dense rung. The table has one row per registry id. Rows follow
caps.json's order: layouts in registry order, then posts in ledger order. The server's test compares sets,
not order.

| id | kind | complexity | scale_ceiling | largest n ≤ 30 s (m) | wall ms at that n | peak RSS MiB at that n | dense rung (m / ms) | cap n | cap m |
|---|---|---|---|---|---|---|---|---|---|
| `layout.grid` | layout | O(n) | 4600000 | 1048576 (1624970) | 543.6 | 1485.9 | 4194304 / 1885.7 | 1048576 | 4194304 |
| `layout.tree.tidy` | layout | O(n) | 4600000 | 1048576 (1624970) | 6045.9 | 1610.2 | 4194304 / 1879.7 | 1048576 | 4194304 |
| `layout.treemap.squarified` | layout | O(n log n) | 4600000 | 1048576 (1624970) | 1083.5 | 1577.3 | 4194304 / 2566.7 | 1048576 | 4194304 |
| `layout.circular.radial` | layout | O(n + m) | 4600000 | 1048576 (1624970) | 1155.1 | 1517.7 | 4194304 / 2394.3 | 1048576 | 4194304 |
| `layout.packing.circle` | layout | O(n) exact path | 5000 | 2048 (3152) | 7062.4 | 23 | 8192 / 7800.5 | 2048 | 8192 |
| `layout.spectral` | layout | O(c^3) per component of c <= 256 nodes... | 700 | 700 (1089) | 29.4 | 6.6 | 2800 / 15.6 | 700 | 2800 |
| `layout.mds.pivot` | layout | O(k (n + m)) time and O(n k) memory | 100000 | 100000 (154978) | 310.3 | 158.3 | 400000 / 453.2 | 100000 | 400000 |
| `layout.force.barnes_hut` | layout | O(n log n) per tick x TICKS=112 | 100000 | 100000 (154978) | 13592.7 | 174.2 | 400000 / 15781.6 | 100000 | 154978 |
| `layout.forceatlas2` | layout | O(n^2) per iteration x max_iter=100 | 14000 | 14000 (21712) | 19707.2 | 24.7 | 32768 / 6847.7 | 8192 | 32768 |
| `layout.dag.sugiyama` | layout | one O(m log m) sort of the arc list up front | 200000 | 200000 (310274) | 504 | 335.3 | 800000 / 614.5 | 200000 | 800000 |
| `layout.random` | layout | O(n) | 1000000 | 1000000 (1549929) | 512.4 | 1398.9 | 4000000 / 1131.5 | 1000000 | 4000000 |
| `layout.circular.ring` | layout | O(n) | 1000000 | 1000000 (1549929) | 536.7 | 1406.6 | 4000000 / 1114.4 | 1000000 | 4000000 |
| `layout.spiral` | layout | O(n) | 1000000 | 1000000 (1549929) | 561.9 | 1400.3 | 4000000 / 1114.3 | 1000000 | 4000000 |
| `layout.bipartite` | layout | O(n + m) | 1000000 | 1000000 (1549929) | 883.9 | 1414.4 | 4000000 / 2725.6 | 1000000 | 4000000 |
| `layout.force.yifan_hu` | layout | O(n log n) per tick per level | 100000 | 100000 (154978) | 23398.6 | 201 | 262144 / 17381.7 | 65536 | 101565 |
| `layout.force.fruchterman_reingold` | layout | O(niter * (n^2 + m)) with niter = 500 | 2000 | 2000 (3075) | 1695.5 | 8.2 | 8000 / 1617.5 | 2000 | 8000 |
| `layout.force.kamada_kawai` | layout | O(n^2) set-up (all-pairs BFS | 2000 | 2000 (3075) | 3178.1 | 38 | 8000 / 3506.9 | 2000 | 8000 |
| `layout.force.graphopt` | layout | O(niter * (n^2 + m)) with niter = 500 | 2000 | 2000 (3075) | 5895.3 | 8 | 8000 / 3050 | 2000 | 8000 |
| `layout.force.davidson_harel` | layout | O(rounds * 30 * n * (n + deg(v) m)) with 10... | 500 | 500 (768) | 6740 | 6 | killed | 500 | 768 |
| `layout.force.lgl` | layout | O(L * maxit * (m + n * c)) over L... | 1000 | 1000 (1541) | 238.4 | 7.1 | 4000 / 115.4 | 1000 | 4000 |
| `layout.force.drl` | layout | O(S * n * (deg + 441)) with S about 550... | 5000 | 5000 (7721) | 3248.9 | 12.8 | 20000 / 5286.3 | 5000 | 20000 |
| `layout.twopi` | layout | O(n + m) | 1000000 | 1000000 (1549929) | 1181.9 | 1413.6 | 4000000 / 1929.8 | 1000000 | 4000000 |
| `layout.packing.osage` | layout | O(n) and no more | 1000000 | 1000000 (1549929) | 513.4 | 1389 | 4000000 / 1074.6 | 1000000 | 4000000 |
| `layout.force.spring` | layout | O(n^2) per iteration x iterations=50 | 16000 | 8192 (12692) | 14063.1 | 17.1 | 32768 / 6964.4 | 8192 | 32768 |
| `layout.circular.hierarchy` | layout | O(n + m) | 4600000 | 1048576 (1624970) | 900.3 | 1497.5 | 4194304 / 2992.5 | 1048576 | 4194304 |
| `layout.circular.circo` | layout | O(n + m) to find the blocks and O(k^3) to... | 1000 | 1000 (1541) | 7408.4 | 7.3 | killed | 1000 | 1541 |
| `layout.treemap.patchwork` | layout | O(n) | 1000000 | 1000000 (1549929) | 868.8 | 1397.1 | 4000000 / 1491.4 | 1000000 | 4000000 |
| `layout.force.neato` | layout | O(k n^2) time and O(n^2) space | 10000 | 2048 (3152) | 11552.1 | 31.5 | 8192 / 8347.9 | 2048 | 8192 |
| `layout.force.fdp` | layout | O(pass1 * (m + n)) for the expansion phase... | 1000 | 1000 (1541) | 5560.5 | 6.7 | 4000 / 6088.3 | 1000 | 4000 |
| `layout.basic3d.sphere` | layout | O(n) | 1000000 | 1000000 (1549929) | 873 | 1422 | 4000000 / 1758.4 | 1000000 | 4000000 |
| `layout.basic3d.helix` | layout | O(n) | 1000000 | 1000000 (1549929) | 813.9 | 1425.1 | 4000000 / 1700.5 | 1000000 | 4000000 |
| `layout.basic3d.cube` | layout | O(n) | 1000000 | 1000000 (1549929) | 855.4 | 1424.6 | 4000000 / 1820.9 | 1000000 | 4000000 |
| `layout.hierarchical3d` | layout | O(n + m) for the roots (two BFS sweeps per... | 1000000 | 1000000 (1549929) | 1757.6 | 1430.7 | 4000000 / 3954.4 | 1000000 | 4000000 |
| `layout.force.spring3d` | layout | O(n^2) per iteration x iterations=50 | 16000 | 8192 (12692) | 17182.8 | 17.4 | 16384 / 4407.4 | 4096 | 16384 |
| `layout.force.sfdp` | layout | O((n + m) log n) per iteration over the... | 50000 | 32768 (50768) | 21354.7 | 855.8 | 65536 / 8634.2 | 16384 | 65536 |
| `layout.forceatlas2.barnes_hut` | layout | O(n log n) per iteration (one quadtree build | 250000 | 131072 (203148) | 15157 | 217.4 | 262144 / 6621 | 65536 | 262144 |
| `layout.bipartite_3d` | layout | O(n + m) | 1000000 | 1000000 (1549929) | 1077.6 | 1423.7 | 4000000 / 3349.5 | 1000000 | 4000000 |
| `layout.basic3d.spiral` | layout | O(n + 2^16) | 1000000 | 1000000 (1549929) | 873.4 | 1423.8 | 4000000 / 2210.7 | 1000000 | 4000000 |
| `layout.force.particle_mesh` | layout | O(n + P^2 log P) per tick x TICKS=112 | 800000 | 262144 (406717) | 11634.7 | 420.6 | 1048576 / 15424.2 | 262144 | 406717 |
| `post.route.grid` | post, over `layout.grid` | O(m · cells · log cells) | 5000 | 3225 (4988) | 9447 | 35.3 | 12900 / 34524.9 | 3225 | 4988 |
| `post.bundle.fdeb` | post, over `layout.grid` | O(m^2) to build the pair list once | 6900 | 4451 (6867) | 558.4 | 30.5 | 17804 / 2488.7 | 4451 | 6900 |
| `post.bundle.mingle` | post, over `layout.grid` | O(rounds x passes x (m^2 log m proximity + m... | 3000 | 1935 (2983) | 608.1 | 11.2 | 7740 / 3638.2 | 1935 | 3000 |
| `post.separate.grid` | post, over `layout.packing.circle` | O(n x k x I) | 10000 | 4096 (6338) | 722.3 | 46.1 | 16384 / 472.4 | 4096 | 10000 |
| `post.style.straight` | post, over `layout.grid` | O(n + m x s) | 9100000 | 1048576 (1624970) | 736.8 | 1495.5 | 4194304 / 1135.2 | 1048576 | 4194304 |
| `post.style.orthogonal` | post, over `layout.grid` | O(n + m x s) | 9100000 | 1048576 (1624970) | 1018.8 | 1621.4 | 4194304 / 2287.3 | 1048576 | 4194304 |
| `post.style.bezier` | post, over `layout.grid` | O(n + m x s) | 9100000 | 1048576 (1624970) | 1583.2 | 1647.9 | 4194304 / 4009.8 | 1048576 | 4194304 |
| `post.style.quadratic` | post, over `layout.grid` | O(n + m x s) | 9100000 | 1048576 (1624970) | 871.8 | 1588.5 | 4194304 / 3286.6 | 1048576 | 4194304 |

### Rows bound by time, and the load they ran under

Each row below is capped below its scale_ceiling by a rung that went over 15 s or was killed. All of
them ran on the loaded host, so they could rise on a re-run.

| id | first rung over the limit | ms | load1 |
|---|---|---:|---:|
| `layout.packing.circle` | n 4096 | 36597 | 19.4 |
| `layout.force.barnes_hut` | dense, n 100000 | 15782 | 11.1 |
| `layout.forceatlas2` | n 14000 | 19707 | 9.2 |
| `layout.force.yifan_hu` | n 100000; dense n 65536 is 17382 | 23399 | 15.4 |
| `layout.force.davidson_harel` | dense, n 500 | killed at 40 s | |
| `layout.force.spring` | n 16000 | 30942 | 16.5 |
| `layout.circular.circo` | dense, n 1000 | killed at 40 s | |
| `layout.force.neato` | n 4096 | killed at 40 s | |
| `layout.force.spring3d` | n 8192 | 17183 | 35.7 |
| `layout.force.sfdp` | n 32768 | 21355 | 34.8 |
| `layout.forceatlas2.barnes_hut` | n 131072 | 15157 | 38.75 |
| `layout.force.particle_mesh` | n 524288 at 30025; dense n 262144 is 15424 | 30025 | 28.3 |
| `post.route.grid` | dense, over radial 21945 and over grid 34525 | 34525 | |
| `post.separate.grid` | n 6451 over `layout.packing.circle` | killed at 40 s | |

The `post.separate.grid` kill comes from its input. `layout.packing.circle` runs untimed before the post
and is slow at 6451 nodes. Over `layout.treemap.squarified` the post reached 6451 nodes in 1.58 s. Its cap
of 4096 nodes and 10000 edges also satisfies either reading of `SEPARATE_CEILING`, which
`crates/graph-core/src/post/separate/params.rs` documents in nodes while posts count their ceiling in edges
(`crates/graph-core/src/post/mod.rs:74`). That is a spec gap, and the ladder took the edge reading, which
gives the lower cap.

## Memory per slot (condition 3)

per_slot = body + ingest peak + run peak at cap.

| Term | Bytes | How it was measured |
|---|---:|---|
| body | 67,108,864 | `GRAPH_MAX_BODY` default, 64 MiB |
| ingest, `source=studio` | 141,099,952 | heap peak (counting allocator, `crates/graph-wasm/src/memory_measure.rs`) of `ingest::read_records` + `ingest::index` over a 68,459,255 B seeded document (141,879 nodes, 538 ms). VmHWM rise was 115,249,152 and 126,001,152 B over two runs |
| ingest, `source=contract` | **pending fix-contract-quadratic** | see below |
| run peak at cap | 3,343,908,864 | 3189 MiB, rounded up from 3188.2 MiB: VmHWM of `post.style.orthogonal` over `layout.circular.radial`, dense rung n 1048576, m 4194304 (`ladder.log` line 499). The next highest are `post.style.bezier` at 3187.9 MiB and bezier over grid at 3089.7 MiB |
| **per_slot** | **3,552,117,680** | 3.31 GiB, using the studio ingest term |

- Container limit for N workers is N × per_slot + base. Here `base` is the server's idle RSS, which
  `svc-limits` measures.
  - N = 1: 3,552,117,680 B
  - N = 2: 7,104,235,360 B
  - N = 4: 14,208,470,720 B
  - N = 20 (one per core here): 71,042,353,600 B
- Rule: `workers = min(cores, floor((memory.max − base) / per_slot))`, and the server refuses to start
  when this is 0. With base = 0 this is `min(cores, floor(memory.max / per_slot))`.
  - memory.max 4 GiB gives 1 worker.
  - memory.max 8 GiB gives 2.
  - memory.max 16 GiB gives 4.
  - memory.max 32 GiB gives 9.
  - memory.max 64 GiB gives 19.

### Ingest term for `source=contract`: pending fix-contract-quadratic

The contract reader is quadratic in records, so its term is not final. `graph_contract::ingest::validate`
scans earlier records for every record:

- `check_unique_records` scans `doc.records[..i]` for each record
  (`crates/graph-contract/src/ingest/validate.rs:158-171`);
- `declares` scans every record for each link (`crates/graph-contract/src/ingest/validate/cells.rs:101`);
- `collection()` is a linear find (`crates/graph-contract/src/ingest.rs:241`).

Branch `fix-contract-quadratic` fixes the cause. The 64 MiB run was stopped because it measured the bug.
These are the measurements before the fix, from
`scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture contract_read_time`. The
generated document has one collection, a parent, a tag and one link per record. The test doubles the body
and stops after the first read over 15 s.

| body bytes | nodes | ms, run 1 | ms, run 2 | heap peak bytes | heap / body |
|---|---:|---:|---:|---:|---:|
| 1,048,681 | 6,728 | 194 | 197 | 19,355,531 | 18.46 |
| 2,097,295 | 13,328 | 851 | 739 | 38,637,325 | 18.42 |
| 4,194,445 | 26,341 | 3,403 | 3,511 | 77,095,288 | 18.38 |
| 8,388,753 | 52,172 | 16,050 | 17,705 | 153,814,381 | 18.34 |

- Time grows about 4× per doubling. Before the fix, the largest contract body that reads within 15 s is
  4 MiB.
- The heap is about 18.3× the body, and grows linearly with it.
  - Estimated at 64 MiB: 18.3 × 67,108,864 ≈ 1.23 GB. That would make per_slot about 4,639,109,939 B
    (4.32 GiB).
  - With a contract body limit of 4 MiB, the term is 77,095,288 B. The studio term then binds and per_slot
    stays 3,552,117,680 B.
- These figures will be re-measured with `ingest_peak` once the fix lands.

## Caveat

The caps and per_slot are measurements of one machine under load, not bounds. In detail:

- **Loaded host.** load1 was 3.76–38.76 on 20 cores. Each rung was run once. A rung near 15 s can land on
  either side of the line from one run to the next. Load biases the caps low, which is the safe direction.
  It also means another host can be slower still. The CPU is the host's, not the service image's reference
  CPU.
- **Doubling gap.** Sizes between two rungs are never tried. The largest size under 15 s can be up to
  twice the cap reported.
- **Synthetic density.** The model's m/n is 1.52–1.6, plus one dense rung at m = 4n. A graph with the same n
  but a different degree distribution, or a time-bound layout given a denser input than 4n, can run longer
  at the same cap. cap m bounds the edge count, not the shape.
- **How RSS was sampled.** peak_mib is VmHWM after a `clear_refs` reset, for the whole process. Pages the
  allocator kept from earlier frees are not counted again. The ingest term uses the counting allocator's
  heap peak instead, which does count them.
- **per_slot over-estimates.**
  - The run peak already includes the records and topology that ingest built, so adding the ingest term
    counts them twice.
  - A 64 MiB studio body carries about 142k nodes, so the 2^20-node run peak cannot be reached through the
    body limit. A tighter bound would be the run peak at the largest n a 64 MiB body can carry.
  - The double count and the unreachable size both raise per_slot, so the error is on the safe side.
- **Not covered.**
  - The contract ingest term: pending, see above.
  - Chained posts at cap: these are bounded by `GRAPH_TIMEOUT_MS`, not by the caps.
  - Allocator fragmentation across many requests in a long-lived server process.
