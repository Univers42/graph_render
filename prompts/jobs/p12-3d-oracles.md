# Job p12-3d-oracles (agent build, branch p12-3d-oracles, worktree ~/goinfre/wt/p12-3d-oracles)

Context: branch `p12-t4a` (971318dc, base bdb04f15) added five 3-D SciGraphs arms on a tree that is
1573 files behind develop. Develop has since registered four of the five under other ids. The
assessment is `git show origin/assess-3d:docs/measurements/assess-3d-branches.md` (read the
"Branch 1", "Overlaps" and "Recommendation" sections first). Decision taken: **Option A, develop's
ids win.** Port only what develop lacks; never `git merge`/`cherry-pick` p12-t4a whole. Read the
branch with `git show 971318dc:<path>` and `git diff bdb04f15 971318dc -- <path>`.

Id map (t4a → develop): `layout.spectral.3d` → `layout.spectral3d`; `layout.mds.pivot.3d` →
`layout.mds.pivot3d`; `layout.spiral.3d` → `layout.basic3d.spiral`; `layout.bipartite.3d` →
`layout.bipartite_3d`; `layout.random.3d` stays `layout.random.3d` (new on develop, matches the
`.3d` suffix of `layout.force.*.3d`). Confirm each develop id with `git grep -n` before using it.

Exact tasks:
1. Random 3-D arm: port the `ID_3D` arm of `crates/graph-core/src/layout/random.rs` and register
   `layout.random.3d` by APPENDING one entry at the END of `LAYOUTS` in
   `crates/graph-core/src/registry/layouts.rs` (append-only; never insert or reorder) with full
   `Metadata` (oracle, complexity, scale_ceiling, degradation, ponytail). Use develop's
   `BASIC_3D_CEILING` convention (`registry/three_d.rs`), not t4a's `CLOSED_FORM_3D_CEILING`.
2. 3-D oracles: port the `dims` threading of `harness/oracle-spectral.py` and the
   `random_3d`/`spiral_3d`/`bipartite_3d` arms + xyz `block()` reader of
   `harness/oracle-closed-form.py`, keyed to develop's ids. Port the matching graph-cli rows and
   ceilings (`crates/graph-cli/src/oracle_python/{spectral,closed_form}.rs`,
   `capabilities/registry/layout_row.rs` `SCIPY_ORACLE_LAYOUTS`), keyed to develop's ids.
3. Run the differentials (three steps each, CLAUDE.md "Python differentials"; find the closed-form
   commands with `git grep -n oracle-closed-form -- scripts crates/graph-cli/src`). Paste each
   3-D row's measured max deviation and its ceiling.
4. Spectral internals (`layout/spectral/space.rs`, `linalg/lobpcg/{ops,tests}.rs`,
   `layout/spectral/tests/dims_3d.rs`, `examples/diag_*.rs`): port ONLY if develop's
   `spectral_stage::spectral_3d` / `pivot_mds_3d` fail the step-3 differential, and then only the
   part that makes it pass. If develop passes, do not port them; say so with the numbers.
5. Count literals (snapshots, ledger, hashgate report tests): regenerate from the test output after
   step 1, never hand-merge t4a's numbers.
6. Rows: `scripts/orch/rows/p12-3d.rows`, adapted from `git show 971318dc:scripts/orch/rows/p12-t4a.rows`
   with develop's ids, plus one row per 3-D oracle differential and a negative control for each.
7. Docs: `docs/decisions/3d-ids.md` (Option A, ≤ 20 lines, cite the assessment and the id map);
   `docs/measurements/p12-3d-oracles.md` (commands, measured table, what was not ported and why).
   Commit `docs/measurements/assess-3d-branches.md` too: `git checkout origin/assess-3d -- docs/measurements/assess-3d-branches.md`.

Paths allowed: `crates/graph-core/**`, `crates/graph-cli/**`, `harness/oracle-spectral.py`,
`harness/oracle-closed-form.py`, `fixtures/**` (emitted only), `scripts/orch/rows/p12-3d.rows`,
`docs/decisions/3d-ids.md`, `docs/measurements/{p12-3d-oracles,assess-3d-branches}.md`,
`prompts/jobs/p12-3d-oracles.md`. Not allowed: `crates/graph-wasm/**`, `packages/**`, `app/**`,
`server/**`, `deploy/**`.

Done when, each with its command and exit pasted: `scripts/orch/gate.sh target/gate-p12-3d
scripts/orch/rows/quick.rows` all PASS; `scripts/orch/gate.sh target/gate-p12-3d-own
scripts/orch/rows/p12-3d.rows` all PASS; `capabilities --check` and `codegen --check` exit 0.
