# Job perf-open-synth-columns (agent build: the studio's synthetic graph goes to the motor as GMC1 columns, never as JSON)

Why: opening a 400k synthetic graph in the studio takes 4.4–6.9 s. A CPU profile of the open
(`deploy/perf/open.py 400000 webgl2`, 2026-10-03) puts about 3.3 s of it in the JSON round trip:
building the records and `JSON.stringify` (~0.78 s), staging the text into wasm memory (~0.6 s),
and `gm_build`'s JSON parse, record strings and `index_model` (~1.98 s). This branch starts from
`perf-open-columns`, which adds the binary build `gm_build_columns` and the SDK's
`encodeColumns`/`buildColumns`. Its measurement (`docs/measurements/perf-open-columns.md`) found
the general encoder too slow for the studio, because it dedupes every string through a `Map`. The
generator does not need that dedupe: the contract allows duplicate table entries, and the generator
already knows every endpoint as a row number, so it needs no id → row lookup either.

Read first: `docs/contract/ingest-columns.md` (the format), `crates/graph-sdk-js/src/columns.ts`,
`packages/graph-studio/src/source/synthetic.ts`, `packages/graph-studio/src/motor/session.ts`
(`generated` at about line 106, `replace` at about line 268).

## Done when

1. Opening a synthetic source in the studio calls `buildColumns`, never `build(json)`. No JSON
   text and no `IngestEdge` object is made on that path.
2. For every spec in the differential corpus, the columns document decodes to exactly the records
   `syntheticRecords(spec)` returns: every field, every row, in order. Weights are compared with
   `Object.is`.
3. The JSON output of `syntheticIngest` is unchanged byte for byte. `tests/ingest.test.ts:34`
   (`SEED_1_EXPECTED`) and every other existing test pass unchanged.
4. Every row of `scripts/orch/rows/perf-open-synth-columns.rows` passes.
5. `docs/measurements/perf-open-synth-columns.md` gives the open time at 400k and 1M before and
   after, with the commands, and the per-stage times from step 6.

## Do, in order

1. **SDK: one assembler** (`crates/graph-sdk-js/src/columns.ts`; if it passes 300 lines, move
   the assembler into a new `columns-assemble.ts` and import it).
   - Export:
     ```ts
     export interface ColumnRows {
       /** The string table in index order. Duplicates are allowed: the contract does not dedupe. */
       readonly strings: readonly string[];
       /** The 8 node u32 columns, column-major: column c of row r is at c * nodeCount + r. */
       readonly nodeCells: Uint32Array;
       /** The 8 edge u32 columns, column-major, endpoints as node rows. */
       readonly edgeCells: Uint32Array;
       readonly weights: Float64Array;   // nodeCount long
       readonly versions: Float64Array;  // nodeCount long
       readonly strengths: Float64Array; // edgeCount long
     }
     export function assembleColumns(rows: ColumnRows): Uint8Array
     ```
   - `nodeCount = weights.length` and `edgeCount = strengths.length`. A column array of the
     wrong length is refused with a `GraphMotorError` that names the array.
   - **The blob, ASCII fast path.** `const text = rows.strings.join("")`. Size the output buffer
     as if the blob were `text.length` bytes, then `encodeInto(text, out.subarray(blobAt, blobAt
     + text.length))`. If `read === text.length && written === text.length`, every code unit was
     ASCII. The offsets are then the running sums of `strings[i].length`, written as a
     `Uint32Array`, with no per-string encoding.
   - **Otherwise, the exact path.** Refuse any string with `isWellFormed() === false`, naming its
     index (`ColumnsEncoderError("strings[" + i + "]")`); check per string, because two lone
     surrogates joined can make a valid pair. Measure each string with the existing
     `utf8Length`, allocate the exact buffer and write the blob with `encodeInto`.
   - **Floats and ints.** Copy them with typed-array views:
     `new Float64Array(out.buffer, at, n).set(weights)` and
     `new Uint32Array(out.buffer, at, 8 * n).set(nodeCells)`. Both offsets are aligned: the
     columns start at a multiple of 8 and the buffer is fresh. Views are host-endian and the
     format is little-endian, so test endianness once at module load
     (`new Uint8Array(new Uint16Array([1]).buffer)[0] === 1`) and fall back to the `DataView`
     writers that exist now. Mark the fallback with a `Ponytail:` comment (no big-endian JS host
     is known).
   - **`encodeColumns(doc)` uses it.** It keeps its `Map` dedupe and its endpoint check, builds a
     `ColumnRows` (cells column-major) and calls `assembleColumns`. Its output bytes must not
     change: `crates/graph-sdk-js/test/columns.test.mjs` must pass unchanged. Add one test there
     that asserts byte equality between `encodeColumns` and `assembleColumns` over a hand-built
     `ColumnRows` for the same small document.
   - **New SDK tests.** Each exercises the decoder through `buildColumns`:
     - an all-ASCII table with duplicate entries builds;
     - a multi-byte string takes the exact path and builds, and its id reads back the same;
     - a lone surrogate is refused with the field `strings[<i>]`;
     - two lone surrogates that join into a valid pair are refused;
     - a column of the wrong length is refused.
