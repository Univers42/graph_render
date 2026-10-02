# graph-motor — project status (2026-10-02)

Read this first, then `prompts/CONTINUE.md` (how to work on this host) and `prompts/RESUME.md`
(the newest handoff is the HANDOFF 2026-10-01 block at the top of it). Long history:
`docs/reports/HANDOFF.md`, `docs/reports/phase-NN*.md`. The tree and `git` are the final
authority over all three.

Every fact below was checked on 2026-10-02 against `origin/develop` = **701b46a** (2026-10-02,
436 commits). The previous edition of this file was written on 2026-09-30 against **f261baf**
(251 commits), so **185 commits landed in three days**. Check any row yourself:
`git log --oneline origin/develop..origin/<branch>` — 0 lines = merged.

## 1. What is on develop, by layer

### 1.1 Motor (`crates/graph-core`, `graph-contract`, `graph-wasm`, `graph-cli`)

- **35 layout ids** in `LAYOUTS` (`crates/graph-core/src/registry.rs:108`, array closes at `:289`).
  The list is **append-only**: entries are never inserted, because `hashgate`'s stage order and
  `docs/measurements/p12-t3.md:282` both depend on the index (`registry.rs:254-258`).
  *The job brief for this refresh said 34; the array is 35.*
- **SciGraphs coverage is complete.** `missing` = 0 (`docs/measurements/scigraphs-coverage.md:74`);
  the last five names landed with p12-t3 (commit `a39f968`, 2026-10-01) as real 3D ports
  (`docs/measurements/p12-t3.md:3`).
- **The 3D seam** landed 2026-10-01 (commit `6185be8`, `docs/measurements/p13-3d-seam.md`):
  `Geometry.z` + `Geometry::in_space`, `dim()` as the single reader of "is this 3D"
  (`crates/graph-core/src/layout/mod.rs:94-99`), `snapshot()` following the geometry
  (`crates/graph-contract/src/snapshot.rs:150,178`; `p13-3d-seam.md:29`). 2D bytes are unchanged —
  `binary/tests/pinned.rs` green unedited, `hashgate --seeds 8` PASS (`p13-3d-seam.md:38`).
- **Graphviz engines, 7 ids** under `crates/graph-core/src/layout/graphviz/`, one registry module
  each (e.g. `registry/graphviz_sfdp.rs`, `registry/graphviz_osage.rs`): twopi, osage, patchwork
  (`p13-gv1`, 3 names over 3 ids), circo (`p13-gv1-circo`), neato, fdp, sfdp (`p13-gv2*`) —
  the per-engine name-to-id map is `docs/measurements/scigraphs-coverage.md:64-70`. `dot` is
  **not** among them.
- **The ledger reads any oracle record by name** since the `graphviz-verdict` job (commit
  `04bfa76`, 2026-10-01): `Evidence::oracle_record` is a map lookup, not an arm per engine
  (`crates/graph-cli/src/capabilities/verdict.rs:52-58`, `:36-45`). Adding an engine therefore
  adds no arm to the ledger.
- **Conformance, byte for byte**: `scripts/scigraphs-conformance.sh` +
  `crates/graph-cli/src/oracle_python/conformance/` (commit `5d75aeb`, 2026-10-01), gated against
  a pinned baseline (`conformance/baseline/table.rs`), documented in
  `docs/measurements/scigraphs-conformance.md`.
- **wasm session hardening** (commit `d3fb0b6`, 2026-10-01, job `trap-followups`): the wasm force
  session returns a `Code` instead of trapping on a bad address —
  `u32::try_from(address).map_err(|_| Code::IndexOutOfRange)` at
  `crates/graph-wasm/src/session.rs:225`. The same commit splits `circo/blocks.rs` and keeps the
  studio test that a genuine trap still throws
  (`packages/graph-studio/tests/session.motor.test.ts:164`, negative control at `:180-202`).
