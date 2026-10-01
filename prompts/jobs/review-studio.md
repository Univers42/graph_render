# Job review-studio (agent build, review only, docs)

Why: the user asked that every graph be drawn in its correct shape (2026-10-01). The motor emits the
geometry; the renderer must draw each kind as the contract says, or a correct layout looks wrong.

Scope: `packages/graph-render/src/**` and `packages/graph-studio/src/**`.

Check: each geometry kind of `docs/contract/` (nodes Point/Circle/Box, edges Line/Polyline/Curve, the
3D columns if present) is decoded and drawn as specified: Box size and anchor, Curve control points,
Polyline vertex order, arrow side, the y axis direction; the camera fits the frame it shows
(known defect: `prompts/jobs/studio-switch-fit.md`; do not repeat it, find others); decode of a short or
corrupt snapshot fails loudly; every action goes through `src/actions/registry.ts`; the layer rules in
`CLAUDE.md` "Architecture (studio)" hold (no `src/` import, no React in graph-render).
Use the `pw` MCP on the dev server (127.0.0.1:5174) to confirm a drawing defect with a screenshot to
`/out/review-studio-*.png`; one fixture per geometry kind (`app/public/fixtures/`).

Rules for every finding: `file:line`, severity BLOCKER/MAJOR/MINOR, the concrete failing input, and evidence: a command with its output (a scratch test you ran and did not commit counts), or the reference `file:line` the code disagrees with. A finding without evidence is listed under "unverified", never as a finding. No style nits without a house rule (`CLAUDE.md` "House limits", `prompt.md` §6 D1-D10). Fan out first: one `explore` subagent per module in ONE message; you merge, deduplicate and verify.

Output: `docs/reviews/review-studio.md`, same table shape as `prompts/jobs/review-core-post.md`.

Paths: `docs/reviews/review-studio.md` only. No code change.

Done when: every geometry kind is named with its verdict; every BLOCKER/MAJOR row has evidence.
