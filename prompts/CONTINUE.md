# CONTINUE.md — how to work on this host (read after `docs/reports/STATUS.md`)

Written 2026-10-02. You are the orchestrator for graph-motor, a pure-Rust graph geometry motor
whose output is bit-identical native vs wasm32. Drive work to merged-and-green **without**
re-deciding what is already decided. The standing rules live in `CLAUDE.md`, `prompt.md` §0 and
§6, `prompts/ONBOARDING.md` and `prompts/AGENT_BRIEF.md` (`AGENTS.md` links to it). Do not modify
`.claude/`, `src/`, `tests/` or `verify/` unless the job says so.

## 0. Non-negotiables

- **Commits**: author `LESdylan <dev.pro.photo@gmail.com>`, message exactly `updated`. No
  Co-Authored-By, no model names anywhere.
- **Branches**: each job gets its own branch, pushed to `origin` under its own name. Never rebase
  published branches — merge `develop` into them. **No pull requests.**
- **osionos is READ ONLY.** A missing pinned reference is a stop, never an improvisation.
  UNKNOWN = FAIL; SKIP is not a pass. Never print secrets.
- **Autonomy**: the user is away for long stretches. Take the recommended option for every
  non-critical decision and record it. Report tersely, at phase ends and blockers only.

## 1. The merge floor (user, 2026-09-29, unchanged)

**Merge first, repair on develop.** A branch merges when `scripts/orch/rows/quick.rows` is green
**on the merged tree**. No per-branch 1000-seed hashgate, no per-branch mutants gate before a
merge — that gating became perfectionism and blocked the train for hours behind one host-wide
lock.

`scripts/orch/rows/quick.rows` is **11 rows**, and **4 of them are negative controls that must go
red** — a floor run where they pass is a broken floor:

| # | row | what it proves |
|---|---|---|
| 1 | `fmt` | `gr cargo fmt --all --check` |
| 2 | `clippy` | `gr cargo clippy --workspace --all-targets -- -D warnings` |
| 3 | `test` | `gr cargo test --workspace --no-fail-fast` |
| 4 | `wasm32-core` | `gr cargo build -p graph-core --target wasm32-unknown-unknown` |
| 5 | `hashgate-8` | `gr … hashgate --seeds 8` — the 4-way hash over every stage |
| 6 | `negctl-degree` | `GM_MUTATE_REFERENCE_DEGREE=9` must exit **1** |
| 7 | `negctl-dim-z-mismatch` | `GM_MUTATE_NODE_Z=1 roundtrip --seeds 8` must exit **non-zero** |
| 8 | `force-gate-4` | `gr … force-gate --seeds 4` |
| 9 | `negctl-force-gravity` | `GM_MUTATE_FORCE_SESSION_GRAVITY=0.5` must exit **1** |
| 10 | `scigraphs-conformance` | `scripts/scigraphs-conformance.sh` |
| 11 | `negctl-scigraphs-conformance` | the same with `--break`, must exit **1** |

The land step gates on exactly this file (`scripts/orch/queue.sh:37`).

```sh
scripts/orch/wt-new.sh <branch>                       # worktree at $GM_SCRATCH/wt/<branch>
scripts/orch/gate.sh <logdir> scripts/orch/rows/quick.rows   # <logdir>/summary.txt, exit 1 if red
```

A fresh worktree needs `npm ci` before `cargo test` (`cli_oracles`) — `wt-new.sh` already does it.

## 2. The orchestration, as it stands

Four scripts do everything. `scripts/orch/scratch.sh` sets `GM_SCRATCH` (on `dlesieur42` that is
`$HOME/goinfre`; there is no `/goinfre` or `/sgoinfre`).

### 2.1 `queue.sh` — the ledger

`scripts/orch/queue.sh run|status|land <label>` (`queue.sh:109`).

- **The queue is `scripts/orch/queue.txt`**, one row per line:
  `label|agent|brief|rows|land`, where `brief` and `rows` are paths relative to the repo top and
  `land` is `yes|no` (`queue.txt:3-4`). Example:
  `p12-t2|build|prompts/jobs/p12-t2.md|scripts/orch/rows/quick.rows|no`.
- **State lives in `$GM_SCRATCH/orch/queue/<label>.{pid,rc,land}`** plus a `max` file
  (`queue.sh:12,23,63,65,67`). There is no append-only journal; these three files per label are
  the whole ledger. Job logs are `$GM_SCRATCH/orch/logs/job-<label>.out`. The agent transcript is
  `$GM_SCRATCH/wt/<label>/target/wf/<label>.jsonl`.
