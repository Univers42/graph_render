# fix-sdk — the TypeScript SDK and its adapters

Job: `prompts/jobs/fix-sdk.md`. Review: `docs/reviews/review-harness-sdk.md`, ids `B`, `M`, `m`,
`U` (B1, B2; M1–M10, M14, M36; m1–m31, m113, m114; U3, U6). The other ids belong to
`fix-harness-py` and `fix-harness-mjs`.

Nothing in `crates/graph-wasm/**`, `crates/graph-core/**` or `harness/sdk-smoke/**` was edited:
the SDK's behaviour is fixed inside the SDK, and every harness change is inside
`harness/adapter-convergence.mjs`.

## BLOCKER

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| B1 | BLOCKER | fixed | `B1: rowsToIngest refuses a non-finite cell naming its path` (+ nested) | `src/adapters/cells.ts:92` (`finiteAtDepth`), one predicate both adapters read |
| B2 | BLOCKER | fixed | `B2: notionToIngest refuses a non-finite number property naming its path` | `src/adapters/notion-cells.ts:63`, through the same predicate |

## MAJOR

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| M1 | MAJOR | fixed | `M1: session.tick refuses a negative count instead of running 2^32-1 ticks` | `src/force-params.ts:106` (`asU32`), called by `force.ts` `tick` |
| M2 | MAJOR | fixed | `M2: ticksRun is the count that ran, and tick(0) is zero` | `src/force.ts` `tick` |
| M3 | MAJOR | fixed | `M3: forceSession over a released graph handle is InvalidHandleError, not a session refusal` | `src/force.ts` `#create` |
| M4 | MAJOR | fixed | `M4: a non-finite position written through the view is refused on the next read` / `…on the next tick` | `src/force-columns.ts` `ForceColumns.read` / `assertFinite` |
| M5 | MAJOR | fixed | `M5 a u32 face carrying a label past 0xffffffff is refused` | `src/analysis-face.ts:104` |
| M6 | MAJOR | fixed | `M6: releasing a handle this motor never issued is refused with InvalidHandle` | `src/motor.ts` `release` |
| M7 | MAJOR | fixed | `M7: a column answering an illegal (ptr, len) pair is refused, not turned into a window` | `src/views.ts` `#view` |
| M8 | MAJOR | fixed | `M8: a rows cell keyed __proto__ survives as an own key` | `src/adapters/cells.ts:76` |
| M9 | MAJOR | fixed | `M9: a notion property keyed __proto__ survives as an own key` | `src/adapters/notion.ts`, same helper |
| M10 | MAJOR | fixed | `M10: a declared link.symmetric that is not a boolean is refused` | `src/adapters/rows.ts:183` |
| M14 | MAJOR | fixed | live: `expected-graph.json` with the `ingest` member removed exits **2** naming the member | `harness/adapter-convergence.mjs:74-96` |
| M36 | MAJOR | **false** | — | `scripts/orch/rows/develop-full.rows:59` (`sdk-test`) and `:66` (`sdk-test-control`) already run `crates/graph-sdk-js/test/`; the review read the tree without them |