2. **The generator keeps edges as columns** (`packages/graph-studio/src/source/synthetic.ts`).
   - Both generators write each edge into preallocated `from: Uint32Array`, `to: Uint32Array` and
     `kind: Uint8Array` (an index into `["hierarchy", "note_link", "tag", "relation"]`), in place
     of an `IngestEdge` object.
   - The edge count is exact and known before generation:
     - `random`: `degree * max(0, count - max(1, degree))`;
     - `vault`: `degree * (count - 1)`.
     Allocate exactly that, and throw if the filled count differs at the end.
   - Also record each node's group as an index into a fixed group list (`GROUPS` then `TOPICS`) in
     a `Uint8Array`, so the columns path needs no string lookup per node.
   - Keep the draw order exactly. `syntheticRecords(spec)` keeps its signature and return shape: it
     makes the `IngestEdge[]` from the columns with the same field values and order. Every caller
     (`deploy/perf/wasm-open.ts`, `harness/ingest-ceiling.mjs`, the tests) keeps working
     unchanged.
   - The file is at 222 lines. Put the new code in a sibling `source/synthetic-columns.ts`, and
     move the generators into a sibling module if `synthetic.ts` would pass 300 lines.
3. **`syntheticColumns(spec)`** (new `packages/graph-studio/src/source/synthetic-columns.ts`).
   - Returns `{ rows, nodes, edgeCount }`:
     - `rows` has the structural shape of the SDK's `ColumnRows`. Declare a local interface:
       `source/` may not import the SDK (`app/eslint.config.js` line 65), and TypeScript's
       structural typing makes it fit.
     - `nodes` is the same `IngestNode[]` that `syntheticRecords` makes, with the same weights,
       because the studio UI reads it.
   - The string table:
     1. A fixed head first: the 4 node kinds, the 4 edge kinds, the group list, `db-0` … `db-7`,
        `"studio"`.
     2. Then, for node row i, its id at `head + 2i` and its label at `head + 2i + 1`. Reuse
        `nodes[i].id` and `nodes[i].label`; never make a second copy.
     3. Then, for edge row j, its id `e-${j}` at `head + 2n + j`.
   - The edge label column is the kind's head index, since the JSON's label equals the kind.
     `database_id` is the head index of `db-${i % 8}`, `source` is the head index of
     `"studio"`, icon is `0xFFFF_FFFF` and `has_note` is `kind === note ? 1 : 0`. On edges,
     `record_id` is `0xFFFF_FFFF`, `directed` is `kind === hierarchy ? 1 : 0` and `child_first`
     is 0.
   - `weights` comes from the same pass as `applyDegreeWeights` (one shared function, not a copy:
     `rules/library-first.md`). `versions` is zero-filled, and `strengths.fill(0.5)`.
   - Complexity is O(n + m) time with one pass per column. No `Map`, no per-row object and no
     per-row typed array.
