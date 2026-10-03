# Job perf-p4b-abi (agent build): the wasm exports, the SDK verbs and the stream gate for live growth

Why: P4's second slice (`docs/contract/delta.md`, accepted with conditions 4, 5, 6 and 9 in
`docs/decisions/delta-abi.md`). P4a gave graph-core `Topology::extend` and `ForceSession::grow`. This
slice puts them on the wire, in the SDK and under a cross-target hash gate. No studio, no bench (P4c).

Facts (verified on perf-p4a-extend f15c9b64; re-check each on the tree you start from, which is
perf-p4a-extend after P4a's commits):

- **Spec.** `docs/contract/delta.md` §"The wasm ABI: two exports" and §"The SDK" are the spec. They name
  `gm_graph_extend(graph, ptr, len) -> u32` and `gm_force_session_grow(session, graph) -> u32`, with no
  new error code.
- **ABI version.** `crates/graph-wasm/src/lib.rs:132-139` `ABI_VERSION = 2`, and `wasm-abi.md:31` says
  the same. `delta.md` still says "`gm_abi_version` stays 1"; it stays **2**. Adding an export bumps
  nothing (`lib.rs:132-135`).
- **Codes** (`crates/graph-wasm/src/errors.rs`): `InvalidHandle = 1` (`:21`), `IngestInvalid = 4`
  (`:27`), `InvalidSession = 15` (`:79`), `SessionRefused = 17` (`:90`), `IngestTooLarge = 19` (`:100`).
- **Ingest.** `crates/graph-wasm/src/ingest.rs:165` `read_records(bytes)` reads JSON v1 into
  `(Vec<NodeRecord>, Vec<EdgeRecord>)`. A batch goes through it, then `Topology::extend`.
- **Build exports.** `exports/build_paths.rs:20` `gm_build`, `:117` `gm_build_columns`. Model the
  `(ptr, len)` copy-never-free handling (C5, C7) on `gm_build`.
- **Sessions.** `exports/session.rs:39` `gm_force_session_create(graph, params_ptr, params_len)`;
  `session.rs:103` `create(topology, params, engine)` does not record the graph id. It must, for
  `gm_force_session_grow`'s `SessionRefused` on a foreign graph.
- **Comments that P4 makes false** (condition 4; rewrite them, don't delete them):
  - `graph-wasm/src/handle.rs:24-28`: "topology (fixed at `gm_build`)", "Never replaced after `gm_build`";
  - `graph-wasm/src/session.rs:198-205`: "no path in this ABI resizes" the session columns. After
    `grow`, a column address is valid until the next `gm_force_session_grow` or tick;
  - `graph-core/src/layout/force/barnes_hut/link.rs:31-34`: "the topology never changes across ticks".
- **SDK** (condition 5):
  - `crates/graph-sdk-js/src/wasm.ts:62-75` `EXPORT_NAMES`; add both names so an older module is
    refused by name at load;
  - `motor.ts:264` shows `views.bump()` after a mutating export; `Motor.extend` does the same;
  - `force-columns.ts:112` sets `#views = null`; `ForceSession.grow` does the same, and `force.ts:117-128`
    (`tick`) is the call pattern, `#columns.forget()` included.
- **The live-session gate is `force-gate`, not `hashgate`.** `crates/graph-cli/src/forcecheck.rs`:
  native ×2 against wasm ×2 (`forcecheck/native.rs`, `forcecheck/arm.mjs`), one stage `STAGE`
  (`:51`), `TICKS = 50` (`:60`), compared by `hashgate::compare`. Its control is
  `Knob::ForceSessionGravity` (`hashgate/knob.rs:301-306`, `knob/arms.rs:70,124`,
  `knob/records.rs:68`). `delta.md` says "a hash gate `stream` arm"; put it in `force-gate` and
  correct `delta.md`'s wording.
- **Knob list.** `crates/graph-cli/tests/common/mod.rs:44-69` is the one list of `GM_MUTATE_*` names.
- **Peer branch.** `origin/svc-native-seam` (peer 41) edits `graph-wasm/src/{lib.rs,errors.rs,
  exports/build.rs,stage_exports.rs}` and adds `service.rs`. Edit `lib.rs` and `errors.rs` additively
  only; whichever lands second merges develop and keeps both intents.

Do, in order:

1. **Exports** in a new `crates/graph-wasm/src/exports/delta.rs`:
   - `gm_graph_extend(graph, ptr, len) -> u32`: `1` appended, `0` refused with `InvalidHandle`,
     `IngestInvalid` (an `ExtendError` too) or `IngestTooLarge`. On success it clears the handle's
     snapshot, as a failed `gm_run` does (C4). On refusal the topology is unchanged (P4a's guarantee;
     test it through the export).
   - `gm_force_session_grow(session, graph) -> u32`: `1`, or `0` with `InvalidSession`, `InvalidHandle`,
     or `SessionRefused` when `graph` is not the graph the session was created over or `grow` refuses.
   - Record the graph id in `create`; every constructor of the session record gets the field.
   - Wasm-side tests (`graph-wasm`, native `cargo test`): extend then grow equals build-all then
     create-then-carry, by the session's `x`/`y` bits after 10 ticks; one test per refusal; a released
     graph refuses grow with `InvalidHandle`; a grow against a second graph refuses with `SessionRefused`.
2. **ABI docs** (condition 4):
   - `docs/contract/wasm-abi.md`: both rows in "Exports"; the version line says adding an export bumps
     nothing; C7 lists the two new invalidation events (a handle's views after `gm_graph_extend`, a
     session's column addresses after `gm_force_session_grow`).
   - `docs/contract/delta.md`: "stays 1" becomes "stays 2"; "hash gate `stream` arm" becomes the
     `force-gate` stream stage.
   - Rewrite the three comments listed in Facts.
3. **SDK** (condition 5): `Motor.extend(handle, { nodes, edges })` (encode JSON v1, alloc, call, free,
   `views.bump()`), `ForceSession.grow(handle)` (`#views = null`, `#columns.forget()`). Both throw the
   existing typed errors on refusal. Add both names to `EXPORT_NAMES`. Tests in
   `crates/graph-sdk-js/test/` (a new `delta.test.mjs`, ≤ 300 lines): grow after extend moves the new
   rows; a view taken before `extend` is stale after it; each refusal throws its typed error.
4. **Stream fixtures** (condition 6): `graph-cli emit-stream-fixtures` writes `fixtures/stream-*.jsonl`,
   one JSON v1 batch per line, the first line the initial graph. Seeded, no clock. Three fixtures:
   - `stream-small.jsonl`: 50 nodes then 8 batches of 10 nodes, edges to old and new nodes;
   - `stream-hub.jsonl`: one hub gaining 200 edges over 5 batches, parallel edges and a self-loop;
   - `stream-pow2.jsonl`: batches that take the node count across 64 and 128, and one empty batch.
   Register the command additively in `main.rs`.
5. **`force-gate` stream stage** (condition 6): for each fixture, build the first line, create a
   session, then per batch: `extend`, `grow`, tick `TICKS`, hash `x` then `y` bits. Native through
   graph-core, wasm through `arm.mjs` over the real artifact. All four arms equal per batch. Add a
   native-only check that each batch's grown session equals rebuild plus `carry`, by the same bits.
6. **Negative control** `GM_MUTATE_DROP_DELTA=<k>`: the native arm skips batch `k` (both `extend` and
   `grow`). Add it to `Knob`, its arms/records/env wiring, and once to `tests/common/mod.rs`. With
   `k=2`, `force-gate` must exit non-zero and name the first diverging batch.
7. **Gates.** Report each exit code:
   - `scripts/orch/gr cargo fmt --all --check` → 0
   - `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` → 0
   - `scripts/orch/gr cargo test --workspace --no-fail-fast` → 0
   - `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` → 0
   - `scripts/orch/gr cargo run -q -p graph-cli -- force-gate --seeds 4` (check the flag with `--help`) → 0
   - the same with `-e GM_MUTATE_DROP_DELTA=2` → non-zero
   - `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → 0
   - `scripts/orch/node-slim.sh npm run sdk:typecheck`, `sdk:test`, `sdk:smoke` → 0
8. **Report** `docs/measurements/perf-p4b-abi.md`: what was built, the gate table, the negative control
   output, the fixtures' sizes, and "what it does not do" (no studio, no timing: P4c).

Paths you may edit:
- `crates/graph-wasm/src/exports/delta.rs` (new), `exports/mod.rs`, `session.rs`, `handle.rs`,
  `lib.rs` and `errors.rs` (additive only), and their tests;
- `crates/graph-core/src/layout/force/barnes_hut/link.rs` (the comment only);
- `crates/graph-sdk-js/src/{wasm.ts,motor.ts,force.ts,force-columns.ts,index.ts}` and a new
  `crates/graph-sdk-js/test/delta.test.mjs`;
- `crates/graph-cli/src/forcecheck.rs`, `crates/graph-cli/src/forcecheck/`, `crates/graph-cli/src/hashgate/knob*`,
  a new emit module under `crates/graph-cli/src/`, `main.rs` (additive), `crates/graph-cli/tests/common/mod.rs`;
- `fixtures/stream-*.jsonl`;
- `docs/contract/wasm-abi.md`, `docs/contract/delta.md`, `docs/measurements/perf-p4b-abi.md`.

Do not touch `packages/`, `app/`, graph-core code other than the one comment, or `.claude/rules/devil/`.
No new dependency; no `unsafe` beyond what `graph-wasm`'s exports already use. House limits: 40 lines a
function, 4 parameters, 300 lines a file, nesting 3.

Done when:
- every step-7 gate has its expected exit code;
- the `GM_MUTATE_DROP_DELTA=2` run names the first diverging batch;
- the report exists.

Return: the branch tip, the gate table, the control's output line, and every deviation from this brief.
