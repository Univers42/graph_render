# Job review-host-api (agent reviewer, branch review-host-api from origin/host-api 64ec2408)

Review the branch adversarially against `origin/develop`. Read-only on code: the only file you write is
`docs/reviews/review-host-api.md`. Diff: `git diff origin/develop...HEAD`. Contract:
`docs/contract/host-api.md` — its "## Verdict" (PROCEED-WITH-CONDITIONS, conditions) and "As built"
(deviations).

Exact checks, each answered with file:line evidence:
1. Each verdict condition: met / not met, where. Each "As built" deviation: acceptable or breaks a
   condition, and why.
2. Trust boundary: `loadGraph(doc)` validation (type, 2^28-character size cap, ids); any
   `innerHTML`/HTML sink fed host data; event `detail` leaking internals; `composed` events crossing the
   shadow root; CSP (no `'unsafe-inline'` needed: adopted stylesheet).
3. Races and leaks: overlapping `loadGraph`, disconnect mid-load, superseded call → `CancelledError`;
   listeners, worker and timers released on disconnect.
4. `crates/graph-sdk-js/src/errors.ts`: all 15 classes set a literal `name`; nothing else changed.
5. Gates: every row in `scripts/studio-embed.sh` / `deploy/nav/embed.py` has a break that turns that row
   red for its own reason; list any row whose break passes for an unrelated reason.
6. House limits in changed files: ≤40 lines/function, ≤4 params, ≤300 lines/file
   (`wc -l`), no unexplained lint suppression, `Caveat:` on heuristics/timeouts.
7. Layering (CLAUDE.md "Architecture (studio)"): graph-studio never imports `src/`; graph-render has no
   React/SDK runtime/`fetch`; `app/` depends only on graph-studio.

Write `docs/reviews/review-host-api.md`: first line `verdict: LAND | LAND-WITH-FIXES | BLOCK`, then a
table `severity | file:line | defect | failure scenario | fix`. Only verified findings; UNKNOWN where
you could not tell. Do not build or run browsers.

Done when: the review file exists with a verdict line and every check 1–7 answered.
