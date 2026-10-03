# Perf open synth-columns: the studio opens a synthetic graph through `buildColumns`

Measured 2026-10-03 on branch `perf-open-synth-columns`, which carries `perf-open-columns` (the
`gm_build_columns` export and the SDK encoder) and `perf-open-intern` (`RowBySlot` and the memo,
`docs/measurements/perf-open-intern.md`), merged with develop `accd14e`. Host: dlesieur42,
i5-13600KF, 32 GB. Node from `scripts/orch/node-slim.sh`, Chromium from `scripts/studio-probe.sh`
(gm-chromium, SwiftShader).

Why: opening 1M synthetic nodes built a JSON document of 490 MiB, then parsed it in wasm with
`gm_build`, which peaked at 2651 MiB of linear memory and took about 15 s. The generator already
holds the graph as columns, so the text round trip was pure waste.

## Design

| Piece | Where | What changed |
|---|---|---|
| assembler | `graph-sdk-js/src/columns-assemble.ts` | `assembleColumns(rows)` writes the binary columns document straight from typed columns plus a string table; `encodeColumns` (records in) and it give the same bytes |
| generator | `graph-studio/src/source/synthetic.ts`, `synthetic-draw.ts` | edges stay as columns (`Uint32Array` endpoints, kind and weight columns); no record object per edge |
| columns | `graph-studio/src/source/synthetic-columns.ts` | `syntheticColumns(spec)` gives the rows the assembler takes; `syntheticIngest` (JSON) is unchanged byte for byte |
| session | `graph-studio/src/motor/session.ts`, `documents.ts`, `worker.ts` | a synthetic source opens with `motor.buildColumns(deps.assemble(rows))`; `assemble` is injected, because the session may not import the SDK |
| staging | `graph-sdk-js/src/staging.ts` | a columns document is already bytes, so it takes the copied path; text keeps develop's in-place `encodeInto` path |

The merge with develop removed a second handle-insert helper in `graph-wasm/src/exports/build_paths.rs`
(develop's `build::insert` does the same thing) and an import only the native build used.

## Gates

| Check | Result |
|---|---|
| `cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings` | 0, 0 |
| `cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0, no warning after `8bd428a` |
| `sdk:typecheck`, `sdk:test`, `sdk:smoke` | 0; `sdk:test` 14 of 14 (`abi-version` 2, `columns` 12) |
| `scripts/studio.sh check` | ok: tsc ×3, eslint, vite build; 1114 tests, 1114 pass, 0 fail, 0 skipped |
| `scripts/studio-smoke.sh` / `STUDIO_SMOKE_BREAK=1` | PASS, 6 of 6 rows / exit 1, as the negative control expects |
| `scripts/orch/gate.sh … rows/perf-open-synth-columns.rows` | see the commit after this doc; `land.sh` re-runs `quick.rows` on the merged tree |

The first `studio.sh check` after the merge failed: two develop tests built a session without
`assemble` and a staging fake without `gm_build_columns`. Both were fixed in `8bd428a`.

## Node: the open path at 400k and 1M

```
for r in 1 2 3; do for n in 400000 1000000; do for arm in json columns; do
  ARM=$arm scripts/orch/node-slim.sh node --experimental-strip-types --max-old-space-size=12288 \
    deploy/perf/wasm-open.ts $n layout.forceatlas2.barnes_hut > target/bench-columns/n$n-$arm-r$r.json
done; done; done
```

Both arms run this branch's wasm. `json` = generate records, `JSON.stringify`, `gm_build`;
`columns` = `syntheticColumns`, `assembleColumns`, `gm_build_columns`. `total` = generate +
encode/assemble + build. Medians of 3 runs, ms; MiB is wasm linear memory.

| n | arm | total, 3 runs | generate | encode / assemble | build | total | document MiB | build MiB |
|---:|---|---|---:|---:|---:|---:|---:|---:|
| 400000 | json | 4989, 4852, 5550 | 552 | 518 | 3952 | **4989** | 195 | 1086 |
| 400000 | columns | 1447, 1670, 1203 | 458 | 208 | 820 | **1447** | 70 | 263 |
| 1000000 | json | 18076, 16814, 17632 | 1389 | 1297 | 14946 | **17632** | 490 | 2651 |
| 1000000 | columns | 6885, 4470, 4281 | 1688 | 464 | 2493 | **4470** | 177 | 730 |

400k: −71% open, −76% build memory. 1M: −75% open, −72% build memory. Layout time and the snapshot
bytes are the same work in both arms (the bytes are equal, `synthetic-columns.motor.test.ts`).

## Browser: `scripts/studio-probe.sh open N webgl2`

```
PERF_MEMORY=10g scripts/studio-probe.sh open 400000 webgl2     # then 1000000
```

Run one at a time, alternated before, after, before, after; raw output in
`~/goinfre/logs/synth-merge/probe/`. "Before" is the studio build of `perf-pm-serial` (`ab8487b`),
which is develop plus the particle-mesh deposit and has develop's JSON open path. "After" is this
branch at `8bd428a`.

| n | before open s | after open s | change | first frame ms, before / after | load (1 min) |
|---:|---:|---:|---:|---|---|
| 400000 | 5.09 | 2.14 | −58% | 133 / 134 | 14.6–15.5 |
| 1000000 | 12.41 | 5.72 | −54% | 137 / 134 | 14.6–15.5 |

Where the 1M open goes after (self time, sampled): the motor worker is busy about 4.8 s of 6.64 s.
The top rows are the worker's own JavaScript, the generator and assembler (936 ms), GC (519 ms),
`Snapshot::new` (353 ms), the id `decode` (343 ms) and the arena's `IndexMap` probe (275 ms). On the
main thread `transferToImageBitmap` is 639 ms, which is SwiftShader rasterising, not a GPU.

## What it does not do

- A file or fixture source still opens through `build(json)`. Only synthetic sources take the
  columns path.
- The generator still runs in the worker before the build, serially: 936 ms at 1M. Generating
  straight into the document bytes would drop the intermediate columns.
- `Snapshot::new` and the per-id `decode` that follows the build are untouched; at 1M they cost
  about 0.7 s together.
- Caveat: the host was shared (load 12.4–22.9 over the node bench, 14.6–15.5 over the probes;
  landers and OpenCode jobs ran alongside). The 1M columns runs 1 and 2 first overlapped each
  other and were re-run serially; run 1 (6885 ms, load 18.6) is still the outlier. Only medians
  are claimed for the node bench, and each browser number is a single run.
- Caveat: the browser "before" is `perf-pm-serial`, not this branch's base commit, as the brief
  asked. Its diff against develop is the particle-mesh deposit and one test, neither on the open
  path, so the arms differ only by the open path.
- `ColumnsInvalid = 20` collides with `fix-sdk` (`ParamOutOfRange = 20`, `ParamsMalformed = 21`,
  `ParamsNotAccepted = 22`). The codes are dense, so whichever lands second renumbers; this branch
  takes 23 if `fix-sdk` lands first.
- `crates/graph-sdk-js/src/index.ts` is 345 lines after the merge, over the 300-line limit; develop's
  copy was already over it at 318.
