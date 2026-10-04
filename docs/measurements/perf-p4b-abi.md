# P4b (agent build): the wasm growth exports, the SDK verbs, and the stream gate

Steps 1–8 of `prompts/jobs/perf-p4b-abi.md`. Steps 1–3 (the exports, the ABI docs, the SDK
verbs) were done and committed at `171d4a8b`; this report covers the whole slice and marks
which half wrote what.

## What was built

| Step | What | Where |
|---|---|---|
| 1 | `gm_graph_extend`, `gm_force_session_grow`, the graph id a session records | `crates/graph-wasm/src/exports/delta.rs` |
| 2 | Both exports in the ABI contract; `delta.md` corrected | `docs/contract/wasm-abi.md`, `docs/contract/delta.md` |
| 3 | `Motor.extend`, `ForceSession.grow`, both names in `EXPORT_NAMES` | `crates/graph-sdk-js/src/{motor,force,wasm}.ts` |
| 4 | `emit-stream-fixtures`, three JSON Lines fixtures, `ingest_document` re-exported | `crates/graph-cli/src/stream_fixtures.rs`, `crates/graph-cli/src/stream_fixtures/tests.rs`, `crates/graph-wasm/src/lib.rs` |
| 5 | The stream stage: `force.session.stream`, four arms, its own comparator | `crates/graph-cli/src/forcecheck/stream.rs`, `forcecheck/stream/arm.rs`, `forcecheck/stream/tests.rs`, `forcecheck/stream-arm.mjs` |
| 6 | `Knob::DropDelta`, mirrored at every `ForceSessionGravity` site | `hashgate/knob.rs`, `hashgate/knob/{arms,records,compute,value}.rs`, `hashgate/knob/setting.rs`, `tests/common/mod.rs` |
| 7 | Every row of `target/wf/p4b-rest.rows`, below | — |
| 8 | This report | `docs/measurements/perf-p4b-abi.md` |

Two supporting edits outside the brief's list, both forced by the 300-line house limit and
both listed under deviations: `forcecheck.rs` was split rather than grown past 300 lines, and
`tests/cli_p3.rs`'s pinned knob count was 49 → 50.

## The gate table

Every row of `target/wf/p4b-rest.rows`, run through `scripts/orch/gr` / `scripts/orch/node-slim.sh`
with `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3`.

| Row | Exit | Expected |
|---|---|---|
| `fmt` | 0 | 0 |
| `clippy` | 0 | 0 |
| `test` | 0 | 0 |
| `wasm32-core` | 0 | 0 |
| `wasm-release` | 0 | 0 |
| `fixtures-fresh` | 0 | 0 |
| `codegen-check` | 0 | 0 |
| `capabilities-check` | **1** | 0 |
| `force-gate-4` | 0 | 0 |
| `negctl-drop-delta` | 0 | 0 |
| `negctl-force-gravity` | 0 | 0 |
| `hashgate-8` | 0 | 0 |
| `negctl-degree` | 0 | 0 |
| `sdk-typecheck` | 0 | 0 |
| `sdk-test` | 0 | 0 (104 pass, 0 fail) |
| `sdk-smoke` | 0 | 0 |

**`capabilities-check` exits 1, and it is not this slice's doing.** All 36 problems it prints
are stale-ledger rows for stages this slice never touches: `no evidence: nothing recorded backs
it` for the `layout.force.*` layouts, and `hashgate ran 8 seeds, need 1000`. `target/gates/` did
not exist when this job started (`stat` birth 02:32, every file born after my first gate run),
so there was no ledger to be stale against. `scripts/orch/rows/develop-full.rows:46` runs this row
*after* `roundtrip-1000` and `hashgate-1000`, and `scripts/orch/rows/p12-t2.rows:75` says so in
as many words: "`capabilities --check` exits 1 on any tree whose gate records are stale, which is
every worktree that has not just run the full develop gate". This slice's obligation — that none
of the problems name anything it added — holds: `grep -cE "force-gate|force\.session|stream"` over
the check's output is **0**.

## The negative control, verbatim

```
force-gate: stream diverged at stream-small batch 2 (wasm32 run 1 differs from native run 1)
```

`GM_MUTATE_DROP_DELTA=2 cargo run -q -p graph-cli -- force-gate --seeds 4` exits **1** with that
line and nothing else red. The seed stage stays 4-way equal on 4/4 seeds under the same control,
which is the point: the control is for the growth path and nothing else sees it.

## The fixtures

