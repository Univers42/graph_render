# Job merge-sim (agent build, merge and finish)

Why: `sim` (force-session M1, the live simulation behind the Obsidian-style drag) is the second branch
of the merge train (`prompts/RESUME.md` "Branches pushed but NOT on develop"). Its head is 5ae4210.
The live-forces chain (force-wasm, then the studio wiring) waits for it.

First, in this worktree (a branch made from develop): `git merge --no-commit origin/sim`. Then resolve
every conflict by editing files. After the merge, never run git commands that change state; never delete
a file except `scratch/fmtprobe_p.rs` (list it under `delete:` if you cannot).

Facts:
- `sim` adds `crates/graph-core/src/layout/force/session.rs`, `session/`, `barnes_hut/sim/` and
  `docs/decisions/live-force-session.md` (read it first: it states the session's contract).
- Expected conflicts: `barnes_hut.rs` and `sim.rs`. develop has p11's compute tiers: the `How` tick
  (the runner choice, `crates/graph-core/src/exec/`) must be kept; the session must drive the same tick,
  so a session step and a batch step stay bit-identical.
- The 26-knob hashgate from `followups2` is on develop; the knob list exists once
  (`crates/graph-cli/tests/common/mod.rs`, `Knob::ALL`). Do not add a knob unless a test needs it.
- Determinism: CLAUDE.md "Determinism" (libm, fixed-order reductions, no HashMap, no clock).
- House limits: ≤ 40 lines/function, ≤ 4 params, ≤ 300 lines/file (split into child modules).

Finish (RESUME, "To finish" column for `sim`):
1. Frozen acceptance: a session with every node frozen (or alpha at 0) must not move any node; a test.
2. Setters: each session setter (repulsion, link distance, gravity, centre, pin, drag) has a test that
   it changes the next tick and nothing before it.
3. Golden check: the batch layouts' digests must not change because of the merge. BEFORE the
   `git merge`, run `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` and copy the
   report it writes under `target/gates/` to `target/golden-before.json`. After the merge, run it again
   and compare the per-layout, per-stage hashes: every layout id present in both must be equal. Paste
   the comparison command and its output.
4. Checks: `scripts/orch/gr cargo fmt --all --check`, `... clippy --workspace --all-targets -- -D warnings`,
   `... cargo test --workspace --no-fail-fast`, `... cargo build -p graph-core --target wasm32-unknown-unknown`,
   `... cargo run -q -p graph-cli -- hashgate --seeds 8` and its negctl
   (`-e GM_MUTATE_REFERENCE_DEGREE=9`, must exit 1). Paste each last line.

Paths you may touch: the conflicted files, `crates/graph-core/src/layout/force/**`,
`crates/graph-core/src/layout/barnes_hut*`, their tests, `scratch/fmtprobe_p.rs` (delete).

Done when: no conflict marker (`git grep -n -e '^<<<<<<<' -e '^>>>>>>>' -- crates` empty), the checks
pass, and the return block lists the conflict resolutions hunk by hunk in one line each.
