# Job perf-open-path (agent build: the studio's synthetic open hands the motor its bytes with one copy, not three)

Why: opening 1 000 000 nodes takes about 20 s and 200 000 about 3.7 s, on hardware and on software
alike (docs/measurements/perf-p5c.md). The worker's document path is about 38% of it. open-synth,
open-ingest and open-core already took the generator's string maps, the parser's allocations and
the id check; what is left is the text itself.

Facts (perf-p5c profile at 1M, worker self time; re-run the probe for your own baseline first):
- `packages/graph-studio/src/motor/session.ts` `generated()` builds the records, then
  `JSON.stringify({ version: 1, nodes, edges })`: 9.0%.
- `crates/graph-sdk-js/src/staging.ts:21` `buildStaged`: `encoder.encode(text)` makes a fresh
  `Uint8Array` (6.8%), then `gm_alloc(len)`, then `.set(bytes)` copies it again into linear memory.
- The ingest text the studio builds is ASCII: ids are `n-<i>`/`e-<i>`, kinds and topics come from
  fixed tables in `src/source/synthetic.ts`. A document a user drops in may not be.

Do, in order:
1. Baseline: `scripts/studio.sh build`, then `deploy/perf/open.py 200000 webgl2` and
   `deploy/perf/open.py 1000000 webgl2`, 3 runs each, hardware arm if the host has one.
2. `buildStaged`: `gm_alloc(text.length)`, then `encoder.encodeInto(text, view)`. When
   `read === text.length` the bytes are already in place: pass `written` to the build export and
   free with the allocated length. Otherwise (non-ASCII text) free that buffer and fall back to
   today's path. One test per branch over the real motor, beside
   `packages/graph-studio/tests/settle.motor.test.ts` (only `*.motor.test.ts` may import the SDK):
   an ASCII document and one with a multi-byte id build the same graph as before (`toBytes` equal).
3. The synthetic document: measure whether writing the JSON text directly from the generator's
   columns (one array of string parts, joined once) beats `JSON.stringify` of the record objects.
   Keep it only if > 3% faster at 1M and the text is byte-equal to `syntheticIngest` for seeds
   1..5 at 10, 1 000 and 50 000 nodes (a `deepStrictEqual` test on the strings).
4. Write `docs/measurements/perf-open-path.md`: before and after, 3 interleaved runs at 200k and 1M,
   the commands, and the worker self-time rows that moved.

Out of bounds: `crates/graph-core`, `crates/graph-wasm`, `crates/graph-contract` (the motor side
is open-core-slot's), `src/motor/{session,settle,live,bridge}.ts` except the one `generated()`
call site in step 3, and `packages/graph-render`. No new dependency. House limits: 40 lines a
function, 300 a file, 4 parameters. No type assertions.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/orch/node-slim.sh npm run sdk:typecheck` exits 0, and
  `scripts/orch/node-slim.sh node --test --experimental-strip-types crates/graph-sdk-js/test/*.test.mjs` passes (node 22 does not expand a bare directory).
- studio-smoke exits 0 and `STUDIO_SMOKE_BREAK=1` exits non-zero.
- perf-p5.rows is green.
- docs/measurements/perf-open-path.md is written with the numbers.
