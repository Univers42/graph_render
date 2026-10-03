# Job fix-sdk (agent build, review repairs: the TypeScript SDK and its adapters)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-harness-sdk.md` (ids `B`, `M`,
`m`, `U`). Ids: B1, B2 (BLOCKER); M1-M10, M14, M36 (MAJOR); m1-m31, m113, m114 (MINOR); U3, U6.
The other ids are owned by `fix-harness-py` and `fix-harness-mjs`.

Judgement notes:
- B1, B2: one shared finiteness predicate, used by both adapters (`rows.ts`, `notion-cells.ts`), so
  the two cannot drift. A non-finite cell is refused with `bad()` naming the dotted path. The
  negative cases go in `harness/adapter-convergence.mjs` and must fail on the current code.
- M1-M4, M6 (U6): the review read these, it did not run them. Run each against the built module
  (`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`) before you
  fix it, and paste the run. A claim the run refutes is `false`, and its test stays.
- The SDK's exports are public surface: a fix that changes a signature or a thrown type is additive
  or it is "decisions needed".
- m113: `docs/contract/wasm-abi.md`. `fix-wasm-ingest` edits the ingest ceiling's number in the same
  file: edit only your lines.

Paths: `crates/graph-sdk-js/**`, `harness/adapter-convergence.mjs`, `docs/contract/wasm-abi.md`
(m113 only), `docs/measurements/fix-sdk.md`.

Done when, in addition to fix-common: `scripts/orch/node-slim.sh npm run sdk:typecheck`,
`npm run sdk:test` (added by `fix-wasm-ingest`, which lands first) and `npm run sdk:smoke` exit 0 (paste each); the adapter-convergence run passes,
and fails with the B1/B2 fix reverted (paste both).
