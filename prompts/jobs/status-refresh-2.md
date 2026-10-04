# Job status-refresh-2 (agent build, branch status-refresh-2, worktree ~/goinfre/wt/status-refresh-2)

Context: the first refresh (`status-refresh`, landed at develop 6d4bdb8d) updated `docs/reports/STATUS.md`
and the resume files, but four items of its brief were not done. Facts measured on develop 6d4bdb8d
(2026-10-04); re-check each one with `git grep -n` or `wc -l` and cite `file:line` for what you write.
If one is wrong, say so and leave that text alone.
- `scripts/orch/rows/develop-full.rows` has the studio rows (221-225) but no `negctl-node-z` row.
  The row to copy is `scripts/orch/rows/p12-t3.rows:84`.
- `crates/graph-core/src/layout/pivot_mds/tests.rs` is 353 lines (house limit 300).
- `docs/decisions/live-force-session.md:3` says "M2–M4 not started", but
  `crates/graph-wasm/src/exports/delta.rs` exports `gm_force_session_*` and the studio steps a live
  session (`packages/graph-studio/src/motor/liveSession.ts`).
- `docs/decisions/contract-3d.md:3` says **proposed** while `docs/decisions/contract-3d-verdict.md:3`
  says accepted with conditions.

Dispatch at most 2 subagents.

Exact tasks:
1. `scripts/orch/rows/develop-full.rows`, ADDITIVE only: add `negctl-node-z`, copied verbatim from
   `scripts/orch/rows/p12-t3.rows:84`, beside develop-full's other `negctl-*` hashgate/roundtrip rows,
   with a one-line comment naming where it came from. Touch no other row. Run
   `scripts/orch/drun-check.sh` (expect exit 0) and the new row's command once by hand (expect exit 0);
   paste both commands and exits.
2. Split `crates/graph-core/src/layout/pivot_mds/tests.rs` into child modules (for example
   `pivot_mds/tests.rs` + `pivot_mds/tests/<topic>.rs`) so every file is at most 300 lines. Pure move:
   same test names, same count, no assertion changed. Paste the `test result:` line before and after
   from `scripts/orch/gr cargo test -p graph-core pivot_mds`.
3. `docs/decisions/live-force-session.md`: change only the status line (line 3), then add a dated
   addendum at the end (at most 15 lines, heading `## Addendum 2026-10-04`) naming what of M2 and M3 is
   on develop, each with `file:line`, and what is still not done (M3's frame-budget measurement against
   `prompt.md` §5.2 if you find none; M4 beyond the registered force arms). Do not rewrite the record.
4. `docs/decisions/contract-3d.md`: change only the status line to match the verdict (accepted with
   conditions; condition 5 superseded by `docs/decisions/studio-3d.md:4` — check that line says so),
   and add one line pointing to `docs/decisions/contract-3d-verdict.md:90` for the still-open 1.0
   declaration (check what line 90 says and cite the right line if it moved).

Paths allowed: `scripts/orch/rows/develop-full.rows`, `crates/graph-core/src/layout/pivot_mds/tests.rs`,
`crates/graph-core/src/layout/pivot_mds/tests/**`, `docs/decisions/live-force-session.md`,
`docs/decisions/contract-3d.md`, `prompts/jobs/status-refresh-2.md`. Not allowed: everything else.

Done when, each with its command and exit pasted: `scripts/orch/gr cargo test -p graph-core pivot_mds`
passes with the same count as before; `wc -l crates/graph-core/src/layout/pivot_mds/tests.rs
crates/graph-core/src/layout/pivot_mds/tests/*.rs` shows every file at most 300; `scripts/orch/drun-check.sh`
exits 0; the `negctl-node-z` command exits 0.
