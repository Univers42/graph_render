# Job sdk-lint: lint the JS SDK under the repo's eslint, zero warnings, with a gate row

Worktree `~/goinfre/wt/sdk-lint`, branch `sdk-lint`, from develop. Today nothing lints
`crates/graph-sdk-js`: the root `lint` script is `eslint src --max-warnings=0` (`package.json:50`).
Measured on develop bff2bd8b with `scripts/orch/node-slim.sh npx eslint crates/graph-sdk-js`, there are
52 errors, all in `.mjs` files under `test/` and `scripts/`. The `src/*.ts` files are clean:

- 50 × `no-undef`, all of them Node/web globals: `process` 11, `Buffer` 10, `URL` 10, `Response` 8,
  `console` 7, `TextEncoder` 2, `WebAssembly` 1, `fetch` 1. The root `eslint.config.js` declares no
  globals for `.mjs` files.
- 1 × `no-loss-of-precision` at `test/analysis-face.test.mjs:71`. **This one is a real test defect.**
  - `face` is `JSON.stringify(Object.fromEntries(pairs))` (`:21`), and `JSON.stringify([1e999])` is
    `"[null]"`.
  - So test "M5 an f64 face carrying a non-finite value is refused" never sends `1e999`. It sends
    `null`, and it passes for a different reason than its comment (`:69-70`) states.
- 1 × `@typescript-eslint/no-unused-vars` at `test/motor.test.mjs:22` (`BuildRefusedError`).

Do not dispatch subagents: the job is about six small edits.

## Exact tasks

1. **`eslint.config.js`.** Append one config object, `files: ["crates/graph-sdk-js/**/*.mjs"]`, with
   `languageOptions.globals` listing exactly the eight names above, each `"readonly"`.
   - Do not add the `globals` package. It is not a direct dependency of the root `package.json`, and
     eight names do not justify one.
   - Put one comment line above the object: why the list is inline, and that a ninth global is added
     to this list.
   - Change nothing else in the file.
2. **`test/analysis-face.test.mjs:68-72`.** Make the parser receive the literal token `1e999`.
   - Build the text with `face(...)`, using a placeholder string value, then replace the quoted
     placeholder with `1e999`.
   - Assert, in the test, that the text contains `1e999` before calling `refuses`.
   - Keep the test name and its `/values/` pattern.
   - If the parser then does **not** refuse with a message matching `/values/`, STOP. Do not change
     `src/`: report the parser's actual behaviour under "decisions needed". That would be a product
     bug in the finiteness check.
3. **`test/motor.test.mjs:22`.** Remove the unused `BuildRefusedError` import. If a test was meant to
   use it, say which one under "decisions taken"; do not write one.
4. **`package.json`.** Add the script `"sdk:lint": "eslint crates/graph-sdk-js --max-warnings=0"` after
   `sdk:test` (`:54`). No other change: no dependency and no lockfile change.
5. **Rows in `scripts/orch/rows/develop-full.rows`.** A row `sdk-lint` already exists at `:67`, and it
   lints only `crates/graph-sdk-js/src`, which is why the 52 errors went unseen.
   - Replace that one line's command with `scripts/orch/node-slim.sh bash -c 'npm ci --ignore-scripts >/dev/null && npm run sdk:lint'`.
     Keep the row's name, and keep it at `:67`.
   - Above it, add one `#` comment line in the file's style: the row now covers `test/` and
     `scripts/` too, and it used to lint `src` only.
   - Right after it, insert `negctl-sdk-lint|0|...`, with a `#` comment line above it. The row:
     - runs `scripts/orch/node-slim.sh bash -c '...'`, which pipes the text `undefinedGlobalForNegctl;`
       to `npx eslint --max-warnings=0 --stdin --stdin-filename crates/graph-sdk-js/test/negctl.mjs`;
     - requires that eslint call to exit 1;
     - requires its output to contain `no-undef`;
     - writes no file under `crates/`.
   - Do not touch `scripts/orch/rows/quick.rows`: it is the merge floor, and the orchestrator owns it.

## Allowed paths

`eslint.config.js`, `crates/graph-sdk-js/test/analysis-face.test.mjs`,
`crates/graph-sdk-js/test/motor.test.mjs`, `package.json` (one script line),
`scripts/orch/rows/develop-full.rows` (`:67` and the lines added next to it), `prompts/jobs/sdk-lint.md`, `target/**`. Nothing
else. A fix that needs another path is a stop: report it.

## Run, in order, and paste each exit code and its last 3 lines

1. `scripts/orch/node-slim.sh npm run sdk:lint` (expect 0)
2. the `negctl-sdk-lint` command (expect 0)
3. `scripts/orch/node-slim.sh npm run sdk:test` (expect 0, with no case skipped)
4. `scripts/orch/node-slim.sh npm run sdk:typecheck` (expect 0)
5. `scripts/orch/node-slim.sh npm run lint` (expect 0: the root `src` lint must not change)

## Return block

- per task, the file:line changed;
- task 2: the text the test now sends (one line) and the parser's refusal message;
- each command's exit code;
- "decisions taken" / "decisions needed".

## Done when

All five commands exit as expected, and `git status --porcelain` shows only the allowed paths.
