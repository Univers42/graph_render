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
| `layout.forceatlas2.forcesim` | layout | O(n^2) per iteration (one f64 coefficient... | 2000 | 2000 (3075) | 621.2 | 38.7 | 8000 / 1136.2 | 2000 | 8000 |
| `layout.spectral3d` | layout | O(c^3) per component of c <= 256 nodes... | 700 | 256 (390) | 72.6 | 7.3 | 1024 / 61.2 | 256 | 1024 |
| `layout.mds.pivot3d` | layout | O(k (n + m)) time and O(n k) memory | 100000 | 100000 (154978) | 314.6 | 156.8 | 400000 / 460.3 | 100000 | 400000 |
| `layout.force.yifan_hu.2z` | layout | O(n log n) x (112 + 48 x levels) for the 2D... | 100000 | 65536 (101565) | 19452.9 | 132.1 | 131072 / 7680.2 | 32768 | 131072 |
| `layout.force.fruchterman_reingold.3d` | layout | O(niter * (n^2 + m)) with niter = 500 | 2000 | 2000 (3075) | 4721.3 | 8.5 | 8000 / 4761.6 | 2000 | 8000 |
| `layout.force.kamada_kawai.3d` | layout | O(n^2) set-up (all-pairs BFS | 2000 | 2000 (3075) | 7582.3 | 38.3 | 8000 / 8534.5 | 2000 | 8000 |
| `layout.force.drl.3d` | layout | O(S * n * deg) with S about 550 sweeps | 5000 | 2048 (3152) | 12491.5 | 8.6 | 8192 / 13623.8 | 2048 | 8192 |
| `layout.forceatlas2.3d` | layout | O(n^2) per iteration | 14000 | 14000 (21712) | 27039.5 | 25.3 | 32768 / 14486.7 | 8192 | 32768 |
| `layout.random.3d` | layout | O(n) | 1000000 | 1000000 (1549929) | 581.7 | 1399.7 | 4000000 / 1346 | 1000000 | 4000000 |
| `layout.force.yifan_hu.3d` | layout | O(n log n) x (112 + 48 x levels) for the... | 100000 | 65536 (101565) | 20743.1 | 140.4 | 131072 / 6671.2 | 32768 | 131072 |
| `layout.dag.dot` | layout | O(r + 2m + 2s) after k pivots on the... | 9200000 | 8192 (12692) | 13310.3 | 38.6 | failed | 8192 | 12692 |
largest peak at cap: 257.7 MiB (`layout.mds.pivot3d`)
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
| `layout.force.yifan_hu.3d` | n 65536 | 20743 | 12.44 |
| `layout.dag.dot` | n 16384; dense n 8192 exit 137 | killed at 40 s | 9.84 |
| `post.route.grid` | dense, over radial 21945 and over grid 34525 | 34525 | |
| `post.separate.grid` | n 6451 over `layout.packing.circle` | killed at 40 s | |

`layout.spectral3d` is bound by a refusal, not by time: at n 512 the motor refused the seeded graph with "parameter topology: no component passed the eigensolver's residual and orthonormality gate", while `layout.spectral` ran to its ceiling of 700. Its cap of 256 nodes and 1024 edges is the last rung that ran. The three rows `layout.forceatlas2.forcesim`, `layout.spectral3d` and `layout.mds.pivot3d` came from one later ladder run (2026-10-04, load1 14.9 on 20 cores), with the commands above and those three ids.

The five rows `layout.force.yifan_hu.2z`, `layout.force.fruchterman_reingold.3d`, `layout.force.kamada_kawai.3d`, `layout.force.drl.3d` and `layout.forceatlas2.3d` came from one more ladder run (2026-10-04, 34 rungs, load1 11.5 to 30.1 on 20 cores), with the commands above and those five ids, after develop registered them. `layout.force.yifan_hu.2z`, `layout.force.drl.3d` (its n 4096 rung was killed past 40 s) and `layout.forceatlas2.3d` are bound by time; the two others are at their ceiling of 2000.

The two rows `layout.random.3d` and `layout.force.yifan_hu.3d` came from one more ladder run (2026-10-04, 25 rungs, load1 8.48 to 12.90 on 20 cores), with the commands above and those two ids, after develop registered them; `tests/caps.rs` had been red on develop without them. `layout.random.3d` is at its ceiling of 1000000. `layout.force.yifan_hu.3d` is bound by time: n 65536 took 20743 ms, so its cap is the n 32768 rung (11183 ms), with the dense rung at 131072 edges in 6671 ms. The larger dense peak of the two, 2491.6 MiB for `layout.random.3d` at n 1000000, is under the run peak at cap below, so the per-slot budget is unchanged.

The row `layout.dag.dot` came from one more ladder run (2026-10-05, 7 rungs, load1 9.84 on 20 cores), with the commands above and that id, after develop registered it; `tests/caps.rs` had been red on develop without it. It is bound by time: n 16384 was killed past 40 s, so its cap is the n 8192 rung (13308 ms, 38.6 MiB). Its dense rung at n 8192 failed with exit 137, so its cap_m is the sparse rung's 12692 edges.

The row `layout.dag.lanes` came from one more ladder run (2026-10-06, 14 rungs, load1 3.39 to 3.67 on 20 cores), with the commands above and that id, before the layout landed, so `tests/caps.rs` never went red on develop for it. It is at its ceiling of 1000000: the n 1000000 rung took 979.3 ms (1473.5 MiB), and the dense rung at 4000000 edges took 2359.2 ms with a peak of 2775.7 MiB, under the run peak at cap below, so the per-slot budget is unchanged.

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
| ingest, `source=contract` | 1,224,659,341 | heap peak (counting allocator, `crates/graph-wasm/src/memory_measure.rs`) of `contract::derive` over a 67,108,938 B document (405,773 nodes, 3,257 ms and 5,045 ms over two runs). VmHWM rise was 1,943,719,936 and 2,120,769,536 B over the same two runs |
| run peak at cap | 3,343,908,864 | 3189 MiB, rounded up from 3188.2 MiB: VmHWM of `post.style.orthogonal` over `layout.circular.radial`, dense rung n 1048576, m 4194304 (`ladder.log` line 499). The next highest are `post.style.bezier` at 3187.9 MiB and bezier over grid at 3089.7 MiB |
| **per_slot** | **4,635,677,069** | 4.32 GiB, using the contract ingest term |

- Container limit for N workers is N × per_slot + base. Here `base` is the server's idle footprint,
  measured on the image below ("Base").
  - N = 1: 4,635,677,069 B
  - N = 2: 9,271,354,138 B
  - N = 4: 18,542,708,276 B
  - N = 20 (one per core here): 92,713,541,380 B
- Rule: `workers = min(cores, floor((memory.max − base) / per_slot))`, and the server refuses to start
  when this is 0. With base = 11 MiB (`BASE_BYTES`) and per_slot = 4,635,677,069 B:
  - memory.max 4 GiB gives 0 workers (4,283,386,880 / 4,635,677,069 = 0.92 → 0).
  - memory.max 8 GiB gives 1.
  - memory.max 16 GiB gives 3.
  - memory.max 32 GiB gives 7.
  - memory.max 64 GiB gives 14.
- The figure above is the default body's, and it is unchanged by review condition 1
  (`docs/reviews/review-svc-r3.md`): `svc-limits` and `negctl-svc-limits` above measured it at
  `GRAPH_MAX_BODY` = 64 MiB and the numbers stand. `per_slot_bytes(GRAPH_MAX_BODY)`
  (`server/graph-server/src/config/slots.rs`) now scales the body term and the ingest term with
  `GRAPH_MAX_BODY` instead of holding the 64 MiB figure, so only a body other than the default
  changes the budget: at the 1 GiB range ceiling one slot is 24,012,200,144 B and 8 GiB holds none,
  which is what row `svc-max-body` (`scripts/service-max-body.sh`) observes. The ingest term is a
  ratio read off the single 64 MiB body measured here, so a scaled figure is an estimate and not a
  measurement; an explicit `GRAPH_WORKERS` overrides the whole budget.

### Base

`BASE_BYTES` (`server/graph-server/src/config/slots.rs`) is the server's idle footprint, kept out of
the slots so a slot is never short of it. Measured 2026-10-04 on the image
`graph-motor:4412ecaad92ed124`: three starts at `--memory 8g --memory-swap 8g`, each read with
`docker exec <name> cat /sys/fs/cgroup/memory.current` once the `listening` line was in the log, no
request in flight.

| run | memory.current (B) |
|---|---:|
| 1 | 11,534,336 |
| 2 | 10,940,416 |
| 3 | 8,011,776 |

`BASE_BYTES = 11,534,336`, the largest of the three and already a whole MiB (11 MiB). What the
constant gets wrong:

- The three readings of an idle server differ by 3.5 MiB, so this is one sample's high-water mark, not
  an idle floor. It is the largest reading on purpose: `base` is subtracted from the budget, so
  overstating it grants one slot too few and understating it grants one too many.
- `memory.current` is the cgroup's, not the process's: it counts page cache and socket buffers the
  process never held as resident memory. The binary's own RSS is below it.
- It says nothing about what a busy slot's allocator arenas add on top. That is what `per_slot`
  carries, and `per_slot` is measured, not derived from `base`.
- It was measured on this host with an idle server, once. A different kernel, cgroup or base image
  moves it, and nothing in a row re-measures it.

### Ingest term for `source=contract`

The contract reader was quadratic in records (`graph_contract::ingest::validate` scanned earlier records
for every record: `check_unique_records` scanned `doc.records[..i]` for each record, `declares` scanned
every record for each link). Branch `fix-contract-quadratic` fixed the cause. These are the measurements
after the fix, from `scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture
ingest_peak` and `contract_read_time`. The generated document has one collection, a parent, a tag and one
link per record.

After the fix, `contract_read_time` doubles the body from 1 MiB to 64 MiB:

| body bytes | nodes | ms | heap peak bytes | heap / body |
|---|---:|---:|---:|---:|
| 1,048,681 | 6,728 | 39 | 19,355,531 | 18.46 |
| 2,097,295 | 13,328 | 125 | 38,637,325 | 18.42 |
| 4,194,445 | 26,341 | 271 | 77,095,288 | 18.38 |
| 8,388,753 | 52,172 | 324 | 153,814,381 | 18.34 |
| 16,777,249 | 103,764 | 701 | 307,241,473 | 18.31 |
| 33,554,583 | 205,183 | 2,117 | 613,816,759 | 18.29 |
| 67,108,938 | 405,773 | 4,764 | 1,224,659,341 | 18.25 |

- Time grows about 2× per doubling (linear), not 4×. The largest contract body that reads within 15 s is
  now 64 MiB.
- The heap is about 18.3× the body, and grows linearly with it. At 64 MiB the heap peak is 1,224,659,341 B
  (1.14 GiB), which is the contract ingest term that binds per_slot.

`ingest_peak` confirms the contract row at 64 MiB over two runs: body 67,108,938 B, 405,773 nodes,
3,257 ms and 5,045 ms, heap peak 1,224,659,341 B, VmHWM rise 1,943,719,936 and 2,120,769,536 B.

Before the fix, `contract_read_time` stopped at 4 MiB (the first read over 15 s). Those measurements, from
the same test on the unfixed reader:

| body bytes | nodes | ms, run 1 | ms, run 2 | heap peak bytes | heap / body |
|---|---:|---:|---:|---:|---:|
| 1,048,681 | 6,728 | 194 | 197 | 19,355,531 | 18.46 |
| 2,097,295 | 13,328 | 851 | 739 | 38,637,325 | 18.42 |
| 4,194,445 | 26,341 | 3,403 | 3,511 | 77,095,288 | 18.38 |
| 8,388,753 | 52,172 | 16,050 | 17,705 | 153,814,381 | 18.34 |

- Time grew about 4× per doubling (quadratic). The largest contract body that read within 15 s was 4 MiB.

## Measured on the image (2026-10-04)

The four rows of `scripts/orch/rows/service-limits.rows` below the `svc-max-body` pair, against the image
`graph-motor:4412ecaad92ed124` (`scripts/service.sh build`, 86,958,004 B). Each script starts the image
detached under a unique container name through `scripts/orch/drun`, mints its own key, and removes the
container on every exit path. `scripts/service-limits.sh` reads `PER_SLOT_BYTES` and `BASE_BYTES` from
`server/graph-server/src/config/slots.rs` with `grep`; none of the three scripts writes a number of the
budget down.

| row | command | exit |
|---|---|---:|
| `svc-limits` | `timeout 1800 scripts/service-limits.sh` | 0 |
| `negctl-svc-limits` | `SERVICE_LIMITS_MEM=1g …`, then `test $? -eq 1 && grep -q '^FAIL oom' …` | 0 |
| `svc-caps-time` | `timeout 5400 scripts/service-caps-time.sh` | 0 |
| `negctl-svc-caps-time` | `SERVICE_CAPS_TIME_TIMEOUT_MS=1 …`, then the `grep -c '^FAIL' …` equal to the tsv's row count | 0 |

### svc-max-body: GRAPH_MAX_BODY is inside the budget

Run 2026-10-04 on the image `graph-motor:a63165dbaacf9556` (87,092,840 B), row
`svc-max-body` of `scripts/orch/rows/service-limits.rows`. `GRAPH_WORKERS` unset,
`--memory 8g --memory-swap 8g` (the documented floor), and the body at the two ends of the range.
`scripts/service-max-body.sh` reads `BODY_BYTES`, `RUN_PEAK_BYTES` and `INGEST_PEAK_BYTES` from
`server/graph-server/src/config/slots.rs` with `grep` and does the same arithmetic as
`per_slot_bytes`, so it reports the figure it asked about rather than its own copy of it.

| `GRAPH_MAX_BODY` | `per_slot_bytes` | container | expected | container did | row |
|---:|---:|---|---|---|---|
| 1,073,741,824 (range ceiling) | 24,012,200,144 | 8 GiB | 0 slots, refuse | exit 2, `GRAPH_WORKERS: unset, and memory.max holds no slot` | PASS |
| 67,108,864 (default) | 4,635,677,069 | 8 GiB | 1 slot, serve | exit 124 at the 60 s bound, `listening` in the log | negctl PASS |

The negctl row passes only because the report holds `FAIL started`: at the default body the same
container serves, which is the whole point — the refusal above is a reading of the body, not of the
memory limit.

### svc-limits: one slot inside M

`GRAPH_WORKERS=1`, `--memory M --memory-swap M`, both requests
`layout.circular.radial` + `post.style.orthogonal`, the default `GRAPH_MAX_BODY` (64 MiB) and nothing
else raised.

| term | value |
|---|---:|
| M = `PER_SLOT_BYTES + BASE_BYTES`, rounded up to a whole MiB | 4,647,288,832 B (4432 MiB) |
| contract body | 67,108,770 B, 405,756 records → n 405,772, m 1,217,266 |
| studio body | 67,013,078 B, 134,217 nodes → n 134,217, m 208,010 |
| cgroup `memory.peak` over both requests | 1,620,025,344 B (34.9% of M) |
| load1 at the start and the end | 42.99 → 44.67 |

| check | verdict | detail |
|---|---|---|
| `status-contract` | PASS | HTTP 200, n 405,772, m 1,217,266 |
| `status-studio` | PASS | HTTP 200, n 134,217, m 208,010 |
| `oom` | PASS | OOMKilled false, running true |
| `peak` | PASS | memory.peak 1,620,025,344 ≤ 4,647,288,832 |

The peak is 34.9% of M, which is the over-estimate the `Caveat:` on `PER_SLOT_BYTES` names: the run peak
inside `per_slot` is the peak at n 1 048 576 (3,343,908,864 B), and a 64 MiB body carries 405,772 nodes at
most, so that run peak is not reachable through the default body limit. The budget holds with room to
spare; nothing here says it holds at n 1 048 576, which no body can ask for at the default limit.

The negative control, `SERVICE_LIMITS_MEM=1g`, is the same run under a 1 GiB container: the kernel
OOM-kills the server during the first request (OOMKilled true, running false, `memory.peak` unreadable
because the container is gone), so all four checks FAIL and the script exits 1.

### svc-caps-time: every cap at its cap, within GRAPH_TIMEOUT_MS

One container for the whole run: `--memory 8g`, `GRAPH_WORKERS=1`, `GRAPH_MAX_BODY` raised to its own
maximum 1,073,741,824 B, `GRAPH_TIMEOUT_MS` at its 30,000 default. The graph of a row is the one the cap
ladder used, `graph-cli bench --n <cap_n> --seed 1 --emit-scale-fixture`, whose m came out 1.5474 n.
`reduced:n>1000000` marks a row asked at 1,000,000 nodes because `graph-cli`'s own ceiling
(`graph_core::registry::MAX_BENCH_NODES`) is below the row's cap_n; `reduced:m>cap_m` would mark a graph
whose edges are past the cap, and no row hit it. Four runs so far. The first covered the 50 rows before `layout.mds.pivot3d`, load1 5.67 → 17.73. The second covered 55 rows after five rows were added on 2026-10-04 (image `graph-motor:5145db1479a10e68`, load1 9.79 → 10.19). The third, on 2026-10-04 at the tree of `0c1f32eb`, added `layout.random.3d` and `layout.force.yifan_hu.3d` and covered all 57 (image `graph-motor:a4839887f4b88f3f`, load1 5.48 → 1.98, run on its own under the host's timed lock). The fourth, on 2026-10-05 at the tree of `e6359099`, added `layout.dag.dot` and covered all 58 (image `graph-motor:28fed23253e99b82`, load1 1.94 → 5.92, run on its own under the host's timed lock). All 58 answered 200, and the table below is the fourth run.

| id | cap_n | cap_m | layout asked | n | status | ms | reduced |
|---|---:|---:|---|---:|---:|---:|---|
| `layout.grid` | 1048576 | 4194304 | `layout.grid` | 1000000 | 200 | 3223 | reduced:n>1000000 |
| `layout.tree.tidy` | 1048576 | 4194304 | `layout.tree.tidy` | 1000000 | 200 | 3152 | reduced:n>1000000 |
| `layout.treemap.squarified` | 1048576 | 4194304 | `layout.treemap.squarified` | 1000000 | 200 | 3299 | reduced:n>1000000 |
| `layout.circular.radial` | 1048576 | 4194304 | `layout.circular.radial` | 1000000 | 200 | 3038 | reduced:n>1000000 |
| `layout.packing.circle` | 2048 | 8192 | `layout.packing.circle` | 2048 | 200 | 5989 | - |
| `layout.spectral` | 700 | 2800 | `layout.spectral` | 700 | 200 | 19 | - |
| `layout.mds.pivot` | 100000 | 400000 | `layout.mds.pivot` | 100000 | 200 | 513 | - |
| `layout.force.barnes_hut` | 100000 | 154978 | `layout.force.barnes_hut` | 100000 | 200 | 13899 | - |
| `layout.forceatlas2` | 8192 | 32768 | `layout.forceatlas2` | 8192 | 200 | 5998 | - |
| `layout.dag.sugiyama` | 200000 | 800000 | `layout.dag.sugiyama` | 200000 | 200 | 987 | - |
| `layout.random` | 1000000 | 4000000 | `layout.random` | 1000000 | 200 | 2958 | - |
| `layout.circular.ring` | 1000000 | 4000000 | `layout.circular.ring` | 1000000 | 200 | 2953 | - |
| `layout.spiral` | 1000000 | 4000000 | `layout.spiral` | 1000000 | 200 | 2990 | - |
| `layout.bipartite` | 1000000 | 4000000 | `layout.bipartite` | 1000000 | 200 | 3013 | - |
| `layout.force.yifan_hu` | 65536 | 101565 | `layout.force.yifan_hu` | 65536 | 200 | 12673 | - |
| `layout.force.fruchterman_reingold` | 2000 | 8000 | `layout.force.fruchterman_reingold` | 2000 | 200 | 1614 | - |
| `layout.force.kamada_kawai` | 2000 | 8000 | `layout.force.kamada_kawai` | 2000 | 200 | 3925 | - |
| `layout.force.graphopt` | 2000 | 8000 | `layout.force.graphopt` | 2000 | 200 | 4408 | - |
| `layout.force.davidson_harel` | 500 | 768 | `layout.force.davidson_harel` | 500 | 200 | 2987 | - |
| `layout.force.lgl` | 1000 | 4000 | `layout.force.lgl` | 1000 | 200 | 166 | - |
| `layout.force.drl` | 5000 | 20000 | `layout.force.drl` | 5000 | 200 | 3195 | - |
| `layout.twopi` | 1000000 | 4000000 | `layout.twopi` | 1000000 | 200 | 3199 | - |
| `layout.packing.osage` | 1000000 | 4000000 | `layout.packing.osage` | 1000000 | 200 | 2919 | - |
| `layout.force.spring` | 8192 | 32768 | `layout.force.spring` | 8192 | 200 | 6367 | - |
| `layout.circular.hierarchy` | 1048576 | 4194304 | `layout.circular.hierarchy` | 1000000 | 200 | 3132 | reduced:n>1000000 |
| `layout.circular.circo` | 1000 | 1541 | `layout.circular.circo` | 1000 | 200 | 1152 | - |
| `layout.treemap.patchwork` | 1000000 | 4000000 | `layout.treemap.patchwork` | 1000000 | 200 | 2969 | - |
| `layout.force.neato` | 2048 | 8192 | `layout.force.neato` | 2048 | 200 | 8801 | - |
| `layout.force.fdp` | 1000 | 4000 | `layout.force.fdp` | 1000 | 200 | 5258 | - |
| `layout.basic3d.sphere` | 1000000 | 4000000 | `layout.basic3d.sphere` | 1000000 | 200 | 4707 | - |
| `layout.basic3d.helix` | 1000000 | 4000000 | `layout.basic3d.helix` | 1000000 | 200 | 3935 | - |
| `layout.basic3d.cube` | 1000000 | 4000000 | `layout.basic3d.cube` | 1000000 | 200 | 3310 | - |
| `layout.hierarchical3d` | 1000000 | 4000000 | `layout.hierarchical3d` | 1000000 | 200 | 3274 | - |
| `layout.force.spring3d` | 4096 | 16384 | `layout.force.spring3d` | 4096 | 200 | 1685 | - |
| `layout.force.sfdp` | 16384 | 65536 | `layout.force.sfdp` | 16384 | 200 | 1072 | - |
| `layout.forceatlas2.barnes_hut` | 65536 | 262144 | `layout.forceatlas2.barnes_hut` | 65536 | 200 | 5424 | - |
| `layout.bipartite_3d` | 1000000 | 4000000 | `layout.bipartite_3d` | 1000000 | 200 | 3369 | - |
| `layout.basic3d.spiral` | 1000000 | 4000000 | `layout.basic3d.spiral` | 1000000 | 200 | 3730 | - |
| `layout.force.particle_mesh` | 262144 | 406717 | `layout.force.particle_mesh` | 262144 | 200 | 8937 | - |
| `layout.forceatlas2.forcesim` | 2000 | 8000 | `layout.forceatlas2.forcesim` | 2000 | 200 | 1102 | - |
| `layout.spectral3d` | 256 | 1024 | `layout.spectral3d` | 256 | 200 | 72 | - |
| `layout.mds.pivot3d` | 100000 | 400000 | `layout.mds.pivot3d` | 100000 | 200 | 558 | - |
| `layout.force.yifan_hu.2z` | 32768 | 131072 | `layout.force.yifan_hu.2z` | 32768 | 200 | 6902 | - |
| `layout.force.fruchterman_reingold.3d` | 2000 | 8000 | `layout.force.fruchterman_reingold.3d` | 2000 | 200 | 1884 | - |
| `layout.force.kamada_kawai.3d` | 2000 | 8000 | `layout.force.kamada_kawai.3d` | 2000 | 200 | 4446 | - |
| `layout.force.drl.3d` | 2048 | 8192 | `layout.force.drl.3d` | 2048 | 200 | 6904 | - |
| `layout.forceatlas2.3d` | 8192 | 32768 | `layout.forceatlas2.3d` | 8192 | 200 | 8340 | - |
| `layout.random.3d` | 1000000 | 4000000 | `layout.random.3d` | 1000000 | 200 | 3445 | - |
| `layout.force.yifan_hu.3d` | 32768 | 131072 | `layout.force.yifan_hu.3d` | 32768 | 200 | 8792 | - |
| `layout.dag.dot` | 8192 | 12692 | `layout.dag.dot` | 8192 | 200 | 13628 | - |
| `post.route.grid` | 3225 | 4988 | `layout.grid` | 3225 | 200 | 9129 | - |
| `post.bundle.fdeb` | 4451 | 6900 | `layout.grid` | 4451 | 200 | 369 | - |
| `post.bundle.mingle` | 1935 | 3000 | `layout.grid` | 1935 | 200 | 394 | - |
| `post.separate.grid` | 4096 | 10000 | `layout.treemap.squarified` | 4096 | 200 | 1301 | - |
| `post.style.straight` | 1048576 | 4194304 | `layout.grid` | 1000000 | 200 | 3184 | reduced:n>1000000 |
| `post.style.orthogonal` | 1048576 | 4194304 | `layout.grid` | 1000000 | 200 | 3183 | reduced:n>1000000 |
| `post.style.bezier` | 1048576 | 4194304 | `layout.grid` | 1000000 | 200 | 3217 | reduced:n>1000000 |
| `post.style.quadratic` | 1048576 | 4194304 | `layout.grid` | 1000000 | 200 | 3266 | reduced:n>1000000 |

No cap was lowered: every row came in under `GRAPH_TIMEOUT_MS`, the slowest being
`layout.force.barnes_hut` at 13,899 ms, 46% of the mark, then `layout.dag.dot` at 13,628 ms (19,514 ms for `layout.force.yifan_hu` in the second run, under load1 near 10). The negative control,
`SERVICE_CAPS_TIME_TIMEOUT_MS=1`, answers 503 on every row of the table — all 58 in the fourth run (load1 5.92 → 5.69) — so the PASS above is a
reading of the status and of the elapsed time and not of anything else.

### Caveat of this section

- **One host, one request per row, no median.** `docs/measurements/service-caps.md` "Caveat" above
  applies to these numbers too, and load1 reached 44.67 during the `svc-limits` run. Load only makes a row
  slower, so a loaded host can turn a PASS into a FAIL and never the reverse.
- **Nine rows are not at their cap_n.** They are 4.7% under it, because the generator's own ceiling is
  1,000,000 nodes. A cap of 1,048,576 with no body limit behind it is unmeasured here.
- **`source=studio` for every row of this table.** No command line in the repo writes a contract document,
  and the contract reader derives its topology from records and links, so a contract document of the same
  size is a different graph and would not measure the rung the ladder measured. The studio body of
  n 1,000,000 is 499,854,493 B, which is why `GRAPH_MAX_BODY` had to be raised to the server's maximum.
- **`post.separate.grid` is asked over `layout.treemap.squarified`**, the post's other documented input.
  Over the binding input `layout.packing.circle` the pair is capped at 2,048 nodes, half the post's own
  cap, so the row would have measured half the work.
- **The ms include the upload and the ingest.** `GRAPH_TIMEOUT_MS` starts before the body is read, so the
  mark covers the whole request; the 4.5–8.0 s of the million-node rows is mostly a 500 MB body crossing
  the loopback and the studio reader parsing it, not the layout.
- **`memory.peak` is the cgroup's, not the process's**, and it never comes down between the two requests,
  so `status-studio` is charged `status-contract`'s peak.

## Caveat

The caps and per_slot are measurements of one machine under load, not bounds. In detail:

- **Loaded host.** load1 was 3.76–38.76 on 20 cores. Each rung was run once. A rung near 15 s can land on
  either side of the line from one run to the next. Load biases the caps low, which is the safe direction.
  It also means another host can be slower still. The CPU is the host's, not the service image's reference
  CPU. ("Measured on the image" above is still the host's CPU: the image runs the same binary on the same
  20 cores, with the body read and the ingest the ladder never timed.)
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
  - Chained posts at cap: these are bounded by `GRAPH_TIMEOUT_MS`, not by the caps.
  - Allocator fragmentation across many requests in a long-lived server process.