| Fixture | Lines | Bytes | Nodes after each line | Edges |
|---|---|---|---|---|
| `stream-small.jsonl` | 9 | 52 344 | 50, 60, 70, 80, 90, 100, 110, 120, 130 | 210 |
| `stream-hub.jsonl` | 6 | 36 788 | 21 ×6 | 220 |
| `stream-pow2.jsonl` | 6 | 45 635 | 60, 70, 70, 128, 129, 149 | 148 |

21 batch lines in total, which is the number `force-gate` prints on its equal path:
`force-gate: stream equal, 21 batches x 4 arms`.

`stream-hub` carries 200 parallel edges over 20 spokes (every spoke takes ten) and exactly one
self-loop, `n0`-`n0`, in batch 3. `stream-pow2`'s line 3 is an empty document. Neither the
self-loop nor the parallel edges was refused by `service::build` / `service::extend` or by
`ForceSession::grow`, so nothing had to be dropped from the fixture shapes.

## Deviations

1. **`crates/graph-wasm/src/lib.rs` needed two lines, not one.** `mod seed_ingest` was gated
   `any(test, target_arch = "wasm32")`, and a native `graph-cli` is neither — so
   `pub use seed_ingest::document as ingest_document;` alone would not compile. The module is
   now `pub mod seed_ingest` (ungated) plus the re-export. Ungated rather than re-gated because
   a native caller needs it; `pub` rather than private because `seed_ingest::for_seed` is only
   reachable through the exports, which are wasm32-only, and a private ungated module would make
   it dead code that `-D warnings` refuses on a plain `cargo build`. `seed_ingest`'s own module
   doc still calls the module "Gate-only", which is now not quite true; that file was not in this
   job's paths.
2. **`crates/graph-cli/tests/cli_p3.rs:252`** pins `KNOBS.len() == 49`. Adding the fiftieth
   knob made that stale, and the `test` row could not pass without editing it. Bumped to 50 and
   the message's last clause reworded from "the live session's own" to "the live session's two own".
   A test file outside the brief's list, but a mechanical consequence of the edit it names.
3. **`forcecheck.rs` was split, not grown.** It was 252 lines and my additions took it to 382,
   past the house limit. The stream stage's arm collection and its report line moved to
   `forcecheck/stream/arm.rs`; `forcecheck.rs` is back to 279. `record` keeps its pre-existing
   five parameters (the brief's structure, not this job's).
4. **A dropped batch takes the later ones with it.** The brief says the native arm skips batch
   `k` with "no extend, no grow". Taken literally the arm exits 2 on batch `k + 1`, because
   `Topology::extend` refuses an edge whose endpoint names no node and batch `k + 1` names the
   nodes batch `k` brought — so the control would answer "could not run", not "diverged at batch
   `k`". The arm therefore stays behind once it is behind: batch `k` and every later batch are
   skipped, the session keeps stepping, and the gate reports the batch the control named. The
   wasm arm, which reads no environment variable, keeps every batch.
5. **The knob list is mirrored in two places beyond `tests/common/mod.rs`.**
   `hashgate/knob/wiring.rs:24` pins `ALL`'s length and `hashgate/tests/knob/table.rs`'s
   `PARAMETER_KNOBS` (18 → 19) is the twin that keeps two knobs from sharing a variable or a
   record. Both are the `ForceSessionGravity` pattern's own sites.
6. **Oversize files kept.** `hashgate/knob.rs` 309 → 317 and `hashgate/knob/setting.rs`
   372 → 404, as the brief instructed; neither was split.

## What it does not do

- **No studio.** Nothing in `app/` or `packages/` reads the stream fixtures; the gate is the only
  consumer, and `ForceSession.grow`'s `#columns.forget()` invalidation is tested by the SDK's own
  tests, not by anything visual.
- **No timing.** Every number here is a digest, a byte count or a line count. Nothing measures
  how long a batch takes, how a 1M-node batch behaves, or whether growing is fast — that is P4c,
  and this slice's numbers say nothing about it either way.
- **No 1M batch numbers.** The largest fixture is 149 nodes. The capacity crossings the fixtures
  do exercise are the node-count ones (64 and 128) and nothing near the adjacency limit.
- **The stream stage compares four arms over three fixtures, not over seeds.** It has no
  `seeds × stages` matrix, so it has none of `compare::diverged`'s vacuity refusals beyond its
  own: an arm that printed a different number of lines is refused (exit 2) rather than compared,
  but there is no "one input tested N times" check, because three fixtures is three inputs and the
  digest is per batch.
- **No `hashgate --seeds 1000`, no mutants**, so no row outside `p4b-rest.rows` was run and the
  36 pre-existing `capabilities --check` problems above were not addressed.