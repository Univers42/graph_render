# Job perf-p4c-bench (agent build): measure a 10 000-node batch at 1M, natively and in wasm

Why: P4's exit (`docs/contract/delta.md` "Done when"): at 1M nodes with 10 000-node batches, `extend`
plus `grow` takes ≤ 30 ms per batch as the median of 3 alternated rounds, natively and in wasm, with the
p95, the max and the host load beside it. P4a built the motor half, P4b the exports and the SDK. This
slice measures them, and may keep the particle mesh's FFT plan when its side does not change (deferred to
"P4c's bench" by the P4a brief, step 5).

Facts (re-check each on the tree you start from, perf-p4b-abi after P4b's commits):
- `docs/measurements/perf-p4-carry.md`: the rebuild path at 1M is `index_model` 1.3–2.7 s plus `carry`
  320–341 ms per batch. That is the reference arm this bench alternates with.
- `delta.md` §"Costs" lists the O(n) floors of a batch: `px`/`py` memset, collide `Grid::new(rows)`, the
  mesh's per-node slots, and a column at capacity (once per doubling). Reallocation makes the max much
  larger than the median; the target gates on the median.
- The live session's two engines: Barnes-Hut and particle mesh (`ForceSession::new`,
  `ForceSession::new_mesh`; check the names). Measure both.
- graph-cli benches live under `crates/graph-cli/src/bench/` (`campaign.rs`). Model the new command on
  the nearest existing one; register it additively in `main.rs`.
- The wasm arm runs under Node on the real artifact
  (`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`), through the SDK's
  `Motor.extend` and `ForceSession.grow` (P4b).
- Host: load 10–17 is normal. Alternate the arms, 3 rounds, claim medians only, and say the host was
  loaded. Check `free -g` ≥ 12 GB available and the 1-minute load < 14 before each 1M run.

Do, in order:
1. **Native bench** `graph-cli bench-stream`: seeded, no clock in the motor (the bench may time). Build
   1M nodes (the synth generator the open path uses, or `gm_seed_ingest`'s; name it), create a session,
   tick 10, then append 10 batches of 10 000 nodes and ~20 000 edges (half to old nodes, half inside the
   batch), timing `extend` and `grow` separately per batch. Also time, once per round, the reference:
   `index_model` over all records plus `carry`. Print per-batch ms, then median, p95 and max.
2. **Wasm bench**: a Node script under `harness/` that does the same through the SDK on the release
   artifact, printing the same table. One process per round.
3. **Mesh plan reuse** (only if step 1 shows the mesh rebuild above 30 % of a particle-mesh batch's
   median): keep the FFT plan, kernel and spectrum when the side does not change. `grow_equals_carry`
   (P4a) must stay green, bit for bit. Otherwise skip, and say so with the number.
4. **Structure snapshot cost** at 1M: time `gm_run(layout.random)` then `toBytes` then the studio's
   `describe` path (`packages/graph-studio/src/motor/session.ts:138`) once, in the wasm script. Report it;
   P4c-studio rebuilds it at most twice a second.
5. **Report** `docs/measurements/perf-p4-delta.md`: the per-batch table for both engines and both
   targets (median, p95, max per round, then the median of the 3 medians), the reference arm, the
   structure snapshot cost, `/proc/loadavg` per round, and the verdict against 30 ms. A miss is recorded
   as a miss, with the largest cost named.

Paths you may edit: a new `crates/graph-cli/src/bench/stream.rs` (and children), `main.rs` (additive),
a new `harness/bench-stream.mjs`, `docs/measurements/perf-p4-delta.md`, and, for step 3 only,
`crates/graph-core/src/layout/force/session/` and the mesh module P4a's grow calls.

Done when:
- fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast` and the graph-core wasm32 build exit 0;
- `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exits 0 if step 3 changed graph-core;
- the report has all three rounds for both arms, both engines and both targets.

Return: the branch tip, the medians table (native and wasm, both engines), the verdict against 30 ms,
and every deviation from this brief.
