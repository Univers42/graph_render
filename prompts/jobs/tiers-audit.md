# Job tiers-audit (agent build, docs only)

Why: user request, CPU only, every layout multi-threaded (`prompts/RESUME.md`, "Queued" and "Update
2026-09-29 late": "Every layout gets the Phase 11 thread tier and a bench-driven optimization pass.
Order: easiest first."). p11 is on develop (merge c84c869): `exec/` runners (`Serial`, threaded),
hashgate Threads/Tiers arms and knobs, `bench --tiers --workers`. Which layouts already use a
threaded runner, and what each of the others needs, is not written down anywhere.

Facts:
- Determinism rules: CLAUDE.md "Determinism" and `prompt.md` §6 (D1–D10). A threaded kernel must be a
  gather (element i reads start-of-step state, writes only out[i]); reductions run in a fixed order.
- p11's design and state: `prompts/phase-11-compute-tiers.md`, `docs/reports/phase-11-progress.md`,
  `crates/graph-core/src/exec/`, `crates/graph-cli/src/hashgate/` (Threads/Tiers arms), `crates/graph-cli/src/bench/`.
- The layouts: `LAYOUTS` in `crates/graph-core/src/registry.rs` and `registry/*.rs`.

Do:
1. `docs/measurements/tiers-audit.md`: one row per registered layout: id, the kernel file(s) with
   `file:line`, whether it already runs through an `exec` runner, the parallel shape (per-node gather,
   per-edge scatter, tree walk, sequential by nature: say which and why), the reduction that needs a
   fixed order, the expected speed-up class (none / ≤2× / near-linear) with the reason, and the
   effort (S/M/L). Sort by effort, easiest first.
2. Draft one brief per group of S-effort layouts as `prompts/jobs/tier-<group>.md`: the runner to
   use, the hashgate Threads arm that must stay 4-way equal, the knob that must turn it red, and the
   bench command (`bench --layout X --n 220,10000 --tiers --workers 1,4,8`; check the flags with
   `scripts/orch/gr cargo run -q -p graph-cli -- bench --help`).

You may run `--help` commands and read code. Do not run benches or hashgate.

Paths you may touch: `docs/measurements/tiers-audit.md`, `prompts/jobs/tier-*.md`. Nothing else.

Done when: every registry layout id has one row with `file:line` evidence; the S-group briefs exist.
