# fix-harness-mjs — the Node harness and the SDK smoke

Review: `docs/reviews/review-harness-sdk.md` at `origin/develop` `1cca30e`. Ids owned here:
M11–M13, M24, M25, M30–M35, m32–m43, m61, m72–m76, m78–m87, m88–m105, and the unverified
items U1, U2, U4 that fall in these paths. U1 turned out to be owned with `oracle-diff.mjs`
and is recorded below.

Every row's test is a run in this task. Where a negative case is a mutation of a committed
or emitted artifact, both the "before" (defect reproduced) and the "after" (fixed) runs are
pasted under "Negative cases". Bench arms (`oracle-tick-bench.mjs`, `wasm-tick-bench.mjs`)
are measurements, not gates: no fix below changes the workload, the warm-up, the repeat
count, the tick count or the metric. Every one of them is a refusal before the timer starts
or a read-only assertion on the finished arrays.

## Verdicts

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| M11 | MAJOR | fixed | `snapshot-raw` row, `harness/read-snapshot-raw.test.mjs` (12 tests) | `harness/read-snapshot-raw.mjs:158-175`, `scripts/orch/rows/develop-full.rows:111` |
| M12 | MAJOR | fixed | `an open oneOf branch in the schema fails naming the def and the kind` | `harness/read-snapshot-raw.mjs:100-121` |
| M13 | MAJOR | fixed | `an existing graph half survives byte for byte` | `harness/write-expected-ingest.mjs:46-49, 84-95` |
| M24 | MAJOR | fixed | `the dag dump is refused when its bytes changed under an unchanged tree` | `harness/oracle-dag.mjs:79-96`, `harness/oracle-attest.mjs` |
| M25 | MAJOR | fixed | `the twelve H9 lines are anchored to rebuild's own line range` | `harness/oracle-h9.mjs:29-48` |
| M30 | MAJOR | fixed | `m30: a z column on a 2D run is a reported problem, not a pass` | `harness/sdk-smoke/lib.mjs:59-113` |
| M31 | MAJOR | fixed | `degraded_motor_{layout,column,toJSON,toBytes,release}_fails_predictably_kill_switch` | `harness/sdk-smoke/degraded.mjs:20-33, 84-100` |
| M32 | MAJOR | fixed | `every position a new session reports is finite`, `every position after the drag is finite` | `harness/sdk-smoke/force.mjs:52-57, 82-86` |
| M33 | MAJOR | fixed | `a pinned node sits exactly where it was pinned`, `and it stays there through a later tick`, `an unpinned node is free to move again` | `harness/sdk-smoke/force.mjs:89-102` |
| M34 | MAJOR | fixed | `M34: a producer that declares closeness a u32 is a reported problem` | `harness/sdk-smoke/analysis.mjs:16-27, 52-57`, `analysis.test.mjs` |
| M35 | MAJOR | fixed | `a released handle is refused by both stages as InvalidHandle (C6)` | `harness/sdk-smoke/analysis.mjs:136-155` |
| m32 | MINOR | doc-only | — (by instruction: no gate row, kept out of the harness on purpose) | `harness/write-expected-ingest.mjs:12-20` |
| m33 | MINOR | fixed | `an unparseable fixture refuses with 2, not a SyntaxError` | `harness/write-expected-ingest.mjs:71-83` |
| m34 | MINOR | fixed | `a 200k-node snapshot summarises instead of overflowing the stack` | `harness/read-snapshot-raw.mjs:143-155` |
| m35 | MINOR | fixed | `an unknown node kind is one recorded failure, not a TypeError` | `harness/read-snapshot-raw.mjs:240, 258-262` |
| m36 | MINOR | fixed | `a missing or unparseable schema refuses with exit 2, not a stack` | `harness/read-snapshot-raw.mjs:58-67` |
| m37 | MINOR | fixed | `` `hash 0` writes no digest line and no C20 verdict, at exit 2 `` | `harness/wasm-run/lib.mjs:44-47`, `hash.mjs:41, 77` |
| m38 | MINOR | fixed | `the C20 pair and the arm's stage table name one layout on both sides` | `harness/wasm-run/lib.mjs:34`, `abi.mjs:151` |
| m39 | MINOR | fixed | `a module without the exports this mode drives is refused by name` | `harness/wasm-run/module.mjs:29, 66` |
| m40 | MINOR | fixed | `every file of the wasm arm is within the house's 300-lines limit` (394 → 102+90+83+201+80+131) | `harness/wasm-run.mjs:102`, `harness/wasm-run/*.mjs` |
| m41 | MINOR | fixed | `a hand-edited fixture line is caught even when the digest is re-sealed` | `harness/oracle-diff.mjs:87-115` |
| m42 | MINOR | fixed | `the persisted record names the worst failing function's seed and both sides` | `harness/oracle-diff.mjs:250-275` |
| m43 | MINOR | fixed | `a fixture-supplied array is reduced with a loop, not a spread` | `harness/oracle-diff.mjs:207` |
| m61 | MINOR | fixed | `the widened and narrowed groups come from one function` | `harness/oracle-h9.mjs:55-87` |
| m72 | MINOR | fixed | `the virtual root is discriminated by id, not by a null weight` | `harness/oracle-layout-value.mjs:1-14`, `oracle-layouts.mjs:90` |
| m73 | MINOR | fixed | `a missing dag harness refuses with 2, not an unhandled rejection` | `harness/oracle-layouts.mjs:201-208` |
| m74 | MINOR | fixed | `seeds: 0 with an empty jsonl is refused` | `harness/oracle-layouts.mjs:79-84` |
| m75 | MINOR | fixed | `the binary contract: magic/format/dim/padding, CSR framing, column order` | `harness/oracle-wire-bytes.mjs` (163 lines), `oracle-wire.test.mjs` (16 tests) |
| m76 | MINOR | fixed | `canonical refuses a non-finite number and an undefined member` | `harness/oracle-wire.mjs:23-40` |
| m78 | MINOR | fixed | `` a d3-force other than the pinned one is refused by name `` | `harness/d3-version.mjs`, `harness/oracle-tick-bench.mjs:120, 246` |
| m79 | MINOR | fixed | `buildSyntheticModel(n) produced 100000 nodes, not 200000` | `harness/oracle-tick-bench.mjs:216-224, 252` |
| m80 | MINOR | fixed | `oracle-tick-bench: could not run: EEXIST … mkdir` (exit 2, was an unhandled rejection) | `harness/oracle-tick-bench.mjs:280-287` |
| m81 | MINOR | fixed | `target/red/broken.wasm did not load, so no layout can be timed through it` | `harness/wasm-tick-bench.mjs:236-250` |
| m82 | MINOR | fixed | `m82: the member lists come from the committed contract, not from beside the writer` | `harness/wasm-tick-bench/contract.mjs`, `wasm-tick-bench.test.mjs` (7 tests) |
| m83 | MINOR | fixed | `m83: only the pinned d3-force version is accepted` | `harness/stress-d3.mjs:69-80`, `d3-version.mjs` |
| m84 | MINOR | fixed | `m84` run: `stress-d3: could not read …: ENOENT` (exit 2, was exit 1) | `harness/stress-d3.mjs:135-139` |
| m85 | MINOR | fixed | `m85: an edge naming a node outside 0..n-1 is refused with its case number` | `harness/stress-d3/validate.mjs:19-32` |
| m86 | MINOR | fixed | `m86: a non-finite position is refused, naming the case and the node` | `harness/stress-d3/validate.mjs:39-47`, `stress-d3.mjs:158-160` |
| m87 | MINOR | fixed (witness) | `m87: the golden-spiral seed is load-bearing — one ulp of angle moves it` | `harness/stress-d3/seed.mjs:28-40` |
| m88 | MINOR | fixed | `sdk-smoke: could not run: ENOENT … open 'target/red/nope.wasm'` (exit 2, was exit 1) | `harness/sdk-smoke.mjs:41-46` |
| m89 | MINOR | fixed | `piped=377 direct=377 double-piped=377` on a 26004-byte stdout | `harness/sdk-smoke/lib.mjs:196-204` |
| m90 | MINOR | fixed | `m90: the SDK's ColumnId must spell the contract's numbers` | `harness/sdk-smoke/lib.mjs:59-88`, `lib.test.mjs` |
| m91 | MINOR | fixed | `motor.release(stagedHandle)` moved into the `analysed === 0` branch | `harness/sdk-smoke/analysis.mjs:73-79` |
| m92 | MINOR | fixed | `it reports the fixture's five nodes` | `harness/sdk-smoke/analysis.mjs:51-52` |
| m93 | MINOR | fixed | `{postId}: it names itself` | `harness/sdk-smoke/post.mjs:79-80` |
| m94 | MINOR | fixed | `before.length === 4 and after.xs[1] !== before[1]` | `harness/sdk-smoke/force.mjs:71-84` |
| m95 | MINOR | fixed (recorded) | comment on the negative-control block; the leak is on the failing path only | `harness/sdk-smoke/force.mjs:104-112` |
| m96 | MINOR | fixed | `every refusal names the kill switch, not an unrelated load failure` | `harness/sdk-smoke/degraded.mjs:35, 92-100` |
| m97 | MINOR | fixed | `degraded_motor_*_fails_predictably_compile_failure` (12 rows, was 1) | `harness/sdk-smoke/degraded.mjs:112-118` |
| m98 | MINOR | fixed | `the kill switch is restored exactly: absent stays absent` | `harness/sdk-smoke/degraded.mjs:63-69` |
| m99 | MINOR | fixed | `{what} is refused as ContractInvalid, not derived` | `harness/sdk-smoke/convergence.mjs:11-20, 84-92` |
| m100 | MINOR | **false** (the strength half) / doc-only (the comment) | — the wasm ABI has no strength column, so no assertion here could catch it | `harness/sdk-smoke/convergence.mjs:29-37` |
| m101 | MINOR | fixed | the layout comes from `contractMotor.layouts()`, never a literal | `harness/sdk-smoke/convergence.mjs:39-47` |
| m102 | MINOR | fixed | `check(name, condition, detail)` now prints the detail | `harness/sdk-smoke/lib.mjs:12-21` |
| m103 | MINOR | fixed | `the bytes motor.toBytes returns are a copy, not a view over its own memory` | `harness/sdk-smoke/transport.mjs:15-27, 55-68` |
| m104 | MINOR | fixed | `a released handle is refused as InvalidHandle, not silently answered (C6)` | `harness/sdk-smoke/transport.mjs:81-95` |
| m105 | MINOR | fixed | `the node x column is a live, non-empty view`, `and a face read before the release is not answered after it (C6)` | `harness/sdk-smoke/transport.mjs:35-42, 97-108` |
| U1 | — | fixed-by m41 | see m41; the manifest is no longer self-attesting over `expect.jsonl` | `harness/oracle-diff.mjs:87-115` |
| U2 | — | **fixed** (verified, not false) | `a def the walk cannot resolve is a hole, not a pass` | `harness/read-snapshot-raw.mjs:124-129` |
| U4 | — | **fixed** (verified, not false) | `` `hash 0` writes no digest line and no C20 verdict, at exit 2 `` | `harness/wasm-run/lib.mjs:44-47` |

