# Job perf-p3-wasm-replicas (agent build): browser threads, model (b) — one wasm instance per worker

Why: the plan's P3 (`prompts/perf-plan.md`) builds two browser thread models, measures both, and the
user picks one. This job is model (b). Each worker holds its own wasm instance with a full copy of the
graph. All instances run the same stage in lockstep. In each gathered pass, an instance computes only
its own range of nodes, then an all-gather through a `SharedArrayBuffer` fills in the other ranges.
Every instance then applies the same full array, so every instance stays byte-identical to the serial
run. Model (a), one shared memory with a thread pool, is built by the orchestrator in another worktree.
Do not build it here.

Facts:

- **The seam.** `Runner::run(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>)`
  (`crates/graph-core/src/exec/partition.rs`).
  - `O: StepRange`, with `O::Out: Copy + Default + Send`.
  - `kernel.step_range(range, &mut out[range])` reads only start-of-pass state and writes only its
    own span, so any division of `0..len` gives the same bytes.
  - `Serial` in the same file is the reference implementation.
  - The division to use is `graph_core::exec::partition(n, k)`, which returns a `Vec<Range<u32>>`.
    `ranges` beside it is not re-exported.
- **The passes.** A Barnes-Hut tick (`crates/graph-core/src/layout/force/barnes_hut/sim.rs:161`)
  calls `runner.run` three times: link, charge, collide.
  - Each call is followed by a serial apply of the deltas to the velocities.
  - Each pass reads the previous pass's writes for *other* nodes (collide reads `x + vx`). So a sync
    once per tick is not enough; the sync must happen inside every `runner.run`. That is why model (b)
    needs a blocking all-gather, and so `SharedArrayBuffer` and `Atomics.wait`.
- **The stages.**
  - `BarnesHut::run_with(topology, params, runner, workers)` is at
    `crates/graph-core/src/layout/force/barnes_hut.rs:147`.
  - `ParticleMesh::run_with` (same shape) is at `crates/graph-core/src/layout/force/particle_mesh.rs:66`.
  - Find the default `ForceParams` the registry's `run` uses with `git grep -n 'fn run' crates/graph-core/src/layout/force/barnes_hut.rs crates/graph-core/src/layout/force/particle_mesh.rs`.
- **The wasm ABI.**
  - `gm_run(handle, layout_id, params_ptr, params_len)` is at
    `crates/graph-wasm/src/exports/build.rs:152`. Copy its shape for the handle, the snapshot and the
    error code.
  - `gm_seed_ingest(seed)` is at `:255`. It builds the gate model of `2 + seed % 600` nodes, as
    ingest JSON, through `seed_ingest::for_seed` (`crates/graph-wasm/src/seed_ingest.rs:20`).
    `seed_ingest::document(nodes, edges)` (`:32`) writes any records, and
    `graph_core::seeded_model(seed, count, reference_degree)`
    (`crates/graph-core/src/stage/topology.rs:26`) builds a model of any size.
- **The default artifact must not change.**
  - `harness/wasm-run.mjs` and the SDK (`crates/graph-sdk-js/src/wasm.ts:131`, `refuseImports`)
    require the shipped `graph_wasm.wasm` to import nothing.
  - So everything in this job sits behind a new cargo feature `replicas` in
    `crates/graph-wasm/Cargo.toml`, next to `probe`, and builds into its own target dir.
- **Node is the first host.**
  - Node's `worker_threads` share a `SharedArrayBuffer` without any COOP/COEP header, and both Node's
    main thread and its workers may call `Atomics.wait`.
  - `scripts/orch/gr` has the pinned Node, so run the harness as `scripts/orch/gr node harness/…`.
  - The browser measurement is a later slice. Do not touch `app/`, `packages/`, `deploy/` or the SDK.

Do:

1. `crates/graph-wasm/src/replica.rs`, compiled only under `#[cfg(feature = "replicas")]`:
   - `struct Replica { rank: u32, ranks: u32 }` implementing `Runner`:
     - `out.clear(); out.resize(len, Default)`;
     - run `kernel.step_range` on this rank's range from `graph_core::exec::partition(len, ranks)`;
     - then call the host import `gm_host_allgather(ptr: u32, elem_bytes: u32, len: u32, lo: u32, hi: u32) -> u32`
       (`#[link(wasm_import_module = "env")]`), with `ptr = out.as_mut_ptr()` and
       `elem_bytes = size_of::<O::Out>()`.
     - A non-zero return means the host refused (the buffer is too small). Abort with a clear panic
       message: the all-gather has no error path back through `Runner::run`, and a silent short read
       would hand back wrong bytes.
     - The `workers` argument is ignored in favour of `ranks`; say why in one line.
   - `gm_seed_ingest_n(seed, n) -> u32`: `seeded_model(seed, n, <the gate's reference degree>)` into
     `seed_ingest::document`, then `publish`, exactly as `gm_seed_ingest` does. The gate's degree is
     `graph_core::REFERENCE_DEGREE` (8).
   - `gm_run_replica(handle, layout_id, rank, ranks) -> u32`: like `gm_run`, but it calls `run_with`
     with `&Replica { rank, ranks }` and `ranks` workers.
     - Only `layout.force.barnes_hut` and `layout.force.particle_mesh` are accepted; refuse any other
       id with the existing `Code::UnknownLayoutId`.
     - Refuse `ranks == 0` and `rank >= ranks`.
   - Keep each fn ≤ 40 lines and each file ≤ 300 lines. If `unsafe` is needed, add a `// SAFETY:`
     line on it, as the existing exports do.
