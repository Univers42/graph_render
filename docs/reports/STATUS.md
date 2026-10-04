# graph-motor — project status (2026-10-04)

Read this first, then `prompts/CONTINUE.md` (how to work on this host) and `prompts/RESUME.md`
(the newest handoff is the 2026-10-04 block at the top of it). Long history:
`docs/reports/HANDOFF.md`, `docs/reports/phase-NN*.md`. The tree and `git` are the final
authority over all three.

Every fact below was checked on 2026-10-04 against `origin/develop` = **11664d74** (2026-10-04,
**1668 commits**; `origin/ci-green` and `origin/perf-p4e-columns` sit on the same sha). The previous
edition of this file was written on 2026-10-02 against **701b46a** (436 commits), so **1232 commits
landed in two days**. The ref moved under the previous writer (it read 802f0f30 mid-session, then
fba1a288), so re-read the ref before quoting a sha. Check any row yourself:
`git log --oneline origin/develop..origin/<branch>` — 0 lines = merged.

## 1. What is on develop, by layer

### 1.1 Motor (`crates/graph-core`, `graph-contract`, `graph-wasm`, `graph-cli`)

- **47 layout ids** in `LAYOUTS`, declared as `[Capability; 47]`
  (`crates/graph-core/src/registry/layouts.rs:47`, array closes at `:292`), re-exported at
  `registry.rs:53`. The list is **append-only** — `hashgate`'s stage order depends on the index.
  *The 2026-10-02 edition of this file said 35; the array is 47.*
- **The 3D seam** (commit `6185be8`, 2026-10-01, `docs/measurements/p13-3d-seam.md`): `Geometry.z`
  + `Geometry::in_space`, `dim()` as the single reader of "is this 3D"
  (`crates/graph-core/src/layout/mod.rs:94-99`), `snapshot()` following the geometry
  (`crates/graph-contract/src/snapshot.rs:150,178`). 2D bytes unchanged — `binary/tests/pinned.rs`
  green unedited, `hashgate --seeds 8` PASS (`p13-3d-seam.md:38`). Seven 3D layouts are registered
  (`registry/three_d/{basic,bipartite_3d,spiral3d,graph}.rs`).
- **SciGraphs name coverage is complete**: `missing = 0`
  (`docs/measurements/scigraphs-coverage.md:82`, counted 2026-09-30, re-checked 2026-10-01 after
  p12-t3). Coverage is not agreement, though — the byte-level answer is §1.2.
- **The `dot` engine, partial** (`crates/graph-core/src/layout/graphviz/dot.rs:1-19`): the rank pass
  is **ported** (`dot/{acyclic,class1,simplex,rank}.rs`), and `class2` + `simplex` are ported as the
  building blocks `dot_mincross` and `dot_position` need — but **neither pass is assembled** and
  there is **no `layout.dag.dot` row** in the ledger, so `GRAPHVIZ_DOT` is still `shape`/
  `reference-absent` (`conformance/rows.rs:27,195`). `dot_splines` is not needed: the motor emits
  polylines through the virtual nodes.
- **Graphviz engines, 7 modules** under `crates/graph-core/src/layout/graphviz/`: `circo.rs`,
  `dot.rs`, `fdp.rs`, `neato.rs`, `osage.rs`, `patchwork.rs`, `sfdp.rs`, each with a child
  directory; `layout.twopi` lives beside the radial layouts
  (`crates/graph-core/src/registry/graphviz_circo.rs:3`). `layout/graphviz/text_width.rs` pins the
  label width Graphviz feeds the x-coordinate simplex.
- **The ledger reads any oracle record by name** (`graphviz-verdict`, commit `04bfa76`):
  `Evidence::oracle_record` is a map lookup, not an arm per engine
  (`crates/graph-cli/src/capabilities/verdict.rs:47-53`, `load()` at `:34`). Adding an engine adds
  no arm to the ledger.
- **`layout.packing.osage` reached `gated`** on the `osage-knob` job: the per-stage control
  `negctl-osage-nodes` (`GM_MUTATE_PACKING_OSAGE_NODES=1`) landed at
  `scripts/orch/rows/develop-full.rows:183`, which is exactly what the row was refused for
  (`crates/graph-cli/src/capabilities/registry/unproven.rs:141-154`).
- **wasm session hardening** (`trap-followups`, `wasm-gm-build-trap`): the force session returns a
  `Code` instead of trapping on a bad address — `u32::try_from(address)` →
  `Code::IndexOutOfRange` at `crates/graph-wasm/src/session.rs:230-237`.
