# Job fix-harness-mjs (agent build, review repairs: the Node harness and the SDK smoke)

Read `prompts/jobs/fix-common.md` first. Review: `docs/reviews/review-harness-sdk.md` (ids `B`, `M`,
`m`, `U`). Ids: M11-M13, M24, M25, M30-M35 (MAJOR); m32-m43, m61, m72-m76, m78-m105 (MINOR); U1,
U2, U4. The other ids are owned by `fix-sdk` and `fix-harness-py`.

A harness that judges must fail when the thing it judges is wrong. Each fix ships a negative case
that the current harness passes and the fixed one fails; U1, U2 and U4 were read, not run: run each
(build the artifact and the fixtures first) before you fix it. Use `node:test` for new tests, the
runner the harness already uses.

Judgement notes:
- M11 names a row in `scripts/orch/rows/develop-full.rows`. `fix-wasm-ingest` adds an `sdk-test`
  row to the same file: edit only your row, additively.
- `harness/oracle-layouts.mjs`: `sg-sugiyama` adds a `--dag` fixture to its list. Merge develop
  first and keep that entry.
- A bench (`*-tick-bench.mjs`) is a measurement, not a gate: a fix there must not change what it
  measures; say so in the row.

Paths: `harness/{read-snapshot-raw,write-expected-ingest,oracle-dag,oracle-h9,oracle-diff,
oracle-layouts,oracle-wire,oracle-tick-bench,wasm-run,wasm-tick-bench,stress-d3,sdk-smoke}.mjs`,
`harness/sdk-smoke/**`, new test files beside them, `scripts/orch/rows/develop-full.rows` (M11's row
only), `docs/measurements/fix-harness-mjs.md`.

Done when, in addition to fix-common: `npm run sdk:smoke` exits 0 and `ge-check.sh` (`npm run check`)
exits 0 (paste both); each harness you touched passes on develop's artifact and fails on its negative
case (paste both).