## MINOR

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| m1 | MINOR | fixed | `m1: column() on a released handle names the recorded code` | `src/motor.ts` `#released`, `column` |
| m2 | MINOR | fixed | `m2: a column id this ABI does not register is refused, never coerced to 0` | `src/views.ts` `isRegisteredColumn`, used by `motor.ts` `column` |
| m3 | MINOR | fixed | doc-only + split: `index.ts` is 22, `motor.ts` 297, `stages.ts` 123 | `src/index.ts`, `src/motor.ts`, `src/stages.ts` |
| m4 | MINOR | fixed | `m4: a non-string document is refused as a document, naming its type` | `src/staging.ts` `stage` |
| m5 | MINOR | fixed | `m5: a trap inside gm_build surfaces as the trap, never as gm_free's refusal` | `src/staging.ts` `buildStaged` |
| m6 | MINOR | fixed | `m6: a frame at 0 is refused rather than read as an empty length` (+3) | `src/calls.ts` `frame` |
| m7 | MINOR | fixed | `m7: a POST registry scan that refuses is a PostRefusedError, not a RunRefusedError` | `src/calls.ts` `readRegistry` + `src/registries.ts` |
| m8 | MINOR | fixed | `m8 a module importing m.f is refused naming m.f, not by the empty import object` | `src/wasm.ts:144` |
| m9 | MINOR | fixed | `m9 a different source is refused naming both, and resetForTests() loads it` | `src/wasm.ts:227` |
| m10 | MINOR | fixed | `m10: a refused setParams at creation throws the parameter refusal, not a cleanup one` | `src/force.ts` constructor |
| m11 | MINOR | fixed | `m11: a row that is not a u32 is refused by pin and by unpin` | `src/force-params.ts` `asU32`, used by `pin`/`unpin` |
| m12 | MINOR | fixed | `m12: a field passed as undefined keeps the motor's own value` | `src/force-params.ts` `mergeParams` |
| m13 | MINOR | fixed | split: `force.ts` 286, plus `force-params.ts` 134, `force-columns.ts` 128, `force-calls.ts` 84 | `src/force*.ts` |
| m14 | MINOR | fixed | `m14: a parameter buffer that cannot be reserved is an allocation failure` | `src/errors.ts` `AllocationFailedError`, thrown by `force-params.ts` `withStagedParams` |
| m15 | MINOR | fixed | `m15 null is refused as InvalidOptionsError, not a raw TypeError` | `src/options.ts:13` |
| m16 | MINOR | fixed | `m16: a registry hands out ids, never the mutable map behind them` | `src/registries.ts` (the map is now private) |
| m17 | MINOR | doc-only | — | `src/types.ts:97` (`AnalysisResult`'s three optionals) |
| m18 | MINOR | fixed | `m18 a negative depth level is refused, naming max` | `src/analysis-face.ts:40` (`optionalU32`) |
| m19 | MINOR | fixed | `m19 a reordered face is refused, naming the pair that is out of order` | `src/analysis-face.ts:57` |
| m20 | MINOR | fixed | `m20: the reserved note columns are absent for every kind, and dim does not change it` | `src/views.ts` `columnApplies` |
| m21 | MINOR | fixed | `m21: an undefined cell is absent, exactly as a Notion null cell already was` | `src/adapters/cells.ts:79` |
| m23 | MINOR | fixed | `m23: a rows updatedAt outside the contract's u32 is refused` | `src/adapters/rows.ts:214` + `cells.ts` `u32` |
| m24 | MINOR | fixed | `m24: a declared title outranks the title type, in both directions` | `src/adapters/notion.ts:222` |
| m25 | MINOR | fixed | `m25: two property pairs that read one override key are refused, naming both` | `src/adapters/notion.ts:153` |
| m26 | MINOR | fixed | `m26: a page whose parent database the export does not carry is refused` | `src/adapters/notion.ts:237` |
| m27 | MINOR | fixed | `m27: a stamp outside the contract's u32 seconds is refused` | `src/adapters/notion-cells.ts:117` |
| m28 | MINOR | fixed | `M28: the barrel re-exports typeToRole` (asserted by `m30: every subpath … resolves`) | `src/adapters.ts:30` |
| m29 | MINOR | fixed | `m29: the ABI version this SDK speaks is 1 — a literal, not the constant it imports` + `this SDK's own ABI version is the pinned literal` | `test/abi-version.test.mjs`, `test/internals.test.mjs` |
| m30 | MINOR | fixed | `m30: every subpath the docs name resolves through the package's exports map` | `test/internals.test.mjs` (imports `@graph-motor/sdk-js`, `/adapters/rows`, `/adapters/notion` by name) |
| m31 | MINOR | fixed | live: `every override key is one the fixture declares` × 6 | `harness/adapter-convergence.mjs:283-296` |
| m113 | MINOR | fixed | doc-only: the four recorded deviations are retired and replaced by where each went | `docs/contract/wasm-abi.md` "File-size deviations" |
| m114 | MINOR | fixed | `m114: cells are emitted in byte order of the key, in both adapters` | `src/adapters/cells.ts:78`; the residual (JavaScript re-orders integer-like own keys numerically) is stated at `cells.ts:66-70` |

## Unverified items promoted

| id | verdict | evidence |
|---|---|---|
| U6 | **answered, not a finding** | M1–M4 and M6 were run against a built module before any edit (below). All four of M1, M2, M3, M4 and M6 exist and none was refuted. M3 was the only one whose proposed fix changes a thrown type — see *decisions needed*. |
| U3 | **answered by M14's fix, and executed** | with the `ingest` member removed from the committed fixture, the adapter-convergence run exits **2** naming the member (it used to be an uncaught `TypeError`, exit 1) |

## Commands and their last lines

Every command below was run in this job, on this tree.

```
# the merge floor (fix-common)
scripts/orch/gr cargo fmt --all --check                                  -> 0   FMT=0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings    -> 0   CLIPPY=0
scripts/orch/gr cargo test --workspace --no-fail-fast                    -> see below
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown-> see below
scripts/scigraphs-conformance.sh                                         -> see below

# this job's done-when
scripts/orch/node-slim.sh npm run sdk:typecheck                          -> 0   (no diagnostics)
scripts/orch/node-slim.sh npm run sdk:test                               -> 0   # tests 86  # pass 86  # fail 0
scripts/orch/node-slim.sh npm run sdk:smoke                              -> 0   "# pass"
scripts/orch/node-slim.sh npx eslint crates/graph-sdk-js/src --max-warnings=0 -> 0
scripts/orch/node-slim.sh node --experimental-strip-types harness/sdk-smoke.mjs \
    --adapter-convergence target/wasm32-unknown-unknown/release/graph_wasm.wasm -> 0  "# pass"

# the negative control the job body asks for: the same run with the B1/B2 finiteness
# predicate disabled (two `if (false && …)` edits, reverted immediately afterwards)
    … --adapter-convergence <wasm>                                       -> 1
      not ok - a NaN weight cell is refused, naming the cell (B1)
      not ok - an Infinity nested inside an array cell is refused, naming the position (B1)
      not ok - a NaN number property is refused, naming the property (B2)
      # 3 failed
```

### U6 — M1–M4 and M6 run against the built module, before any edit

`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` → 0,
`Finished \`release\` profile [optimized] target(s) in 6.59s`. Then, against that module:

```
M1  session.tick(-1)              -> printed "about to run" and had not returned after 60 s:
                                      tick(4294967295) about to run ...
                                      (nothing else; the process was killed)
M2  session.tick(4294967301)       -> status=running alpha=0.7339040224 ticksRun=4294967301 in 0.8ms
                                      (five ticks ran; the report named 4294967301)
M3  motor.forceSession(released)   -> THREW ForceSessionRefusedError code=1 msg=gm_force_session_create refused (InvalidHandle)
                                      isForceSessionRefused=true isInvalidHandle=false
M4  xs[0]=NaN; tick(2)             -> status=running allFinite=false xs0=NaN
M6  motor.release(999)             -> returned without throwing
M6  motor.release(h) twice         -> second release returned without throwing
m1  motor.column(released, NodeX)  -> InvalidHandleError code=undefined msg="handle 5 has no successful run yet"
m11 session.pin(NaN, 5, 5)         -> pinned without throwing; xs0=12
m12 setParams({gravity: undefined})-> ForceSessionRefusedError: gm_force_session_set_params refused (SessionRefused)
```

Every claim the review made about these six held.

## Decisions taken (nobody was there to ask)

1. **`index.ts` became a barrel and the class moved to `motor.ts`.** The contract recorded this
   file as an un-splittable deviation; m113 asks for that record to be retired, and the limit
   is real, so the split was done rather than re-justified. `package.json`'s `"."` still points
   at `src/index.ts`, so no consumer path and no export name moved.
2. **Two new error classes, both additive.** `AbiContractError` (the module answered something
   the ABI forbids: a frame that does not fit, a column pair that is illegal, a column id it
   never registered — none of which has a `gm_last_error` behind it) and `AllocationFailedError`
   (`gm_alloc` refused; no session was asked and nothing refused). m14 required a class distinct
   from `ForceSessionRefusedError`, and inventing a new one is the additive form of that change.
3. **M4 gates on read *and* before the tick.** The review offered either re-validating on read
   or a `setPositions` verb. Re-validating only on read still lets a `NaN` reach the integrator,
   so both places check; the cost (two linear scans per call) is stated in the code with a
   `Ponytail:` line and there is deliberately no switch to turn it off. A tampered session is
   permanently refused rather than repaired, exactly as a tampered handle is.
4. **m25 keeps the `"<db>.<property>"` override key.** It is public surface and `NOTION_OVERRIDES`
   in the harness uses it, so the format stays and a collision between two distinct property
   pairs is made loud instead. The limitation is documented on `key()`.
5. **m114 is fixed best-effort, and says so.** JavaScript orders integer-like own keys
   numerically whatever the insertion order, so the adapter's own order is byte order for every
   other key and the *canonical* byte order of the document remains the canonical writer's job
   — which is what the schema states. Stated at `cells.ts:66-70` rather than claimed.
6. **The `ticks` bound is the caller's `u32`, not a ceiling.** M1 is a coercion hole, and adding
   an arbitrary tick ceiling would be a new policy the ABI does not have; the doc now says the
   batch is bounded by `ticks` and by nothing else.

## Deviations

`crates/graph-sdk-js/src/{motor,stages,force,force-calls,force-columns,force-params}.ts` are new
or rewritten files inside the allowed `crates/graph-sdk-js/**`; `harness/adapter-convergence.mjs`
gained its negative cases, M14's guard and m31's check; `docs/contract/wasm-abi.md` was edited
only in its own "File-size deviations" section (m113), leaving the ingest ceiling's number alone.
No file outside the job's paths was modified — `git status --porcelain` shows exactly ten
entries, all inside them.