- **Perf**: P1/P2 (`docs/reports/perf-p1.md`, `perf-p2.md`), P5 GPU (`docs/reports/perf-p5.md`),
  P6 incremental (`docs/reports/perf-p6.md`), and the plan itself at `prompts/perf-plan.md`. The
  P2 exit (≤120 ms/tick at 1M) is **withdrawn** — it needed 53× less time per tick than the 6338 ms
  measured (`docs/measurements/perf-p1-baseline.md:120-124`).

### 1.2 Oracles (differentials and their evidence)

Each differential is a `graph-cli` subcommand plus a `harness/*.py` (or `.mjs`) arm plus a rows
file plus a `docs/measurements/*.md`.

| arm | ids it covers | measurement | recorded worst |
|---|---|---|---|
| `oracle-diff` (TypeScript) | 8 `topology.*` | `docs/measurements/p1-topology-memory.md:58` | row `develop-full.rows:57` |
| `roundtrip` (hand oracles, in-crate) | grid, circular.radial, packing.circle, dag.sugiyama | `fix-roundtrip-1000.md:129-132` | 1000/1000 seeds each |
| `oracle-layouts` (d3-hierarchy 3.1.2, dagre-d3-es 7.0.14) | tree.tidy, treemap.squarified | `docs/measurements/phase05-crossings.md` | crossings, not bytes |
| `oracle-spectral` (SciGraphs + scipy 1.16.2) | spectral, mds.pivot | `docs/measurements/phase06-eigen.md` | tolerance (solvers differ) |
| `oracle-igraph` (python-igraph 0.11.9) | 6 igraph force ids | `p12-igraph-ceilings.md:48-56` | davidson_harel 51.91, graphopt 15.39 |
| `oracle-fa2`, `oracle-spring`, `oracle-circular-hierarchy` | forceatlas2; force.spring, circular.hierarchy | `docs/measurements/fa2-chaos.md`; `p12-t2.md:22,49` | 7.288e-3 median deficit; 2.380e-7 |
| `oracle-basic-3d`, `oracle-hierarchical-3d` | sphere, helix, cube, hierarchical3d | `p12-t3.md:33-36` | 2.384e-7 / 2.376e-7 / corners exact / 1.192e-7 |
| `oracle-graphviz --engine` (7 engines) | twopi, osage, patchwork, circo, neato, fdp, sfdp | `p13-gv1.md:24`, `p13-gv1-osage.md:10-14`, `p13-gv1-patchwork.md:99`, `p13-gv1-circo.md:11`, `p13-gv2-neato.md:34`, `p13-gv2-fdp.md:22`, `p13-gv2-sfdp.md:17` | see §3.2 |
| `oracle-igraph3d` | the six `*.3d` / `.2z` arms | `docs/measurements/p12-igraph-ceilings.md` | on the unmerged branch `p12-3d-oracles` |
| `scigraphs-conformance` | all 32 SciGraphs names, byte for byte | `docs/measurements/scigraphs-conformance.md` | 11 `bitwise` / 14 `tolerance` / 7 `shape` |
| `oracle-scale` | lod.apply_budget, frustum_cull_spheres, simplify.build_coarse_level | `docs/measurements/fix-scale-oracle.md` | exact equality |
| `stress --oracle d3` | barnes_hut | `docs/measurements/phase06-stress.md:44` | Pearson margin −0.05 vs d3 |

**The conformance matrix is the real scoreboard** (`scigraphs-conformance.md:200-233`, 32 rows):
11 rows are `bitwise`, 14 `tolerance`, 7 `shape`, and **10 of 32 are `f32`-identical on all 1020
coordinates** with the rest of their gap being the narrowing (`:248-252`). The `sg-*` repair series
of 2026-10-03/04 moved rows up: `sg-spiral3d` and `sg-bipartite3d` to f32 1020/1020,
`sg-mt19937` (RANDOM, CUBE) to f32 1020/1020, `sg-grid-scale` to f32 1020/1020, `sg-sugiyama`
597/1020 f64, `sg-spectral-mds` and `sg-fa2-forcesim` to `bitwise`, `sg-graphviz-scale` for
TWOPI and PATCHWORK. The 7 rows still at `shape` are CIRCLE_PACKING, IGRAPH_KK, YIFAN_HU,
GRAPHVIZ_DOT, GRAPHVIZ_SFDP, GRAPHVIZ_CIRCO, GRAPHVIZ_OSAGE
(`crates/graph-cli/src/oracle_python/conformance/baseline/table/*.rs`).

**Graphviz is docker-only, never a dependency** (`docs/decisions/graphviz-oracle.md`); the image is
pinned by sha256 in `scripts/orch/fetch-refs.sh`. `-Gstart` is **inert** for twopi, osage, patchwork
and circo (`p13-gv1.md:44`), so those four are not seed-drift measurements.