### m100 in full

The review says the comment claims "if the derivation moved, or a node id or a strength
changed, this fails" but no strength is compared. Both halves are true and only one is
fixable. The node-id half was already checked (`:42-58`). The strength half cannot be:
`docs/contract/wasm-abi.md`'s column table ends at `EdgeCurveDegree` (11) and `NODE_Z` (12),
`$defs.Edges.properties` in `docs/contract/snapshot-schema.json` is exactly
`["id","source","target"]`, and the wire has no strength column at all. An edge strength is
simply not observable from the wasm ABI, so the honest fix is to stop claiming it. Recorded
`false` for the finding, `doc-only` for the comment, with the reason in the file.

## Negative cases

Every one of these was run on the **pre-fix** code first (defect reproduced) and again after
(exit 2, or the named failure). `node` runs are `scripts/orch/node-slim.sh node …`;
`graph-cli` runs are `scripts/orch/gr cargo run -q -p graph-cli -- …`.

### Group A — read-snapshot-raw / write-expected-ingest / develop-full.rows

```
# M11 RED: the row ran the arm with no path, so the literal in the source was the input
node harness/read-snapshot-raw.mjs                       -> 0  "#4 nodes, 5 edges … # pass"
# M11 GREEN: the row's own two halves, artifact produced by graph-cli
gr cargo run … --out-json target/snapshot-raw.json         -> 0  snapshot: seed 0, layout.grid, 4 nodes, 5 edges
node harness/read-snapshot-raw.mjs target/snapshot-raw.json -> 0  "# 4 nodes, 5 edges …" / "# pass"
node harness/read-snapshot-raw.mjs --selftest             -> 0  "# pass"
node harness/read-snapshot-raw.mjs                        -> 2  "no snapshot given" + usage
# M11 negative: geometry deleted from the artifact between the two halves
node harness/read-snapshot-raw.mjs tmp/no-geometry.json   -> 1  "snapshot.geometry: missing edges; missing nodes" / "# 2 failed"
# M11 RED, before the fix, on the same input
node tmp/red/harness/read-snapshot-raw.mjs <no-geometry>  -> 1  TypeError: Cannot use 'in' operator … in undefined

# M12 RED: the Line branch of $defs.EdgeGeometry.oneOf opened, digest irrelevant (schema arg)
node tmp/red/… -- <schema w/ Line opened>                 -> 0  "ok - every object … refuses an unknown member" / "# pass"
node harness/read-snapshot-raw.mjs --schema <opened> real  -> 1  "#/$defs/EdgeGeometry/oneOf[0]/Line"

# U2 RED: $defs.Nodes renamed to NodeIds with the $ref retargeted — the walk silently skipped it
node tmp/red/… -- <renamed schema> real                    -> 0  "ok - the snapshot carries every member …" / "# pass"
node harness/read-snapshot-raw.mjs --schema <renamed> real -> 1  "snapshot.nodes: the schema states no object shape for #/$defs/Nodes"

# m34 RED: a 200k-node snapshot
node tmp/red/harness/read-snapshot-raw.mjs <big.json>      -> 1  RangeError: Maximum call stack size exceeded at span (…:165:26)
node harness/read-snapshot-raw.mjs <big.json>              -> 0  "# 200000 nodes, 1 edges …" / "# pass" (11.4 MB, no RangeError)
# m35 RED: an unknown node kind
node tmp/red/… <unknown-kind.json>                          -> 1  TypeError: Cannot read properties of undefined (reading 'filter') at …:174:24
node harness/read-snapshot-raw.mjs <unknown-kind.json>      -> 1  "not ok - the node geometry is a kind this reader can place" / "# 1 failed"
# m36 RED: the schema file moved
node tmp/red/… <real.json> (schema moved)                   -> 1  ENOENT … 'snapshot-schema.json' (uncaught)
node harness/read-snapshot-raw.mjs --schema <absent> real   -> 2  "could not read schema …: ENOENT"

# M13 RED: a fixture whose `graph` half carries the committed derivation
node tmp/redw2/harness/write-expected-ingest.mjs           -> 0  wrote …; graph after the run: null (3760 bytes vs 8651)
node harness/write-expected-ingest.mjs --out <graph-only>  -> 0  graph half byte-identical (9 edges / 8 nodes)
node harness/write-expected-ingest.mjs --out <bad-shaped>  -> 2  "`graph` is present and is not the shape …"
# m33 RED: an unparseable fixture
node tmp/redw2/harness/write-expected-ingest.mjs           -> 1  SyntaxError: Expected property name … at …:27:33
node harness/write-expected-ingest.mjs --out <unparseable> -> 2  "… is not JSON …; nothing was written" (fixture untouched)
```

