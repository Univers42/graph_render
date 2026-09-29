# Resume prompt — graph-motor, written 2026-09-29 before a host shutdown

Paste this to the next session as its first instruction. It replaces the "where are we"
parts of `docs/reports/STATUS.md` and `HANDOFF.md` until those are rewritten. The standing
rules are still in `CLAUDE.md`, `prompt.md`, `prompts/ONBOARDING.md`, `prompts/AGENT_BRIEF.md`.

## Mode the user asked for (2026-09-29)

- **Merge first, repair on develop.** The per-branch gating (1000-seed hashgate + mutants
  + review + devil verdict before each merge) had turned into perfectionism and blocked the
  train for hours behind one host-wide lock. The floor to merge a branch is now:
  fmt, clippy `-D warnings` and `cargo test --workspace` all green **on the merged tree**.
  Then one full gate runs on develop, and the red rows become repair tasks.
- Still true: UNKNOWN = FAIL, SKIP is not a pass. A gate that did not run is "not run",
  never "green". Evidence records are fingerprinted to the tree, so a gate is only
  worth running on the tree you are going to keep.
- Be terse. Report to the user at phase ends, blockers, decisions. A few lines.

## State at shutdown

The merge train is on branch `train` (worktree `/goinfre/dlesieur/wt/train`), pushed
to `develop` on 2026-09-29 after fmt 0, clippy 0, `cargo test --workspace` 701 passed /
2 failed — the 2 were `cli_oracles` oracle-layouts tests failing on a missing
`node_modules` in the fresh worktree; after `node-slim.sh npm ci --ignore-scripts` they
pass (4/4). A fresh worktree needs `npm ci` before `cargo test`.

| Phase | State |
|---|---|
| p0–p2 | merged, reports written |
| p3 deterministic layouts | **merged via train**. Its full gate (hashgate-1000, roundtrip, oracle-layouts, mutants) never finished — it runs as part of the develop gate |
| p5 Sugiyama | **merged via train** (inside p6f). No mutants run, no `phase-05.md` report yet |
| p6e spectral / pivot MDS | **merged via train** (inside p6f). Spectral differential measured (ceilings 1e-5 / 1e-7). No `phase-06.md` report yet |
| p6f Barnes-Hut / FA2 | **merged via train**. FA2 differential built (`harness/oracle-fa2.py`, `graph-cli emit-fa2-fixtures` / `oracle-fa2`, generic `oracle_python.rs`). BH and FA2 ledger rows stay `Implemented` |
| p7 analysis | **merged via train** (conflicts in capability tests resolved: 26 rows, 8 `analysis.*` rows `Implemented`) |
| p4 WASM SDK | **merged into develop 2026-09-29 03:53** (507f1c2): fmt 0, clippy 0, `cargo test --workspace --no-fail-fast` 755 passed / 0 failed. Resolution as planned: p4's registry-driven `hashgate/stages.rs` (`stages()` = topology + `LAYOUTS` in registry order + `transport.wasm.columnar`) with develop's knobs routed inside `stage_bytes_for` (BH/FA2 via `run_force` with `setting.force`/`setting.fa2`, Sugiyama via `run_pipeline` with `setting.sugiyama`, `extra_nodes` on the node count); develop's `knob.rs`/`compare.rs`; the C20 tally is in `report::body(control, seeds, tally, c20)` and `conclude()`; `wasm-run.mjs` is p4's real-ABI version (every non-grid layout runs through `gm_run`). **Found by the merge:** the wasm JSON ingest dropped p3's `EdgeRecord.child_first`, so the tidy tree diverged native vs wasm; `child_first` is now an optional 9th edge member (absent = false) written by `seed_ingest.rs`. **Deviation:** p4's CLI test `a_misspelt_knob_or_two_at_once_is_could_not_run_never_a_green_control` was folded into develop's typo loop in `tests/cli.rs` (check it still covers "two knobs at once"); the spacing control's `"transport.wasm.columnar": 0` assertion was re-added; capability rows are now 28, `--check` problems 34 |
| p8 post-routing / bundling | **merged into develop 2026-09-29 11:10** (b980ae8), 836 tests green. Kept slices: `p8-p8-bundle` (fdeb + mingle + ink), `p8-route` (grid_index + routed, petgraph Dijkstra, `post.route.grid` ledger row), `p8-styles`. Dropped as duplicates: `p8-fdeb`, `p8-mingle`, `p8-p8-grid`, `p8-p8-route`, `p8-p8-styles` (branches kept on origin). Repair: bundling and styles have no ledger rows yet |
| p9 scale bench | **merged into develop 2026-09-29 ~12:30** (d9912ad) under the merge floor (fmt 0, clippy 0, tests 0 failed). Conflicts in capability tests resolved by keeping both: 32 ledger rows (develop's `post.route.grid` + p9's three `scale.*` rows), 34 problems. Remaining work in `docs/reports/phase-09-progress.md` (the 10^5/10^6 bench arms time out on this host) |
| p10 ingest/SDK publish | slices 2-6 **merged into develop 2026-09-29** (c5c56a1; merged tree fmt/clippy/test green, 36 ledger rows). Open item 1 decided by the orchestrator: the contract gets an ADDITIVE `gm_build_contract` + `Motor#buildContract`, `gm_build` unchanged (the studio uses it); bunny `p10-abi` building it. `capabilities --check` / `ingest --check` rows added to `develop-full.rows` (`capabilities --check` exits 1 on its 34 known problems — repair items, not noise) |
| p11 compute tiers | branch `p11` from develop after the p9 merge; bunny `p11-start` launched 2026-09-29, died mid-edit (rc=1), relaunched as `p11-cont` on its WIP (`docs/reports/phase-11-progress.md` when it commits) |
| GUI "studio" (user request 2026-09-29) | branch `gui`: Vite + React app in `app/`, driving the wasm SDK and reusing `src/core/render`'s Scene; `scripts/studio.sh` serves it on :5173. Branch `abi-post`: wasm ABI + SDK for POST and ANALYSIS (`gm_post_*`, `gm_analysis_*`, `Motor#posts/#post/#analyses/#analysis`). Specs in `/sgoinfre/students/dlesieur/orch/prompts/{gui-studio,abi-post}.txt`. **Both merged into develop 2026-09-29** (gui 03ade38, abi-post 77ab863). Studio fixes found with Playwright: motor units are ~1 apart, so `app/src/core/worldScale.ts` rescales each run to ~56 px spacing; sprite blit size was ×sprite.width; the layout picker did not store its id; Polyline/Curve `pts` are INTERIOR points only (endpoints = node positions). Next: branch `studio-post` (bunny launched) wires the POST/ANALYSIS panels. abi-post left `index.ts` (476) and `harness/sdk-smoke.mjs` (542) over 300 lines, recorded in `docs/contract/wasm-abi.md` |