- **`queue.sh run`** is the launcher: it re-reads `queue.txt` every 60 s and keeps N jobs live, N
  from `$GM_SCRATCH/orch/queue/max` (default 3), with a 20 s stagger (`queue.sh:88-102`).
- **`queue.sh status`** prints `label`, then one of `done rc=<n> land=<n>`, `live pid=<n>
  journal-age=<n>s`, or `pending` (`queue.sh:72-86`). Read it as: `rc` is `oc-job.sh`'s exit code
  — **0** done and gate green, **2** the agent did not return `done`, **1** the gate is red, **3**
  the worktree could not be proven free (`oc-job.sh:4-5`). `land` is written only when rc=0 and
  the row says `land=yes` (`queue.sh:61-64`): **0** merged, **1** red after the merge, **2** could
  not run. `land=-` means the file is absent.
- **Requeue a job**: delete its `.pid` and `.rc` (`queue.sh:13`). A `.pid` with no `.rc` means the
  runner was killed mid-job and the label will read `live` forever.

### 2.2 `oc-job.sh` — one worker

`scripts/orch/oc-job.sh <label> <worktree> <agent> <body-file> [rows-file]` (`oc-job.sh:2`).
The body file is the queue row's `brief`, i.e. `prompts/jobs/<label>.md` (66 briefs today). It:

1. refuses (exit **3**) unless the worktree is free — `oc-live.sh` asks the OpenCode service
   whether a session is draining there and a `pgrep` scan is the second fence (`oc-job.sh:17-26`);
2. writes `<worktree>/target/wf/<label>.prompt` = `common.md` + the body, then runs `oc-run.sh`
   (hard timeout `OC_TIMEOUT`=14400 s) with a provider-quota resume loop over `OC_FALLBACK`
   (`oc-job.sh:27-54`), resuming up to `OC_RESUMES`=3 times when the agent wrote no return block;
3. exits **2** unless the return block's `status:` is `done` (`oc-job.sh:69`);
4. runs `gate.sh target/gate-<label> <rows>` and exits **1** if red (`oc-job.sh:70-73`);
5. `git add -A`, commits and pushes its own branch (`oc-job.sh:74-76`).

`OC_SESSION=<id>` resumes a dead session, `OC_COMMON` picks the preamble, `OC_MODEL`/`OC_FALLBACK`
pick the model ladder. **Never let a worker run a timed gate** (`hashgate --seeds 1000`,
`mutants.sh`, `gate.sh`) — that is the orchestrator's, and `scripts/orch/timed` holds the
host-wide `flock` that makes it the bottleneck.

### 2.3 `land` — the landing step

There is no `land.sh`: it is `queue.sh land <label>` → `land()` at `queue.sh:28-42`, called
automatically when rc=0 and `land=yes`. Under `flock` on `$GM_SCRATCH/orch/queue/land.lock` it
`cd`s to `$GM_SCRATCH/wt/<label>`, fetches, **merges develop into the branch** (aborting with
exit 1 on conflict), runs `timed gate.sh target/land-<label> scripts/orch/rows/quick.rows`, then
`git push origin HEAD HEAD:develop`. `catch_up()` (`queue.sh:47-53`) permits one retry, and only
when the develop move touched nothing outside `docs/`, `prompts/`, `scripts/orch/` and `*.md`.

So **setting `land=yes` in `queue.txt` is how you authorise a merge** (`queue.sh:61`). But the
column is **not the authority** and `queue.txt` alone will mislead you: `p13-gv2-sfdp`,
`trap-followups`, `p12-t3-knobs`, `studio-3d`, `graphviz-verdict` and `wasm-gm-build-trap` all
carry `land=0` in the state dir and their work **is** on develop, yet every one of their committed
rows reads `land=no`. Read the tree, not the column. Nothing is at risk of being re-run —
`run()` skips any label that still has a `.pid` (`queue.sh:94`) — the risk is only misreading.

### 2.4 The rest of `scripts/orch/`

`gr [-e K=V] <cmd>` (cargo in the `ge-rust` image; `GR_IMAGE=ge-mutants` for mutants),
`node-slim.sh` (node:22-slim), `ge-check.sh` (the TypeScript oracle gate), `gate.sh`, `mutants.sh
<base-rev>`, `oc-run.sh`, `oc-live.sh`, `oc-status.sh [root]` (one line per journal: worktree,
label, done(rc)/RUNNING/STALLED/DEAD/UNKNOWN, age, session, last 200 chars), `job-check.sh
start|wait|status`, `wt-new.sh`, `timed`, `scratch.sh`, `profile.sh`, `fetch-refs.sh`.

