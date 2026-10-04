# Job review-bundle-unify: devil verdict on one bundle of `<graph-studio>` instead of two

Worktree `~/goinfre/wt/review-bundle-unify`, branch `review-bundle-unify`, from develop. This is a
**review**: write one file, `docs/reviews/review-bundle-unify.md`, and change nothing else under
version control. Dispatch at most 2 `explore` subagents.

## The question

develop builds the same element twice, with two configs that disagree:

| | service bundle | pack |
|---|---|---|
| config | `app/vite.embed.config.ts` | `app/vite.pack.config.ts` (PACK_MODE=pack) |
| entry | `app/src/embed-bundle.ts`: calls `defineGraphStudio()` on import, re-exports `element.ts` | `packages/graph-studio/src/element.ts` directly |
| mode | `build.lib`, entries `graph-studio` and `graph-sdk` (`crates/graph-sdk-js/src/index.ts`) | `rollupOptions.input`, `preserveEntrySignatures: "exports-only"` |
| output | `graph-studio.js`, `graph-sdk.js`, chunks under `assets/` (`docs/deploy/service.md:26`) | six flat files: `graph-studio.js`, `worker.js`, `helper.js`, two wasm, `pack.json` (`docs/contract/packaging.md:16-25`) |
| minified | yes (vite default) | no (`packaging.md:27`) |
| built by | `scripts/studio.sh embed DIR` (`scripts/studio.sh:174-184`), called by `scripts/service.sh` | `scripts/studio-pack.sh` |
| served at | `/embed/<version>/` by graph-server, the public service surface | a host serves the pack itself |
| gated by | `scripts/orch/rows/service-image.rows` (find the rows that load `/embed/`) | `studio-pack`, `studio-pack-embed` (`STUDIO_EMBED_PACK=... scripts/studio-embed.sh`) |

`vite.pack.config.ts:13-17` says why it avoids `build.lib`: "A library build is free to fold that
worker into the entry as a blob URL, and `worker-src 'self'` — the CSP a host is asked for
(docs/contract/packaging.md) — refuses a blob: worker." The service bundle **is** a `build.lib`
build. Whether its motor worker is a real file or a `blob:`/`data:` URL is the first fact to measure.

## Exact tasks

1. **Measure the service bundle's worker.** Run `scripts/studio.sh wasm` then
   `scripts/studio.sh embed target/embed-probe`. List the files (`ls -la target/embed-probe`
   `target/embed-probe/assets`) and grep the built JS for how the worker is created:
   `grep -o 'new Worker([^)]*)' -r target/embed-probe | head`, and
   `grep -c 'blob:\|data:application/javascript\|data:text/javascript' -r target/embed-probe`.
   Paste both outputs. Then state, from those lines only, whether a host page under
   `worker-src 'self'` (the CSP in `docs/contract/packaging.md` "Serving it" and
   `deploy/nav/serviceproxy.py:23`) can start that worker. If a row already loads the service's
   `/embed/` bundle under that CSP and draws a node, cite it (`scripts/orch/rows/*.rows` plus the
   script it calls): that row is the measurement, and say whether it ran the threads worker too.
2. **List every consumer of each surface**, with file:line: the path names a host is told to
   import (`docs/deploy/service.md`, `docs/contract/host-api.md`, `docs/contract/packaging.md`),
   `embed/VERSION`, the content-hash version (`scripts/service.sh`), the pack's `pack.json` fields,
   and the gate rows that read the files by name. A unification must keep every one of them or
   name the break.
3. **The options**, each with its diff size (files, rough lines) and what breaks:
   - (a) the service stages the pack (`scripts/studio-pack.sh` output) under `/embed/<version>/`,
     and `vite.embed.config.ts` + `embed-bundle.ts` are deleted; `graph-sdk.js` is built by
     `vite.pack.config.ts` as a second entry, or dropped if nothing consumes it (task 2 decides);
   - (b) `vite.embed.config.ts` imports the pack's build function and only adds the SDK entry;
   - (c) keep two bundles, and add a row that fails when they disagree on the worker form;
   - any other option you find, with the same accounting.
   For the auto-define difference (`embed-bundle.ts` calls `defineGraphStudio()`, the pack does not),
   say which behaviour each documented host snippet relies on (`docs/deploy/service.md:98`,
   `packaging.md`, `host-api.md`) and how the chosen option keeps both working.
4. **The verdict**, in the format of `docs/reviews/review-svc-r3.md`: a first line `verdict:
   PROCEED | PROCEED-WITH-CONDITIONS | BLOCK`, the four axis scores 1-5 (blast radius,
   reversibility, cost on failure, confidence) with the worst named, the recommended option, and
   numbered conditions. Each condition is checkable by a row with a negative control: name the
   rows file, the row, and the break that turns it red. Treat this as a public-surface change
   (`.claude/rules/devil/risk.md`): `/embed/<version>/` is what a host imports.
5. If task 1 shows the service bundle's worker is a `blob:`/`data:` URL, put it first in the
   review, as a finding with severity, file:line and the failure scenario, whatever the verdict.

## Allowed paths

`docs/reviews/review-bundle-unify.md` (new), `prompts/jobs/review-bundle-unify.md`, `target/**`.
Nothing else: the gate row `no-code` (`scripts/orch/rows/docs.rows`) fails on any change under
the code paths it lists.

## Return block

- task 1's two pasted outputs and the CSP conclusion;
- the verdict line, the four scores, the recommended option and the condition count;
- "decisions taken" / "decisions needed".

## Done when

`docs/reviews/review-bundle-unify.md` exists with a verdict line, every claim in it carries a
command with its output or a file:line, and `git status --porcelain` shows only that file and
`target/`.