## Update 2026-09-29 afternoon

- develop 7043edf = p10 (p10-abi: `gm_build_contract`) + studio-post (studio ANALYSIS/POST panels) merged under the floor; an unused `overrides` param in `graph-sdk-js/src/adapters/notion.ts` failed the app's `tsc`, fixed.
- The second develop gate (`/goinfre/dlesieur/wt/p8/target/gate-develop/`) ran on the OLD tree b980ae8 and was interrupted near the end. Three red rows were rows-file bugs, now fixed in `develop-full.rows`: `negctl-force-theta` mutated theta to 0.9 = the default (now 0.5); `only-petgraph-added` flagged the workspace crate `graph-contract` (now allowed; it has no external deps); `sdk-smoke`/`zero-copy` ran before `wasm-release` (reordered). `capabilities-check` was listed three times (deduped).
- Flaky: `evidence::tests::{a_stamp_needs_the_built_tree…, the_real_tree_is_the_one…}` fail inside the full workspace run and pass alone (seen on abi-post and on the p10 merge).
- In flight (bunnies, one worktree each under `/goinfre/dlesieur/wt/`): `p11` (p11-next: threads bench, collide, link), `repair-evidence` (the flake, why `capabilities --check` keeps 34 "record from another tree" problems after a full gate, bundling/style ledger rows), `fa2fix` (item 4 metric, item 5 loose ends), `reports` (phase-03/05/06/07/08), `lesmis` (`fixtures/scigraphs/lesmis.json` from SciGraphs' own code, for the studio's SciGraphs look; plan `~/.claude/plans/mellow-snuggling-quiche.md`). Prompts in `/sgoinfre/students/dlesieur/orch/prompts/<label>.txt`, logs in `/goinfre/dlesieur/orch/logs/<label>.out`.
- Studio: the redesign session works on branch `studio`/`studio-look`; it was sent `orch/prompts/studio-scigraphs-look.txt`.
- Then: merge the repair branches, re-run the full gate on develop, repair its red rows.

## Update 2026-09-29 evening

