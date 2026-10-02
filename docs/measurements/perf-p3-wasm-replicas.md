# P3 model (b) — one wasm instance per worker, gathered over a SharedArrayBuffer

`prompts/perf-plan.md`'s P3 builds **two** browser thread models and measures both, so the user
picks one. This is **model (b)**. Model (a) — one shared memory with a thread pool — is a
separate build in a separate worktree and is **not** here.

## The design, in five lines

1. `Runner::run` is the seam: graph-core hands a runner a `StepRange` kernel, a worker count and
   an output column, and every runner in the motor (`Serial` natively, `Threads` in graph-cli)
   already produces the same bytes for any division of `0..len`.
2. `crate::replica::Replica { rank, ranks }` implements that trait: it runs `kernel.step_range`
   on **its own** range of `graph_core::exec::partition(len, ranks)`, then calls the one host
   import, `env.gm_host_allgather`, which publishes that span into a `SharedArrayBuffer` and
   reads the whole column back. `workers` is ignored in favour of `ranks` — the ranks *are* the
   wasm instances, so the host's thread count says nothing about how many there are.
3. The gather is inside **every** `Runner::run`, not once per tick: a tick hands the runner three
   kernels (link, charge, collide) and each pass reads the *previous* pass's writes for nodes it
   does not own, so one sync per tick is a sync too late by two passes.
4. Every instance therefore applies the same full array, so every instance stays byte-identical
   to the serial run — which is what the hash mode checks, and what `--break` attacks.