### 2.5 The worker's own rules

RULE 0: fan out first — one `subagent` call per independent slice, in one message, **without**
`background`, and report `subagents: <n> explore, <n> general`. Review it with
`jq -r 'select(.part.type=="tool") | .part.tool' <journal> | grep -c subagent`. Ask every worker
for a structured return block: status, changed paths, each command with its real exit code,
findings, deviations, decisions needed. A free-model outage means the queue **waits** (user,
2026-09-30): no paid fallback, so probe before a launch rather than burning the queue on a 429.

## 3. What is in flight (2026-10-02)

`scripts/orch/queue.sh status`: 47 rows — 43 `done`, 2 `live` (`studio-switch-fit`,
`status-refresh`), 2 `pending` (`osage-knob`, `sg-conformance-split`). develop = **701b46a**,
436 commits.

Twelve remote branches are unmerged. The ones that carry work, and what to do with each, are in
`docs/reports/STATUS.md` §2 — three are worth landing (`p12-t4a`, `p13-gv2-dot`, `perf-p2-pm`),
three are superseded and can be deleted (`p12-t2`, `tier-settle`, `studio-force`), one is dropped
(`studio-ux`), three are docs (`review-*`), one is a pure-move (`sg-dedupe`), and
**`p13-gv2-dot-rank` is the same commit as `p13-gv2-dot`** (`f098188`, empty diff) — one branch
wears two names because two queue rows pointed at it.

## 4. The next work, in order

1. **Run the full gate on develop 701b46a and repair its red rows.**
   `scripts/orch/gate.sh <logdir> scripts/orch/rows/develop-full.rows` — 88 rows. This has not
   been run on this tree; every red row is unknown until it is, and UNKNOWN = FAIL.
2. **Land `p12-t4a`** (`origin/p12-t4a`, 971318d, 40 files): the 3D arms of random, spiral,
   bipartite, spectral and mds.pivot. Merge develop into the branch, run the floor, then set
   `land=yes` on `queue.txt:32` or `queue.sh land p12-t4a` by hand.
3. **Land `p13-gv2-dot`** (f098188, 7 files): the `dot` layered port. The brief orders the passes
   rank → mincross → position (`prompts/jobs/p13-gv2-dot.md`); the branch already carries the
   split. Land the branch first, then run `p13-gv2-dot-rank`'s brief as a follow-up job against
   it — and delete the duplicate queue row.
4. **Launch `p12-t4b`** (queued, no branch): the 3D arms of forceatlas2 / yifan_hu / FR / KK /
   DRL plus `layout.force.yifan_hu.2z`. `wt-new.sh p12-t4b` first.
5. **Let the two pending jobs run**: `osage-knob` (unblocks `layout.packing.osage`'s promotion to
   `gated`) and `sg-conformance-split` (three conformance files over 300 lines).
6. **Land the three `review-*` docs branches and `sg-dedupe`** — they are one commit each and merge
   without a floor fight.
7. **`perf-p2-pm`** (2 commits): `layout.force.particle_mesh`, the P2 replacement the plan asks
   for. It is a new capability id and needs its own quality gate, so it is a bigger job than 2–3.
8. **Small repairs found while writing this file**, each already cited in
   `docs/reports/STATUS.md` §5: add `negctl-node-z` to `develop-full.rows` (the full sweep
   currently skips the z control that `quick.rows:7` runs), fix the stale
   "no 3D layout is registered" comment in `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:5-6`,
   and add a studio render row to the gate — `develop-full.rows` runs none (`:198 env-ignored` is
   the only studio-related row; the `scripts/studio-*.sh` gates are run by hand).
9. **Do not** run the 1000-seed hashgate, `mutants.sh` or `gate.sh` concurrently with anything
   else; `scripts/orch/timed` serialises them and that queue is the bottleneck.

## 5. If you are starting cold

```sh
scripts/orch/scratch.sh                      # GM_SCRATCH
git fetch origin && git log --oneline -1 origin/develop
scripts/orch/queue.sh status                 # the ledger
git for-each-ref --no-merged origin/develop --format='%(refname:short)' refs/remotes/origin
scripts/orch/gr cargo run -q -p graph-cli -- capabilities --json   # 69 rows
```

The last one prints `not backed: no <record>: run the gate` on every row in a fresh worktree —
that is the missing `target/gates/`, not a red develop. Read
`docs/reports/STATUS.md` §3.3 before drawing any conclusion from it.