- **Perf P1/P2 measured** (commits `94f7bc4` and `7a11a55`, both 2026-10-01):
  `docs/measurements/perf-p1-baseline.md`, `docs/reports/perf-p1.md`, `docs/measurements/perf-p2.md`,
  `docs/reports/perf-p2.md`, plus `scripts/orch/profile.sh` and `scripts/studio-perf.sh`.
  The plan's P2 exit (≤120 ms/tick at 1M) is **withdrawn**: it needed 53× less time per tick than
  the 6338 ms measured, below the cost of the walk itself
  (`docs/measurements/perf-p1-baseline.md:120-124`). The particle-mesh replacement is proposed
  there (`:130`) and lives on an unmerged branch (§2).

### 1.2 Oracles (differentials and their evidence)

Each differential is a `graph-cli` subcommand plus a `harness/*.py` arm plus a rows file plus a
`docs/measurements/*.md`. Arms that exist on develop:

| arm | ids it covers | measurement | recorded worst |
|---|---|---|---|
| `oracle-layouts` (d3-hierarchy, dagre-d3-es) | tidy tree, treemap, sugiyama crossings | `docs/measurements/phase05-crossings.md` | crossings, not bytes |
| `oracle-spectral` | spectral, mds.pivot | `docs/measurements/phase06-eigen.md` | tolerance (solvers differ) |
| `oracle-igraph` | 6 igraph force ids | `docs/measurements/p12-igraph-ceilings.md` | davidson_harel 51.91, graphopt 15.39 (`:48-56`) |
| `oracle-spring` / `oracle-circular-hierarchy` | spring, circular.hierarchy | `docs/measurements/p12-t2.md` | 7.288e-3 median deficit (`:49`); 2.380e-7 (`:22`) |
| `oracle-basic-3d` / `oracle-hierarchical-3d` | sphere, helix, cube, hierarchical3d | `docs/measurements/p12-t3.md:33-36` | 2.384e-7 / 2.376e-7 / corners exact / 1.192e-7 |
| `oracle-twopi` | twopi | `docs/measurements/p13-gv1.md` | 7.10e-2 pt, ceiling 1e-1 |
| `oracle-osage` | osage | `docs/measurements/p13-gv1-osage.md:10-14` | 6.31e-2 pt = 0.88 of one printed digit |
| `oracle-patchwork` | patchwork | `docs/measurements/p13-gv1-patchwork.md:99` | 6.613e-2 pt, ceiling 1e-1: PASS |
| `oracle-circo` | circo | `docs/measurements/p13-gv1-circo.md:11` | **negative**: does not reproduce on 984 of 1000 |
| `oracle-neato` | neato | `docs/measurements/p13-gv2-neato.md:34` | 6.732e-2 pt, ceiling 1e-1: PASS |
| `oracle-fdp` | fdp | `docs/measurements/p13-gv2-fdp.md:22` | the oracle is **not reproducible run to run** |
| `oracle-sfdp` | sfdp | `docs/measurements/p13-gv2-sfdp.md:25` | seed-sensitive; ours 3.881e+02 vs oracle-vs-oracle 4.81e+02 |
| `scigraphs-conformance` | all 32 SciGraphs names | `docs/measurements/scigraphs-conformance.md` | motor is f32 end to end, so no f64 agreement (`:59-61`) |
| `stress --oracle d3` | barnes_hut | `docs/measurements/phase06-stress.md` | Pearson correlation, margin −0.05 vs d3 (`:44`) |

**Graphviz is docker-only, never a dependency** (`docs/decisions/graphviz-oracle.md`); the image
is pinned by sha256 in `scripts/orch/fetch-refs.sh`. `-Gstart` is **inert** for twopi, osage,
patchwork and circo (`p13-gv1.md:44`), so those are not seed-drift measurements.

### 1.3 Studio

- Two packages plus a Vite shell: `packages/graph-studio` (UI/actions/state/motor) and
  `packages/graph-render` (Canvas2D + 3D + camera); `app/` is the shell
  (`app/src/main.ts`, `app/src/parity.ts`). Driven by `scripts/studio.sh:31`.