- Merged into develop under the floor: `fa2fix` (FA2 gated at `max_iter` 2, `CEILING` 1e-7, measured 3.156e-8), `lesmis` (4f82f36, `fixtures/scigraphs/lesmis.json`; labels differ from fig6, documented in `docs/decisions/scigraphs-reference-fixture.md`), `repair-evidence`, `reports`, `negctl` (7f0cab6: knobs `GM_MUTATE_{TREE_TIDY,TREEMAP,CIRCULAR}_NODES`, `GM_MUTATE_PACKING_SCALE`, each red at 8 seeds; merged tree fmt 0, clippy 0, 1015 tests passed / 0 failed). Their rows are in `develop-full.rows`.
- negctl's open decisions: (1) `phase-06.md:153,196` call `GM_MUTATE_FORCE_THETA` inert; stale since b142c9a, needs a dated addendum, not a rewrite. (2) the four p3 stage ids exist twice (registry literals and `hashgate/stages.rs` consts, held by a test); promote to `Stage::ID` in graph-core.
- Several merge commits carry git's default message instead of `updated`; not rewritten (force push). Merges use `-m updated` from now on.
- In flight (`/goinfre/dlesieur/orch/bin/oc-status.sh`): `ledger-rows` (Phase 8/7 ledger rows), `p11-reconcile` (verifies the uncommitted work two racing agents left in the p11 worktree; p11 must land before `sim`, both edit barnes_hut), `sim-m1` (Track M: the live-simulation session API for the Obsidian-style studio graph, plan `orch/prompts/obsidian-graph-plan.txt`; devil verdict PROCEED-WITH-CONDITIONS, conditions in `orch/prompts/sim-conditions.txt`; next job `sim-m1b.txt` captures goldens from 79aef00), `orchfix` (oc-job/oc-status liveness fenced on the OpenCode service session list instead of pid files).
- Then: merge ledger-rows and p11, run the full gate on develop, repair its red rows.

## Remaining, in order

1. Done: train and p4 are on develop. Still to do: p7's SDK row (p4 → p7 dependency), and check the folded p4 CLI test listed under p4 above.
2. The WIP of every p8*/p9/p10 worktree was committed as `updated` and pushed to its own branch by `/sgoinfre/students/dlesieur/orch/snap-wip.sh` before shutdown: those commits are unreviewed agent output, gate them before trusting them.
3. One full gate on develop: the union of `/sgoinfre/students/dlesieur/orch/rows/p6e-full.rows`
   plus p3's rows, with the bench rows rewritten to the unified CLI
   (`bench --layout X --n 220,10000,100000 [--past-ceiling] [--vs-d3]`; the old `--nodes`
   form is gone), plus: `negctl` rows for `GM_MUTATE_FORCE_THETA` and
   `GM_MUTATE_FA2_SCALING_RATIO`, `stress --oracle d3 --seeds 1000`, and the FA2 rows:
   `emit-fa2-fixtures --seeds 1000` → `docker run --rm -v "$PWD:/w" -w /w ge-python-oracle
   python3 harness/oracle-fa2.py target/fa2-fixtures` → `oracle-fa2`.
   Then mutants over the merged diff. Repair red rows on develop.
   Ran 2026-09-29 on develop b980ae8 (`/goinfre/dlesieur/wt/p8/target/gate-develop/`): fmt, clippy, test, wasm32, hashgate-1000 (3052 s), the negctls and eigen-determinism PASS; `no-mul-add` was a false positive (it matched comments) and the row now greps `mul_add[[:space:]]*[(]`.
