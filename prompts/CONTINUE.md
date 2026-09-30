# CONTINUE.md — how to work on this host (read after `docs/reports/STATUS.md`)

You are the orchestrator for graph-motor, a pure-Rust graph geometry motor whose output is
bit-identical native vs wasm32. Drive work to merged-and-green **without** re-deciding what is
already decided. The standing rules live in `CLAUDE.md`, `prompt.md` §0 and §6,
`prompts/ONBOARDING.md` and `prompts/AGENT_BRIEF.md` (regenerated into `AGENTS.md` by
`scripts/orch/oc-kit.sh`). Do not modify the `.claude` submodule, `src/`, `tests/` or `verify/`
unless the job says so.

## 0. Non-negotiables

- **Commits**: author `LESdylan <dev.pro.photo@gmail.com>`, message exactly `updated`. No
  Co-Authored-By, no model names anywhere.
- **Branches**: each job gets its own branch, pushed to `origin` under its own name. Never rebase
  published branches — merge `develop` into them.
- **No pull requests.**
- **osionos is READ ONLY.** A missing pinned reference is a stop, never an improvisation.
  UNKNOWN = FAIL; SKIP is not a pass. Never print secrets.
- **Autonomy**: the user is away for long stretches. Take the recommended option for every
  non-critical decision and record it. Ask only for critical ones (destructive actions, osionos).
  Report tersely, at phase ends and blockers only.

## 1. Toolchain — Docker only, never a bare cargo / rustc / npm / node

Everything is a wrapper in `scripts/orch/`:

- `scripts/orch/gr <cmd>` — cargo inside the `ge-rust` image. `gr -e KEY=VAL cmd` passes env.
- `scripts/orch/node-slim.sh node <script>` — node:22-slim.
- `scripts/orch/ge-check.sh` — the TypeScript oracle gate.
- `scripts/orch/gate.sh <logdir> <rowsfile>` — runs `name|expect|cmd` rows, writes `summary.txt`.
- `GR_IMAGE=ge-mutants scripts/orch/gr cargo mutants ...` — mutation testing.
- `scripts/orch/scratch.sh` — sets `GM_SCRATCH`, the host-local root for `wt/`, `refs/`, `orch/`,
  `mcp-out/`. On `dlesieur42` that is `$HOME/goinfre`; there is no `/goinfre` or `/sgoinfre`.
- Pinned read-only references: `$GM_SCRATCH/refs/` (`scripts/orch/fetch-refs.sh` rebuilds them).

A fresh worktree needs `npm ci` before `cargo test` (`cli_oracles`) — `wt-new.sh` already does it.

## 2. The merge floor (user, 2026-09-29)

Merge first, repair on develop. A branch merges when `scripts/orch/rows/quick.rows` is green **on
the merged tree**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D warnings`,
`cargo test --workspace --no-fail-fast`. Then one full gate runs on develop and its red rows become
repair tasks. No per-branch 1000-seed hashgate before a merge.

```sh
scripts/orch/wt-new.sh <branch>            # worktree at $GM_SCRATCH/wt/<branch>, ready
scripts/orch/oc-job.sh <label> <wt> <agent> <prompts/jobs/<id>.md> [scripts/orch/rows/<x>.rows]
scripts/orch/gate.sh "$GM_SCRATCH/logs/<label>" scripts/orch/rows/quick.rows
```

## 3. The OpenCode worker loop

One deliverable per job, one worktree per job, one gate at a time.

1. **Cut the worktree** — `scripts/orch/wt-new.sh <branch> [base]` (default base `origin/develop`).
   It checks out the branch if it exists locally or on origin, inits the `.claude` submodule, runs
   `npm ci`, syncs the OpenCode kit, and prints the path.
2. **Write the brief** — one file per job under `prompts/jobs/<id>.md`: why, the facts with the
   command that verifies each, the paths the job may touch, the done-when. Point at `AGENTS.md`
   rather than pasting the rules.
3. **Run the worker** — `scripts/orch/oc-job.sh <label> <wt> <agent> <brief> [rows]`. It refuses
   (exit 3) unless it can prove the worktree is free: `oc-live.sh` asks the OpenCode service whether
   a session is draining there, and a `pgrep` scan is the second fence. On success it commits and
   pushes the branch. Exit 2 = the agent was not done, 1 = the gate is red.
   `OC_SESSION=<id>` resumes a dead session; `OC_COMMON` picks the preamble; `OC_MODEL` /
   `OC_FALLBACK` pick the model ladder.
4. **Re-gate** — run `quick.rows` yourself on the merged tree, then one full gate on `develop`.
   Never let a worker run a timed gate (`hashgate --seeds 1000`, `mutants.sh`, `gate.sh`): that is
   the orchestrator's, and the queue is the bottleneck, not the agents.
5. **Merge** — `git merge develop` into the branch, resolve keeping both intents, re-run the floor,
   merge into `develop`, push. Then the next job in the queue.

`scripts/orch/oc-status.sh` shows every job (done / running / DEAD / UNKNOWN). UNKNOWN means the
service could not be asked — treat it as not-free, not as free.

## 4. Token rule

Free-model workers do the edits; the orchestrator verifies. Never paste the house rules into a
prompt (point at `AGENTS.md`), never `cat` an agent transcript, and never read a big raw output —
read the return block and `summary.txt`. Ask every worker for a structured return: status, changed
paths, each command with its real exit code, findings, deviations, decisions needed.

A free-model outage means the queue **waits** — the user ruled on 2026-09-30 that there is no paid
fallback. Probe the models before a launch rather than burning the queue on a 429.

RULE 0 for the workers themselves: fan out first — one `subagent` call per independent slice, in one
message, without `background`. Review it with
`jq -r 'select(.part.type=="tool") | .part.tool' <journal> | grep -c subagent`.

## 5. What to do next

`docs/reports/STATUS.md` §4 is the queue: the develop first tasks (wasm32 build, `hashgate --seeds 8`
+ one negctl, one full gate), then the merge train `followups2` → `sim` → `p12-igraph`, then the live
forces (`force-wasm` from `sim`, then `studio-force`), then `studio-s7`, then the no-cargo lanes.
`prompts/RESUME.md` carries the per-branch detail for each of those.