- **Live forces landed** (commit `db936cf`, 2026-10-01, job `studio-live`): a force session over the
  wasm ABI (`packages/graph-studio/src/motor/live.ts:32,57`), the adapter
  (`motor/liveSession.ts:54`), the loop (`motor/liveLoop.ts:77`), drag (`motor/liveDrag.ts:2`), the
  worker (`motor/worker.ts:53`) and the panel (`ui/ForcesPanel.tsx:109`). Gate:
  `scripts/studio-live.sh`.
- **Watchdog landed** with it (`motor/watchdog.ts`, armed at `motor/bridge.ts:96,139`;
  row `live-dead-worker` at `deploy/nav/liverows.py:188`).
- **Edge gradient** landed 2026-10-01 (`packages/graph-render/src/canvas2d/edgeGradient.ts:19`,
  row `edge-gradient` at `deploy/nav/gradientrows.py:199-214`); measured in
  `docs/measurements/studio-edge-gradient.md` — both modes exit 1 on `perf-fps`.
- **Smoke** landed 2026-10-01 (`scripts/studio-smoke.sh`, `deploy/nav/smoke.py`,
  `deploy/nav/smokerows.py`). No `docs/measurements/studio-smoke.md` exists.
- **3D rendering** landed 2026-10-01 (`packages/graph-render/src/three/{projection,sort,paint3d,orbit}.ts`,
  `scripts/studio-3d.sh`, `docs/decisions/studio-3d.md:3` "accepted"). This **supersedes
  condition 5** of `contract-3d-verdict.md`, which had required the studio to refuse `dim = 1`
  by name; `packages/graph-render/src/snapshot/decode.ts:192` now refuses only `reserved-dim`.
- **Names/digest** (`packages/graph-studio/src/ui/names.ts:19`, test `tests/ui-names.test.ts:25`).

## 2. Branches pushed, not merged (12)

`git for-each-ref --no-merged origin/develop --format='%(refname:short)' refs/remotes/origin`

| Branch | Head | Ahead | Verdict | What / why |
|---|---|---|---|---|
| `p12-t2` | b1aa19d | 1 | **superseded** | the same files are on develop and `layout/force/spring.rs` has since grown (develop 12070 B vs branch 10631 B) |
| `tier-settle` | 6ab44f9 | 1 | **superseded** | `hashgate/tiered.rs`, `bench/tiers/*` and `docs/measurements/tier-settle.md` are all on develop; develop reworked `tiers/layout.rs` into `bench/tiers/route.rs` |
| `studio-force` | 15ce426 | 1 | **superseded** | the LiveForce port, worker, Forces panel and `deploy/nav/forces*.py` are on develop and larger; only `scripts/studio-forces.sh` is missing |
| `studio-ux` | 855a876 | 2 | **dropped by the user 2026-09-30** | do not plan work on it — `prompts/jobs/context-refresh.md:12-15`, and the HANDOFF 2026-09-30 block in `prompts/RESUME.md` |
| `p12-t4a` | 971318d | 1 | **not landed** | 3D arms of five closed-form/spectral names; `layout/spiral/spiral_3d.rs` and `docs/measurements/p12-t4a.md` absent from develop |
| `p13-gv2-dot` | f098188 | 1 | **not landed** | the `dot` layered port; no `layout/graphviz/dot.rs`, no `layout.dag.dot` anywhere in `crates/` |
| `p13-gv2-dot-rank` | f098188 | 1 | **duplicate ref** | `git diff --stat origin/p13-gv2-dot origin/p13-gv2-dot-rank` is empty: same commit as `p13-gv2-dot`, no rank-specific work |
| `perf-p2-pm` | e64e8df | 2 | **not landed** | `layout.force.particle_mesh` (the P2 replacement, `perf-p1-baseline.md:130`) |
| `sg-dedupe` | 4357222 | 1 | **not landed** | collapses ~158 lines of `layout/circular/hierarchy.rs` to ~30 (2026-10-02) |
| `review-core-base` | 62529fa | 1 | **not landed** | `docs/reviews/review-core-base.md`, 231 lines (2026-10-02) |
| `review-core-post` | ebeeb9d | 1 | **not landed** | `docs/reviews/review-core-post.md`, 133 lines (2026-10-02) |
| `review-studio` | 7dfb5cf | 1 | **not landed** | `docs/reviews/review-studio.md`, 245 lines (2026-10-02) |

