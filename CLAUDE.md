# Standing rules — graph_render / graph-motor (set by the user, LESdylan)

## Talking to the user
- Be terse. No narration between tool calls, no recaps.
- Write to the user only when a phase finishes, when blocked, or when a decision is needed.
- Keep each message to a few lines.
- Phase reports in `docs/reports/` still follow the rules in full (§12 shape).

## Git
- Author every commit as `LESdylan <dev.pro.photo@gmail.com>`. No Co-Authored-By, no "Generated with" trailer.
- Commit message is exactly `updated`.
- Push directly to `develop`. No pull request.
- Commit and push after every green step or phase, so nothing depends on the container surviving.
- No model identifiers in commits, PRs, code or docs.

## Working mode
- Full autonomy: run phases 0 → 10 (`prompts/phase-NN-*.md`) per `prompts/ONBOARDING.md` and `prompt.md`.
- Keep an hourly self check-in armed with `send_later`.
- Follow the `.claude` house rules (rules repo `univers42/claude-deal-with-the-devil`).

## Hard constraints
- osionos (`/home/dlesieur/Documents/osionos`) is READ ONLY.
- Docker-only toolchain; no prebuilt vendor language images (`FROM rust:*`, playwright, ...).
- Never claim an unrun result: UNKNOWN = FAIL, SKIP is not a pass.
- A missing reference is a stop, not an improvisation.
- Stay inside each phase's authorization envelope; report every deviation.
- Never print secret values.
- Do not modify the `graph_render/.claude` submodule.

## Parallel branches (checked on 2026-09-29 with `git merge-tree` and a disk check; worth doing)
- Every independent unit of work (a phase, or a slice inside a phase) gets its own branch and its own worktree under `/goinfre/dlesieur/wt/<branch>`. Exactly one agent per worktree. Two agents in one worktree clobbered each other's edits on 2026-09-28 (p3fix-a and p3fix-c).
- Start a branch as soon as its dependencies allow it; don't wait for unrelated phases.
  - Dependency order: p3 → {p5, p6e, p6f, p8}; p4 → {p7's SDK row, p10}; p6f → p9 → p11. p4 and p7 are independent of p3.
  - A branch may start from an unmerged dependency branch. Once that dependency lands, merge develop into it.
- Slices inside a phase use sub-branches `pN-<slice>`, which merge into `pN` and never straight into develop.
- Merge into develop one branch at a time, in plan order, and only once the branch is resolved (gate green + review + mutants + report). Before the gate, merge the current develop into the branch (never rebase) and gate the merged tree.
- Some files are touched by several branches: `lib.rs`, `registry.rs`, `capabilities.rs`, `main.rs`, `Cargo.lock` and `canonical_json/schema.rs`.
  - Edit them additively only.
  - Resolve conflicts by keeping both intents. The measured conflicts are small (p4+p7: 3 hunks; p5+p6e: 1 hunk).
  - Conflicts in contract or registry files go to an independent judgement.
- CPU is the limit, not git.
  - Run at most one timed gate at a time (hashgate, mutants) — on 2026-09-28, `hashgate --seeds 1000` hit `CHILD_TIMEOUT` 2700 s while mutants ran alongside it.
  - Other cargo jobs pass `CARGO_BUILD_JOBS`/`RUST_TEST_THREADS` (`scripts/orch/gr`).
  - Ponytail: a thread cap is not a CPU cap. A gate that times out is re-run on its own and never counted as a pass.
- Disk: each worktree's `target/` is about 1 GB, and `/goinfre` had 38 GB free. Remove a worktree after its branch merges.

## Reference
- The TypeScript oracle engine (commands, architecture): `docs/oracle-engine.md`. Agent brief: `prompts/AGENT_BRIEF.md`.