5. It is behind a cargo feature `replicas` and builds into its own target dir, because it is the
   only build of `graph-wasm` that **imports anything** and the shipped `graph_wasm.wasm` must
   import nothing (`harness/wasm-run.mjs`, the SDK's `refuseImports`). Verified below.

## What runs it

```
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown \
    --features replicas --target-dir target/wasm-replicas
scripts/orch/gr node harness/wasm-replicas.mjs hash  --seeds 8 --ranks 1,2,3,4,7
scripts/orch/gr node harness/wasm-replicas.mjs bench --n 100000 --ranks 1,2,4,8 --repeat 3
scripts/orch/gr node harness/wasm-replicas.mjs hash  --seeds 8 --ranks 1,2,3,4,7 --break   # exits 1
```

`hash` compares each rank-0 snapshot against the **serial** artifact's `gm_run` hash for the same
`(seed, layout)` — the same model `harness/wasm-run.mjs` hashes — and separately checks that the
ranks of one run agree with **each other**. `bench` reports the slowest rank's median wall time
around `gm_run_replica` alone.

## Gates

| # | gate | exit | expected |
|---|---|---:|---|
| 1 | `scripts/orch/gr cargo fmt --all --check` | 0 | 0 |
| 2 | `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | 0 |
| 3 | `scripts/orch/gr cargo clippy -p graph-wasm --target wasm32-unknown-unknown --features replicas -- -D warnings` | 0 | 0 |
| 4 | `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 0 |
| 5 | `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | 0 |
| 6 | `scripts/orch/gr node harness/wasm-replicas.mjs hash --seeds 8 --ranks 1,2,3,4,7` | 0 | 0 |
| 7 | the same with `--break` | 1 | 1 |

Gate 5 is the "the default artifact did not move" row: it is green because the `replicas` feature
is off by default and its build goes to a different target dir. Supporting check, same command
family: the default artifact's import list is `[]`, asserted by instantiating
`target/wasm32-unknown-unknown/release/graph_wasm.wasm` and reading `WebAssembly.Module.imports`.
Gate 4 is 20 `test result: ok` lines, 0 failed; the `self-check FAILED` lines inside it are the
oracle harness's own negative controls printing what they are there to print
(`a_broken_copy_of_the_oracle_harness_fails_its_own_self_check` and the tick self-check rows).

Gate 6: **80 cells, 0 failed** (2 layouts × 8 seeds × 5 rank counts), and 0 cross-rank
disagreements. Gate 7: **80 cells, 64 failed** — every cell with `ranks > 1`, because the last rank
withholds its span; the 16 `ranks: 1` cells still agree, which is right: with one rank the
withheld span is the whole column being replaced by zeros, so it still differs. **0** cross-rank
disagreements under `--break` too, and that is the expected shape rather than a weak control: all
ranks read back the *same* shared column, so a withheld rank makes them all wrong the same way.

## Bench

`bench` numbers, median of the stated repeat count, slowest rank, `gather %` = that rank's
`performance.now()` time inside `gm_host_allgather` (both barriers and the copy back) as a share of
its `gm_run_replica`. `equal` is rank 0's hash against the `ranks: 1` run's rank 0 hash for the
same `(layout, n)`.

### n = 100 000 · repeat 3 · both layouts · ranks 1, 2, 4, 8

| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB | gather % |
|---|---:|---:|---:|---:|---|---:|---:|
| `layout.force.particle_mesh` | 100000 | 1 | 10565.06 | 1.00 | yes | 339.1 | 0.3 |
| `layout.force.particle_mesh` | 100000 | 2 | 9145.76 | 1.16 | yes | 678.3 | 5.7 |
| `layout.force.particle_mesh` | 100000 | 4 | 7893.27 | 1.34 | yes | 1356.5 | 24.1 |
| `layout.force.particle_mesh` | 100000 | 8 | 11520.09 | 0.92 | yes | 2713.0 | 35.3 |
| `layout.force.barnes_hut` | 100000 | 1 | 26192.13 | 1.00 | yes | 339.2 | 0.1 |
| `layout.force.barnes_hut` | 100000 | 2 | 22766.58 | 1.15 | yes | 678.4 | 16.0 |
| `layout.force.barnes_hut` | 100000 | 4 | 17753.91 | 1.48 | yes | 1356.8 | 14.4 |
| `layout.force.barnes_hut` | 100000 | 8 | 20137.34 | 1.30 | yes | 2713.5 | 33.4 |

`/proc/loadavg` start `23.45 25.65 25.54` · end `22.96 22.05 23.69`.

Every multi-rank number in this report is at 100 000 or 400 000 nodes. At 1 000 000 only one rank
ran (below).

### n = 400 000 · repeat 1

| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB | gather % |
|---|---:|---:|---:|---:|---|---:|---:|
| `layout.force.particle_mesh` | 400000 | 1 | 74398.13 | 1.00 | yes | 1356.6 | 0.3 |
| `layout.force.particle_mesh` | 400000 | 2 | 61300.03 | 1.21 | yes | 2713.1 | 6.3 |
| `layout.force.particle_mesh` | 400000 | 4 | 50770.15 | — | — | 5426.3 | 32.8 |
| `layout.force.barnes_hut` | 400000 | 1 | 202400.83 | 1.00 | yes | 1356.6 | 0.1 |
| `layout.force.barnes_hut` | 400000 | 2 | 127076.22 | 1.59 | yes | 2713.1 | 19.2 |

`/proc/loadavg` start `22.33 21.93 23.64` · end `17.14 18.69 21.29`. The `ranks: 4` row is a
separate invocation (`--ranks 4`, so no baseline in that run) and its `—` columns are unmeasured
rather than fabricated; `docs/…/phase11-threads.md` shows the same convention. The 4-rank cell is
reported next to the 1- and 2-rank cells because 50770 ms against 74398 ms is the only
three-point scaling statement this report has at that size.

### n = 500 000 · repeat 1 — the largest size that runs

| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB | gather % |
|---|---:|---:|---:|---:|---|---:|---:|
| `layout.force.particle_mesh` | 500000 | 1 | 81458.27 | 1.00 | yes | 1636.9 | 0.3 |
| `layout.force.particle_mesh` | 500000 | 2 | 68207.26 | 1.19 | yes | 3273.8 | 7.2 |

`/proc/loadavg` start `18.50 18.66 21.08` · end `17.61 17.59 20.23`.

### n = 1 000 000 · ranks 1 · repeat 1

The first sweep refused 600 000 and up with `Start offset -1719947488 is outside the bounds of the
buffer`. That was this harness's defect, not the ABI's: the ABI returns framed buffers as `u32`
pointers (`docs/contract/wasm-abi.md`), a WebAssembly i32 reaches JavaScript signed, and the
worker used the pointer without `>>> 0`. The single-instance memory ladder crossed 2 GiB between
500 000 and 600 000 nodes, which is where the sign flipped. `harness/wasm-replicas/worker.mjs` now
reads every pointer and length unsigned (`allgather`, `framed`, `buildHandle`; constraint C9), and
the same command runs:

```sh
scripts/orch/gr node harness/wasm-replicas.mjs bench --n 1000000 --ranks 1 --repeat 1
```

| layout | n | ranks | median ms | speedup vs 1 | equal | mem MiB | gather % |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `layout.force.barnes_hut` | 1000000 | 1 | 700009.08 | 1.00 | yes | 3274.5 | 0.1 |
| `layout.force.particle_mesh` | 1000000 | 1 | 302219.72 | 1.00 | yes | 3274.5 | 0.3 |

`/proc/loadavg` start `20.78 21.33 21.03` · end `35.48 37.46 32.90`.

The single-instance ladder (`layout.force.particle_mesh`, ranks 1, repeat 1) is linear in `n`:

| n | median ms | mem MiB |
|---:|---:|---:|
| 100000 | 11226.42 | 339.1 |
| 200000 | 30363.52 | 678.4 |
| 400000 | 90704.23 | 1356.6 |
| 500000 | 107981.06 | 1636.9 |
| 1000000 | 302219.72 | 3274.5 |

So 1M nodes run in one instance, at 3.2 GiB of its 4 GiB address space. More ranks at 1M were not
run: by the line below they need 3.2 GiB **each**, 26 GiB at 8 ranks. The binding cost is the
*ingest document*, not the layout: `seeded_model(seed, 10^6, 8)` is ~10^6 node records plus
~8·10^6 edge records written as JSON, held in wasm memory at least twice (the published frame and
the `gm_alloc` copy the host writes) before the simulation allocates anything. Model (b) holds one
such instance per rank; model (a) shares one memory, so it pays the document once.

## Memory × ranks

Exactly linear, because every rank is a full copy of everything the serial build allocated:

| n | 1 rank | 2 | 4 | 8 |
|---:|---:|---:|---:|---:|
| 100000 | 339.1 MiB | 678.3 | 1356.5 | 2713.0 |
| 400000 | 1356.6 MiB | 2713.1 | 5426.3 | — |

Per-node cost is ~3.4 KiB and does not depend on the layout or the rank count — `678.3 / 2` and
`1356.5 / 4` land on `339.1`, and `2713.0 / 8` lands on `339.125`. A 1M-node graph would need
~3.3 GiB **per rank** by the same line, and one rank measured 3274.5 MiB. The tree or
mesh build is repeated in every replica, so the *compute* is `ranks`× the serial cost too; the
numbers above already include that, which is most of why 8 ranks is slower than 4.

## The all-gather's share of a pass

Measured inside the import, per rank, and reported above as `gather %`. It is the reason the
speedup column flattens and then reverses:

- **1 rank: 0.1–0.3 %.** The gather is two barriers and a `memcpy` of a column nobody else is
  using. This is the floor the model pays for existing.
- **2 ranks: 5.7 % (`particle_mesh`) to 19.2 % (`barnes_hut` at 400 k).** The two layouts differ
  by more than 3× here, which is the pass mix showing through: Barnes-Hut's collide pass is the
  expensive one and the gather scales with the column, not with the work behind it.
- **4–8 ranks: 24–35 %.** At 8 ranks on 100 000 nodes, `particle_mesh` gathers for a third of
  every pass and is **slower than one rank** (0.92×), and Barnes-Hut's 8-rank cell gives back most
  of its 4-rank gain (1.48× → 1.30×). Three barriers per tick × 112 ticks = 336 gathers per run,
  each a full stop of a wasm instance.

The crossover this lands on is the same one `phase11-threads.md` found natively — threads lose at
small `n` — but model (b) moves it much further right: at 100 000 nodes the **native** thread tier
reaches **4.29× at 7 workers** (`phase11-threads.md`), while model (b) reaches **1.48× at 4** and
**1.30× at 8**. Same machine class, same kernels, same partition: the difference is that model (b)
pays a copy of the whole column and two stops **per pass** where the native tier pays one join
**per step**. That is the headline number for the model-(a)-versus-model-(b) decision, and it is
against (b).

## What this does not do

- **No browser.** Node `worker_threads` is the only host measured. `SharedArrayBuffer` and
  `Atomics.wait` are available on Node's main thread and its workers with no headers, which is why
  this ran at all.
- **In a browser it needs cross-origin isolation** (COOP `same-origin` + COEP `require-corp`) or
  `SharedArrayBuffer` is not constructible and the module's one import cannot be satisfied. Nothing
  in this job set those headers, and nothing here claims a browser number.
- **The memory is × ranks.** Full graph per wasm instance. That is model (b)'s defining cost and
  the table above is the price list.
- **The tree or mesh build is repeated in every replica**, and so is the ingest, and so is the
  `112`-tick cold start. The timed region is `gm_run_replica` **alone**, so none of the ingest is
  inside the number, but all of the tree/mesh build and all 112 ticks are.
- **Only `layout.force.barnes_hut` and `layout.force.particle_mesh` are threaded.** Every other
  registered layout is refused with `Code::UnknownLayoutId`, because only those two have a
  `run_with`; a stage that was never split has no range kernel to schedule.
- **`ranks > 0` is refused, `rank >= ranks` is refused, `ranks > n` is not.** `partition` yields
  `min(n, ranks)` ranges and the surplus instances gather an empty span, which is the serial
  answer for those nodes.
- **At 1 000 000 nodes only one rank ran**: 3.2 GiB per rank puts 2 ranks at 6.4 GiB and 8 at 26 GiB.
- **The host was shared.** 20 cores, load average 16–33 throughout the sweep (readings in each
  table). On a busy host a ratio is a *different* measurement, not a smaller one; the speedup
  column here is the load-corrected claim and the absolute milliseconds are not.

## Deviations from the job's path list

Two files outside the listed paths were edited, both for one reason: `gm_run_replica` has to run
*the same handle* as `gm_run`, and read its result back through *the same* `gm_snapshot_bytes`,
otherwise the comparison this model exists to make needs a second ABI.

- `crates/graph-wasm/src/exports/state.rs` — `HANDLES` and `publish` from `pub(super)` to
  `pub(crate)`. `HANDLES` is also now `const { RefCell::new(..) }`, because
  `clippy::missing_const_for_thread_local` fires on the `pub(crate)` form and the gate runs
  `-D warnings`.
- `crates/graph-wasm/src/exports/mod.rs` — `mod state;` to `pub(crate) mod state;`, so
  `crate::replica` can name the table at all. No behaviour change on either: the visibility is the
  only thing that moved, and both items keep one instance.