Three branches are worth merging, in this order: `p12-t4a`, `p13-gv2-dot`, then `perf-p2-pm`.
The three `review-*` branches are docs and merge cleanly whenever.

## 3. The capability ledger, row by row

`scripts/orch/gr cargo run -q -p graph-cli -- capabilities --json` — **69 rows**, of which
**35 are layouts**. Status is one of four strings (`crates/graph-cli/src/capabilities.rs:33-42`):
`absent`, `stub`, `implemented`, `gated`. **No live row is `absent` or `stub`** — those two
appear only in tests. 17 rows are `gated` (8 topology, 8 layout, 1 transport); the other 52 are
`implemented`.

### 3.1 The 8 gated layout rows

| id | oracle |
|---|---|
| `layout.grid` | hand + `roundtrip` |
| `layout.tree.tidy` | d3-hierarchy@3.1.2 `tree()` |
| `layout.treemap.squarified` | d3-hierarchy@3.1.2 `treemap().tile(treemapSquarify)` |
| `layout.circular.radial` | hand + `roundtrip` (`docs/decisions/circular-conventions.md`) |
| `layout.packing.circle` | hand + the Collins-Stephenson Euler certificate |
| `layout.spectral` | SciGraphs `_spectral_component_coordinates`, scipy 1.16.2 |
| `layout.mds.pivot` | SciGraphs `_pivot_mds_component_coordinates`, numpy 2.3.3 |
| `layout.dag.sugiyama` | dagre-d3-es 7.0.14 crossing counts |

### 3.2 The 27 `implemented` layout rows

`implemented` means "real, gate evidence incomplete" (`capabilities.rs:39`) — **not** "unproven".

| id | oracle | why it is not `gated` |
|---|---|---|
| `layout.force.barnes_hut` | d3-force@3.0.0 stress metric | identity cannot be claimed: link/collide are Jacobi gathers where d3 scatters (`phase06-stress.md`) |
| `layout.forceatlas2` | networkx 3.6 `forceatlas2_layout` | chaotic; gated on stress, not coordinates (`fa2-chaos.md`) |
| `layout.random` | networkx 3.6 `random_layout` | shape only |
| `layout.circular.ring` | networkx 3.6 `circular_layout` | — |
| `layout.spiral` | networkx 3.6 `spiral_layout` | — |
| `layout.bipartite` | networkx 3.6 `bipartite_layout` | — |
| `layout.force.yifan_hu` | **none** | no differential exists for this id |
| `layout.force.fruchterman_reingold` | python-igraph 0.11.9 | — |
| `layout.force.kamada_kawai` | python-igraph 0.11.9 | — |
| `layout.force.graphopt` | python-igraph 0.11.9 | ceiling 15.39 (`p12-igraph-ceilings.md:48-56`) |
| `layout.force.davidson_harel` | python-igraph 0.11.9 | ceiling 51.91, same file |
| `layout.force.lgl` | python-igraph 0.11.9 | — |
| `layout.force.drl` | python-igraph 0.11.9 | — |
| `layout.twopi` | Graphviz 16.1.0 twopi | worst 7.10e-2 pt, ceiling 1e-1 (`p13-gv1.md:24`) |
| `layout.packing.osage` | Graphviz 16.1.0 osage | 6.31e-2 pt (`p13-gv1-osage.md:10`) |
| `layout.circular.hierarchy` | SciGraphs `_circular_hierarchy_layout` | 2.380e-7 (`p12-t2.md:22`) |
| `layout.circular.circo` | Graphviz 16.1.0 circo | **negative result**: reproduces on 16 of 1000 (`p13-gv1-circo.md:11`) |
| `layout.treemap.patchwork` | Graphviz 16.1.0 patchwork | 6.613e-2 pt, under the ceiling (`p13-gv1-patchwork.md:99`) |
| `layout.force.neato` | Graphviz 16.1.0 neato | 6.732e-2 pt, under the ceiling (`p13-gv2-neato.md:34`) |
| `layout.force.fdp` | Graphviz 16.1.0 fdp | the oracle does not reproduce against itself (`p13-gv2-fdp.md:22`) |
| `layout.force.sfdp` | Graphviz 16.1.0 sfdp | seed-sensitive oracle (`p13-gv2-sfdp.md:17,25`) |
| `layout.basic3d.sphere` | SciGraphs `_sphere_layout` | 2.384e-7 under a 1e-6 ceiling (`p12-t3.md:33`) |
| `layout.basic3d.helix` | SciGraphs `_helix_layout` | 2.376e-7 (`p12-t3.md:34`) |
| `layout.basic3d.cube` | SciGraphs `_cube_layout` | corners exact; the interior is statistical and **not gated** (7.909 at seed 52, only 7 of 100 seeds interior-identical, `p12-t3.md:93`) |
| `layout.hierarchical3d` | SciGraphs `_hierarchical_layout_3d` | 1.192e-7 (`p12-t3.md:36`) |
| `layout.force.spring3d` | networkx 3.6 `spring_layout` at dim=3 | shares `layout.force.spring`'s kernel with the dimension as a parameter |