### 1.3 Studio (`packages/graph-studio` + `packages/graph-render`, shell `app/`)

There is **no top-level `studio/` and no `server/` on develop** — the studio is the TypeScript
layer and `server/graph-server` exists only on the unmerged branch `svc-image` (§2). Driven by
`scripts/studio.sh:5-11,30`; dev server on `127.0.0.1:5174`; production build to `app/dist`.

- **Live forces** (`studio-live`, commit `db936cf`): the session (`motor/live.ts:32,57`), the
  adapter (`motor/liveSession.ts:54`), the loop (`motor/liveLoop.ts:77`), drag
  (`motor/liveDrag.ts:2`), the worker (`motor/worker.ts:53`) and the panel (`ui/ForcesPanel.tsx:109`).
  Gate `scripts/studio-live.sh`, row `live-dead-worker` at `deploy/nav/liverows.py:188`.
- **3D rendering** (`studio-3d`, accepted `docs/decisions/studio-3d.md:3`):
  `packages/graph-render/src/three/{projection,sort,paint3d,orbit}.ts`, `scripts/studio-3d.sh`.
  It **supersedes condition 5** of `contract-3d-verdict.md`; `snapshot/decode.ts:192` now refuses
  only `reserved-dim`.
- **Layout parameters on the wire** (`docs/decisions/layout-params.md:3`, job `ux-params-abi`,
  2026-10-03): accepted and implemented after a PROCEED-WITH-CONDITIONS ruling, surfaced through
  `ui/ActionForm.tsx` / `ui/controlOf.ts`.
- **Node overlap removal** (`post.separate.grid`): decision `docs/decisions/node-overlap.md`
  (accepted 2026-10-03), measurement `docs/measurements/ux-overlap.md` (2026-10-03).
- **Readable spacing** (`docs/decisions/render-readable-spacing.md`, 2026-10-04): the renderer
  scales by local spacing, `packages/graph-render/src/spacing.ts`.
- **Edge gradient** (`packages/graph-render/src/canvas2d/edgeGradient.ts:19`, row at
  `deploy/nav/gradientrows.py:199-214`), **smoke** (`scripts/studio-smoke.sh`,
  `deploy/nav/smokerows.py`), **names/digest** (`ui/names.ts:19`).
- **The studio is now in the develop gate**: rows `studio-wasm`, `studio-check`, `studio-build`,
  `studio-smoke` and `negctl-studio-smoke` at `scripts/orch/rows/develop-full.rows:221-225`. This
  closes the 2026-10-02 gap "the gate runs no studio row".

## 2. Branches pushed, not merged (7)

`git for-each-ref --no-merged=origin/develop --format='%(refname:short)' refs/remotes/origin`

**Every path named in the "What / why" column below is on the branch, not on develop** — read them
with `git show origin/<branch>:<path>`. Ahead counts are `git rev-list --count
origin/develop..origin/<branch>`; the last column is `git diff --stat origin/develop...<branch> |
tail -n 1`.

| Branch | Head | Ahead | Verdict | What / why |
|---|---|---|---|---|
| `p13-gv3-dot-position` | b879c74b | 16 | **not landed** | the third `dot` pass: `layout/graphviz/dot/position{.rs,/}` + `harness/oracle-dot-probe.py`, `docs/measurements/p13-gv2-dot.md`, 27 files, 2262 insertions (§4.1) |
| `circo-dup-leaf` | 9c787964 | 6 | **not landed** | the circo skeleton tree and its `path` tests, `docs/measurements/p13-gv1-circo.md`, 5 files, 244 insertions |
| `circo-cross-fast` | e9dbed2f | 5 | **tree already identical to develop** | 5 commits over `.github/workflows/floor.yml`, `scripts/orch/fetch-refs.sh`, `scripts/scigraphs-conformance.sh`; `git diff --stat origin/develop origin/circo-cross-fast` is **empty**, so only the ancestry is unmerged |
| `gate-repair-178c` | 488e90c3 | 3 | **repair** | `crates/graph-cli/src/ink_cmd.rs` + one `develop-full.rows` row, 2 files, 15 insertions |
| `perf-p4e-motor-fix` | c1c28bef | 2 | **not landed** | the P4e incremental-append fix in the motor and wasm: `index/extend/`, `exports/delta/`, `forcecheck/stream.rs`, `graph-wasm/src/lib.rs`, `docs/contract/wasm-abi.md`, 9 files |
| `perf-p4e-sdk` | dccd07c1 | 1 | **not landed** | the SDK/studio half of P4e: `graph-sdk-js/src/columns-assemble.ts` + its two test arms, 3 files, 356 insertions |
| `status-refresh-3` | 5a9f3ecc | 1 | **docs only** | this file and `prompts/RESUME.md` |

