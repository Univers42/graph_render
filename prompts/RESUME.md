# Resume prompt — graph-motor, first written 2026-09-29 before a host shutdown

This is an append-only handoff log. **The newest block is the 2026-10-04 status refresh at the
top**; older blocks are kept as history and several of their claims are now false — the tree is
the authority. `docs/reports/STATUS.md` and `prompts/CONTINUE.md`, both rewritten 2026-10-04, are
the current rollups. The standing rules are still in `CLAUDE.md`, `prompt.md`,
`prompts/ONBOARDING.md`, `prompts/AGENT_BRIEF.md`.

## 2026-10-04 — status refresh, on develop at fba1a288 (1429 commits)

A docs-only job (`prompts/jobs/status-refresh.md`) re-derived every claim from the tree. Nothing
below is from a brief; each line cites a commit, a `file:line` or a command.

- **Develop moved 993 commits in two days.** The 2026-10-02 rollup was written against `701b46a`
  (436 commits); develop is now `fba1a288` (1429). The ref moved *while this file was being
  written* — `origin/ux-params-dock` landed and took the branch head with it — so re-read the ref
  before quoting a sha (`docs/reports/STATUS.md:9-13`).
- **`LAYOUTS` is 47, not 35** (`crates/graph-core/src/registry/layouts.rs:47`,
  `[Capability; 47]`). The ledger is **82 rows**, **18 gated** (8 topology, 9 layout, 1 transport),
  64 implemented. `layout.packing.osage` is the ninth gated layout, discharged by the
  `negctl-osage-nodes` row at `scripts/orch/rows/develop-full.rows:183`.
- **`dot` is half-landed**: the rank pass is ported
  (`crates/graph-core/src/layout/graphviz/dot.rs:1-19`) with `class2` and `simplex` beside it, but
  mincross and position are not assembled and there is still **no `layout.dag.dot` row**
  (`p13-gv2-dot-mincross`, ab760496, is unmerged).
- **The studio now has gate rows** — `studio-wasm` / `-check` / `-build` / `-smoke` and
  `negctl-studio-smoke` at `develop-full.rows:221-225`. The full gate is 100 rows with 20 negative
  controls. The 2026-10-02 claim "the gate runs no studio row" is false.
- **Conformance is the scoreboard**: 32 SciGraphs rows, 11 `bitwise` / 14 `tolerance` / 7 `shape`,
  10 of 32 `f32`-identical on all 1020 coordinates
  (`docs/measurements/scigraphs-conformance.md:200-252`). No row is `f64`-exact and none can be —
  the motor is `f32` end to end (`:119-124`).
