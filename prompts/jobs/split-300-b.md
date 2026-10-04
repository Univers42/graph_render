# Job split-300-b (agent build, branch split-300-b, worktree ~/goinfre/wt/split-300-b)

Context: the house limit is 300 lines per file. Two test files on develop d26e4c11 are over it
(re-check with `wc -l`):
- `tests/graph-engine.test.ts`: 482 lines, 27 test cases. Run by `npm test`
  (`package.json:49`, glob `tests/*.test.ts`) inside `scripts/orch/ge-check.sh`.
- `crates/graph-sdk-js/test/columns.test.mjs`: 384 lines, 12 test cases. Run by `npm run sdk:test`
  (`package.json:54`, glob `crates/graph-sdk-js/test/**/*.test.mjs`); it reads the release wasm
  artifact, so build it first (row `wasm-release`).

Out of scope, leave untouched: `verify/parity/main.tsx` (550 lines) is the frozen parity page that
`prompt.md:446` cites by line number.

Exact tasks:
1. Before any edit, record the baseline: run `scripts/orch/ge-check.sh` and, after
   `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`,
   `scripts/orch/node-slim.sh npm run sdk:test`. Paste each runner's summary lines (`# tests`,
   `# pass`, `# fail`, `# skipped`).
2. Split `tests/graph-engine.test.ts` into sibling files `tests/graph-engine-<topic>.test.ts`
   (topics from the file's own grouping), each at most 300 lines. Shared fixtures or helpers used
   by more than one new file go into one non-test module `tests/graph-engine-fixtures.ts` (it must
   not match `*.test.ts`). Keep the file header style the original uses.
3. Split `crates/graph-sdk-js/test/columns.test.mjs` the same way into
   `crates/graph-sdk-js/test/columns-<topic>.test.mjs`, each at most 300 lines; shared setup (the
   wasm loader) into `crates/graph-sdk-js/test/columns-fixtures.mjs`. Keep the header comment's
   "what it pins" list, divided among the files it now describes.
4. Pure move: same test names, same count, no assertion or input changed. Re-run step 1's commands
   and paste the summary lines: `# tests` must equal the baseline in both runners, `# fail 0`,
   `# skipped 0`. Paste `wc -l` of every new and changed file.

Limits: files at most 300 lines, functions at most 40 lines and 4 parameters, no type assertions
(`as`), eslint `--max-warnings 0` stays green.

Paths allowed: `tests/graph-engine.test.ts`, `tests/graph-engine-*.ts`,
`crates/graph-sdk-js/test/columns.test.mjs`, `crates/graph-sdk-js/test/columns-*.mjs`,
`prompts/jobs/split-300-b.md`. Not allowed: everything else (`src/`, `package.json`, the
lockfile, `tests/ts-extension-loader.mjs` are fingerprinted or shared).

Done when, each with its command and exit pasted:
- `scripts/orch/gate.sh target/gate-split-300-b scripts/orch/rows/split-300-b.rows` all PASS;
- both runners' `# tests` equal the baseline, `# fail 0`, `# skipped 0`;
- `wc -l` shows every new and changed file at most 300 lines.