`origin` now lists only **12 refs** (`git for-each-ref refs/remotes/origin`), so the 2026-10-04
edition's other twelve — `svc-image`, `p12-3d-oracles`, `p13-gv2-dot-mincross`, `yifan-hu-octree`,
`perf-p4d-extend`, `perf-p3-wasm-replicas`, `perf-p3-steal`, `studio-pack`, `assess-3d`,
`fix-rows-drift`, `fix-rustdoc`, `fix-sc-misalign` — **no longer exist on the remote at all**; which
of them landed and which were deleted is not recorded here. `p12-t4a` survives only as the tag
`archive/p12-t4a` (2026-10-01, 971318dc). Re-run the `--no-merged` command above before acting on
the count; it moved twice while the previous edition was written.

## 3. The capability ledger, row by row

`scripts/orch/gr cargo run -q -p graph-cli -- capabilities --json` — **82 rows**, of which **47 are
layouts**. Status is one of four strings (`crates/graph-cli/src/capabilities.rs:35-43`): `absent`,
`stub`, `implemented`, `gated`. **No live row is `absent` or `stub`** — those two are for rows in
progress and appear only in tests. **18 rows are `gated`** (8 topology, 9 layout, 1 transport), the
other **64** are `implemented`.

### 3.1 The 9 gated layout rows

| id | oracle | the record |
|---|---|---|
| `layout.grid` | hand + `roundtrip` | `fix-roundtrip-1000.md:129` |
| `layout.tree.tidy` | d3-hierarchy@3.1.2 `tree()` | `oracle-layouts`, `develop-full.rows:44` |
| `layout.treemap.squarified` | d3-hierarchy@3.1.2 `treemap().tile(treemapSquarify)` | same row |
| `layout.circular.radial` | hand (`docs/decisions/circular-conventions.md`) | `fix-roundtrip-1000.md:130` |
| `layout.packing.circle` | hand + the Collins–Stephenson Euler certificate | `fix-roundtrip-1000.md:131` |
| `layout.spectral` | SciGraphs `_spectral_component_coordinates`, scipy 1.16.2 | `oracle-spectral`, `develop-full.rows:91` |
| `layout.mds.pivot` | SciGraphs `_pivot_mds_component_coordinates` | same row |
| `layout.dag.sugiyama` | dagre-d3-es 7.0.14 crossing counts | `fix-roundtrip-1000.md:132` |
| `layout.packing.osage` | Graphviz 16.1.0 osage | `p13-gv1-osage.md:10-14`, control `develop-full.rows:183` |

### 3.2 The 38 `implemented` layout rows

`implemented` means "real, gate evidence incomplete" (`capabilities.rs:41`) — **not** "unproven".

