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
