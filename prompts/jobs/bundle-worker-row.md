# Job bundle-worker-row: gate the service bundle's worker form and file list (review-bundle-unify, conditions 2-4)

Worktree `~/goinfre/wt/bundle-worker-row`, branch `bundle-worker-row`, from develop. The devil review
`docs/reviews/review-bundle-unify.md` (branch `review-bundle-unify`, commit 30d456b9; read it with
`git show 30d456b9:docs/reviews/review-bundle-unify.md`) ruled PROCEED-WITH-CONDITIONS on option (c):
keep the two bundles of `<graph-studio>` (the service's `app/vite.embed.config.ts` and the pack's
`app/vite.pack.config.ts`) and gate what they must agree on. Its sections 0, 2 and 5 are the facts this
job builds on. Dispatch at most 2 subagents (disjoint files).

## Decisions already taken (do not reopen them)

- Option (c): two bundles. Neither `vite.embed.config.ts` nor `app/src/embed-bundle.ts` is deleted.
- `graph-sdk.js` **stays** in the service bundle: it is the SDK the micro-service serves to a host that
  drives the motor without the element. Removing it would change the file set, so `<version>`
  (`scripts/service.sh:44-46`), so every host's tag. The review's objection is that nothing consumes it
  (section 2, "graph-sdk.js: built, staged, served — and consumed by nobody"). Task 4 adds the consumer.
- Condition 1 (auto-define) only applies to option (a). Record it in the decision file (task 6). No code.

## Exact tasks

1. **`scripts/worker-form.sh DIR...`**, the one worker predicate. Today it exists once, inline, at
   `scripts/studio-pack.sh:137-141`. The new script greps every `*.js` under each DIR, recursively
   (`assets/` included), with `grep -rnF 'new Worker'`. It then:
   - exits 1 if there is no such line at all, so a bundle with no worker cannot pass vacuously;
   - exits 1 if one of those lines carries `blob:`, `data:` or `createObjectURL` (`grep -F -e`), and
     prints `file:line` for at most 5 of them;
   - exits 2 if a DIR is missing;
   - exits 0 otherwise.

   Two rules on the pattern:
   - Never use `[^)]*`: section 0 of the review measured it as a false negative on minified output.
   - Header: what it checks, its exit codes, and a `Caveat:` line. The predicate is line-granular. A
     minified chunk can be one long line, so any `data:` on the same line as a `new Worker` (an inline
     icon, say) is reported as a worker the CSP refuses. That is over-reporting, the safe direction.
     A worker whose URL is built on a later line than `new Worker` is not seen, which is
     under-reporting. The escape hatch is the unminified pack.

   Then make `studio-pack.sh` call it in place of its inline grep (`:137-141`), keeping its
   `bad "a worker built from a blob: or data: URL: ..."` message. The pack's bytes must not change.

2. **`scripts/embed-files.sh DIR`**: the staged file list equals the documented one. Do both checks
   below and exit 1 on any drift, naming each file. Exit 2 on a missing DIR or doc bullet.
   - The documented list is the backticked file names in the `embed/<version>/` bullet of
     `docs/deploy/service.md` (`:25-26` today). Parse that bullet, not a copy of it in the script.
   - Every regular file at DIR's top level must be named in that bullet.
   - Every file the bullet names must exist in DIR.
   - `assets/` must hold at least one `*.js`. The bullet says "their chunks under `assets/`", so
     treat `assets/` as one entry.

   `scripts/service.sh:78-86` moves exactly the `studio.sh embed` output to
   `target/service/stage/embed/<v>/`. So checking that output is checking the staged list, with no
   docker build. Say so in the header, citing `service.sh:81,84`.