| id | oracle | why it is not `gated` |
|---|---|---|
| `layout.force.barnes_hut` | d3-force@3.0.0 stress metric | identity cannot be claimed: link/collide are Jacobi gathers where d3 scatters (`phase06-stress.md`) |
| `layout.forceatlas2` | networkx 3.6 | chaotic; gated on stress, not coordinates (`fa2-chaos.md`) |
| `layout.forceatlas2.barnes_hut` | the exact dense sum this layout does not use | — |
| `layout.forceatlas2.forcesim` | SciGraphs' own ForceSim | the conformance arm's id for `FORCEATLAS2` (`sg-fa2-forcesim.md:11`) |
| `layout.random` | networkx 3.6 `random_layout` | shape only |
| `layout.circular.ring` | networkx 3.6 `circular_layout` | — |
| `layout.spiral` | networkx 3.6 `spiral_layout` | — |
| `layout.bipartite` | networkx 3.6 `bipartite_layout` | — |
| `layout.force.yifan_hu` | `stress` | a stress differential, no coordinate oracle (`crates/graph-cli/src/capabilities/registry/unproven.rs:103`) |
| `layout.force.fruchterman_reingold` | python-igraph 0.11.9 | — |
| `layout.force.kamada_kawai` | python-igraph 0.11.9 | — |
| `layout.force.graphopt` | python-igraph 0.11.9 | ceiling 15.39 (`p12-igraph-ceilings.md:48-56`) |
| `layout.force.davidson_harel` | python-igraph 0.11.9 | ceiling 51.91, same file |
| `layout.force.lgl` | python-igraph 0.11.9 | — |
| `layout.force.drl` | python-igraph 0.11.9 | — |
| `layout.twopi` | Graphviz 16.1.0 twopi | worst 7.10e-2 pt, ceiling 1e-1 (`p13-gv1.md:24`) |
| `layout.circular.hierarchy` | SciGraphs `_circular_hierarchy_layout` | 2.380e-7 (`p12-t2.md:22`) |
| `layout.circular.circo` | Graphviz 16.1.0 circo | **negative result**: reproduces on 16 of 1000 (`p13-gv1-circo.md:11`) |
| `layout.treemap.patchwork` | Graphviz 16.1.0 patchwork | 6.613e-2 pt, under the ceiling (`p13-gv1-patchwork.md:99`) |
| `layout.force.neato` | Graphviz 16.1.0 neato | 6.732e-2 pt, under the ceiling (`p13-gv2-neato.md:34`) |
| `layout.force.fdp` | Graphviz 16.1.0 fdp | the oracle does not reproduce against itself (`p13-gv2-fdp.md:22`) |
| `layout.force.sfdp` | Graphviz 16.1.0 sfdp | seed-sensitive; improved but still `shape` (`sg-sfdp-collapse.md:137`) |
| `layout.basic3d.sphere` | SciGraphs `_sphere_layout` | 2.384e-7 under a 1e-6 ceiling (`p12-t3.md:33`) |
| `layout.basic3d.helix` | SciGraphs `_helix_layout` | 2.376e-7 (`p12-t3.md:34`) |
| `layout.basic3d.cube` | SciGraphs `_cube_layout` | corners exact; the interior is statistical and **not gated** (`p12-t3.md:93`) |
| `layout.basic3d.spiral` | SciGraphs `_spiral_layout_3d` | the conical 3D spiral, ported by `sg-spiral3d` |
| `layout.hierarchical3d` | SciGraphs `_hierarchical_layout_3d` | 1.192e-7 (`p12-t3.md:36`) |
| `layout.bipartite_3d` | SciGraphs `_bipartite_layout_3d` | f32 1020/1020 after `sg-bipartite3d.md:89` |
| `layout.spectral3d` | SciGraphs `_spectral_layout_3d` | `bitwise` tier, Proc median 4.67e-16 (`sg-spectral-mds.md:88`) |
| `layout.mds.pivot3d` | SciGraphs `_mds_layout_3d` | `bitwise` tier, Proc median 3.11e-16 (same file) |
| `layout.force.spring` | networkx 3.6 `spring_layout` at dim=2 | median stress deficit 7.288e-3 against a 1e-1 ceiling; the ceiling is bracketed, not a run (`p12-t2.md:49,137`) |
| `layout.force.spring3d` | networkx 3.6 `spring_layout` at dim=3 | shares `layout.force.spring`'s kernel with the dimension as a parameter; its stress deficit against the 16 000 ceiling was **not re-measured** (`p12-t3.md:37`, *Not measured* at `:313`) |
| `layout.force.yifan_hu.2z` | `stress` | a stress differential, no coordinate oracle (`unproven.rs:89`; `.3d` likewise at `:94`) |
| `layout.force.fruchterman_reingold.3d` | python-igraph 0.11.9 at dim=3 | the `p12-3d-oracles` arm is unmerged (§2) |
| `layout.force.kamada_kawai.3d` | python-igraph 0.11.9 at dim=3 | same |
| `layout.force.drl.3d` | python-igraph 0.11.9 at dim=3 | same |
| `layout.forceatlas2.3d` | networkx 3.6 at dim=3 | same |
| `layout.force.particle_mesh` | `stress --oracle d3` | the proposed P2 replacement (`perf-p1-baseline.md:130`) |

### 3.3 What `--check` says here, and what it does not

`scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` printed
**82 rows, 36 problems**, all of the form `gated, but no <record>: run the gate` or
`no evidence: nothing recorded backs it`. **That is an artefact of this worktree, not a defect on
develop**: `Evidence::load()` reads `$GM_GATES_DIR` or `<workspace>/target/gates`
(`crates/graph-cli/src/evidence.rs:6,40`) and a freshly cut worktree has no gate records. A row is
only backed by a record whose `fingerprint` equals the current tree's
(`capabilities/verdict.rs:151`). Run the gate and read the summary before concluding anything.

## 4. Work in flight and queued

`scripts/orch/queue.sh status` on 2026-10-04: **135 labels, and every one reads `done`** — zero rows
`live`, zero `pending`. (Grep the status column, not the whole line: `studio-live` and
`perf-p6-live-copy` are `done` labels whose *names* contain "live".)

`rc` is `oc-job.sh`'s exit code (0 = done **and** gate green, 2 = the agent
did not return `done`, 1 = gate red, 3 = the worktree could not be proven free); `land` is `land()`'s,
written only when rc=0 and the row's `land=yes`. The split is **65 `rc=0`, 58 `rc=2`, 10 `rc=1`,
2 `rc=3`**. `rc=2` does **not** mean the work is missing — the 2026-10-02 edition listed eleven
such jobs that landed their work anyway.