### Group B — wasm-run.mjs

```
# U4 RED: the vacuous pass, run on the pre-refactor arm
node tmp/red/harness/wasm-run.mjs <wasm> hash 0 layout.grid transport.wasm.columnar
   -> 0  wasm-run: C20 ok — transport.wasm.columnar == layout.grid on all 0 seeds
node harness/wasm-run.mjs <wasm> hash 0 layout.grid transport.wasm.columnar
   -> 2  wasm-run: 0 seeds: a tally over nothing proves nothing — run `hash` with a seed count of 1 or more
# m39 RED: a module missing 17 of the exports the mode drives
node harness/wasm-run.orig.mjs <truncated.wasm> hash 1 topology -> 1  TypeError: exports.gm_topology is not a function
node harness/wasm-run.mjs        <truncated.wasm> hash 1 topology -> 2  wasm-run: missing 17 export(s): gm_topology, gm_layout_grid, gm_alloc, …, gm_snapshot_bytes, …
# m37/m38 RED: only half of the C20 pair requested — a silent skip, exit 0, no message at all
node harness/wasm-run.orig.mjs <wasm> hash 2 topology transport.wasm.columnar -> 0  (4 digest lines, no C20 line)
node harness/wasm-run.mjs        <wasm> hash 2 topology transport.wasm.columnar -> 2  wasm-run: 2 stages requested but the C20 pair is half of that: transport.wasm.columnar without layout.grid. …
# m40: the refactor is byte-identical
node harness/wasm-run.mjs <wasm> hash 8 …                          -> 0  wasm-run: C20 ok … on all 8 seeds   (stdout+stderr diff vs pre-refactor: identical)
node harness/wasm-run.mjs <wasm> --assert-zero-copy                 -> 0  # view re-derivation: 209.2 ns/call over 200000 calls / # pass
node harness/wasm-run.mjs <wasm> stages                            -> 0  (56 ids)
node --test harness/wasm-run.test.mjs                              -> 0  # tests 12 / # pass 12 / # fail 0
```

