# Job status-refresh (agent build, branch status-refresh, worktree ~/goinfre/wt/status-refresh)

Context: `docs/reports/STATUS.md` §4.1 ("Open, from the briefs") and §5 ("Known gaps") were written
before several jobs landed. Facts measured on develop 802f0f30 (2026-10-04) that contradict them:
- `osage-knob` landed: `crates/graph-cli/src/hashgate/knob/kind.rs:259` (`GM_MUTATE_PACKING_OSAGE_NODES`),
  `capabilities/registry/unproven.rs:141-152`.
- `sg-conformance-split`: no file under `crates/graph-cli/src/oracle_python/conformance/` is over 300 lines.
- `p12-t4b`: `layout.forceatlas2.3d`, `layout.force.fruchterman_reingold.3d`, `layout.force.kamada_kawai.3d`,
  `layout.force.drl.3d` are registered (`git grep -n '\.3d"' -- crates/graph-core/src`).
- `studio-switch-fit`: `deploy/nav/switchrows.py` exists.
- `docs/decisions/live-force-session.md:3` says "M2–M4 not started", but `crates/graph-wasm/src/exports/delta.rs`
  exports `gm_force_session_*` and the studio steps a live session (`packages/graph-studio/src/motor/liveSession.ts`).
- `docs/decisions/contract-3d.md:3` says **proposed** while `contract-3d-verdict.md:3` says accepted with conditions.
- `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:5-10` is already current (no "no 3D layout is registered").
- `scripts/orch/rows/develop-full.rows` has no `negctl-node-z` row and no studio row.
- `crates/graph-core/src/layout/pivot_mds/tests.rs` is 353 lines (house limit 300).

Every fact above is a starting point, not evidence: re-check each one yourself with `git grep -n` or
`wc -l` and cite `file:line` for what you write. If one is wrong, say so and leave that text alone.

Exact tasks:
1. `docs/reports/STATUS.md` §4.1 and §5: for each row or bullet, check it against the tree. Move what
   landed to the "Closed since" line with the commit or `file:line` that proves it; keep what is
   still open, with its current evidence. Add a dated line at the top of §4.1:
   "Re-checked 2026-10-04 against develop <sha>". Do not touch other sections.
2. `docs/decisions/live-force-session.md`: change only the status line, then add a dated addendum
   (≤ 15 lines) naming what M2 and M3 are on develop, each with `file:line`, and what is still not
   done (M3's frame-budget measurement against `prompt.md` §5.2, if you find none; M4 beyond the
   registered force arms). Do not rewrite the record.
3. `docs/decisions/contract-3d.md`: status line to match the verdict (accepted with conditions,
   condition 5 superseded by `docs/decisions/studio-3d.md:4`), and a one-line pointer to
   `contract-3d-verdict.md:90` for the still-open 1.0 declaration.
4. `scripts/orch/rows/develop-full.rows`, ADDITIVE only (another branch edits line 90; do not touch it):
   - `negctl-node-z`, copied from `scripts/orch/rows/p12-t3.rows:84` verbatim.
   - A studio group at the end, before `# slow:`: `studio-check` (`scripts/studio.sh check`, expect 0),
     `studio-build` (`scripts/studio.sh build`, expect 0), `studio-smoke` (expect 0) and its control
     (`STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh`, expect nonzero), `studio-nav` (expect 0) and its
     control (`STUDIO_NAV_BREAK=1`, expect nonzero). Read each script's header for its prerequisites
     (the build must precede the browser gates; `scripts/studio.sh wasm` may be needed first).
   - Run `scripts/orch/drun-check.sh` (exit 0) and each new row once by hand; paste command and exit.
5. Split `crates/graph-core/src/layout/pivot_mds/tests.rs` into child modules so every file is
   ≤ 300 lines. Pure move: same test names, same count. Paste the count before and after from
   `scripts/orch/gr cargo test -p graph-core pivot_mds`.

Paths allowed: `docs/reports/STATUS.md`, `docs/decisions/live-force-session.md`,
`docs/decisions/contract-3d.md`, `scripts/orch/rows/develop-full.rows`,
`crates/graph-core/src/layout/pivot_mds/tests.rs`, `crates/graph-core/src/layout/pivot_mds/tests/**`,
`prompts/jobs/status-refresh.md`. Not allowed: everything else.

Done when, each with its command and exit pasted: `scripts/orch/gate.sh target/gate-status-refresh
scripts/orch/rows/quick.rows` all PASS; `scripts/orch/drun-check.sh` exit 0; the six new rows run
once each with the expected exit.