3. **Rows**, appended to `scripts/orch/rows/host-api.rows`, each with a one-line `#` comment above it in
   that file's style:
   - `embed-bundle|0|scripts/studio.sh embed target/embed-bundle && scripts/worker-form.sh target/embed-bundle && scripts/embed-files.sh target/embed-bundle`
   - `negctl-worker-form`. One row, in this order:
     - make a fixture dir `target/worker-form-fixture`;
     - write `ok.js` containing exactly `new Worker(new URL(` + newline + `"x.js",import.meta.url))`
       (the review's multi-line case). `worker-form.sh` on the dir must exit **0**;
     - add `bad.js` containing `new Worker(URL.createObjectURL(new Blob([""])))`. It must now exit
       **1**, and its output must name `bad.js`;
     - an empty dir must exit **1**.
   - `negctl-embed-files`: copy `target/embed-bundle` to `target/embed-files-break` and add `stray.js`
     at its top level. `embed-files.sh` must exit 1 and name `stray.js`. Then restore the copy and
     delete `graph-sdk.js`. It must exit 1 and name `graph-sdk.js`.
   - `embed-sdk` and its `negctl-embed-sdk` (task 4).

   `negctl-embed-files` and `embed-sdk` read `target/embed-bundle`, so they come after `embed-bundle`.

4. **The consumer of `graph-sdk.js`**: `harness/sdk-bundle-smoke.mjs DIR`.
   - It loads the **built** file with `await import(pathToFileURL(DIR/graph-sdk.js))`, never
     `crates/graph-sdk-js/src`.
   - It calls `createMotor` with the bytes of `DIR/graph_wasm.wasm`. Pass the wasm source the way
     `harness/sdk-smoke.mjs` does; read it and cite the line.
   - It lays out one fixture (`fixtures/force/clustered.json`, 60 nodes) with the default layout the
     SDK names. Read `sdk-smoke.mjs` for the call.
   - It asserts the result has 60 node positions, all finite, and exits 0. Exit 1 on any failure,
     with the error's name.

   Rows:
   - `embed-sdk|0|scripts/orch/node-slim.sh node harness/sdk-bundle-smoke.mjs target/embed-bundle`;
   - `negctl-embed-sdk`: a copy whose `graph_wasm.wasm` is truncated to 64 bytes must exit 1, and its
     output must carry the error name. Read the log, not only `$?`.

   If the built `graph-sdk.js` cannot load under Node (a browser-only global at module top level),
   STOP. Do not change `crates/graph-sdk-js`. Report the global and its file:line under "decisions
   needed". Tasks 1-3, 5 and 6 still land.

5. **Docs.**
   - Rewrite the comment at `app/vite.pack.config.ts:13-17` to the measured reason the pack does not
     use `build.lib`: flat, unhashed, auditable file names (`docs/contract/packaging.md:19-20,59-60`),
     and the element's exports kept. The blob-URL claim was measured false on vite 8.3.1 (review
     section 0). Say that `scripts/worker-form.sh` now watches the worker form in both bundles. The
     comment only; no config change.
   - In `docs/deploy/service.md`, after the `embed/<version>/` bullet, add one sentence: `graph-sdk.js`
     is the motor's JS SDK (`crates/graph-sdk-js`) for a host that runs layouts without the element;
     a host imports it from `/embed/<version>/graph-sdk.js`; its gate is `embed-sdk`. Do not change
     the bullet's file names: task 2 parses them.
6. **`docs/decisions/two-bundles.md`**: the decision, about 30 lines, in the style of a short file in
   `docs/decisions/` (read two first). It records:
   - why two bundles (the review's option (c), with a link to it);
   - what each bundle is for;
   - the four rows that keep them honest;
   - the auto-define difference: `embed-bundle.ts` defines the element on import, the pack does not,
     and `element.ts:147` keeps the first definition. Say that unifying them (option (a)) must first
     answer review condition 1, and quote that condition's two choices.

## Allowed paths

- New files: `scripts/worker-form.sh`, `scripts/embed-files.sh`, `harness/sdk-bundle-smoke.mjs`,
  `docs/decisions/two-bundles.md`.
- Edits:
  - `scripts/studio-pack.sh` (`:137-141` only);
  - `scripts/orch/rows/host-api.rows` (append only);
  - `app/vite.pack.config.ts` (the comment at `:13-17` only);
  - `docs/deploy/service.md` (one sentence).
- Also allowed: `prompts/jobs/bundle-worker-row.md` and `target/**`.
- Nothing else: not `crates/`, `packages/`, `src/`, `app/src`, `server/`, the root
  `package.json`/lockfile, `scripts/orch/gate.sh`. A fix that needs another path is a stop: report it.

Limits:
- Scripts: `set -euo pipefail`, functions at most 40 lines, a header that is the manual.
- Every container goes through the `scripts/orch/` wrappers; never a bare `docker run`, `node` or `npm`.

## Run, in order, and paste each exit code and its last 3 lines

1. `scripts/studio.sh wasm`
2. every new row's command, by hand, in the order of task 3
3. `scripts/studio-pack.sh target/pack`
4. `STUDIO_PACK_BREAK=1 scripts/studio-pack.sh target/pack-break` (expect 1)
5. `scripts/orch/drun-check.sh`

## Return block

- per task, the file:line changed;
- `scripts/worker-form.sh target/embed-bundle`'s output: the worker lines it found, and the count;
- `scripts/embed-files.sh target/embed-bundle`'s output;
- `sdk-bundle-smoke.mjs`'s output, and the cited `sdk-smoke.mjs` lines;
- each command's exit code;
- "decisions taken" / "decisions needed".

## Done when

Every one of these holds:
- `embed-bundle`, `embed-sdk` and the three negctl rows exit 0 by hand;
- `studio-pack.sh target/pack` exits 0;
- `STUDIO_PACK_BREAK=1` exits 1;
- `drun-check.sh` exits 0, or is red only on `scripts/orch/rows/p12-t4b.rows:49,58` (a known drift
  that branch fix-rows-drift fixes; say so in the return block);
- `git status --porcelain` shows only the allowed paths.