### Group C — oracle-diff / oracle-dag / oracle-h9 / oracle-layouts / oracle-wire

```
# M24 RED: the dump is the arm under test and carried no digest
set d[0].our_crossings = 0 in target/dag-crossings.json ; node harness/oracle-layouts.mjs --dag
   (pre-fix) -> 0   verdict=ok
   (fixed)   -> 2   oracle-layouts: could not run: oracle-layouts --dag: the measured bytes changed under an
                       unchanged tree since the last passing run (7a4c3bc79dcb -> 0448038ea73b): the dump or
                       fixture set was edited, or re-sealed by hand
# restored: dump regenerated by graph-core's own `dump_crossing_measurements`
gr cargo test -p graph-core dump_crossing_measurements -- --ignored     -> 0
node harness/oracle-layouts.mjs --dag -> 0  dump dag-crossings.json: sha256 7a4c3bc79dcb, source fingerprint 3081fcd91270 / status: pass
# m73 RED: the --dag import sits outside the try/catch
   (pre-fix) -> 1  unhandled rejection
   (fixed)   -> 2  oracle-layouts: could not run: … (exit 2, via fail())
# m74 RED: a manifest with seeds: 0 and an empty layouts.jsonl, digest re-sealed
   (pre-fix) -> 0  printed a summary
   (fixed)   -> 2  refused
# m76 RED: canonical's NaN swallow
   canonical({x: NaN})  == canonical({x: null})  == '{"x":null}'   (pre-fix)
   (fixed) -> throws, naming the member
node --test harness/oracle-wire.test.mjs -> 0  # tests 16 / # pass 16 / # fail 0
node --test harness/oracle-attest.test.mjs -> 0  # tests 10 / # pass 10 / # fail 0
# M25 RED: the twelve H9 lines moved into a dead `legacyGroups()`, re-indented
   checkTranscription passes (pre-fix); refused (fixed) — anchored to rebuild's own line range
# m41/m42/m43/m61: unit-pinned; m42's persisted record now carries the worst failing
   function's seed, fn, args and both sides (jq .functions target/gates/oracle-diff.json)
```