`land` records that are not `0`: `contract-3d` and `p12-t4b` `land=1` (red after the merge);
`osage-knob` `land=2` (could not run — **its work is on develop**, §1.1); `studio-3d`,
`review-layout-rest`, `review-gates`, `rv-sg-spiral3d` `land=landed`; `tier-settle` and
`sg-graphviz-scale` `land=superseded`; `sg-igraph-dims` `land=discarded`. The column is **not the
authority** (`scripts/orch/queue.sh:61`) — read the tree.

### 4.1 Open, from the briefs

| job | deliverable | state on develop |
|---|---|---|
| `p13-gv2-dot` | the `dot` layered port: rank, then mincross, then position | **partial**: rank landed (`layout/graphviz/dot.rs:1-19`); mincross and position are each on their own unmerged branch (`p13-gv2-dot-mincross` ab760496, `p13-gv3-dot-position` 535d1f10) and **they overlap** — position was written without mincross merged, so diff them against each other first. No `layout.dag.dot` row |
| `p12-t4a` | 3D arms of random / spiral / bipartite / spectral / mds.pivot | **closed**: the deliverable landed by another route (the `sg-*` conformance series) and the branch is now the tag `archive/p12-t4a` (2026-10-01, 971318dc) — there is no `origin/p12-t4a` and no local `p12-t4a` to diff |
| `p12-t4b` | 3D arms of forceatlas2 / yifan_hu / FR / KK / DRL, plus `yifan_hu.2z` | **landed**: all five ids are in `LAYOUTS`; `land=1` (red after the merge) |
| `p12-3d-oracles` | the 3D igraph differential for those five | **not landed**: branch `dae8b2c1`, 9 commits |
| `studio-switch-fit` | 0 node pixels for 6 s after a layout switch | **landed**: `deploy/nav/switchrows.py` is the probe, `deploy/nav/navrows.py:16,242` calls `row_layout_switch`, and `deploy/nav/nav.py:78` passes `expect_switch_stale=broken` under `--break` (`deploy/nav/nav.py:106`), which `scripts/studio-nav.sh:31,41` passes for `STUDIO_NAV_BREAK=1`. It runs under the `nav` / `negctl-nav` rows at `scripts/orch/rows/force-warm.rows:27-28` and `scripts/orch/rows/render-spacing.rows:10-11` — **not** in `develop-full.rows` |
| `sg-conformance-split` | pure-move split of three over-300-line conformance files | **open**: `split-300.md` is dated 2026-10-04 and covers Rust files; the conformance files are a separate split |
| `trap-followups` | the wasm force session returns a `Code`, never traps | **landed** (`crates/graph-wasm/src/session.rs:230-237`) |

**Closed since 2026-10-02**: `p12-t2`, `p12-t3`, `p12-t3-knobs`, `p12-t4a`, `p12-t4b`, `p13-3d`,
`p13-3d-seam`,
the seven Graphviz engines and their differentials, the osage differential gate and its knob,
`graphviz-verdict`, `wasm-gm-build-trap`, `trap-followups`, `studio-live`, `studio-watchdog`,
`studio-edge-gradient`, `studio-smoke`, `studio-3d`, `studio-switch-fit`, `ux-params-abi`, `ux-overlap`,
`scigraphs-conformance`,
`split-300`, the whole `sg-*` conformance repair series, and the `scratch/` hygiene commit (624d50e9,
2026-10-04): the tracked `g.dot`, `g.dot.dot` and `scratch/*/` run outputs are gone and `/scratch/`
is in `.gitignore:35` (`scratch/stress.mjs` is still tracked).

## 5. Known gaps, each with the file that records it

**Motor**
- `layout.force.yifan_hu`, `.2z` and `.3d` have **no coordinate oracle**: the registry now names
  `stress` for all three (`crates/graph-cli/src/capabilities/registry/unproven.rs:103,89,94`), so
  they are gated on a stress differential, not on coordinates.
- `layout.circular.circo` does not reproduce Graphviz on 984 of 1000 seeds; the `qsort` tie order is
  not reproducible and `p13-gv1-circo.md:177,187,208` lists the closers not done. `GRAPHVIZ_CIRCO`
  and `GRAPHVIZ_OSAGE` are still `shape` in the conformance matrix.
- fdp and sfdp cannot be gated tighter than the reference meets itself
  (`p13-gv2-fdp.md:22,80`; `sg-sfdp-collapse.md:137` — median Procrustes 0.8476 → 0.3400, cause still
  `algorithm`; at defaults sfdp never coarsens, `levels=0`, `sfdpinit.c:213`).
