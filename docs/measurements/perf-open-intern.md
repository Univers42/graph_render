# Perf open intern: id rows by arena slot, and one resolve per string-table entry

Measured 2026-10-03 on branch `perf-open-intern` (from `perf-open-columns` at `373b442`). Host:
dlesieur42, i5-13600KF, 32 GB. Node from `scripts/orch/node-slim.sh` (node:22-slim), Rust from
`scripts/orch/gr`.

Why: after `perf-open-columns`, `gm_build_columns` at 1M nodes was still about a second, and the
profile put most of it in hashing. Every node id and every edge id went into an `IndexSet<Interned>`,
one probe into a table as large as the graph. Every node and edge cell (id, label, group, kind,
endpoints) went through `StringArena::intern` and its `IndexMap<Span, _>` probe, even though the
studio's synthetic document repeats a few hundred distinct strings across millions of cells.

## Design

| Piece | Where | What changed |
|---|---|---|
| id → row | `graph-core/src/index/slots.rs` `RowBySlot` | the node and edge id sets are a `Vec<u32>` indexed by the interned handle's arena slot: one write to admit, one read to look up, no hash (`e088944`) |
| cells | `graph-core/src/index/columns/cells.rs` | `index_columns` takes `NodeCells`/`EdgeCells` (string-table entry numbers) over an `EntryTable`, not decoded `&str` rows. `RowEdge` is gone (`91946a6`) |
| memo | `cells.rs` `Memo<V>` | one `Option<V>` per table entry: an entry's handle, node kind or edge kind is resolved once and read back by index after that. `EdgeKind::from_name` runs once per distinct kind, not once per edge |
| admit | `graph-core/src/index/admit.rs` | `claim_*` (refuse a taken id) and `push_*` (append the row) are shared by the columnar path; `index_model` keeps `admit_node`/`admit_edge` |
| decoder | `graph-contract/src/ingest_columns/row.rs` | `node_cells`/`edge_cells` yield entry numbers; the text is read only on the memo's first miss |

Why the bytes cannot move: the arena interns in the same order as before (the memo only skips a
probe whose answer it already holds, and the first occurrence of each entry is still interned at
the same point in row order), and `RowBySlot` returns the same row an `IndexSet` lookup returned.
The differential in `index/columns/tests/differential.rs` compares the whole `Topology` (`Debug`)
of `index_columns` and `index_model` over `n220` and generated corpora.

## Gates

`scripts/orch/gate.sh ~/goinfre/logs/gate-perf-open-intern-memo scripts/orch/rows/quick.rows` on
`76dbe7f`. The commit after it changes one comment in `slots.rs`; `land.sh` re-runs the rows on the
merged tree.

| Row | Result |
|---|---|
| fmt, clippy `-D warnings`, wasm32-core | PASS |
| test (`cargo test --workspace --no-fail-fast`) | PASS, 1510 s |
| hashgate-8 / negctl-degree | PASS / PASS (the negative control turned red, as the row expects) |
| negctl-dim-z-mismatch | PASS |
| force-gate-4 / negctl-force-gravity | PASS / PASS |
| scigraphs-conformance / negctl-scigraphs-conformance | PASS, 172 s / PASS, 152 s |

## Bench: `gm_build_columns`, three artifacts, alternated

```
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown   # once per arm
scripts/orch/node-slim.sh node --experimental-strip-types target/bench/emit.mts 1000000 target/bench/n1000000.gmc1
for r in 1 2 3; do for n in 400000 1000000; do for a in base intern memo; do
  scripts/orch/node-slim.sh node --experimental-strip-types --max-old-space-size=12288 \
    target/bench/build.mts target/bench/$a.wasm target/bench/n$n.gmc1 3
done; done; done
```

`emit.mts` writes the studio's synthetic graph (seed 1, degree 2, `random`) through
`encodeColumns`; `build.mts` copies the document into linear memory and times `gm_build_columns`
alone, three builds per process. Arms: `base` = `373b442`, `intern` = `e088944`, `memo` =
`91946a6`. Raw output: `target/bench/runs-final.txt`. Each cell is the median of 9 builds (3 rounds
× 3 builds), in ms.

| n | base, 9 builds | intern, 9 builds | memo, 9 builds | base | intern | memo | base → memo |
|---:|---|---|---|---:|---:|---:|---:|
| 400000 | 534, 311, 299, 490, 465, 252, 439, 477, 310 | 368, 221, 204, 412, 356, 195, 428, 384, 412 | 269, 274, 287, 319, 246, 252, 289, 206, 210 | 439 | 368 | 269 | −38.7% |
| 1000000 | 1104, 811, 1332, 1009, 857, 1294, 1144, 980, 797 | 933, 679, 1077, 694, 672, 688, 1131, 1037, 993 | 743, 592, 660, 715, 513, 589, 768, 584, 598 | 1009 | 933 | 598 | −40.7% |

Linear memory after the first build: 400k 4209 → 3439 pages, 1M 11686 → 8915 pages (−173 MiB at
1M), from `RowBySlot` replacing the two `IndexSet`s. The memo adds no pages at the first build.

## Profile: where the 1M build goes

`node --cpu-prof` over `build.mts` at 1M, one profile per arm: `target/bench/prof` (base),
`prof2` (intern), `prof3` (memo). Self time summed per function over every sample.

| function | base ms | intern ms | memo ms |
|---|---:|---:|---:|
| arena probe: `IndexMap<Span,_>` entry (+ its `reserve_rehash` in base) | 469 + 289 | 1416 | 885 |
| `StringArena::intern` | 190 | 397 | 248 |
| `IndexMap<Interned, usize>::insert_full` (the id sets in base) | 268 | 21 | 17 |
| `NodeKind`/`EdgeKind::from_name` | 14 + 46 | 20 + 65 | — (`Memo` hits: 11 + 5) |
| decode: `ColumnsDoc::node` + `edge` → `node_cells` + `edge_cells` + `text` | 106 + 128 | 139 + 180 | 20 + 89 + 55 |
| admit: `admit_node` + `admit_edge` → `claim_*` + `push_*` | 79 + 86 | 137 + 173 | 20 + 22 + 75 + 137 |
| `Memo<Interned>::get` | — | — | 130 |
| profile total | 2624 | 3890 | 2690 |

The arena probe is still the top row. Every memo miss is one probe; a hit is an array read.

## What it does not do

- The arena probe is still a hashed `IndexMap<Span, _>`: 885 ms of the memo profile. A custom
  open-addressing table keyed by the span's bytes is the next lever; it is not built here.
- The memo helps when table entries repeat. The synthetic document repeats a few hundred labels
  and groups across millions of cells, which is its best case. A document where every cell holds a
  distinct string pays one `Option<V>` per entry for nothing and gains nothing.
- Caveat: the host was shared (load 16.8–19.6 over the bench; landers, OpenCode jobs and a second
  open bench ran alongside). The arms alternated, so they saw the same load, but single builds spread
  by up to 2× (1M base: 797–1332 ms). Only the medians are claimed.
- Caveat: the three profiles are not comparable in absolute terms. The intern profile ran on a busier
  host (total 3890 ms against 2624 for base, for less work), so compare rows within a profile and use
  the alternated bench for wall-clock.
- `gm_build` (the JSON path) is untouched.