### 3.3 What `--check` says here, and what it does not

`scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` printed
**69 rows, 34 problems**, all of the form `gated, but no <record>: run the gate`. **That is an
artefact of this worktree, not a defect on develop**: `Evidence::load()` reads
`$GM_GATES_DIR` or `<workspace>/target/gates` (`crates/graph-cli/src/evidence.rs:33-38`) and a
freshly cut worktree has no gate records. A row is only backed by a record whose `fingerprint`
equals the current tree's (`capabilities/verdict.rs:60-69`). Do not read those 34 as red rows on
develop; run the gate and read the summary.

## 4. Work in flight and queued

`scripts/orch/queue.sh status` at 2026-10-02: **47 rows — 43 `done`, 2 `live`**
(`studio-switch-fit` pid 2602962, `status-refresh`), 2 `pending` (`osage-knob`,
`sg-conformance-split`). Read the columns as `queue.sh:74-84`: `rc` is `oc-job.sh`'s exit code
(0 = done **and** gate green, 2 = the agent was not `done`, 1 = gate red, 3 = worktree not
provably free), and `land` is `land()`'s exit code, written only when rc=0 and the row's
`land=yes` (`queue.sh:61-64`).

`rc=2` means the agent did not return `done` — **not** that the work is missing. Eleven `rc=2`
jobs landed their work anyway: `p13-gv1-twopi`, `-circo`, `-osage`, `-patchwork`,
`p13-gv1-osage-gate`, `p13-gv2-neato`, `p13-gv2-fdp`, `wasm-gm-build-trap`, `graphviz-verdict`,
`studio-smoke`, `studio-3d`. The three `rc=2` jobs whose work is genuinely not on develop are
`p13-gv2-dot`, `p13-gv2-dot-rank` and `p12-t4b`. `land=1`: `contract-3d`, `tier-settle`.

Note also that the `land` column in `queue.txt` does not match the state dir: six jobs carry
`land=0` and their work is on develop while their committed rows read `land=no`. The tree is the
authority (`scripts/orch/queue.sh:61` runs the land step only on `yes`).

### 4.1 Open, from the briefs