- neato: disconnected graphs, iteration stopped rather than finished, a float/double split, rotation
  invariance and the `-Tplain` five-digit quantum are all open (`p13-gv2-neato.md:252-291`).
- `layout.basic3d.cube`'s interior is not gated (3.2 above). `CIRCLE_PACKING`, `IGRAPH_KK` and
  `YIFAN_HU` are the other `shape` rows.
- **No conformance row is `f64`-byte-exact and none can be**: the motor is `f32` end to end, so
  `f64` agreement is structurally unreachable (`scigraphs-conformance.md:119-124`).
- Ceilings that are brackets, not runs: 200k and 14k force arms never ran and nothing was measured
  on wasm (`phase06-force.md:143,152,168`); the 100k neato number is extrapolated
  (`p13-gv2-neato.md:199-200`); the spring ceiling is bracketed (`p12-t2.md:137`).
- `simd_nodes` in the committed threshold table is **still not measured** and inert
  (`docs/decisions/tier-thresholds.md:15,116`); `bench --tiers` times nothing but Barnes-Hut
  (`tiers-audit.md:12,91`).
- The LOBPCG iteration cap can under-report, tie signs are ambiguous, and the 4096-node path fails to
  converge (`docs/decisions/eigensolver.md:63,105,115,199,301,341`).
- Live force session **M1–M3 landed; M4 is not started** (`docs/decisions/live-force-session.md:3`,
  addendum 2026-10-04); the snapshot cache's build id does not exist, so D2 is blocked
  (`docs/decisions/snapshot-cache.md:33-34`).
- The 3D contract decision is now **accepted, with conditions** (`docs/decisions/contract-3d.md:3`,
  no longer "proposed") and its verdict is "accepted, with conditions" (`contract-3d-verdict.md:3`),
  condition 5 is superseded by `studio-3d.md:4`, and the 1.0 declaration is still open
  (`contract-3d-verdict.md:90`).
- `server-and-write-path.md:3` and `server-dependencies.md:3` are still "accepted in principle" /
  "proposed" and owe a devil verdict; the code is 73 commits away on `svc-image` (§2).
- `develop-full.rows` **does** carry the z control: `negctl-node-z` at
  `scripts/orch/rows/develop-full.rows:191` (the strong form — it pins the exit to **2** and greps the
  z-column refusal, copied from `p12-t3.rows:84`), so the full sweep does exercise it.
- Every Rust file is back under the 300-line house limit (`docs/measurements/split-300.md`,
  2026-10-04, tree `43ab3af6`).

**Studio**
- `perf-fps` has **never passed**: 45.5 fps against a floor of 54 at 120 nodes, 2.4 fps against a
  floor of 14 at 2000 (`docs/measurements/studio-s7.md:13,31`); `perf-idle`/`perf-block` are red
  (`studio-perf-baseline.md:49-50`). Every edge-gradient and perf run exits 1 on this row
  (`studio-edge-gradient.md:51,57-62`), so its negative control proves nothing.
- Studio perf is software raster on one machine class, 20k nodes never measured
  (`studio-perf-baseline.md:14-16,38`).
- 3D above 5 000 nodes is unmeasured: the studio settles on the 2D mesh, so the probe refuses with
  `view.orbit() is null` (`docs/measurements/perf-3d-gl.md:41-45`). The 3D gate is one run per case
  on SwiftShader, the gap is floored at 16.7 ms and `frameMs` excludes raster (`perf-3d-gl.md:21-26`).
- 3D scope exclusions, by decision: no live forces in 3D (`:105,164`), no z-buffer (`:49,165`),
  no labels/glow/spheres/arrows or edge gradient on a 3D frame (`:154`), straight chords only
  (`:152-153`), no animation (`:108`), a node behind the eye is dropped (`:78`)
  — all `docs/decisions/studio-3d.md`.
- The layout-switch fit defect is unfixed (§4.1).

## 6. Merge rule (user, 2026-09-29 — unchanged)

**Merge first, repair on develop.** A branch is merged when the floor is green **on the merged
tree**, then one full gate runs on `develop` and its red rows become repair tasks. No per-branch
1000-seed hashgate, no per-branch mutants gate before the merge.

The floor is `scripts/orch/rows/quick.rows` — **11 rows**, of which **4 are negative controls**:
`fmt`, `clippy`, `test`, `wasm32-core`, `hashgate-8`, `negctl-degree`, `negctl-dim-z-mismatch`,
`force-gate-4`, `negctl-force-gravity`, `scigraphs-conformance`, `negctl-scigraphs-conformance`. The
land step gates on exactly this file (`scripts/orch/queue.sh:37`).