### Group E — oracle-tick-bench / wasm-tick-bench / stress-d3

```
# m78/m83 RED: a shadow tree whose node_modules/d3-force/package.json says 3.0.1
node target/red/shadow/harness/stress-d3.mjs …              -> 0  wrote positions, exit 0
node target/red/shadow/harness/stress-d3.mjs …  (fixed)    -> 2  stress-d3: could not run: resolved d3-force is 3.0.1, not the pinned 3.0.0.
node target/red/ots/harness/oracle-tick-bench.mjs --n 220 --repeat 1 (fixed)
   -> 2  oracle-tick-bench: could not run: resolved d3-force is 3.0.1, not the pinned 3.0.0; this arm's rows are attributed to d3-force@3.0.0
# m79 RED: --n above the generator's own cap
node harness/oracle-tick-bench.mjs --n 200000 --repeat 1   -> 2  buildSyntheticModel(200000) produced 100000 nodes, not 200000: …
   (pre-fix: it timed 100000 nodes and printed `nodes: 200000`)
# m80 RED: an unwritable --out
node harness/oracle-tick-bench.mjs --n 220 --repeat 1 --out target/red/ok.jsonl/x.md
   (pre-fix) -> 1  Error: EEXIST: file already exists, mkdir 'target/red/ok.jsonl'   (stack, unhandled rejection)
   (fixed)   -> 2  oracle-tick-bench: could not run: EEXIST: file already exists, mkdir 'target/red/ok.jsonl'
# m81 RED: a four-byte "module"
node harness/wasm-tick-bench.mjs --wasm target/red/broken.wasm --n 220 --repeat 1
   (pre-fix) -> 1  WasmUnavailableError: wasm module failed to load        (uncaught, from layouts())
   (fixed)   -> 2  wasm-tick-bench: could not run: /w/target/red/broken.wasm did not load, so no layout can be timed through it: wasm module failed to load
# m82 RED: NODE_FIELDS/EDGE_FIELDS were compared against themselves
node harness/wasm-tick-bench.mjs --self-check (fixed) -> 0  ok - a node record carries exactly the contract's members
   node --test harness/wasm-tick-bench.test.mjs      -> 0  # tests 7 / # pass 7 / # fail 0
   (`m82: the lists track the file` is the negative case: a hand copy cannot pass it.)
# m84 RED: an absent input
node harness/stress-d3.mjs target/red/nope.jsonl …  (pre-fix) -> 1  Error: ENOENT … (uncaught)
node harness/stress-d3.mjs target/red/nope.jsonl …  (fixed)   -> 2  stress-d3: could not read target/red/nope.jsonl: ENOENT: …
# m85 RED: {"n":2,"edges":[[0,7]]} and a case with no seed
node harness/stress-d3.mjs target/red/bad.jsonl   (pre-fix) -> 1  Error: node not found: 7   (three frames deep, no case number)
node harness/stress-d3.mjs target/red/bad.jsonl   (fixed)   -> 2  stress-d3: case 0: edge 0 names a node outside 0..1
node harness/stress-d3.mjs target/red/noseed.jsonl (fixed)  -> 2  stress-d3: case 0: no integer seed   (pre-fix: exit 0, a line with no `seed`)
node harness/stress-d3.mjs target/red/frac.jsonl  (fixed)   -> 2  stress-d3: case 0: edge 0 is not a pair of integers
# m86 RED: JSON.stringify writes NaN as null and the arm exited 0
   unit-pinned (no e2e path exists: no case in the input format reaches a non-finite position)
node --test harness/stress-d3.test.mjs -> 0  # tests 9 / # pass 9 / # fail 0
# m87 RED: nothing asserted the spiral's sensitivity
node -e '…seedMovesWithAngle(8)' -> true
# No fix changed what any bench measures: n=220 row identical before and after,
#   | 220 | 327 | 0.831 | 114.7 |  (oracle) and  | 220 | 11.32 | 30.467 | 0.272 |  (wasm)
```

