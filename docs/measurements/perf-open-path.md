# Perf — the open's document path: one copy into linear memory, not three

Measured 2026-10-03 in worktree `perf-open-path`, commit `d3b0e8e`, host dlesieur42 (20 cores,
load average 6.7–7.5 from other jobs), image `gm-chromium` (Chrome 154.0.8037.92, viewport
1920x1080, DPR 1), renderer strings printed by the probe itself in every line below:

```text
hardware : ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23) (0x000073FF)), radv)
software : ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)
```

What this measures: how long the studio's synthetic open takes to hand the motor its bytes, and
which worker rows moved when `buildStaged` stopped building a second byte array. It answers the
question `docs/measurements/perf-p5c.md` §6 left open — `encode` at `staging.ts:21` was 6.8% of the
worker's self time at 1M, the third-largest row in the document path.

**Caveat:** the `open s` column is the wall of `window.__perf.open` **with the profiler on** (this
probe always profiles) and the host was not idle, so the wall column moves by more than the change
does; the percentage rows, which are sampled CPU time inside one target, are the load-independent
evidence and are what the verdict rests on. The three shapes at step 3 were measured in node, not
in the Chromium worker: same V8, but not the same process, so those numbers are a comparison
between two writers, not an open measurement.

## Reproduction

```sh
scripts/studio.sh build                                    # build first, always
# interleaved rounds, 3 per arm per size: software 200k, software 1M, hardware 200k, hardware 1M
PERF_MEMORY=10g scripts/studio-probe.sh open 200000  webgl2 before
PERF_MEMORY=10g scripts/studio-probe.sh open 1000000 webgl2 before
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh open 200000  webgl2 before
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh open 1000000 webgl2 before
# ... the same twelve with `after` as the label, against the rebuilt bundle
# the three text writers at 1M (scratch, target/perf-open-path/text-bench.ts, not committed)
scripts/orch/node-slim.sh node --experimental-strip-types --max-old-space-size=12288 \
  target/perf-open-path/text-bench.ts 1000000 2 vault stringify
# the two staging branches over the real motor
scripts/orch/node-slim.sh node --test --experimental-strip-types \
  packages/graph-studio/tests/staging.motor.test.ts
# gates
scripts/studio.sh check; scripts/studio-backend.sh; STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh
scripts/studio-smoke.sh; STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh
```

## 1. What changed

`crates/graph-sdk-js/src/staging.ts` staged a document in three steps: `encoder.encode(text)` built
a fresh `Uint8Array` of the whole document, `gm_alloc(len)` reserved linear memory, and `.set()`
copied the array in. An ASCII document now goes in one copy: `gm_alloc(text.length)`,
`encoder.encodeInto(text, view)`, and when `read === text.length` the bytes are already where
`gm_build` wants them. The `gm_build` export is handed `written` and the buffer is freed under the
length it was reserved with. A character outside ASCII is more than one byte, so the encoded bytes
do not fit a reservation made for the characters; that buffer is freed and the encoded array is
staged the old way, which is the only behaviour change for a document a user drops in.

The branch is on `read`, never on `written`: `encodeInto` fills a too-small buffer to the brim, so a
`written === text.length` test passes on a truncated document and hands the motor half a document.
`packages/graph-studio/tests/staging.motor.test.ts` is what found that — it fails with
`BuildRefusedError` on the wide document until the test reads `read`.

The golden digests in that test are the snapshot bytes the **pre-change** staging path produced for
its two documents, taken by running `gm_alloc`/`set`/`gm_build` by hand over the same module
(`target/perf-open-path/golden.ts`, scratch, not committed). So a byte lost, doubled or reordered on
the way into linear memory moves a digest, in either branch.

## 2. Open time, 3 interleaved rounds per arm per size

| nodes | arm | open s before (r1/r2/r3) | median | open s after (r1/r2/r3) | median |
|---:|---|---|---:|---|---:|
| 200 000 | software | 1.91 / 2.31 / 2.22 | **2.22** | 1.78 / 1.70 / 1.83 | **1.78** |
| 200 000 | hardware | 1.89 / 3.14 / 2.13 | **2.13** | 1.74 / 1.79 / 1.77 | **1.77** |
| 1 000 000 | software | 10.74 / 11.88 / 11.10 | **11.10** | 10.93 / 10.83 / 10.85 | **10.85** |
| 1 000 000 | hardware | 10.64 / 15.78 / 13.50 | **13.50** | 9.97 / 10.48 / 11.12 | **10.48** |

The 200k medians are the clean read: **−20% software, −17% hardware**. The 1M `before` column is not:
its hardware round spread is 10.64 → 15.78 → 13.50 on a loaded host, wider than any effect measured
here, so its −22% median is noise, not a result. The 1M after column is tight (10.93 / 10.83 / 10.85
software), which is what the row below explains.

## 3. Worker self time, 1M, hardware arm — the rows that moved

Percentages of one target's sampled time, `open.py`'s `report()` (`deploy/perf/open.py:66`), r1/r2/r3.