4. **FA2 ceiling: the coordinate differential cannot gate, and the port is not at fault.**
   Measured 2026-09-29 over 1000 seeds (release build), gap = max |ours - networkx| /
   extent: n < 100 median 2.1e-8 (f32 floor), worst 1.8e-5; n 300-400 median 2.0e-3;
   worst 0.165 (seed 340, n=342); 327 seeds above 1e-3. **networkx against itself** with the
   start perturbed by 1 ulp diverges just as far on the same seeds (seed 340: 0.151,
   seed 846: 0.200 vs ours 0.133), so it is chaos, not a port bug. The placeholder
   `CEILING = 1e-3` in `crates/graph-cli/src/oracle_python/fa2.rs` will go red. Replace the
   metric: compare after few iterations (measure where nx-vs-nx 1-ulp divergence stays
   < 1e-6, e.g. `max_iter` 10-20, which the fixture's `params.max_iter` already carries), or
   gate on stress-1 "not worse than networkx by more than X". Probe script:
   the scratch `fa2chaos.py` logic is described here — rebuild the nx.Graph per fixture line,
   `forceatlas2_layout(pos=initial*(1+eps))`, compare. Only then flip `layout.forceatlas2`
   (and BH, on `stress`) to `Gated`.
5. p6f loose ends: `crates/graph-core/src/registry/force.rs` still names the removed
   `examples/force_dump.rs` and a non-existent "oracle-layouts.mjs d3-force arm" for BH;
   `docs/measurements/phase06-force.md` / `phase06-stress.md` / `phase06-eigen.md` still
   show the old `--nodes` bench form and `/home/user/...` paths.
6. Reports `phase-03.md`, `phase-05.md`, `phase-06.md`, `phase-07.md` (the §12 shape), each
   listing its deviations (p6e: lobpcg, python-oracle image, `oracle_python.rs`, bench CLI).
7. Finish p8 (pick one route worktree; commit the dirty slice worktrees or drop them), then
   p9 → p10 → p11.
8. Files still over 300 lines that predate this train: `treemap/tests/golden.rs` (354),
   `treemap/tests/shapes.rs` (420), `tests/graph-engine.test.ts` (482).

## Where things are

- Worktrees: `/goinfre/dlesieur/wt/<branch>` (**/goinfre is wiped when the host changes**;
  every branch is pushed, the uncommitted p8 slice work is not).
- Orchestration helpers: `/goinfre/dlesieur/orch/bin/{gr,gate.sh,timed,mutants.sh,node-slim.sh}`
  (sources in `scripts/orch/`), rows files under `/sgoinfre/students/dlesieur/orch/rows/`,
  bunny jobs via `scripts/orch/oc-job.sh <label> <worktree> <agent> <body> [rows]`.
- Images: `ge-rust`, `ge-mutants`, `ge-python-oracle` (built with
  `--build-context nx=<dir holding networkx-3.6.tar.gz>`).
- References: `/goinfre/dlesieur/refs` (networkx 3.6, d3-*, scipy 1.16.2, jama); the docs
  write them as `/home/user/refs/...` (same files, older host).

## Errors not to repeat (lessons for the workflow system)

- **One agent per worktree, one gate at a time.** Two agents in one worktree clobbered each
  other (2026-09-28). On 2026-09-29 a repair agent started its own unlocked 1000-seed
  hashgate beside the orchestrated one in the same worktree, and the workflow started two
  builders for the same p8 slice in two worktrees. The orchestrator must check for an
  existing job per slice, not only per worktree, and agents must never run gates.
- **The gate queue is the bottleneck, not the agents.** A 1000-seed hashgate in a debug
  build took > 80 min; several branches queued behind one `flock` for hours. Gate once on
  the merged tree, not per branch; build the gate binaries `--release` where the result
  does not depend on the profile.
- **Stack dependent branches early** (p5 → p6e → p6f) so a conflict is resolved once.
  Merging p3 then p6f into develop was clean; p4 (which restructured hashgate in parallel)
  was not — a branch that rewrites a shared file must merge first or be rebased on intent.
- **`pkill -f <pattern>` kills your own shell** when the pattern appears in the command line.
  Kill by PID.
- **Row indices in capability tests move** every time a registry entry is inserted before
  the end (the dag row went 15 → 17). Prefer finding a row by id over `rows[N]`.
- **Knob lists were copied** into three test crates and had drifted (2, 3, 6 knobs); they
  now live once in `crates/graph-cli/tests/common/mod.rs`. Extract before the second copy.
- **zsh (`hellish`) quirks**: `echo ===` expands `=`; unmatched globs abort the command
  (`--include=*`), use `git grep`; `sed -n 'a,bp'` needs the comma.
- Host rustfmt is not configured; always `gr cargo fmt`.
- **A merge that looks too deep usually isn't.** The p4 merge (34 hunks) was aborted once as "too deep"; done hunk by hand with a small resolver script it took 10 minutes and was green on the third test run. Save a half-resolved merge to a `*-wip` branch (conflict markers and all) instead of aborting, so the work survives.
- **Run `cargo test --no-fail-fast`**: without it cargo stops at the first failing test binary and hides the other crates' failures.
- **A new struct field needs every constructor across crates, including the wire formats**: p3's `child_first` was missing from p4's wasm ingest, and only the 4-way hash gate caught it.
- Don't answer "is it done" from memory: check `git rev-list origin/develop..origin/<b>`,
  the gate `summary.txt`, and running processes first.