| job | deliverable | state on develop |
|---|---|---|
| `p12-t4a` | 3D arms of random / spiral / bipartite / spectral / mds.pivot | **not landed**; branch `971318d` holds all 40 files |
| `p12-t4b` | 3D arms of forceatlas2 / yifan_hu / FR / KK / DRL, plus `yifan_hu.2z` | **not landed**; no artefact, only the brief |
| `p13-gv2-dot` | the `dot` layered port (rank, then mincross, then position) | **not landed**; branch `f098188`, 7 files |
| `osage-knob` | a per-stage knob so `layout.packing.osage` can reach `gated` | **not landed**; the absence is deliberate and recorded at `capabilities/registry/unproven.rs:112-125` |
| `sg-conformance-split` | pure-move split of three over-300-line conformance files | **not landed**; still 309/309/302 lines |
| `studio-switch-fit` | 0 node pixels for 6 s after a layout switch | **not landed**; `deploy/nav/nav.py` has no switch probe |
| `p12-t3-knobs` | one hashgate knob per p12-t3 3D layout | **partial**: the five knobs landed (`hashgate/knobs.rs:125-151`, `hashgate/knob/three_d.rs`); the brief's "exit 1" goal was replaced by pinning `negctl-node-z` to exit **2** with the cause written (`scripts/orch/rows/p12-t3.rows:84`, `p12-t3.md:208-225`) |

**Closed since 2026-09-30**: p12-t2, p12-t3, p13-3d, p13-3d-seam, p13-gv1 and p13-gv1-circo
(twopi, osage, patchwork, circo), p13-gv2-neato / -fdp / -sfdp, the osage differential gate,
graphviz-verdict, wasm-gm-build-trap, trap-followups, studio-live, studio-watchdog,
studio-edge-gradient, studio-smoke, studio-3d, scigraphs-conformance.

## 5. Known gaps, each with the file that records it

**Motor**
- `layout.force.yifan_hu` has **no oracle at all** — the only 2D layout row whose oracle field
  reads `none`.
- `layout.circular.circo` does not reproduce Graphviz on 984 of 1000 seeds; the `qsort` tie order
  is not reproducible and `p13-gv1-circo.md:177,187,208` lists the closers not done.
- fdp and sfdp cannot be gated tighter than the reference meets itself
  (`p13-gv2-fdp.md:22,80`; `p13-gv2-sfdp.md:17,30,33`).
- neato: disconnected graphs, iteration stopped rather than finished, a float/double split,
  rotation invariance and the `-Tplain` five-digit quantum are all open
  (`p13-gv2-neato.md:252-291`).
- `layout.basic3d.cube`'s interior is not gated (3.2 above).
- Ceilings that are brackets, not runs: 200k and 14k force arms never ran and nothing was measured
  on wasm (`phase06-force.md:143,152,168`); the 100k neato number is extrapolated
  (`p13-gv2-neato.md:199-200`); the spring ceiling is bracketed (`p12-t2.md:137`).
- `simd_nodes` in the committed threshold table was never measured and is inert
  (`docs/decisions/tier-thresholds.md:3,11,23`); `bench --tiers` times nothing but Barnes-Hut
  (`tiers-audit.md:12,91`).
- The LOBPCG iteration cap can under-report, tie signs are ambiguous, and the 4096-node path
  fails to converge (`docs/decisions/eigensolver.md:63,105,115,199,301,341`).
- Live force session M2–M4 are not started (`docs/decisions/live-force-session.md:3`); the
  snapshot cache's build id does not exist, so D2 is blocked
  (`docs/decisions/snapshot-cache.md:33-34`).
- The 3D contract decision is still marked **proposed** (`docs/decisions/contract-3d.md:3`) while
  its verdict is "accepted, with conditions" (`contract-3d-verdict.md:3`); condition 5 is
  superseded by `docs/decisions/studio-3d.md:4`, and the 1.0 declaration is still open
  (`contract-3d-verdict.md:90`).
- No `negctl-node-z` row in `scripts/orch/rows/develop-full.rows:150-164`, so the full-suite sweep
  would not exercise the z control that `quick.rows:7` and `p12-t3.rows:84` do run.