### Group F — sdk-smoke

```
# m88 RED: no path, and a path that does not exist
node harness/sdk-smoke.mjs target/red/nope.wasm  (fixed) -> 2  sdk-smoke: could not run: ENOENT: … open 'target/red/nope.wasm'
node harness/sdk-smoke.mjs                       (fixed) -> 2  sdk-smoke: could not run: usage: sdk-smoke.mjs <wasm> | --adapter-convergence
# m89: 26004 bytes of stdout through one pipe and two, against a direct redirect
piped=377  direct=377  double-piped=377          (process.exitCode, not process.exit)
# m90 RED: ColumnId keyed by the SDK's own constants — `NodeR: 3, NodeW: 2` agreed with itself
node --test harness/sdk-smoke/lib.test.mjs -> 0  # tests 6 / # pass 6 / # fail 0
   ok 5 - m90: the SDK's ColumnId must spell the contract's numbers
# M30 RED: the table had no NodeZ row, so a views.ts answering `true` for NodeZ on every run passed
   unit-pinned against a stub motor (the negative cases the live loop could not fail):
   ok 1 - m30: a z column on a 2D run is a reported problem, not a pass
   ok 2 - m30: a 3D run with no z column is a reported problem, not a pass
   ok 3 - m30: a 3D run with a z column is not a problem
   ok 4 - m30: the kind-keyed rows still key on kind, not on dim
# M31 RED: five of the twelve named methods had no coverage at all
   before: 8 `fails_predictably` rows (7 kill-switch + 1 compile-failure)
   after:  24 (12 × 2 blocks) — build, layouts, posts, analyses, post, layout, column,
          toJSON, toBytes, analysis, forceSession, release
# M32/M33/M94/m95: force.mjs — the pin/unpin pair is a differential inside the smoke:
   ok - a pinned node sits exactly where it was pinned
   ok - and it stays there through a later tick
   ok - an unpinned node is free to move again
   (replace force.ts's unpin body with {} and the third check fails; the first two hold.)
# M34 RED: kind checked as a two-value set — closeness declared u32 passed every loop check
node --test harness/sdk-smoke/analysis.test.mjs -> 0  # tests 3 / # pass 3 / # fail 0
   ok 2 - M34: a producer that declares closeness a u32 is a reported problem
# M35 RED: bare `catch { refused = true }` — a RangeError reported ok
   fixed: `a released handle is refused by both stages as InvalidHandle (C6)` asserts the class
   AND the code name. (The contract names `InvalidHandle`, not the stage's own class:
   `docs/contract/wasm-abi.md:47`. The review proposed Post/AnalysisRefusedError; the
   contract wins per fix-common.md.)
# m93/m99/m101/m102/m103/m104/m105: all exercised by the live run; each is a check the
#   previous code did not make, so its failing input is a mutation, not a fixture:
#   post.mjs: `id: "post.style.straight"` for every pass -> `{postId}: it names itself` goes red
#   convergence.mjs: HandlesExhausted instead of a refusal -> `refused as ContractInvalid` goes red
#   transport.mjs: delete `frame()`'s `.slice()` -> `the bytes … are a copy` goes red
```

### Two bugs my own edits introduced, caught by the harness and fixed here

1. `appliesTo` in `sdk-smoke/lib.mjs` tested `typeof wanted === "object"` before
   `Array.isArray`, so `["Polyline","Curve"]` was read as a `{dim}` rule. The live loop went
   red on 8 checks (`edge column 9 is present but does not apply to Polyline`) before it was
   fixed, and `lib.test.mjs:65` now pins the ordering.
2. `checkPredictable` in `sdk-smoke/degraded.mjs` set the kill switch for each method and
   never restored it, so the stale-module block after it ran with the switch on and its
   `gm_dim` refusal became a kill-switch refusal. Exactly the m96 hazard the check exists
   for. `withKillSwitch` now owns the global's lifetime.

## The gate row I own (M11)

`scripts/orch/rows/develop-full.rows:111-115`, additive — no other row touched:

```
# The arm reads the artifact `graph-cli` writes, not a literal pasted beside the checker
# (M11): a snapshot from the code this row also builds is the only thing that shows drift
# on the producer's side of the JSON face. The worked example stays reachable, behind
# `--selftest` only — with no path at all the arm now refuses with 2 rather than fall back.
snapshot-raw|0|scripts/orch/gr cargo run -q -p graph-cli -- snapshot --seed 0 --nodes 4 --layout layout.grid --out-json target/snapshot-raw.json && scripts/orch/node-slim.sh bash -c 'npm ci --ignore-scripts >/dev/null && node --experimental-strip-types harness/read-snapshot-raw.mjs target/snapshot-raw.json'
```

