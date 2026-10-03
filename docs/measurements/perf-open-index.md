# Perf open index: one id probe per record on the 1M open path

Measured 2026-10-03 on branch `perf-open-index` (from `fix-wasm-ingest` `797f853`). Host: dlesieur42,
i5-13600KF. Node from `scripts/orch/gr`.

Why: a 1M-node studio open (`deploy/perf/open.py`, `~/goinfre/orch/logs/probe-open-1m.txt`) spent
5.6 s of the motor worker in `gm_build`. 720 ms of it was `ingest::check_ids`, which interns every node
and edge id into arenas of its own. `index_model` then interns the same ids again, and probes each one
once more in `node_ids`/`edge_ids`.

## Design

| Piece | Where | What changed |
|---|---|---|
| one pass on the happy path | `crates/graph-wasm/src/ingest/ids.rs` `index` | `index_model` runs first. `check_ids` runs only when the topology kept fewer nodes or edges than the input held, to name the refused id. |
| node admission | `crates/graph-core/src/index.rs` `admit_node` | one `intern` plus one `insert_full`. It was a `find`, then an `intern`, then a set probe. A taken id is already interned, so `intern` adds nothing for it. |
| edge admission | `index.rs` `admit_edge` | the endpoints are resolved first, then the id is interned and inserted once. A dropped edge interns nothing, as before. |
| entry point | `crates/graph-wasm/src/exports/build.rs` | `read_records(bytes)` then `ingest::index`. `ingest::read` is `#[cfg(test)]`: it was the only caller of the old order. |

The invariant that makes skipping `check_ids` safe: `index_model` drops a node only for a taken id,
and an edge only for a taken id or a missing endpoint. Those are exactly the inputs `check_ids`
refuses (C12). So if every record was kept, `check_ids` cannot refuse anything. Two tests check it:

- `crates/graph-wasm/src/ingest/tests/index.rs` compares `read_records` + `index` with the old
  `read` on 7 documents. Those are a duplicate node, a duplicate edge, a dangling target, a dangling
  source, a dangling edge followed by a valid edge with the same id, a clean graph with a self-loop,
  and an empty graph. It requires the same error, or the same node and edge counts.
- `crates/graph-core/src/index/tests.rs`, `the_arena_holds_kept_strings_once_in_first_seen_order`,
  checks that the arena's strings and their order are unchanged. The `Interned` values are slots in
  first-seen order, so the snapshot bytes cannot move.

## Gates

`scripts/orch/gate.sh target/gate-open-index scripts/orch/rows/quick.rows`:

| Row | Expect | Exit | Note |
|---|---|---|---|
| `fmt`, `clippy` | 0 | 0 | |
| `test` | 0 | 0 | `cargo test --workspace --no-fail-fast`, 1063 s |
| `wasm32-core` | 0 | 0 | |
| `hashgate-8`, `negctl-degree`, `negctl-dim-z-mismatch` | 0 | 0 | the 4-way hash is unchanged; both controls turn it red |
| `force-gate-4`, `negctl-force-gravity` | 0 | 0 | |
| `scigraphs-conformance` | 0 | **1** | YIFAN_HU and GRAPHVIZ_SFDP: "reference bytes are not the pinned ones" (sha `78ccfd5e…`). The same two rows fail with the same sha in `open-core-slot`, `fix-force-quadtree` and `ux-params-abi`, whose changes do not touch ingest. Those are the reference arm's bytes, not the motor's. The cause is not found here: this is not a pass. |
| `negctl-scigraphs-conformance` | 0 | 0 | |

## Bench: `gm_build` on one 1M document

`target/bench/open-bench.mjs <wasm> <doc>`: one process per run (fresh linear memory), timing
`gm_build` over `random-n1000000-d3-p0.json` (678 MB). The before and after artifacts are the
release `graph_wasm.wasm` of `797f853` and of this branch. Four runs per arm, alternated.

| arm | build ms, 4 runs | median | wasm pages |
|---|---|---:|---:|
| before | 7170, 7658, 8185, 9553 | 7922 | 57766 |
| after | 6377, 6642, 6821, 7118 | 6732 | 57430 |

The median falls by 15% (1.19 s). Peak memory falls by 336 pages (21 MB): the two `check_ids` arenas
are gone.

## Where the 1M build goes now

`node --cpu-prof` over the after artifact (one run, 6.23 s): `target/bench/prof/`.

| Inclusive | ms | What it is |
|---|---:|---|
| `ingest::read_records` | 3800 | `canonical_json` parse 2230 (`Parser::value` 743 self, `read_string` 1061); `record::edge` 976, `record::node` 397; `malloc` 584 |
| `index_model` | 2230 | `StringArena::find` 1081, the 3M edges' 6M endpoint lookups; `IndexMap<Interned>` probes 328 + 291; `intern` 350 |

Each endpoint lookup costs two hash probes: the arena's (about four cache misses: control byte, index,
entry, text), then `node_ids`. That is the next lever on this path.

## What it does not do

- The JSON parse and the record strings are untouched: 3.8 s of the 6.2 s. A binary columnar ingest
  would remove both; it changes the public ABI, so it waits on a risk review.
- An error is still named by a second pass (`check_ids`), which costs the old 720 ms again, on
  refused input only.
- Caveat: the host was shared with landers during the runs (load not recorded per run), and the
  before arm spread by 33%. The arms alternated, so they saw the same load, but the 15% is one
  session's median.
