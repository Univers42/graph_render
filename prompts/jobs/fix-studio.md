# Job fix-studio (agent build, review repairs: the renderer and the studio)

Read `prompts/jobs/fix-common.md` first (its Rust gates apply only to `motor-alone`; this job's
gates are the studio's). Review: `docs/reviews/review-studio.md`. Ids: ST-1 … ST-6 (MAJOR),
ST-7 … ST-13.

Judgement notes:
- The review's open questions are decided here, easiest to undo: ST-2, the impostor path keeps the
  frame's node kind (a `Box` stays a box; the sphere impostor is for `Point` and `Circle` only);
  ST-4, `view.fit` fits the filter's survivors, and `showAll` fits the whole graph; ST-6, the y
  flip follows `docs/contract/` (if it is silent on y orientation, "decisions needed").
- ST-5 (Inspector, NodeMenu and pipeline bypass the actions registry). Every user action goes
  through `src/actions/registry.ts` `resolve` (CLAUDE.md, Architecture (studio)).
- Layer rules: graph-render imports nothing at runtime; no import of `src/` (gate row
  `no-oracle-import`).
- A test per finding in the package's own runner (`node:test`, as the review's evidence did).

Paths: `packages/graph-render/**`, `packages/graph-studio/**`, `scripts/studio.sh` (ST-12 only).

Done when, in addition to fix-common's report: `scripts/studio.sh wasm`, `scripts/studio.sh check`,
`scripts/studio.sh build`, then `scripts/studio-smoke.sh` and `scripts/studio-nav.sh` exit 0 and
their `STUDIO_SMOKE_BREAK=1` / `STUDIO_NAV_BREAK=1` controls exit non-zero; one screenshot per
geometry kind fixed (ST-1 boxes and curves in 3D, ST-2 boxes under the impostor budget) saved under
`docs/measurements/fix-studio/`, with the console's error list empty. Paste each command and its
last lines.