| row | before | after |
|---|---|---:|
| `encode` — `TextEncoder.prototype.encode`, was `staging.ts:21` | 8.5 / 8.2 / 10.1 → **8.5%** | gone (0 samples) |
| `encodeInto` — `staging.ts`, in place | — | 1.9 / 1.9 / 1.7 → **1.9%** |
| `Xt`/`Qt` — the synthetic document, `JSON.stringify`, `src/source/synthetic.ts:189` | 10.6 / 10.8 / 12.3 → **10.8%** | 10.9 / 10.9 / 10.4 → **10.9%** |
| `indexmap` lookup in the interning arena (`arena.rs:154`) | 9.7 / 10.3 / 8.1 → **9.7%** | 10.3 / 11.8 / 11.8 → **11.8%** |
| `canonical_json::Parser::value` (`parse.rs:68`) | 4.8 / 5.1 / 4.5 → **4.8%** | 4.6 / 4.3 / 4.6 → **4.6%** |
| `dlmalloc` malloc / free | 3.8 / 3.9 / 4.3 → **3.9%** | 4.1 / 4.2 / 4.0 → **4.1%** |
| `(garbage collector)` | 4.0 / 3.8 / 3.1 → **3.8%** | 3.4 / 4.5 / 3.1 → **3.4%** |

At 200k the same row moves from **8.3 / 8.8 / 8.4 / 9.3 / 8.9 / 10.6%** (`encode`, six runs over both
arms) to **1.7 / 1.9 / 2.0 / 2.0 / 2.0 / 1.7%** (`encodeInto`).

**Verdict: the row this job came for is gone.** The 500 MB `Uint8Array` the old path allocated per
open at 1M is not built, and the copy that read it into linear memory is not made: 8.5% of the
worker's self time becomes 1.9%. The two neighbours of the change that are *not* the generator —
the arena's interning lookup (9.7 → 11.8%) and the JSON parse (4.8 → 4.6%) — are the same work,
now a larger share of a smaller total, which is what a fixed cost does when the cost above it goes.

Two things did **not** move and are worth saying plainly, because the change was expected to move
them: `(garbage collector)` is 3.4–3.8% on both sides, and `dlmalloc` is 3.9–4.2% on both. The GC row
is the wasm allocator and the parse's own allocations, not the JS heap — the dropped 500 MB was a
large-object allocation the sampling profiler attributes to `encode` itself, so it left with it.

The profile names `Xt` before and `Qt` after for the same minified function: the worker chunk is
rehashed by the build (`worker-BnV2y1mr.js` → `worker-DVmd7SZt.js`), and minified names move with
the hash. `Xt` is the `JSON.stringify` call at `src/source/synthetic.ts:189`, reached from
`session.ts:113` — the profile's own naming, not a guess: it is the only row that walks the whole
record array.

## 4. Step 3, measured and not kept: writing the text from the columns

The job asked whether writing the document text directly from the generator's columns — one array of
string parts, joined once — beats `JSON.stringify` of the record objects, and to keep it only if it
is **>3% faster at 1M** and byte-equal to `syntheticIngest`. All three writers produce the identical
**516 614 428**-character string for seed 1, 1 000 000 nodes, degree 2, `vault`
(`bytes 516614428 chars 516614428` on each, and `stringify byte-equal true` for both others), so
equality is not the obstacle. Speed is:

| writer | 1M median of 9 | 200k median of 9 | vs `JSON.stringify` at 1M |
|---|---:|---:|---:|
| `JSON.stringify({version: 1, nodes, edges})` — today's | **1209 ms** | **240 ms** | — |
| parts array of per-record templates, `join("")` | **4311 ms** | **540 ms** | **+256%** |
| `+=` rope of the same per-record templates | **7533 ms** | — | **+523%** |

**Not kept.** The parts join is 3.6× the cost of `JSON.stringify` at 1M and 2.3× at 200k, which is
not a marginal miss against a 3% bar but the wrong side of the argument by two orders of magnitude.
`JSON.stringify` is native code walking the same records; the JS writer builds one short-lived string
per field group and then either an array of 2M+ entries or a rope, and the allocation traffic costs
more than the native walk saves. So `session.ts:113` still calls `JSON.stringify`, no text-writer
change was made, and the byte-equality test that would have pinned it has nothing to pin.

The measurement stands as the answer for whoever asks next: at 1M the generator's own
`JSON.stringify` row is **10.9%** of the worker's self time, and a JS text writer cannot take any of
it. Cutting that row means not producing the text at all — a wasm-side ingest that takes the columns
rather than JSON — which is `open-core-slot`'s side of the motor, not this job's.

## 5. Exit codes, controls and hazards

| command | exit | expectation |
|---|---:|---|
| `scripts/studio.sh check` | 0 | 427 + 560 unit tests, 98 render tests, 0 skipped |
| `scripts/orch/node-slim.sh npm run sdk:typecheck` | 0 | — |
| `node --test --experimental-strip-types crates/graph-sdk-js/test/*.test.mjs` | 0 | 2 pass (the bare-directory caveat in this job's done-when) |
| `packages/graph-studio/tests/staging.motor.test.ts` | 0 | 2 pass, 0 skipped |
| `scripts/orch/node-slim.sh npm run sdk:smoke` | 0 | both build paths and the degraded paths over the real module |
| `scripts/studio-backend.sh` | 0 | — |
| `STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh` | 1 | non-zero |
| `scripts/studio-smoke.sh` | 0 | 5 rows PASS |
| `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | 1 | non-zero, 5 rows FAIL |

Every row above was run on the tree as it stands, after the last edit to `staging.ts`.
`scripts/orch/rows/perf-p5.rows` is green: those are the same commands.

**Hazard:** the fast path is chosen by the *characters*, not by a scan for non-ASCII, so the cost of
a non-ASCII document is one wasted `encodeInto` pass over its prefix before the fallback — at 1M
ASCII that is bounded by the 1.9% row in §3, and on a mostly-ASCII document with one wide character
at the end it is one extra pass over almost the whole text, not a second copy of it. The escape hatch
is `normaliseIngest`, which is the only way a document gets here with content the tables did not
produce.