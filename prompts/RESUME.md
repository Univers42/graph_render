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
| p4 WASM SDK | **NOT merged** (tried 2026-09-29, aborted: too deep to do safely before shutdown). 34 conflict hunks. p4 moved the stage list into `hashgate/stages.rs`: registry-driven `stages()` = topology + `LAYOUTS` + `transport.wasm.columnar`, native bytes from `(layout.run)(&topology)` except the grid (run_pipeline with `setting.grid`), a `stage_bytes_for(seed, setting, layouts)` test seam and a `check()` for duplicate ids. develop (p6f) instead has a literal `STAGES: [&str; 11]`, 6 knobs in `hashgate/knob.rs` (`Setting` carries `sugiyama`, `extra_nodes`, `force`, `fa2`), and `stage_bytes` special-cases `BarnesHut::ID`/`ForceAtlas2::ID` (`run_force` with `setting.force`/`setting.fa2`) and runs Sugiyama last via `run_pipeline::<Sugiyama>` with `setting.sugiyama`. **Resolution:** keep p4's `stages.rs` shape and its transport stage; keep develop's `knob.rs`; inside `stage_bytes_for`, route BH / FA2 / Sugiyama / NodeCount through `setting` exactly as develop's `stage_bytes` does; drop the literal `STAGES` for `stages()` and fix every `STAGE_COUNT`/array-size use; merge `harness/wasm-run.mjs` so it has p4's real-ABI path and develop's per-layout exports (spectral, mds_pivot, force_barnes_hut, forceatlas2, dag_sugiyama). Then update the pinned stage lists in `report.rs`, `snapshot_cmd/tests.rs`, `tests/snapshot.rs`, `tests/cli*.rs` and the capability row counts |
| p8 post-routing / bundling | slices on branches `p8-*`, several worktrees have **uncommitted** work (see below). Two bunny builders were running on the route slice in two different worktrees (`p8-route` and `p8-p8-route`) — duplicate work; keep one |
| p9 scale bench | branch `p9` started from `train`; a bunny builder was started 03:4x on the native slice; see `docs/reports/phase-09-progress.md` on that branch if it committed |
| p10, p11 | not started (p10 needs p4; p11 needs p9) |

## Remaining, in order

1. If `train` is not on develop yet: run fmt/clippy/test on `train`, fix, push `train:develop`.
2. Merge **p4** into develop by hand (see the conflict list above). Then p7's SDK row.
3. One full gate on develop: the union of `/sgoinfre/students/dlesieur/orch/rows/p6e-full.rows`
   plus p3's rows, with the bench rows rewritten to the unified CLI
   (`bench --layout X --n 220,10000,100000 [--past-ceiling] [--vs-d3]`; the old `--nodes`
   form is gone), plus: `negctl` rows for `GM_MUTATE_FORCE_THETA` and
   `GM_MUTATE_FA2_SCALING_RATIO`, `stress --oracle d3 --seeds 1000`, and the FA2 rows:
   `emit-fa2-fixtures --seeds 1000` → `docker run --rm -v "$PWD:/w" -w /w ge-python-oracle
   python3 harness/oracle-fa2.py target/fa2-fixtures` → `oracle-fa2`.
   Then mutants over the merged diff. Repair red rows on develop.
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
- Don't answer "is it done" from memory: check `git rev-list origin/develop..origin/<b>`,
  the gate `summary.txt`, and running processes first.