4. **Session** (`packages/graph-studio/src/motor/session.ts`).
   - Add `buildColumns(bytes: Uint8Array): Handle` to `MotorLike`; the SDK `Motor` has it on this
     branch.
   - Replace `Document.json` with `payload: { kind: "json"; text: string } | { kind: "columns";
     bytes: Uint8Array }`.
   - `generated()` makes the columns: `assembleColumns` lives in the SDK, which `session.ts` may
     not import. Inject it like `digest`: add `readonly assemble: (rows: ColumnRowsLike) =>
     Uint8Array` to `SessionDeps`, and wire it in `worker.ts` and `tests/motor.ts` (both may import
     the SDK). `documentFor` gets the function as an argument.
   - Documents (`normalised`) and fixtures stay on JSON.
   - `replace` dispatches on `payload.kind`.
   - Update every `MotorLike` fake in `packages/graph-studio/tests` (find them with
     `git grep -n "MotorLike\|motorFrom" packages/graph-studio/tests`). A fake that never builds
     columns may throw from `buildColumns`; it must not return a handle it never made.
   - Keep `session.ts` at or under 300 lines; it is at 293. If the change pushes it over, move
     `generated`/`normalised`/`documentFor` into `motor/documents.ts`.
5. **The differential** (`packages/graph-studio/tests/synthetic-columns.motor.test.ts`).
   - A test-only GMC1 decoder of about 40 lines that turns bytes back into `{ nodes, edges }`
     records, resolving every string and every endpoint row to its id.
   - The corpus: shapes `random` and `vault` × seeds 1 and 7 × node counts 2, 3, 50 and 2000 ×
     degrees 0, 1, 3 and 12. Include a spec where `degree >= count`.
   - For each spec, `assert.deepStrictEqual(decode(assembleColumns(syntheticColumns(spec).rows)),
     syntheticRecords(spec))`. That is done-when 2.
   - End to end: for `random` and `vault` at 2000 nodes, degree 3, the snapshot bytes after
     `layout.forceatlas2.barnes_hut` from `motor.build(syntheticIngest(spec))` equal those from
     `motor.buildColumns(assembleColumns(...))` (`toBytes`, compared with `Buffer.compare`).
   - **Negative control:** swap the target rows of edges 0 and 1 in the bytes, or flip one
     `has_note`, and assert the decoded records are **not** deep-equal. With the same mutation fed
     to `buildColumns`, the snapshot bytes differ.
6. **Measure** (`docs/measurements/perf-open-synth-columns.md`).
   1. Extend `deploy/perf/wasm-open.ts` with an `ARM=columns` mode that times
      `syntheticColumns`, `assembleColumns` and `buildColumns` separately. The default JSON arm
      times records, `stringify` and `build`. Run both at 400000 and 1000000 nodes, 3 runs each
      under `scripts/orch/node-slim.sh`, and report medians and the wasm MiB.
   2. Browser open, before and after: `scripts/studio.sh build`, then `PERF_MEMORY=10g
      scripts/studio-probe.sh open 400000 webgl2` and the same at 1000000.
      - "Before" is the build of this branch's base commit (`git stash` is not enough: check out
        the base into a scratch directory under `target/`, or record the base numbers first,
        before step 4 lands).
      - Run the probes one at a time.
      - Record `uptime` beside each number, because the host is shared.
   3. Follow the house report shape: design table, gates, numbers, and "what it does not do",
      with a `Caveat:` for the host load.

## Out of scope (do not touch)

- `crates/graph-core`, `crates/graph-wasm`, `crates/graph-contract`: no Rust changes. The wasm
  export is already on this branch.
- `packages/graph-render`, `app/`, the JSON path for documents and fixtures, and
  the threads staging in `crates/graph-sdk-js/src/staging.ts`.
- No new dependency anywhere.

## Report

Return the house block: what changed, every row's result, the before/after numbers, deviations
(each with a reason), and decisions needed. Never report a row you did not run as passing.