`sg-sugiyama`'s `--dag` fixture entry in `oracle-layouts.mjs` is untouched and still there.

## Commands (real exit codes)

```
scripts/orch/gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release -> 0
npm run sdk:smoke                                                          -> 0  # pass
scripts/orch/ge-check.sh  (npm run check)                                   -> 0  # tests 27 / # pass 27 / # fail 0
scripts/orch/gr cargo fmt --all --check                                    -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings      -> 0
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown   -> 0
scripts/orch/gr cargo run -q -p graph-cli -- emit-fixtures                  -> 0
scripts/orch/gr cargo run -q -p graph-cli -- oracle-layouts                 -> 0  PASS
scripts/orch/node-slim.sh node harness/oracle-layouts.mjs --dag             -> 0  status: pass
scripts/orch/node-slim.sh bash -c 'node --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs harness/oracle-diff.mjs'
                                                                             -> 0  PASS
scripts/orch/gr cargo run -q -p graph-cli -- stress --oracle d3 --seeds 8   -> 0  median margin=+0.08455 over 7 correlated cases / PASS
scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-run.mjs <wasm> hash 8 layout.grid transport.wasm.columnar
                                                                             -> 0  C20 ok … on all 8 seeds
scripts/orch/node-slim.sh node --experimental-strip-types harness/wasm-run.mjs <wasm> --assert-zero-copy
                                                                             -> 0  # pass
node --test  harness/{read-snapshot-raw,write-expected-ingest,wasm-run,oracle-attest,oracle-wire,stress-d3,wasm-tick-bench}.test.mjs
               -> 0  12 + 8 + 12 + 10 + 16 + 9 + 7 tests, 0 failures
node --test  harness/sdk-smoke/{lib,analysis}.test.mjs                    -> 0  6 + 3 tests, 0 failures
node --experimental-strip-types harness/sdk-smoke.mjs <wasm>                -> 0  # pass
node --experimental-strip-types harness/sdk-smoke.mjs --adapter-convergence <wasm>
                                                                             -> 0  # pass (18 checks, convergence only)
```

## Merge floor

| check | exit | note |
|---|---:|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 101 | **1647 passed, 1 failed** — see below |
| `bash scripts/scigraphs-conformance.sh` | 0 | `PASS`, every row at its pinned ceiling |
| `npm run sdk:smoke` | 0 | `# pass` |
| `scripts/orch/ge-check.sh` (`npm run check`) | 0 | `typecheck` + `lint` + 27 tests, 0 failures |

The single failure is **not in this job's paths and not caused by this job**:

```
---- layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them ----
thread '…' panicked at crates/graph-core/src/layout/graphviz/dot/oracle_probe.rs:58:9:
/w/crates/graph-core/../../target/probe/rank1000.txt is missing; see this module's doc for
the command that writes it
test result: FAILED. 0 passed; 1 failed; … 1243 filtered out
```

`crates/graph-core/src/layout/graphviz/dot/oracle_probe.rs:49-52` states the opposite of
what the code does: *"The file is a probe under `target/`, so it is absent from a clean
checkout; the tests that read it say so and skip rather than fail, because a missing
measurement file is not a wrong answer."* It panics instead. The file and its generator
(`target/probe/rank_oracle.py`, run inside `ge-graphviz-oracle`) are absent from this
worktree, and nothing under `harness/**` is read by that test. Reported, not fixed:
`crates/graph-core/**` belongs to `fix-core-base` / `fix-core-post`. The recommended fix is
one `if (!path.exists()) { return Vec::new(); }` — or, better, a
`#[ignore]`d probe test — so a clean checkout skips it as documented.

Also note, from the first (discarded) workspace run: five `graph-cli` integration binaries
(`cli_force`, `cli_force_gate`, `cli_igraph`, `cli_ledger`, `cli_oracles`) failed there and
pass in isolation. That was self-inflicted — `scripts/scigraphs-conformance.sh`,
`emit-fixtures`, `oracle-layouts` and `stress` were all writing into the same `target/` while
the suite ran. The clean re-run above is the real result. Worth recording because the
attestation seals group D added (`target/gates/<gate>.seal.json`) are keyed by **gate name**,
not by fixture directory, so two concurrent runs of the same gate over *different* fixture
sets would contend for one seal. `cargo test` runs its integration binaries sequentially, so
this is latent rather than live, but it is the shape to know about:
`harness/oracle-attest.mjs`'s `Ponytail:` line records it.

