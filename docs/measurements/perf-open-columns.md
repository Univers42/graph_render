# Perf open columns: `gm_build_columns` against `gm_build` on the 1M open path

Measured 2026-10-03 on branch `perf-open-columns` (from `72d91d1`). Host: dlesieur42,
i5-13600KF, 32 GB. Node from `scripts/orch/node-slim.sh` (node:22-slim), Rust from
`scripts/orch/gr`.

Why: `docs/decisions/ingest-columns.md` — a 1M-node, 3M-edge `gm_build` took 6.73 s
(`docs/measurements/perf-open-index.md`), of which the JSON parse, the per-record `String`s
and the per-edge endpoint lookups were most of it. A binary document with one string table
and endpoints given as node row numbers removes all three. The decision set the bar: **if the
measured median saving is under 3 s, the studio stays on JSON.**

## Design

| Piece | Where | What changed |
|---|---|---|
| one admit path | `crates/graph-core/src/index.rs` | `admit_node` takes `&NodeView`; `admit_edge` splits into `endpoints` (resolution) and one push taking `(source, target)` plus `&EdgeFields`. `index_model` calls both. |
| the columnar admit | `crates/graph-core/src/index/columns.rs` | `index_columns` over two `ExactSizeIterator`s of `NodeView` and `RowEdge`, **refusing** a taken id where `index_model` drops it. |
| the decoder | `crates/graph-contract/src/ingest_columns.rs` (+ `header`/`layout`/`check`/`row` children) | `decode(&[u8]) -> Result<ColumnsDoc<'_>, ColumnsError>`: checked `u64` section sizes compared to the buffer length before any slice, then one pass over the values. |
| the bridge | `crates/graph-wasm/src/ingest/columns.rs` | Resolves the two kinds and hands rows through, so graph-core sees no wire format. |
| the export | `crates/graph-wasm/src/exports/build_paths.rs` | `gm_build_columns(ptr, len)`, same handle lifecycle as `gm_build`, new code `ColumnsInvalid = 23` (20 at measurement time; renumbered when it merged after `ParamOutOfRange`..`ParamsNotAccepted` took 20-22). |
| the encoder | `crates/graph-sdk-js/src/columns.ts` | `encodeColumns`: one `Uint8Array` sized up front, `TextEncoder.encodeInto` into it. |

Since `perf-open-intern` the columnar admit takes `NodeCells`/`EdgeCells` over an `EntryTable`, resolves
the kinds itself and interns through a per-entry memo; `RowEdge` is gone (`perf-open-intern.md`).

The invariant the differential rests on: **row `r` of the node columns is dense index `r`**.
That is why a duplicate id is a refusal here and a drop there — dropping a row renumbers
every row after it and silently repoints every edge that follows.

## Gates

```
scripts/orch/gate.sh target/gate-columns scripts/orch/rows/perf-open-columns.rows
```

See the orchestrator's own run; this job did not run a timed gate.

## Bench: `gm_build_columns` against `gm_build`

```
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
cp /mnt/storage/bench/perf-keep/open-bench.mjs target/bench/
cp /mnt/storage/bench/perf-keep/random-n1000000-d3-p0.json target/bench/
scripts/orch/node-slim.sh node --experimental-strip-types --max-old-space-size=12288 \
  target/bench/columns-bench.mjs \
  target/wasm32-unknown-unknown/release/graph_wasm.wasm \
  target/bench/random-n1000000-d3-p0.json <json|columns>
```

Document: 1,000,000 nodes, 3,000,000 edges, 678,016,813 bytes of JSON. One artifact
(`graph_wasm.wasm`, 1,372,711 bytes), four runs per arm, alternated, one process per run so
each arm starts with fresh linear memory. The Node-side parse (4.4–6.5 s) is outside both
timed regions and is identical work in both arms.

| arm | encode/stringify, 4 runs | build, 4 runs | median total | wasm pages |
|---|---|---|---|---|
| `json` | stringify 1640, 1518, 2001, 2370 → **1820** | 7454, 8815, 7741, 9383 → **8278** | **10 099 ms** | 56 836 |
| `columns` | encode 5533, 7719, 7190, 5568 → **6379** | 1748, 2730, 2070, 1821 → **1946** | **8 324 ms** | 13 573 |

The columns document is 238,241,440 bytes against 678,016,813 of JSON — 2.8× smaller — and
linear memory after the build is 13,573 pages against 56,836, a saving of 43,263 pages =
169 MB.

**Verdict: the columns arm does not beat the JSON arm by 3 s. The median saving is 1 774 ms.
The studio stays on JSON**, exactly as `docs/decisions/ingest-columns.md` says it should if
the measured saving falls short.

## What the two numbers say

The build itself is where the format wins: **8278 → 1946 ms at the median, a 6.3 s saving.**
That is the whole of the decision record's argument — no JSON parse, no `String` per record
field, no arena probe per edge endpoint — and it is real.

The saving does not survive the encoder. `encodeColumns` costs 6.4 s at the median against
1.8 s for `JSON.stringify`, so 4.6 s of the 6.3 s the build wins is handed straight back.

**Caveat, and the next measurement:** the encoder's cost is 23 million `Map<string, number>`
probes over roughly 7 million distinct strings — eight per node and five per edge, each one
hashing a JS string. Its spread (5533–7719 ms) is the widest of anything measured here, so
part of it is host load rather than the encoder. The obvious lever is not a faster hash but
**no dedupe at all**: the contract explicitly allows two table entries to hold the same
bytes, so a producer that emits one entry per field occurrence would replace every one of
those probes with an array append, at the cost of a larger blob. That was not measured here
and is not claimed.

## What this does not do

- `src/` (the studio wiring) is untouched, so nothing is switched over: this branch adds a
  third way in and nothing uses it. That is deliberate — the decision says the studio stays
  on JSON under 3 s, and wiring it would be a change with no measured benefit behind it.
- The hash gate still covers only `gm_build`; `gm_build_columns` is differentially tested
  (whole-`Topology` `Debug`, over `n220` and generated corpora, with two negative controls),
  but it is not a gate stage and no `hashgate` arm runs it.
- The document used here is `fixtures/scale/random-n1000000-d3-p0.json`, the same one
  `perf-open-index.md` used. Its node and edge ids are short and highly repetitive, which is
  the friendliest possible case for the string table's dedupe — a document with a million
  distinct labels would make `encodeColumns` worse and `JSON.stringify` about the same.
- Host load is shared and not controlled. The build medians are tight enough to trust the
  6.3 s build gap; the encoder median is not tight enough to trust to better than about a
  second.