- `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:5-6` still says "no 3D layout is registered";
  five are (`registry.rs:259-283`).
- `scripts/orch/rows/develop-full.rows` is 88 rows and carries **no studio render/interaction row**:
  the only studio-related row is `env-ignored` at `:198` (the studio's `.env` is ignored by the
  image build). The live, smoke, nav, interact, parity and 3D gates are run by hand through
  `scripts/studio-*.sh` and by queue jobs, not by the gate.

**Studio**
- `perf-fps` has **never passed**: 45.5 fps against a floor of 54 at 120 nodes, 2.4 fps against a
  floor of 14 at 2000 nodes (`docs/measurements/studio-s7.md:13`, "the row has never passed"
  at `:31`), and `perf-idle`/`perf-block` are red
  (`studio-perf-baseline.md:49-50`). Every edge-gradient and perf run exits 1 on this row
  (`studio-edge-gradient.md:51,57-62`), so its negative control proves nothing
  (`studio-s7.md:18`).
- Studio perf is software raster on one machine class, 20k nodes never measured
  (`studio-perf-baseline.md:14-16,38`).
- The layout-switch fit defect is unfixed (§4.1).

## 6. Merge rule (user, 2026-09-29 — unchanged)

**Merge first, repair on develop.** A branch is merged when the floor is green **on the merged
tree**, then one full gate runs on `develop` and its red rows become repair tasks. No per-branch
1000-seed hashgate, no per-branch mutants gate before the merge.

The floor is `scripts/orch/rows/quick.rows` — **11 rows**, of which **4 are negative controls**:
`fmt`, `clippy`, `test`, `wasm32-core`, `hashgate-8`, `negctl-degree`,
`negctl-dim-z-mismatch`, `force-gate-4`, `negctl-force-gravity`, `scigraphs-conformance`,
`negctl-scigraphs-conformance`. The land step gates on exactly this file
(`scripts/orch/queue.sh:37`).

Per merge: `git merge develop` into the branch (never rebase — history is published), resolve
keeping **both** intents, run the floor on the merged tree, merge into `develop`, push. A fresh
worktree needs `npm ci` before `cargo test` (`wt-new.sh` does it).

Still true: UNKNOWN = FAIL, SKIP is not a pass, a gate that did not run is "not run" and never
"green". Evidence records are fingerprinted to the tree.

## 7. Host

`dlesieur42` since 2026-09-30: **20 cores, 31 GB RAM** (`nproc`, `free -g`). No `/goinfre`, no
`/sgoinfre`. Host-local, unversioned, lost on every host change — `scripts/orch/scratch.sh` puts
it under `$GM_SCRATCH` = `$HOME/goinfre`: `wt/` (one worktree per job, `wt-new.sh`), `refs/` (the
pinned read-only references, `fetch-refs.sh`), `orch/{bin,logs,locks}`, `mcp-out/`.

## 8. Decisions already taken (do not re-ask)

- **2026-09-30 — free-model outage means the queue waits.** No paid fallback model.
- **2026-09-30 — the Graphviz engines must match Graphviz's own output**, gated against a
  Graphviz oracle; no Graphviz binary, library or server on this host.
- **2026-09-30 — CI approved**: a GitHub Actions merge-floor workflow with an arm64 hashgate arm.
- **2026-10-01 — the studio draws 3D** (`docs/decisions/studio-3d.md:3`), superseding the
  contract verdict's condition 5.
- Compute tiers: tuned scalar, then SIMD, then threads, then GPU only on measurement; a GPU
  layout gets its own capability id. D10: gather-form kernels. CPU only, multi-threaded (user).
- Hierarchy policy is "repair + record"; note codes 1–3 in p3, 4–5 in p5, 6 reserved.
- FA2 is a port of networkx 3.6; JAMA tred2/tql2 for n ≤ 256, scipy LOBPCG (block 4) above.
- References are pinned read-only and a missing one is a STOP, never an improvisation.
- The user delegated every non-critical decision to "the recommended option" for autonomous runs.