The full develop gate is `scripts/orch/rows/develop-full.rows` — **113 rows**, **27 of them negative
controls** (`negctl-*`; counted on develop c2578d07 with `grep -c '^negctl-'`), and it now carries the studio rows (§1.3). Run it under the host lock:
`scripts/orch/timed scripts/orch/gate.sh <logdir> scripts/orch/rows/develop-full.rows`.

Per merge: `git merge develop` into the branch (never rebase — history is published), resolve
keeping **both** intents, run the floor on the merged tree, merge into `develop`, push. A fresh
worktree needs `npm ci` before `cargo test` (`wt-new.sh` does it).

Still true: UNKNOWN = FAIL, SKIP is not a pass, a gate that did not run is "not run" and never
"green". Evidence records are fingerprinted to the tree.

## 7. Host

`dlesieur42` since 2026-09-30: **20 cores, 31 GB RAM** (`nproc`, `free -g`, re-measured 2026-10-04).
No `/goinfre`, no `/sgoinfre`. Host-local, unversioned, lost on every host change —
`scripts/orch/scratch.sh` puts it under `$GM_SCRATCH` = `$HOME/goinfre`: `wt/` (one worktree per
job, `wt-new.sh`), `refs/` (the pinned read-only references, `fetch-refs.sh`),
`orch/{bin,logs,locks}`, `mcp-out/`.

Memory guard since 2026-10-03 (`docs/decisions/memory-guard.md`): job containers run through
`scripts/orch/drun` under `gm.slice` (`gm-slice.sh check`), and `gm-memwatch.service`
(`memwatch.sh status`) watches RAM pressure, VRAM and gfx ring hangs. A new host needs
`gm-slice.sh install` and `memwatch.sh install`; without them drun warns and caps each container
alone.

## 8. Decisions already taken (do not re-ask)

- **2026-09-30 — free-model outage means the queue waits.** No paid fallback model.
- **2026-09-30 — the Graphviz engines must match Graphviz's own output**, gated against a Graphviz
  oracle; no Graphviz binary, library or server on this host.
- **2026-09-30 — CI approved**: a GitHub Actions merge-floor workflow with an arm64 hashgate arm.
- **2026-10-01 — the studio draws 3D** (`docs/decisions/studio-3d.md:3`), superseding the contract
  verdict's condition 5.
- **2026-10-02 — browser threads**: one shared memory plus a pool of wasm threads, both models built
  and measured before one is picked (`docs/decisions/browser-threads.md`).
- **2026-10-03 — growing a live graph appends, it does not rebuild** (P4), PROCEED-WITH-CONDITIONS
  (`docs/decisions/delta-abi.md`).
- **2026-10-03 — the live force session starts warm** on the layout's own picture
  (`docs/decisions/force-session-warm-seed.md`).
- **2026-10-03 — the GPU force tier** is the particle mesh's tick on WebGPU
  (`docs/decisions/gpu-force-tier.md`).
- **2026-10-03 — node overlap removal** goes where `docs/decisions/node-overlap.md` says.
- **2026-10-03 — the ingest columnar contract** is decided with conditions
  (`docs/decisions/ingest-columns.md`), and the wasm ingest ceilings are decided and amended
  (`docs/decisions/wasm-ingest-limits.md`).
- **2026-10-03 — the Barnes-Hut opening jiggle** is keyed on the preorder cell, not the arena slot
  (`docs/decisions/bh-jiggle-key.md`).
- **2026-10-03 — note code 7** (`dag.phase_skipped`) is deferred above `PRIORITY_NODE_BUDGET`
  (`docs/decisions/note-code-7-deferred.md`).
- **2026-10-04 — the sfdp spring-electrical iteration is in gather form**, the prolongation is not
  (`docs/decisions/sfdp-gather-form.md`).
- **2026-10-04 — the renderer scales by local spacing**, not by the bounding box
  (`docs/decisions/render-readable-spacing.md`).
- Compute tiers: tuned scalar, then SIMD, then threads, then GPU only on measurement; a GPU layout
  gets its own capability id. D10: gather-form kernels. CPU only, multi-threaded (user).
- Hierarchy policy is "repair + record"; note codes 1–3 in p3, 4–5 in p5, 6 reserved, 7 deferred.
- FA2 is a port of networkx 3.6; JAMA tred2/tql2 for n ≤ 256, scipy LOBPCG (block 4) above. The
  conformance row `FORCEATLAS2` runs `layout.forceatlas2.forcesim`, because SciGraphs runs ForceSim
  (`docs/decisions/layouts-igraph.md`; `sg-fa2-seed.md`).
- References are pinned read-only and a missing one is a STOP, never an improvisation.
- The user delegated every non-critical decision to "the recommended option" for autonomous runs.