2. `harness/wasm-replicas.mjs` (with a child module beside it if it passes 300 lines):
   - It loads the replicas artifact from `--wasm`.
     - Build the artifact with `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown --features replicas --target-dir target/wasm-replicas`.
     - The serial reference is `target/wasm32-unknown-unknown/release/graph_wasm.wasm`, built the
       usual way.
   - It spawns `ranks` workers with `worker_threads`. Each worker:
     - instantiates the module with an `env.gm_host_allgather` that copies its own span into one
       shared data `SharedArrayBuffer`;
     - waits at a barrier and copies `[0, len × elem_bytes)` back into its own memory;
     - waits at a second barrier, so no rank overwrites the buffer before every rank has read it.
     - The barrier is a sense-reversing one on an `Int32Array` over a small control SAB, built with
       `Atomics.add`, `Atomics.wait` and `Atomics.notify`.
     - The data SAB holds `64 × n` bytes. The import returns 1 when `len × elem_bytes` exceeds it.
   - Every worker ingests the same model and builds its own handle, then reaches a barrier.
     - The timed region is `gm_run_replica` alone. Report the slowest rank's time.
     - Rank 0 returns the SHA-256 of `gm_snapshot_bytes`.
   - Modes, with exit codes as graph-cli's (0 ran clean · 1 a check failed · 2 could not run):
     - `hash --seeds 8 --ranks 1,2,3,4,7 --layouts barnes_hut,particle_mesh`:
       - for every seed, compare rank 0's hash against the serial artifact's `gm_run` hash on the
         same `gm_seed_ingest` model;
       - one line per cell;
       - exit 1 on any mismatch.
     - `bench --n 100000,1000000 --ranks 1,2,4,8 --layouts particle_mesh,barnes_hut --repeat 3`:
       - one markdown row per (layout, n, ranks): median ms, speed-up over ranks 1,
         `equal` (the rank-0 hash against ranks 1);
       - the summed `memory.buffer.byteLength` of all replicas;
       - `/proc/loadavg` at the start and end.
       - Barnes-Hut at 1M is slow, so allow `--layouts` to skip it, and say in the report whether
         you ran it.
     - `--break`: the last rank skips writing its span into the data SAB, so the hashes must differ.
       This is the negative control.
3. Gates, each with its exit code in the report:
   - `scripts/orch/gr cargo fmt --all --check`
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings`
   - `scripts/orch/gr cargo clippy -p graph-wasm --target wasm32-unknown-unknown --features replicas -- -D warnings`
   - `scripts/orch/gr cargo test --workspace --no-fail-fast`
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` (expect 0: the default artifact
     is unchanged)
   - `scripts/orch/gr node harness/wasm-replicas.mjs hash --seeds 8 --ranks 1,2,3,4,7` (expect 0)
   - the same with `--break` (expect 1)
4. Measure with the bench mode at 100 000 and 1 000 000 nodes. Record the load before and after.
5. Report `docs/measurements/perf-p3-wasm-replicas.md`:
   - the design in five lines;
   - the gate table with exit codes;
   - the bench table;
   - memory × ranks;
   - the share of a pass spent in the all-gather, if you can time it inside the import (a
     `performance.now()` sum per rank);
   - a "what it does not do" list: no browser yet; it needs cross-origin isolation in a browser; the
     memory is × ranks; the tree or mesh build is repeated in every replica.

Paths you may edit:
- `crates/graph-wasm/Cargo.toml` (the feature line only);
- `crates/graph-wasm/src/lib.rs` (one `#[cfg(feature = "replicas")] mod replica;` line);
- `crates/graph-wasm/src/replica.rs` and `crates/graph-wasm/src/replica/`;
- `harness/wasm-replicas.mjs` and one child module beside it;
- `docs/measurements/perf-p3-wasm-replicas.md`.

Nothing in graph-core, graph-contract, the SDK, `packages/`, `app/` or `deploy/`.

Done when: the seven gates exit as stated, the bench ran at both sizes for `particle_mesh`, and the
report states each.
