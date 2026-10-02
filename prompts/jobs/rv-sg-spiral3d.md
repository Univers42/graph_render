# Job rv-sg-spiral3d (agent build, review only, docs)

Review the sg-spiral3d branch before it lands. This worktree is that branch, already merged with
develop (the merge resolved 8 conflicts by hand); its delta is `git diff origin/develop...HEAD`.
Brief it answered: `prompts/jobs/sg-spiral3d.md`. Reference:
`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-62` (SPIRAL_3D) in the worktree, and
numpy 2.x `linspace`/`interp` under `$GM_SCRATCH/refs` (`prompts/REFERENCES.md`).

Rules, checks and output shape: as `prompts/jobs/review-core-post.md` says (read it first), with
`docs/reviews/rv-sg-spiral3d.md` as the report and the only path you write. Check in particular:

1. `layout/basic_3d/spiral.rs` against `basic.py:36-62` line by line: the parameter defaults, the
   `linspace` port (endpoint, the `step` formula numpy uses, the last sample), any `interp` port,
   and that every transcendental goes through `libm` with no FMA (prompt.md §6 D1-D3).
2. Registry: `layout.basic3d.spiral` is APPENDED at the end of `LAYOUTS` in `registry.rs` (the
   order is index-mapped by `graph-wasm/src/exports/build.rs`), after `layout.bipartite_3d`; every
   `Metadata` field in `registry/three_d.rs` (or its child) is true of the code. Run
   `capabilities --check` and `codegen --check` and paste the last lines.
3. The merge resolution: `registry/three_d.rs`, `layout/basic_3d.rs`,
   `capabilities/tests/registry/routing.rs` and `ids.rs`, `hashgate/tests/report.rs`,
   `snapshot_cmd/tests.rs`, `snapshot_cmd/roundtrip/tests.rs` (`"snapshots":195`) and
   `oracle_python/conformance/gaps.rs`. Say whether each keeps both develop's and the branch's
   intent, and whether `layout.bipartite_3d` (landed on develop) is untouched.
4. Run `hashgate --seeds 8` (exit 0) and its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero);
   say whether any existing layout's stage hash moved (it must not).
5. Run `scripts/scigraphs-conformance.sh` and paste the `SPIRAL_3D` line; it must reach f32 N/N or
   the tolerance tier with the reason written in `docs/measurements/sg-spiral3d.md`. Any other row
   that moved is a FIX.
6. Complexity O(n), house limits (40-line functions, 300-line files, 4 parameters), a `Ponytail:`
   on every heuristic, a negative control on every new gate row.

End the report with a verdict line `VERDICT: LAND` or `VERDICT: FIX` and, for FIX, a numbered list
of what to change with `file:line`.

Done when: the report has every check above with its command output or `file:line`, and a verdict.