## Not run — UNKNOWN, and therefore not claimed

- `gate.sh`, `hashgate --seeds 1000`, `mutants.sh`: the job body forbids timed gates; the
  orchestrator gates after this returns.
- `bench-oracle-tick` / `bench-wasm-tick` at the row's full `--n 220,10000,100000
  --repeat 5`: the `n=100000` leg alone exceeded 15 minutes in this container. Both arms
  were run at `--n 220` and `--n 220,10000`, and the `--self-check` mode of each, which are
  the paths every fix touches; every fix is a refusal or a read-only assertion placed
  **before** the timer starts.
- `scripts/oracle-tick-bench.mjs --n 220,10000,100000 --repeat 5` — same reason.
- `cargo test --workspace` was launched in the background; see "Merge floor" above.

## Decisions taken

1. **m100 recorded `false`, not fixed.** An edge `strength` is not observable through the
   wasm ABI at all (`$defs.Edges.properties` is `["id","source","target"]`; the column
   table ends at 11/12 with neither carrying strength). The contract wins over the review's
   suggested fix per `fix-common.md`, so the comment was corrected to stop claiming what no
   assertion here could check.
2. **M35 asserts `InvalidHandleError`, not `Post`/`AnalysisRefusedError`.** The review
   proposed the stage's own classes; `docs/contract/wasm-abi.md:47` names `InvalidHandle`
   for a released handle, and that is what the SDK actually throws. Contract wins.
3. **m82 reads the required members from the fenced JSON example in
   `docs/contract/wasm-abi.md:415-436`, not from `docs/contract/ingest-schema.json`.** The
   schema describes the *other* build path (`gm_build_contract`). The example omits
   `child_first`, which `crates/graph-wasm/src/ingest/record.rs` makes optional in version 1,
   so the arm's edge list is the contract's members ∪ `{child_first}` — stated in the code.
4. **M24/M41 attested rather than producer-stamped.** The producer of
   `target/dag-crossings.json` is `crates/graph-core/.../sugiyama/mod.rs:277` and the
   manifest's writer is `graph-cli`'s; neither is in this job's paths. The additive answer
   is `harness/oracle-attest.mjs`: a seal beside each gate record that refuses measured
   bytes which changed under an unchanged tree since the last passing run. Its `Ponytail:`
   line names what it does **not** buy (a first run over an unattested measurement passes;
   a tree edit re-attests).
5. **`npm run snapshot:raw` and three doc sites now exit 2.** M11 requires the arm to refuse
   with no path rather than fall back to the pasted literal, and `package.json`,
   `crates/graph-sdk-js/EXAMPLES.md`, `docs/reports/phase-10-progress.md` and
   `prompts/phase-10-ingest-sdk-publish.md` are not this job's paths. Resolved by the
   orchestrator: `npm run snapshot:raw` and both `EXAMPLES.md` commands now pass `--selftest`;
   the phase report and the phase prompt are history and keep the old command.
## Review fixes (2026-10-03, after the first branch gate)

The 21-row branch gate went red on two rows; neither was the seal.

| Row | Cause | Fix |
|---|---|---|
| `test` | `cli_oracles`' two `oracle_layouts_*` tests got exit 1: the rows file ran `cargo test` before any `npm ci`, so `d3-hierarchy` was not installed | `npm-ci` is now the first row |
| `oracle-diff` | no `emit-fixtures` row before it, so the fixtures predated the tree (exit 2) | `emit-fixtures-1000` and `oracle-layouts` rows before it |

Found while reading the `test` log: the two tick-bench negative controls passed **vacuously**.
The staged mutant sits in `target/`, and the sibling modules this job split out
(`./d3-version.mjs`, `./wasm-tick-bench/contract.mjs`) do not resolve from there, so the copy
died on `ERR_MODULE_NOT_FOUND` before the self-check ran. `bench/staging.rs` now rewrites the
copy's `from "./` imports to `from "../harness/`, and each control first runs an *unchanged*
staged copy, which must pass, so a staging fault can no longer pass as a red self-check.
`cargo test -p graph-cli --bin graph-cli bench::tests::harness`: 4 passed, no
`ERR_MODULE_NOT_FOUND` in the output.

The seal is now scoped: `oracle-diff` and `oracle-layouts` pass `scope: "seeds=<n>"`, so an
8-seed fixture set after a 1000-seed one on the same tree is a new measurement, not a refused
edit. `harness/oracle-attest.test.mjs`: 11 pass, including the new scope case.

Also: `harness/wasm-tick-bench/contract.mjs` line 60 held raw NUL bytes (git treated the file
as binary); it now spells them `"\0"`. One duplicated JSDoc line removed from
`oracle-layouts.mjs`; `d3-version.mjs` ends with a newline.