- **14 branches unmerged**, and the list moved twice while this was written — `ux-params-dock`
  landed (taking develop's head with it) and `p13-gv3-dot-position` was pushed. The one that
  matters most: **`svc-image`** (3bf7223, **73 commits**, 96 files) is the entire
  `server/graph-server` workspace, and develop has no server at all. The two `dot` branches
  (`p13-gv2-dot-mincross`, `p13-gv3-dot-position`) **overlap** — diff them before landing either.
- **The queue is idle**: 135 labels, every one `done`, zero `live` and zero `pending`. `rc=2` (58 of
  them) means the agent did not write `status: done`, not that the work is missing.
- **New decisions on develop** (all 2026-10-03/04): `browser-threads`, `delta-abi`,
  `force-session-warm-seed`, `gpu-force-tier`, `ingest-columns`, `layout-params`, `memory-guard`,
  `node-overlap`, `obsidian-force`, `bh-jiggle-key`, `note-code-7-deferred`, `wasm-ingest-limits`,
  `sfdp-gather-form`, `render-readable-spacing` — the list is in `STATUS.md` §8, do not re-ask.
- **Still open**: `perf-fps` has never passed (`studio-s7.md:13,31`), `studio-switch-fit`,
  `layout.circular.circo` (16 of 1000 seeds), `layout.force.yifan_hu` and `.2z` (no oracle at all),
  `simd_nodes` inert (`tier-thresholds.md:15,116`), no `negctl-node-z` in `develop-full.rows`.

The next session's first page is `prompts/CONTINUE.md` §4. The first task is unchanged in kind and
larger in scope: **run the full gate on develop and repair its red rows.** The 10^5/10^6 bench
arms and `hashgate-1000` (3052 s) time out on this host class, so run them under
`scripts/orch/timed`, last.

## 2026-10-03 — the memory guard (host freezes), on develop at 19d2ae9b

The host froze twice and needed a hard reset: RAM on 2026-10-03 20:32 (11-18 uncapped job
containers under `vm.overcommit_memory=1`) and the GPU on 2026-10-02 23:36 (a gfx ring timeout
with llama-server holding 6.3 of 8.6 GB VRAM). Decision and evidence:
`docs/decisions/memory-guard.md`, `docs/measurements/memory-profile.md`.

- Containers: every `docker run` goes through `scripts/orch/drun` into `gm.slice` (60 % of RAM, no
  swap); `drun-check.sh` flags a bare one. An excess is now an exit 137 in one gate, not a freeze.
- Host: `gm-memwatch.service` (`scripts/orch/memwatch.sh`) kills the largest job cgroup under RAM
  pressure, the largest unprotected GPU client when VRAM is full, and a process that hangs the gfx
  ring twice in 120 s.
- Motor: `graph_core::budget` refuses quadratic tables over 1 GiB (KK above 11 585 nodes, neato
  above 13 376). valgrind (`memprofile.sh`): about 1.2 KB of heap per node, memcheck 0 lost.
- Studio: document and link caps (`packages/graph-studio/src/source/limits.ts`), the motor worker
  retired on a new source or a trap (wasm memory never shrinks), GL contexts lost on destroy, and a
  source that took the page down is not replayed at the next start.
- Outside the repo: the llama container is capped at 8g and stopped (`docker start
  hellish-ai-llama-vulkan-1` brings it back). `vm.overcommit_memory` stays 1; the decision says why.

## HANDOFF 2026-10-01 (written 2026-10-02) — read this first, it overrides the 2026-09-30 block

`docs/reports/STATUS.md` and `prompts/CONTINUE.md` were rewritten on 2026-10-02 and are the
current rollups. This block is the dated handoff; everything below it is history and is kept.

### develop now

- `origin/develop` = **701b46a** (2026-10-02), **436 commits**. The 2026-09-30 handoff's `f261baf`
  was 251, so **185 commits landed in three days**. The tree is the authority; check any row with
  `git log --oneline origin/develop..origin/<branch>`.
- **35 layout ids** in `LAYOUTS` (`crates/graph-core/src/registry.rs:108`; the previous handoff's
  "34" in `prompts/jobs/status-refresh.md:5` was already wrong). `docs/measurements/scigraphs-coverage.md:74`:
  `missing` = **0**.
- **The ledger has 69 rows** (`capabilities --json`), 17 of them `gated`. Status is one of four
  strings (`crates/graph-cli/src/capabilities.rs:33-42`): `absent`, `stub`, `implemented`, `gated`.
  No live row is `absent` or `stub`.
- Landed 2026-10-01 and cited in `docs/reports/STATUS.md` §1: p12-t2, p12-t3 (the five 3D
  SciGraphs layouts, `a39f968`), p13-3d and its seam (`6185be8`, the z column), the Graphviz
  ports twopi/osage/patchwork/circo (p13-gv1) and neato/fdp/sfdp (p13-gv2), the osage
  differential gate, `graphviz-verdict` (the ledger now reads any oracle record by name,
  `capabilities/verdict.rs:52-58`), the wasm session trap repair (`d3fb0b6`,
  `graph-wasm/src/session.rs:225` returns a `Code` instead of trapping), studio-live,
  studio-watchdog, studio-edge-gradient, studio-smoke and studio-3d.
- **Not run on this tree**: the full gate (`scripts/orch/rows/develop-full.rows`, 88 rows).
  Nothing is known about develop's red rows until it runs — UNKNOWN = FAIL.

### Still open

`p12-t4a` (3D arms of the five closed-form/spectral names, branch `971318d` holds all 40 files),
`p12-t4b` (3D arms of the force names, **no branch yet**), `p13-gv2-dot` (the `dot` port,
`f098188`; the brief orders rank → mincross → position), `osage-knob` (unblocks osage's promotion
to `gated`), `sg-conformance-split`, `studio-switch-fit` (live), `trap-followups` (**landed** — the brief's item
list is satisfied), `studio-3d` (**landed**). `p12-t3-knobs` is **partial**: the five knobs are in
`hashgate/knobs.rs:125-151`, and the brief's "exit 1" goal was replaced by pinning
`negctl-node-z` to exit **2** with the cause written (`scripts/orch/rows/p12-t3.rows:84`).
Full table with evidence: `docs/reports/STATUS.md` §4.1.

### Branches pushed but not on develop (12)

Superseded and deletable: `p12-t2` (b1aa19d), `tier-settle` (6ab44f9), `studio-force` (15ce426) —
develop carries the same work and has since grown past each. `studio-ux` (855a876) stays
**dropped** by the user, 2026-09-30: do not plan work on it. Work in them: `p12-t4a` (971318d),
`p13-gv2-dot` (f098188), `perf-p2-pm` (e64e8df, `layout.force.particle_mesh`), `sg-dedupe`
(4357222), and three docs-only review branches (62529fa, ebeeb9d, 7dfb5cf).

**`p13-gv2-dot-rank` is the same commit as `p13-gv2-dot`** — `f098188`, empty diff between the two
refs — because two `queue.txt` rows pointed at one branch. Merge it once and delete the duplicate
row.

### The queue, and a lesson from it

`scripts/orch/queue.sh status`: 47 rows — 43 done, 2 live, 2 pending. The state is three files
per label in `$GM_SCRATCH/orch/queue/` (`<label>.pid`, `.rc`, `.land`); there is no append-only
journal.
**The `land` column in `queue.txt` is not the authority.** `p13-gv2-sfdp`, `trap-followups`,
`p12-t3-knobs`, `studio-3d`, `graphviz-verdict` and `wasm-gm-build-trap` all carry `land=0` in
`$GM_SCRATCH/orch/queue/` and their work **is** on develop, yet every one of their committed rows
reads `land=no` (`scripts/orch/queue.sh:61` only runs the land step when the column says `yes`).
The same is true of `rc`: eleven jobs read `rc=2` — "the agent did not return `done`" — and eleven
of those landed their work anyway. Read the tree and `git log`, never the column: `run()` skips
any label that still has a `.pid` (`queue.sh:94`), so nothing gets re-run, only misread.

### First tasks for the next session, in order

1. `scripts/orch/gate.sh <logdir> scripts/orch/rows/develop-full.rows` on develop, one timed gate
   at a time. Red rows become repair tasks.
2. Land `p12-t4a`, then `p13-gv2-dot`, then `perf-p2-pm` — each: merge develop into the branch,
   run `scripts/orch/rows/quick.rows` on the merged tree, then `scripts/orch/queue.sh land <label>`
   or set `land=yes`.
3. `wt-new.sh p12-t4b` and launch it; it has no branch yet.
4. Land the three `review-*` docs branches and `sg-dedupe`.

### The Obsidian drag + forces panel — CLOSED

The chain in the 2026-09-30 block (4 steps: `sim` → `force-wasm` → `studio-force` → merge) is
**done**: the force session is on develop and the studio drives it
(`packages/graph-studio/src/motor/live.ts:32,57`, `motor/liveDrag.ts:2`, `ui/ForcesPanel.tsx:109`),
gated by `scripts/studio-live.sh` (commit `db936cf`) with a watchdog at `motor/watchdog.ts` armed
from `motor/bridge.ts:96,139`.

### What the 3D verdict decided, and what superseded it

`docs/decisions/contract-3d-verdict.md:48` is **PROCEED-WITH-CONDITIONS**; conditions 1–4 held
(the 2D bytes never moved: `binary/tests/pinned.rs` green unedited,
`docs/measurements/p13-3d-seam.md:38`). **Condition 5 — refuse `dim = 1` by name — was superseded
on 2026-10-01** by `docs/decisions/studio-3d.md:4`: the studio now draws the 3D layouts, and
`packages/graph-render/src/snapshot/decode.ts:192` refuses only `reserved-dim`. The 1.0
declaration is still open (`contract-3d-verdict.md:90`).

### Environment facts that bit the last session

- **The gate queue is the bottleneck, not the agents.** Never let a worker run a timed gate;
  `scripts/orch/timed` holds the host-wide `flock`.
- `p13-gv1-circo` is a recorded **negative result**: our circo reproduces Graphviz's on 16 of 1000
  seeds (`docs/measurements/p13-gv1-circo.md:11`). Do not spend a job "fixing" it without reading
  that file first.
- fdp and sfdp cannot be gated tighter than the Graphviz oracle meets itself
  (`p13-gv2-fdp.md:22`, `p13-gv2-sfdp.md:17`).
- `layout.force.yifan_hu` is the one layout row with **no oracle at all**.
- Studio `perf-fps` has never passed and every studio perf run exits 1 on it, so its negative
  control proves nothing (`docs/measurements/studio-s7.md:18,31`).

## HANDOFF 2026-09-30 00:40 — superseded by the block above; kept for history

The previous session ran out of time at a user deadline. The user's order: everything goes on
`develop` with this prompt. What landed, what did not, and how to finish each item follows.

### develop now

- develop = the `integ` branch (worktree `/goinfre/dlesieur/wt/integ`), fast-forwarded.
  It holds, on top of 63cde33 (last develop that passed the full merge floor):
  - **p11 compute tiers** (merge c84c869): `exec/` runners (`Serial`, threaded), `hashgate` Threads/Tiers
    arms and 11 knobs in `hashgate/knob.rs` (new `hashgate/tests/knob/p3.rs`), `bench/` tier plans,
    `main.rs` enum moved to `command.rs`.
  - **p12-t1** (merge b182b7e): yifan_hu multilevel over Barnes-Hut (`barnes_hut/settle.rs`),
    `EmitClosedFormFixtures` / `OracleClosedForm` commands, `bench --tiers --workers`.
    Merge fix: `settle.rs` now calls `sim.tick(&mut How { runner: &Serial, workers: 1, .. })`
    (p11 changed `tick`'s signature).
- Merge floor on that tree (b182b7e), run 2026-09-30 00:10-00:20: fmt 0, clippy `-D warnings` 0,
  `cargo test --workspace --no-fail-fast` rc=0, 17 test binaries, 1122 passed / 0 failed / 11 ignored.
  The two `self-check FAILED: the settle is 112 ticks` lines in that log are the harness's own
  negative-control output inside passing tests, not failures. Log: `/goinfre/dlesieur/integ-test.log`
  (wiped on host change).
- **Not run on this tree:** wasm32 build of graph-core, hashgate (8 or 1000 seeds), negctl rows,
  oracles, mutants, `capabilities --check`, `codegen --check`. Run them first — UNKNOWN = FAIL.

### First tasks for the next session, in order

1. `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown`, then
   `hashgate --seeds 8` and one negctl (`GM_MUTATE_REFERENCE_DEGREE=9` must be NON-zero).
   `report.rs`'s expected `equal` set now lists yifan_hu; check it matches the run.
2. Full gate on develop, one timed gate at a time:
   `scripts/orch/gate.sh <logdir> /sgoinfre/students/dlesieur/orch/rows/develop-full.rows`.
   Add rows for p11 (threads arms, the 11 knobs each red) and p12-t1 (closed-form oracle,
   `p12-t1.rows`). Red rows become repair tasks.

### Branches pushed but NOT on develop (unverified agent output; each has 1 commit on top)

| Branch | Head | What | To finish |
|---|---|---|---|
| `p12-igraph` | 8238039 | igraph layouts DRL, LGL, DavidsonHarel, Graphopt, Kamada-Kawai, Fruchterman-Reingold; `registry/igraph.rs`; `harness/oracle-igraph.py`; capabilities/registry split | (a) merge develop in, compile the registry split; (b) fix the tests that hard-code layout counts: `hashgate/tests/report.rs:64`, snapshot_cmd, roundtrip, 3 `cli_ledger` tests (look rows up by id); (c) Rust `oracle-igraph` reads 0 cases for DRL/LGL while Python writes 100 — fixture/case-name mismatch; (d) DavidsonHarel measured 51.91 and Graphopt 15.39 against a 2e0 ceiling: decide (stress metric or a documented wider ceiling) in `docs/measurements/`; (e) `.hypot` at `drl/tests.rs:82`, `lgl/tests.rs:32` must go through `libm`; (f) confirm the negctl rows in `orch/rows/p12-igraph.rows` fail; (g) lint 0 ERROR |
| `sim` | 5ae4210 | force-session M1 (live simulation for the Obsidian-style drag): `layout/force/session.rs`, `session/`, `barnes_hut/sim/`, `docs/decisions/live-force-session.md` | merge develop in (conflicts in `barnes_hut.rs`/`sim.rs`: p11's `How` tick must be kept); frozen-acceptance and setters tests; the 65-digest golden check against 8e8e93b (`orch/prompts/sim-m1b.txt`); delete `scratch/fmtprobe_p.rs`; floor + wasm32 + hashgate 8. Conditions: `orch/prompts/sim-conditions.txt` |
| `followups2` | 3ddd308 | phase-7 follow-ups: graph-wasm analysis/post registries, hashgate knobs/staged | merge develop in (hashgate conflicts with p11 likely: keep both), floor |

`followups2` merge was tried at 00:15 and aborted (not enough time to run the floor after):
7 files, 17 hunks, all in `crates/graph-cli/src/hashgate/{knob.rs, tests/{knob,mod,report,stages}.rs}`
and `crates/graph-cli/tests/{cli,cli_ledger}.rs`. They are additive: p11 has 11 knobs (the 10 plus
`SplitSum`), followups2 has 25 (the 10 plus 15 ANALYSIS/POST stage knobs, held against
`knobs::ANALYSIS_POST_STAGES` by `the_analysis_and_post_controls_are_the_knobs_table`). The union is
26 knobs: keep the 10, then the 15 in table order, then `SplitSum` last so the table test's
slice still lines up, and check any p11 test that indexes `Knob::ALL`.

No job was running at 00:15 (`oc-status.sh`: every job done or DEAD). Nothing was relaunched,
because OpenCode's free models were hanging and the user asked to hand off.

Merge each one at a time into develop under the floor (merge develop into the branch first,
never rebase), in this order: `followups2`, `sim`, `p12-igraph`.

### The Obsidian drag + forces panel (user request, still open)

The user wants to drag nodes with the neighbours following, plus a panel (repulsion, link
distance, gravity, centre) like Obsidian. Chain:
1. `sim` M1 on develop (above).
2. `force-wasm`: worktree from `sim`; prompt `orch/prompts/force-wasm.txt`, rows
   `orch/rows/force-wasm.rows` (wasm ABI for the session: create/tick/set/pin/drag).
3. Studio side, branch `studio-force` (15ce426, owned by the studio session): worker adapter
   over the force-wasm ABI, the forces panel, and a CDP probe proving the neighbours move on drag.
4. Merge `studio-force` into `studio`, rebuild `app/public/graph_wasm.wasm`, then studio → develop.

### Studio branches (owned by the session graph-motor-studio-redesign)

- `studio` (S1-S5, S3 gaps, parity) is on develop since 984f433. Gates on that tree: fmt, clippy,
  cargo test, studio-check, studio-parity (+negctl) green; local, nav, interact (+negctls) green on
  b6a7551.
- `studio-s7` (S7 perf: 10k/20k case, stats() counters, three gating rows, `STUDIO_PERF_BREAK`
  negctl, `docs/measurements/studio-s7.md`) is being finished in `wt/studio-s7`; it merges into
  develop once quick.rows and the studio rows are green on the merged tree.
- `studio-ux` (855a876) is unfinished: drawer wiring; its chrome gate had 11 FAIL + 1 NOT-RUN rows
  (`orch/rows/studio-ux.rows`). Plan: `~/.claude/plans/mellow-snuggling-quiche.md` (slices A-D).
- `studio-force` (15ce426) is ungated; it waits for the force-wasm ABI (chain above).
- `limits` (in `wt/limits`, from 984f433): splits the 19 files over 300 lines and removes the
  unexplained `#[allow(clippy::too_many_arguments)]` in `graph-cli/src/capabilities/post.rs`. It is a
  pure structure change, gated by quick.rows, the hashgate --seeds 8 row and its negctl, and sdk
  typecheck and smoke.
- Still open: S6 (live physics, needs force-wasm) and S8 (`orch/prompts/obsidian-graph-plan.txt`
  lines 146-174), and the §12 reports for the studio phases.

### Queued, not started

- `p12-t2` (its job died with 0 commits; relaunch from develop with `orch/prompts/p12-t2.txt`).
- `p13-gv1` (native Graphviz engines, `orch/prompts/p13-gv1.txt`), then p13-gv2.
- `p13-3d`: a devil verdict FIRST (user approved 3D subject to it), then `orch/prompts/p13-3d.txt`.
- Thread tier bench numbers (p11) into `docs/measurements/`.
- Every SciGraphs layout must be in the motor AND the studio picker (user request).
- CPU only, multi-threaded (user request).

### Environment facts that bit this session

- **Docker**: the rootless daemon was restarted with `--exec-opt native.cgroupdriver=cgroupfs`
  (socket `/tmp/xdg-101889/docker.sock`, picked up by `scripts/orch/docker-env.sh`). The cgroup
  driver is "none", so `--memory` / `GR_MEM` is **not enforced**: a runaway test can take the host.
  If `docker run --rm alpine true` fails with "Interactive authentication required", the daemon is
  back on systemd cgroups; restart it the same way.
- **OpenCode**: all free models hung at 2026-09-29 23:53. Re-probe before any bunny launch
  (a 1-line `opencode run` with a timeout). If they still hang, use the Claude fallback: haiku for
  scouting and gates, sonnet for build and review, opus only for judgement or the last repair.
  Job engine: `/sgoinfre/students/dlesieur/orch/engine/jobs.js`, args
  `{ label, wt, branch, rows, base, kind: 'rust'|'studio', steps?, cycles?, pre?, specs?, licence? }`.
- The user must run, themselves (deleting remote branches was denied to the agent):
  `git push origin --delete p8-fdeb p8-mingle p8-p8-grid p8-p8-route p8-p8-styles`.


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

## Update 2026-09-29 night

- Fan-out is now RULE 0. The user saw bunny jobs working serially.
  - Measured: no job journal before this date contains a `subagent` call.
  - Measured: headless `opencode run` exposes `subagent` (`{agent, description, prompt, sessionID?, background?}`, agents `general`/`explore`); three foreground calls in one message ran concurrently (20 s wall for 3 x 20 s).
  - The rule is in `orch/prompts/common-v2.txt` and `.opencode/agents/*.md` (099cb7b): no `background` flag; the return block reports `subagents: <n> explore, <n> general`.
  - Review check: `jq -r 'select(.part.type=="tool") | .part.tool' <journal> | grep -c subagent`.
- Merged:
  - `orchfix` (15edd32): `scripts/orch/oc-live.sh` asks the service which sessions are draining; `oc-job.sh` refuses unless oc-live exits 1; `oc-status.sh` shows UNKNOWN instead of DEAD when it cannot ask. 34/34 in `test-oc-live.sh`.
  - After a host change, also recreate `/goinfre/dlesieur/orch/bin/oc-live.sh -> scripts/orch/oc-live.sh`. Without it every job reads UNKNOWN.
  - After a host change, also rebuild the browser MCP image before any `ux` job: `docker build -f deploy/mcp-browser.Dockerfile -t gm-mcp-browser deploy`. `opencode.json` runs it with `--pull never`, so a missing image fails the MCP instead of pulling a stranger's name.
  - The browser MCP server is named `pw` (`tools.pw.*` in OpenCode's code-mode `execute`), because OpenCode's own `tools.browser.*` swallowed a server named `browser`. Screenshots need an absolute `/out/<label>/<name>.png`. Smoke run green on 2026-09-29 (`docs/decisions/opencode-browser-mcp.md`).
  - Worktree `.claude` submodules are empty, so bunnies had no skills until `opencode.json` `skills.paths` pointed at the main checkout's `.claude/skills` (verified: `frontend` loaded).
  - `ledger-rows` (3486ca5): `analysis.depth` row, depth re-pointed to `Hierarchy`, index-based row lookups made by-id. Merged tree: fmt 0, clippy 0, 1022 passed / 0 failed.
  - Its `capabilities --check` 34 problems are all "no record: run the gate" in a fresh worktree; the full gate owns them.
- In flight: `followups`, which covers:
  - the phase-06 addendum;
  - the p3 ids promoted to graph-core;
  - deleting the dead `Forest` in `graph-wasm/src/analysis.rs:306`;
  - hashgate stages so the analysis and post rows can reach `gated`. If a wasm export is missing, that is a decision, not an edit.

  Also in flight: `p11-reconcile` and `sim-m1`.
- Worktrees need `npm ci` before `cargo test` (`cli_oracles`). The orchestrator does it at worktree creation.
- `studio-ux` (the studio UX plan, step 2) is running in `/goinfre/dlesieur/wt/studio-ux`.
  - Prompt: `orch/prompts/studio-ux.txt` on top of `common-studio.txt`. Rows: `orch/rows/studio-ux.rows`.
  - Deviation: it was cut from `studio` a0c346d before S2 landed, because S2 was stalled. It touches `view.ts` with one additive line only.
  - A branch cut from `studio` lacks the `ux` agent ("Agent not found"). Fix: check out develop's `opencode.json`, `.opencode/agents/ux.md` and `ux-probe.md` (fa953cd); studio had not changed them.
  - Merge order into `studio` (from the peer session): S2, parity, s5, s3, s4, then studio-ux last. Merge `studio` into studio-ux after s3 lands, since s3 adds display actions to the same registry. The peer owns look/theme and the display panel; studio-ux owns `console/*` and set/get.
  - Before it merges: a devil review, plus `studio-perf` run alone.

## Update 2026-09-29 late

- Merged `followups` into develop (746f3a8): fmt 0, clippy 0, 1023 passed / 0 failed; review verdict MERGE.
- Launched `followups2` (branch `followups2` in `/goinfre/dlesieur/wt/followups`, prompt `orch/prompts/followups2.txt`). Decisions it carries:
  - authorised: graph-wasm `analysis`/`post` reachable natively for hashgate stages, with the ABI unchanged;
  - authorised: `analysis.components` split into weak and strong rows;
  - deferred: `gm_analysis_paths_run` (dijkstra, bellman_ford). It is an ABI change and waits for a user decision.
- Provider limits (measured 17:15Z): new `opencode run` sessions got `provider.quota` 429 on the first step, and running sessions got "stream ended without finish_reason". Both killed studio-ux (rc=2, C and D partly done, A and B missing) and p11-reconcile2 (rc=2 after item 0 went green).
  - Continuations: `studio-ux2.txt` (with `OC_COMMON=common-studio.txt`) and `p11-reconcile3.txt`. They are relaunched through a retry wrapper: sleep 600 s after each 429, 8 tries at most, staggered.
- Hung subagents (a provider stream open for more than 40 min) were interrupted through `POST /api/session/<id>/interrupt`. The peer session was told about its own hung subagents and did not act on ours.
- Fallback model (user decision, 17:50Z). space-bunny-free was still answering 429 after 30 min, and every job, the peer's included, had died on it. Jobs now run with `OC_MODEL=opencode/nemotron-3-ultra-free`, and `opencode/big-pickle` is the second choice. Only the env override changes; the agent files keep space-bunny. Both fallback models passed a probe of 1 shell call and 1 subagent dispatch, rc 0.
  - Relaunched: `followups2b`, `studio-ux2b`, `p11-reconcile3b`, and `sim-m1fix2`. sim-m1fix2 continues sim-m1fix, whose slices B and C were cut; it first maps items 1 to 4, then dispatches whatever is not done.
  - The peer session was told how to relaunch on the fallback model.
- The Obsidian-style forces (gravity, repel, link and center sliders, reheat, freeze) are item S6 of `orch/prompts/obsidian-graph-plan.txt`. S6 depends on M1 (sim-m1fix2), then M2 (contract, wasm and SDK, after a devil verdict), then M3 (gates), and then on M landing on develop. Until then the studio has only `layout.run`: no live physics.
- p8 leftovers (the peer flagged them). `origin/p8` has been an ancestor of develop since its merge, as have `p8-route`, `p8-styles` and `p8-p8-bundle`.
  - Superseded, not to be merged: `p8-fdeb`, `p8-mingle`, `p8-p8-grid`, `p8-p8-route` and `p8-p8-styles`. Each is 1 or 2 commits ahead and 118 behind, and each is an earlier slice attempt.
  - develop already has `post/{fdeb,mingle,grid_index,routed,...}`. `p8-p8-route`'s `routed/{graph,output,search}.rs` became `routed/{csr,measure,trace}.rs` on develop.
  - The user approved deleting them, but the permission classifier refused the delete (Git Destructive). They are still on the remote, and the user runs the delete themselves: `git push origin --delete p8-fdeb p8-mingle p8-p8-grid p8-p8-route p8-p8-styles`.
- Merge train through OpenCode. Worktree `/goinfre/dlesieur/wt/integ`, branch `integ`, cut from develop 8292407, with `npm ci` done.
  - For each green branch:
    - `sed s/BRANCH/<b>/g orch/prompts/merge-train.txt > orch/prompts/merge-<b>.txt`;
    - then `oc-job.sh merge-<b> /goinfre/dlesieur/wt/integ builder <that file> orch/rows/quick.rows`.
  - The bunny merges with `--no-ff --no-commit`, and oc-job commits and pushes `integ` once the floor is green.
  - The orchestrator then fast-forwards develop: `git push origin origin/integ:develop`. That push refuses anything that is not a fast-forward.
  - A contract or registry conflict comes back `blocked`, for an opus verdict.
- About 20:40: nemotron also answered 429 `provider.quota`, and only `opencode/big-pickle` answered a probe.
  - followups2b and p11-reconcile3b were stuck in the quota retry loop and were stopped. sim-m1fix2 had a child hung for more than 30 min on item 3 (golden provenance); it was interrupted and stopped.
  - All three were resumed on big-pickle with `OC_SESSION`, as `followups2c`, `p11-reconcile3c` and `sim-m1fix2c`. studio-ux2b stays on nemotron while it streams.
  - Before each relaunch, probe space-bunny, then nemotron, then big-pickle.
- About 21:10: space-bunny still answers 429. Probes at 20:50 and 21:10: `opencode/longcat-2.5-preview-free` and `opencode/mimo-v2.6-flash-free` run a shell tool (rc 0).
  - `oc-job.sh` now resumes a job on the next model of `OC_FALLBACK` (default: longcat, then mimo) as soon as a run ends in `provider.quota`, and waits `OC_QUOTA_WAIT` only after a full round was refused. Jobs still start on space-bunny (`OC_MODEL` unset).
  - studio-ux2b ended `aborted` (not quota) and was resumed on longcat as `studio-ux2c`.
  - sim-m1fix2c and p11-reconcile3c stalled for about 40 min on big-pickle. Their stale sessions were interrupted, and both were resumed on longcat as `sim-m1fix2d` and `p11-reconcile3d`. followups2c is still on big-pickle.
- About 21:30: SciGraphs layout parity (user: "we need to build all the others"). The picker reads the registry, so a layout appears in the studio once it reaches develop, develop is merged into studio, and the wasm is rebuilt into `app/public/graph_wasm.wasm`.
  - Building, each in its own worktree and branch (prompts in `orch/prompts/`):
    - `p12-igraph`: FR, KK, DrL (2D), LGL, Davidson-Harel, Graphopt;
    - `p12-t1`: random, circular.ring, circular.shell, bipartite, spiral, force.yifan_hu, plus any other networkx 2D entry;
    - `p12-t2`: force.spring (networkx spring_layout 2D), circular.hierarchy (SciGraphs CIRCULAR_HIERARCHY).
  - p12-igraph and p12-t1 died on quota with 0 commits and were resumed on longcat (`p12-igraphb`, `p12-t1b`).
- About 21:40: user decisions on the rest of the SciGraphs list.
  - Graphviz: no Graphviz binary, library or server. The motor reimplements all eight natively. The pinned Graphviz source is an algorithm reference and a docker-only test oracle (EPL-1.0: read it, don't translate it line by line).
  - 3D: approved. It needs a contract change (dim once per snapshot, a z column, 2D bytes unchanged), so the design goes through a devil verdict before any code.
  - Performance: CPU only. Every layout gets the Phase 11 thread tier and a bench-driven optimization pass. Order: easiest first.
  - Launched:
    - `p13-gv1`: twopi, circo, patchwork, osage. Step 0 pins the Graphviz source in `fetch-refs.sh`.
    - `p13-3d`: design doc `docs/decisions/contract-3d.md` only; `docs.rows` = fmt.
  - Next:
    - `p13-gv2` (neato, fdp, sfdp, dot) after p12 lands, reusing KK, FR, Yifan Hu and the Sugiyama pipeline;
    - an opus devil verdict on contract-3d, then the 3D implementation and the easiest 3D layouts;
    - after p11 merges: extend the thread tier to every layout and bench each one.
- About 22:40: quota triage. bunny and longcat answer 429; mimo works. Hung longcat subagent streams
  stalled p13-gv1, p13-3d, p12-t2 and followups2c. Their children were interrupted and p13 and
  followups2c were queued. `oc-job.sh` now also rotates the model on an "aborted ... inactivity" end.
  - OpenCode is capped at four jobs: merge-p11, studio-ux2c, sim-m1fix2e and p12-t2.
  - p12-igraph and p12-t1 each died twice on quota, so both moved to the Claude job engine. It is the
    Workflow `target/orch-engine/jobs.js`, a copy of `/sgoinfre/students/dlesieur/orch/engine/jobs.js`.
    Runs: p12-t1 `wf_31284d19-1d9`, p12-igraph `wf_c5c5371d-09c`.
  - Queued, relaunched with `OC_SESSION` once a slot frees:
    - p13-gv1 `ses_f114f15a3ffe5X0BotPtVWYn3y`;
    - p13-3d `ses_f114ec78affe6pzAwmdN3NeQl3`;
    - followups2c `ses_f11b6612bffeSYUGh7YTLDrxwK`.
- About 23:30: p12-t1 relaunched on the engine as `wf_14bf57f8-c2f` (commit c377215 carries its WIP;
  `capabilities --check` removed from its rows: it exits 1 on every tree without 1000-seed records).
  **Done 23:25, 0c72dc5 pushed**, gate green on the final tree (11 rows incl. the closed-form
  differential vs networkx and its perturbation negctl). Waits in the merge train after p12-igraph.
  Follow-ups: multipartite, arf, bfs, planar (documented as skipped in `docs/decisions/layouts-tier1.md`);
  the `oracle-closed-form` record is not read by capabilities, so ring/spiral/bipartite cannot reach gated.
- Obsidian-style live forces (user request): split into three jobs, because the physics must come
  from graph-core `ForceSession` (branch `sim`, uncommitted WIP) and no TypeScript simulation is allowed.
  - `studio-force` (engine, `orch/prompts/studio-force.txt`): the `LiveForce` port, worker loop,
    drag glue, Forces panel (Center, Repel, Link force, Link distance) and tests. **Pushed 15ce426**
    on branch `studio-force` (merges into `studio`). Gate: studio-check 0 (re-run by the lead: the
    engine's red row did not reproduce; 298+271+55 tests pass), studio-nav 0, studio-forces 0, negctl 1.
    The panel shows "live forces need the motor session (force-wasm)" until the adapter exists.
  - `force-wasm` (`orch/prompts/force-wasm.txt`, `orch/rows/force-wasm.rows`): graph-wasm ABI + SDK
    `ForceSession` + a 4-way session hash row. Starts from `sim` once sim-m1fix2e commits.
  - Then a wiring job: the real adapter in the studio worker, and a CDP probe that neighbours move.
- About 23:55: all three free OpenCode models hang (a 90 s probe gave rc=124 on bunny, longcat and
  mimo); the four OpenCode jobs died with the previous session. Reassigned to the Claude engine:
  sim-m1fix2 `wf_2c97e1c6-ddb` (critical path to force-wasm), p12-igraph `wf_fbd5ccd9-6db` (on its
  WIP), merge-p11 as a sonnet agent in `/goinfre/dlesieur/wt/integ` (18 conflict hunks; the lead
  gates, commits and pushes). Queued: p12-t2 (fresh), studio-ux2c. Re-probe OpenCode before each launch.
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

## New host 2026-09-30

- Host `dlesieur42` since 2026-09-30: 20 cores, 31 GB RAM, no `/goinfre`, no `/sgoinfre`.
  Host-local state is under `$GM_SCRATCH` (`$HOME/goinfre`): `wt/` worktrees, `refs/` pinned
  references, `orch/{bin,logs,locks}`, `mcp-out/` screenshots (`scripts/orch/scratch.sh`).
- Lost with the old host: the `/sgoinfre/students/dlesieur/orch` prompts and rows. Rows now live in
  `scripts/orch/rows/` (only `quick.rows` so far) and job briefs in `prompts/jobs/`.
- The HANDOFF 2026-09-30 00:40 section above is still the newest handoff and still stands. The
  merge floor, the merge train order (`followups2` → `sim` → `p12-igraph`), the dropped `studio-ux`
  and plan S8, and the decisions of 2026-09-30 are unchanged.
- develop is at f261baf and holds p0–p11, p12-t1, the studio (S1–S5) and `limits`.
  Not merged: `followups2`, `sim`, `p12-igraph`, `studio-force`, `studio-s7`, `studio-ux` (dropped).
- The first tasks are unchanged: wasm32 build of graph-core, `hashgate --seeds 8` plus one negctl
  (`GM_MUTATE_REFERENCE_DEGREE=9` non-zero), then one full gate on develop; red rows become repairs.
