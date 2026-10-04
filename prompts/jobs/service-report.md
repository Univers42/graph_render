# Job service-report: the final report of the service DoD

Worktree `~/goinfre/wt/service-report`, branch `service-report`, from develop. This is a **docs-only**
job. Write `docs/reports/service-dod.md` and change nothing else under version control. Gate row
`no-code` (`scripts/orch/rows/docs.rows`) fails on any change under a code path.

The DoD is the six steps below. The user set them on 2026-10-03. Step 6 is "the full gate green on
develop, and the final report", and this file is that report. The full gate (`develop-full.rows`) is
still running. Its result is filled in by the orchestrator, not by you. Dispatch at most 3 `explore`
subagents, each on a disjoint set of steps.

## The DoD (verbatim, from the orchestrator's handoff)

1. **Columnar ingest on develop.** The open-path stack is perf-open-columns, then perf-open-intern,
   then perf-open-synth-columns. Measure the time to open 1M nodes before and after; P5 measured 12.45 s.
2. **A host contract, versioned and typed, in `docs/contract/host-api.md`, with a devil verdict.** It
   covers:
   - `load(columns)` and `applyDeltas(batch)`;
   - DOM `CustomEvent`s carrying ids: `node-select`, `node-open`, `node-hover`;
   - `focus(id)`;
   - an optional `resolve(id) => Promise<preview>`, backed by a bounded LRU with a `Caveat:` line.
3. **P4 live growth end to end.** This means perf-p4a-extend, `gm_force_session_apply`, SDK
   `applyDeltas` coalesced per animation frame, and the studio host verb. Target: at most 30 ms per
   10k batch at 1M.
4. **Packaging.** An ESM bundle plus both wasm artifacts, versioned. COOP/COEP documented. The serial
   fallback stated with its number: about 700 ms/tick at 1M, against 158–170 ms with 8 threads.
5. **An embed example** in `app/`. It loads columns, listens to `node-open`, resolves previews
   through `resolve`, and streams deltas from a JSONL replay. One browser gate.
6. **The micro-service:**
   - `server/graph-server` (axum), with API keys;
   - a Debian-based image;
   - the SDK's remote mode;
   - the supply-chain audit;
   - the full gate.

## Exact tasks

1. **Per step 1–5**, find on develop, and cite each as `git log` output or `file:line`:
   - (a) the commit that landed it. Use `git log --oneline --first-parent origin/develop`, and match
     the branch name in the merge or in `git log --format=%s%n%b`. If none is found, say "landing
     commit not found" rather than guess.
   - (b) the gate rows that check it, with their negative control: rows file plus row name. Use
     `git grep -n <row> -- scripts/orch/rows`.
   - (c) the measured number against its target, read from the file under `docs/measurements/`
     that holds it. Candidates: `perf-open-*.md`, `open-*.md`, `perf-p4*.md`, `studio-pack.md`,
     `service-caps.md`. Quote the number and its line. When the file states no number, write
     "not measured", and the step is then **not met**.
   - (d) a verdict: met / not met / partially met, with the reason in one line.
2. **For step 6**, the same (a)–(d) for each of the following:
   - `server/graph-server`;
   - API keys: grep `server/graph-server/src` for the key check, and cite the test that refuses a
     missing key and the one that refuses a wrong key;
   - the image: `docker/` or `deploy/`, plus `scripts/orch/rows/service-image.rows`;
   - SDK remote mode: `crates/graph-sdk-js/src`, the remote client;
   - `docs/contract/service-api.md`;
   - the reviews `docs/reviews/review-svc-r2.md` and `review-svc-r3.md`. For each, give its verdict
     line, and for each numbered condition, the row or test that meets it;
   - supply-chain: `scripts/orch/rows/audit.rows`, `deny.toml`.
3. **Two checks the orchestrator already made. Re-verify each by reading the cited lines, and write
   it as one row of a table "Resolved questions":**
   - `GRAPH_WORKERS`:
     - `docs/contract/service-api.md:115` says an explicit value overrides the memory budget.
     - `:328-336` says that, when it is unset and no slot is derived, the server exits 2.
     - `server/graph-server/src/config.rs:200-214` `read_workers` refuses only when
       `fallback == 0` and the variable is unset.
     - Spec and code agree. Say whether they do, from the lines.
   - `C6`: handle ids are never reissued. See `docs/contract/wasm-abi.md:51,361` and
     `docs/decisions/force-wasm-abi.md:121`. Find the test or harness line that checks a released id
     reads `InvalidHandle` (`harness/sdk-smoke.mjs`, per `wasm-abi.md:569-574`), and cite it.
4. **The reviews' open items.** List every finding in `docs/reviews/review-host-api.md`,
   `review-svc-r3.md` and `review-bundle-unify.md` that is still open on develop. "Open" means its row
   is not marked closed or fixed. For each, give the id, the severity and one line. Do not fix
   anything.
5. **Write `docs/reports/service-dod.md`** in this shape. Read `docs/reports/phase-g0.md` for the
   house style: short sentences, tables, evidence on every line.
   - `# Service DoD — final report`;
   - a line naming develop's tip at the time of writing (`git rev-parse --short origin/develop`);
   - `## 1. Steps` — a table: step, verdict, landing commit, rows (+ negctl), measured vs target;
   - `## 2. The micro-service` — the task 2 table;
   - `## 3. Resolved questions` — task 3;
   - `## 4. Open review items` — task 4;
   - `## 5. The full gate` — exactly this one line: `Filled in by the orchestrator when develop-full.rows finishes.`;
   - `## 6. What is not done` — every step or item whose verdict is not "met", with why. The
     merge to main needs the user's go-ahead: say so here.
   - No adjectives without a number, no "successfully", and no model identifiers.

## Allowed paths

`docs/reports/service-dod.md` (new), `prompts/jobs/service-report.md`, `target/**`. Nothing else.

## Run, and paste the exit code

`scripts/orch/gate.sh target/wf/docs scripts/orch/rows/docs.rows` (expect 0).

## Return block

- the step table (task 5 §1) as written;
- the count of open review items by severity;
- "decisions taken" / "decisions needed".

## Done when

- `docs/reports/service-dod.md` exists with all six sections;
- every verdict cites a commit, row or `file:line`;
- `docs.rows` passes;
- `git status --porcelain` shows only the allowed paths.
