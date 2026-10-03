# Review `sg-spiral3d` before it lands

Branch `sg-spiral3d`, merged with `develop` (8 conflicts resolved by hand). Delta reviewed:
`git diff origin/develop...HEAD` — 25 files, +1784 / −456.

Everything below was run in this worktree on 2026-10-02. Commands and their real exit codes are in
**Commands run**; nothing is claimed that was not executed here.

## Verdict in one line

Every gate the branch touches is green, the merge is lossless, `layout.bipartite_3d` is untouched and
no existing layout's bytes changed. What blocks the landing is one house rule the branch breaks on
purpose — the new hash-gate stage has no negative control of its own — plus five stale line-number and
count claims in the docs this branch wrote. All six are small and all six are in the branch's own paths.

## Findings

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| S3-1 | MAJOR | `crates/graph-cli/src/hashgate/knobs.rs:127` | `layout.basic3d.spiral` is a hash-gate stage with **no per-stage negative control**, so the gate can never prove it reads this stage's bytes. `THREE_D_LAYOUT_STAGES` is still `[Stage; 5]` (sphere, helix, cube, hierarchical3d, spring3d). `hashgate/knob.rs:17-18` states the rule this table exists to enforce, and `AGENTS.md` ("Every gate row has a negative control that must fail"). | a stage whose bytes went constant or empty — e.g. `interp` returning `0.0`, or `columns` returning a zeroed `Vec` — still prints `4-way equal`, because "equal" only compares four arms with each other. | run: `layout.basic3d.spiral: 4-way equal on 8/8 seeds`, and no `GM_MUTATE_BASIC3D_SPIRAL_*` exists in `knobs.rs` or `Knob::ALL`. Disclosed at `docs/measurements/sg-spiral3d.md:142-166`; `layout.bipartite_3d` has the same hole. `prompts/jobs/knobs-3d-new.md` (on develop) writes the fix for both stages, but neither that job nor `sg-basic3d-spiral-oracle` is in `scripts/orch/queue.txt`. | Land `prompts/jobs/knobs-3d-new.md` before this branch, or accept S3-1 in writing as a named debt with the job queued. Add `spiral` (and `bipartite_3d`) to `THREE_D_LAYOUT_STAGES`, plus the coverage test that fails on any `LAYOUTS` id with no knob. |
| S3-2 | MINOR | `docs/measurements/sg-spiral3d.md:196` | "`capabilities --check` reports **70 rows**, 36 problems" — the post-merge number is **72**. Written before the merge, when the branch's `LAYOUTS` was `35 -> 36`; the merge brought develop's extra rows. | — | run: `capabilities --check: 72 rows, 36 problems`, exit 1. The **36** and the "none names `layout.basic3d.*`" claim both hold: `grep -i "basic3d\|spiral"` over the output finds nothing. | 70 → 72. |
| S3-3 | MINOR | `docs/measurements/sg-spiral3d.md:292` | "`LAYOUTS: 35 -> 36`" — it is **37 -> 38** after the merge. | — | `crates/graph-core/src/registry.rs:78` `pub static LAYOUTS: [Capability; 38]`; `git diff` shows `37` → `38` and the new entry appended last; 38 `id:` fields counted in the array body. | 35 -> 36 becomes 37 -> 38. |
| S3-4 | MINOR | `crates/graph-core/src/registry/three_d.rs:84` | `DEGRADATION` still reads "None of these **five** layouts refuses for any input of its own" — six rows are exported now. | — | `three_d.rs:88-94` exports six consts; the module doc at `three_d.rs:23-27` says so correctly ("Six rows now, not five") while the const below it was not updated. | five → six. |
| S3-5 | MINOR | `crates/graph-core/src/registry/three_d.rs:33`; `crates/graph-core/src/layout/basic_3d.rs:47` | "`BASIC_3D_CEILING`: the node count the **three** graph-free 3D placements were run at … all **three** layouts are `O(n)` in three `f64` columns" and "the dispatcher hands each of the **three** functions it calls". Four placements are graph-free now. | — | `basic_3d.rs:40-44` declares `sphere`/`helix`/`cube`/`spiral`; `dispatcher.py:105-106` reaches `_spiral_layout_3d`, so `SCALE` reaches four. Note S3-6's caveat: the "all are `O(n)`" sentence is now only true of the three that were measured, and `three_d/spiral3d.rs:79-84` says so itself. | "three" → "three, measured" (or four with the measurement named), and basic_3d.rs:47 "three" → "four". |
| S3-6 | MINOR | `crates/graph-cli/src/oracle_python/conformance/gaps.rs:39`, `:45` | Both `at:` fields point at `crates/graph-core/src/layout/basic_3d.rs:43`; `SCALE` is at **line 53**. Already stale on develop (`:44` there), and this branch made it worse by 10 lines. | — | `grep -n "pub const SCALE" crates/graph-core/src/layout/basic_3d.rs` → `53`; `git show origin/develop:…` → `44`. This branch edits `gaps.rs:39` on the adjacent line and could have fixed both. | 43 → 53 in both `at:` fields. |
| S3-7 | MINOR | `prompts/jobs/sg-basic3d-spiral-oracle.md:3-6` (on develop, not in this delta) | That follow-up job's *Why* says "`unproven.rs` routes it to the `oracle-basic-3d` record". On the merged tree it routes to `scigraphs-conformance` (`capabilities/registry/unproven.rs:180`), so the premise is stale. Its step 1 is still the right work. | — | `git diff origin/develop...HEAD -- …/unproven.rs` adds a dedicated arm at `:180`; `capabilities/tests/registry/ids.rs:35` adds `BASIC_3D_UNDIFFERENTIALLED`. | Not a branch defect. Rewrite the follow-up's *Why* before queueing it, or the job will move a row that is already correct. |
| S3-8 | MINOR | `scripts/orch/rows/p12-t3.rows:192` (not in this delta) | `five-new-layouts-are-appended` asserts the last five layout ids are sphere/helix/cube/hierarchical3d/spring3d. The tail is now spring3d, sfdp, forceatlas2.barnes_hut, bipartite_3d, basic3d.spiral, so the row would fail. | — | `awk` over `registry.rs`'s `LAYOUTS`: last six `id:` are hierarchical_3d, SPRING_3D_ID, sfdp, ForceAtlas2BarnesHut, bipartite_3d, spiral. Inert today: `scripts/orch/queue.sh:37` and `.github/workflows/floor.yml:75` run only `quick.rows`, which has no layout rows. | Update the row whenever `p12-t3.rows` is next run; not this branch's to fix. |

No BLOCKER. No weakened test, gap or gate row was found (see **Check 3**).

## Unverified

Listed here, never as findings, because no command in this task produced the evidence.

- **`compiled_base.c` line numbers.** `prompts/jobs/sg-spiral3d.md:24` asked for numpy's `interp` port to cite lines; `sg-spiral3d.md:76-80` records that numpy's C source is not under `$GM_SCRATCH/refs`. The `interp` formula in `spiral.rs:174-185` matches `arr_interp`'s shape as read and reported by the exploring subagent at tag v2.3.3, but **no line numbers are citable from this tree**, and the pinned in-tree words do not discriminate `slope*(x-x0)+y0` from the algebraically equal `y0+(y1-y0)*((x-x0)/(x1-x0))` at n = 1, 2, 7. The reproduction argument (200 000 monotone points, 0 ulp) is the real evidence and it is recorded in `sg-spiral3d.md:82-93`.
- **The `n = 0` divergence is unmeasured by this review.** `spiral.rs:62-76` and `sg-spiral3d.md:217-236` both claim `_spiral_layout_3d(0, 5.0)` returns shape `(1, 3)` and `apply_graph_layout` returns `False`. I did not run that; the conformance matrix's smallest fixture has 2 nodes, so nothing in this review observes it either way. It is a deliberate, documented divergence from the reference, in a module whose other three layouts are total — a judgement call, not a defect.
- **`spiral.rs:182` `partition_point(..) - 1` underflows on a NaN `x`.** Unreachable: `wanted_at` yields `0 <= wanted <= total`, and `axial = 2*SCALE` keeps `length` strictly increasing. Latent, not live. numpy returns NaN there; this would panic in debug and index out of bounds in release.
- **`scale_ceiling = BASIC_3D_CEILING` (1 000 000) is inherited, never measured for this layout.** Disclosed with a full Ponytail marker at `three_d/spiral3d.rs:73-86` and in `sg-spiral3d.md:257-265`. Honest, and correctly flagged as unmeasured.
- **`scripts/scigraphs-conformance.sh --break` was not run.** The job asked only for the hash-gate control; the conformance gate's own control targets `SPRING_3D` (`scigraphs-conformance.sh:44`), a row this branch does not touch.

## Check 1 — `spiral.rs` against `basic.py:36-63`, line by line

Reference read in the worktree: `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63` (`def
_spiral_layout_3d` at 36, `column_stack` returns at 61-63), reached from
`SciGraphs/core/scigraphs_core/mesh/layouts/dispatcher.py:105-106`, whose `apply_graph_layout`
signature at `:14` supplies `scale=5.0`. The job brief's `36-62` is off by one at the end; the code
cites `36-63`, which is right.

| reference line | what it does | the port | verdict |
|---|---|---|---|
| `:36` `def _spiral_layout_3d(num_nodes, scale, turns=None)` | `scale` is **required**, no default; `turns=None` | `basic_3d.rs:53` `pub const SCALE: f64 = 5.0`, read from `dispatcher.py:14`. `turns` cannot be supplied: `_call_with_props` (`layouts/common.py:80-85`) forwards `props` only to a callee with a `props` parameter and `_spiral_layout_3d` has none, so `spiral.rs:91-94` hard-coding the formula covers the whole reachable surface | correct |
| `:41` `turns = max(2, int(round(np.sqrt(n / (0.75*np.pi)))))` | half-to-even, then a floor, then truncation | `spiral.rs:92-93` `libm::sqrt(..).round_ties_even().max(2.0) as u32`. `round_ties_even` is half-to-even (not `floor(x+0.5)`); `as u32` truncates a non-negative value exactly as `int()` does | correct |
| `:43` `omega = 2.0*np.pi*turns` | left-assoc | `spiral.rs:98` `2.0 * PI * f64::from(turns(n))` | correct |
| `:45` `grid = np.linspace(0.0, 1.0, 1<<16)` | `step = delta/div`, `y = arange*step + start`, **and `y[-1] = stop`** | `spiral.rs:103-105` `1.0 / (GRID-1) as f64`; `spiral.rs:117-119` `grid_at(j, step) = j as f64 * step`. No endpoint fix-up branch — and none is needed: `65535.0 * (1.0/65535) == 1.0` exactly, so numpy's overwrite and the plain product are the same bits. Measured and pinned by `the_grid_last_point_is_the_product_not_a_fix_up` (`spiral/tests/structure.rs:238-248`) | correct |
| `:46-48` `speed = sqrt((0.5*s)**2 + (0.5*s*(1+grid)*omega)**2 + (2*s)**2)` | `0.5*s*(1+g)*w` is `((0.5*s)*(1+g))*w`; the three summands add left to right; `**2` is `x*x` in numpy's array fast path | `spiral.rs:123-128`: `radial = 0.5*SCALE`, `climb = radial*(1.0+grid)*omega`, `axial = 2.0*SCALE`, `libm::sqrt(r + c*c + a*c)` left-assoc. Same operand order, same association | correct |
| `:49` `step = grid[1] - grid[0]` | reads back its own arithmetic | `grid_at(1) - grid_at(0) == step` exactly | correct |
| `:50` `length = [0.0] + cumsum(0.5*(speed[1:]+speed[:-1])*step)` | **sequential**, not pairwise; per element `((0.5*(a+b))*step)` | `spiral.rs:135-147`: `0.0` pushed, then 65 535 additions in index order with `running += 0.5*(current+previous)*step`, `current = speed[j]`, `previous = speed[j-1]`. Same order, same association, D10 fixed-order reduction | correct |
| `:52-55` `wanted = linspace(0, length[-1], n)` if `n > 1` else `[0.5*length[-1]]` | the guard is `> 1`, so `n == 0` takes the *else* branch; the fix-up `y[-1] = stop` | `spiral.rs:156-164`: `n <= 1 → 0.5*total`, `i == n-1 → total`, else `i * (total/(n-1))` — numpy's `step = delta/div` then `start + i*step`, with `div == 0` never reached | correct |
| `:56` `t = np.interp(wanted, length, grid)` | `xp = length`, `fp = grid`; `j` = last index with `xp[j] <= x`; `slope = (fp[j+1]-fp[j])/(xp[j+1]-xp[j])`; result `slope*(x-xp[j]) + fp[j]`; outside → `left`/`right` defaults `fp[0]`/`fp[-1]`; `j == lenxp-1` → `fp[j]` without a slope | `spiral.rs:174-185`: `x < length[0] → grid_at(0)`; `x >= length[last] → grid_at(last)`; `j = length.partition_point(|v| *v <= x) - 1`; `slope = (grid_at(j+1)-grid_at(j)) / (length[j+1]-length[j])`; `slope*(x-length[j]) + grid_at(j)`. `>=` at the top end is numpy's own `j == lenxp-1` branch, not a shortcut; both return `1.0` here (`sg-spiral3d.md:95-97`). The exact-hit case (`dx[j] == x`) numpy short-circuits; the port computes `slope*0.0 + grid_at(j)`, which is the same bits for a finite `slope` | correct (caveat above on the NaN path) |
| `:58-63` `angle = t*omega; radius = scale*0.5*(1.0+t); column_stack((radius*cos(angle), radius*sin(angle), scale*(2.0*t-1.0)))` | `scale*0.5*(1+t)` is `(scale*0.5)*(1+t)` | `spiral.rs:211-215`, `columns` is the reference's own order: `let angle = t*omega; let radius = SCALE*0.5*(1.0+t);`, then `radius*libm::cos(angle)`, `radius*libm::sin(angle)`, `SCALE*(2.0*t-1.0)` | correct |

**D1-D3.** Every transcendental goes through `libm`: `libm::sqrt` at `spiral.rs:92,127`,
`libm::cos`/`libm::sin` at `spiral.rs:213-214`. `grep -rn "mul_add\|powi\|powf\|target_feature\|simd128"`
over `spiral.rs` and `spiral/` returns **nothing**. `core::f64::consts::PI` is a bit-exact copy of
`np.pi`. `round_ties_even` (`spiral.rs:93`) is a bit operation, not a transcendental. Nothing is
narrowed to `f32` until `basic_3d.rs:61`, once, in `in_space`.

**Complexity.** The 65 536-entry table is a **constant, built once per call**, not per node:
`parameters` (`spiral.rs:192-199`) calls `arc_length` once and `columns` (`:210`) calls `parameters`
once, so there is no `O(n · 65536)` anywhere. Per node it is one `partition_point` over 65 536
elements (16 comparisons) plus three columns. The ledger says exactly this —
`three_d/spiral3d.rs:68-70` declares `O(n + 2^16)` rather than `O(n)`, and it is true of the code.

## Check 2 — registry: order and `Metadata`

`layout.basic3d.spiral` is **appended at the end** of `LAYOUTS`, after `layout.bipartite_3d`:

```text
crates/graph-core/src/registry.rs:78   pub static LAYOUTS: [Capability; 38] = [
...
crates/graph-core/src/registry.rs:269       id: basic_3d::bipartite_3d::ID,
crates/graph-core/src/registry.rs:277       id: basic_3d::spiral::ID,      <- last, index 37
```

`git diff origin/develop...HEAD -- crates/graph-core/src/registry.rs` is two lines of substance:
`[Capability; 37]` → `[Capability; 38]` and one appended `Capability`. Nothing moved. The
index-mapped consumer `crates/graph-wasm/src/exports/build.rs:23,32,166`
(`gm_layout_count`, `gm_layout_id`, `gm_run`'s `LAYOUTS.get(layout_id)`) therefore keeps every index
below 37 unchanged, and the id string is the module's own `pub const ID`
(`spiral.rs:58`), never a spelling copied into the registry. `hashgate --seeds 8` printing
`layout.basic3d.spiral: 4-way equal on 8/8 seeds` is independent proof the wasm arm reached it.

**All nine `Metadata` fields** (`crates/graph-core/src/registry/capability.rs:11-30`; the struct has
no `Option` field, so nothing can be omitted), from `crates/graph-core/src/registry/three_d/spiral3d.rs:25-103`:

| field | value | true of the code? |
|---|---|---|
| `tier: 1` (`:26`) | — | consistent with `Status::Implemented` in `capabilities/registry/unproven.rs:187` |
| `stage: "layout"` (`:27`) | — | same as `SPHERE`/`HELIX` (`three_d/basic.rs:11,44`) |
| `nodes: Point` (`:28`), `edges: Line` (`:29`) | — | `in_space` emits `NodeGeometry::Point` + `EdgeGeometry::Line` (`basic_3d.rs:63-71`); siblings agree |
| `oracle` (`:30-67`) | `_spiral_layout_3d`, `basic.py:36-63`, `dispatcher.py:105-106`, `scale=5.0` | **every internal line cite checks out against the submodule** — turns `:41`, omega `:43`, grid `:45`, speed `:46-48`, cumsum `:50`, guard `:52`, wanted `:52-55`, interp `:56`, coordinates `:58-63` |
| `complexity: "O(n + 2^16)"` (`:68-70`) | 65 536-entry table once per call, one binary search per node | verified above; honest, **not** claimed as `O(n)` |
| `scale_ceiling: BASIC_3D_CEILING` (`:71`) | 1 000 000, inherited | not measured for this layout; carries a full `Ponytail:` (`:73-86`) saying so |
| `degradation: DEGRADATION` (`:72`) | — | shared verbatim with all five siblings |
| `ponytail` (`:73-102`) | four markers, each with failing input / direction / escape hatch | ceiling, the `n = 1..5` turn floor, the `n == 1` half-arc, the 512 KiB table |

Shape matches the siblings exactly (`three_d/basic.rs:9,42,74`); only the three free-form fields
differ, which is right.

**`capabilities --check`** — last lines, verbatim:

First run of this review, before any gate had written a record into `target/gates/` — last lines:

```text
  layout.packing.osage: gated, but no hashgate record: run the gate
  layout.packing.osage: gated, but no oracle-osage record: run the gate
  transport.wasm.columnar: gated, but no hashgate record: run the gate
  transport.wasm.columnar: gated, but no hashgate record: run the gate
capabilities --check: 72 rows, 36 problems
```

Exit **1**. None of the 36 names `layout.basic3d.*` (`grep -i "basic3d\|spiral"` over the output:
nothing). All 36 are the whole-ledger "run the gate" class on capabilities this branch never touches.

Re-run after `hashgate --seeds 8` had written its record: still `72 rows, 36 problems`, exit **1**, the
message classes shifting from 36 × "no record" to the split `sg-spiral3d.md:201-205` predicts —
19 × `gated, but hashgate ran 8 seeds, need 1000` and 17 × `gated, but no <record>: run the gate`
(8 oracle-diff, 2 oracle-layouts, 1 oracle-osage, 2 oracle-spectral, 4 roundtrip). That document
reports the situation honestly rather than as a pass; its row **count** is stale — see S3-2.

**`codegen --check`** — last lines, verbatim:

```text
  up to date  crates/graph-contract/generated/snapshot-header.schema.json
  up to date  crates/graph-contract/generated/snapshot-header.d.ts
  up to date  docs/contract/snapshot-schema.json
  up to date  docs/contract/ingest-schema.json
```

Exit **0**. All four outputs up to date, and none is layout-specific, so a new `layout.basic3d.*` id
produces no codegen diff — as predicted before the change.

## Check 3 — the hand-resolved merge, file by file

| file | keeps develop's intent | keeps the branch's intent | note |
|---|---|---|---|
| `registry/three_d.rs` + `three_d/{basic,graph,spiral3d}.rs` | yes, lossless | yes | every `Metadata` block is byte-identical to develop's `three_d.rs`. `SPHERE` differs only by `pub(super)` → `pub` (`three_d/basic.rs:9`), required because the const moved into a private child and is re-exported as `pub(super)` at `three_d.rs:92` — same effective visibility. `SPRING_3D`'s one path rebind, `super::force::SPRING_CEILING` → `super::super::force::SPRING_CEILING` (`three_d/graph.rs:99`), is the mandatory extra module level, not a value change. `SPIRAL_3D` added at `three_d/spiral3d.rs:25`. No develop id missing, none duplicated. Stale prose: S3-4, S3-5 |
| `layout/basic_3d.rs` | yes | yes | `bipartite_3d`/`cube`/`helix`/`sphere` `mod` lines and all four wrappers are byte-identical; `pub mod spiral;` appended. `sphere` and `spiral` wrappers carry `debug_assert_eq!(dim, Dim::D3)`; `helix`/`cube`/`bipartite_3d` were left exactly as develop had them |
| `capabilities/tests/registry.rs` → `registry/routing.rs` | yes | yes | `the_registry_covers_every_oracle_function_once_its_ids_are_unique` reappears **verbatim** in `routing.rs:19-162`, branch for branch, every Graphviz comment block included. All 7 `fn`s of develop's `registry.rs` are present (6 still there, `records_of` now `pub(super)` at `registry.rs:17`). One branch added, `routing.rs:61-66` |
| `capabilities/tests/registry/ids.rs` | yes | yes | `IGRAPH`/`BASIC_3D` widened `pub(super)` → `pub(crate)` (needed by the new sibling) and `BASIC_3D_UNDIFFERENTIALLED = ["layout.basic3d.spiral"]` added at `:35`. `BASIC_3D` still names three ids, which is right: `oracle-basic-3d`'s `ARMS` covers sphere/helix/cube only |
| `hashgate/tests/report.rs` | yes | yes | one key inserted, `"layout.basic3d.spiral":3`, in correct lexicographic position in the golden `equal` map. `arm_names`/`arms`/`seeds`/`mutation`/`transport` untouched — the arm count is independent of the stage count |
| `snapshot_cmd/tests.rs`, `snapshot_cmd/roundtrip/tests.rs` | yes | yes | `"snapshots":195` is right: `snapshot_total(seeds) = seeds * (1 + LAYOUTS.len())` (`roundtrip.rs:66-68`) and `LAYOUTS.len() == 38`, so `5 * 39 = 195`. The `layout_names` golden grew by the two tail names and does not collide with the existing `spiral` / `layout.spiral` pair |
| `oracle_python/conformance/gaps.rs` | yes | yes | `G_BASIC3D_SCALE.note` gained `spiral`. Both dropped consts stay — other rows name them, and a const nothing names is a false record. Stale `at:`: S3-6 |
| `oracle_python/conformance/rows.rs`, `baseline/table/networkx.rs` | yes | yes | `SPIRAL_3D` remapped `layout.spiral` → `layout.basic3d.spiral`; gaps `[G_NO_ITERATIONS, G_SNAPSHOT_SCALE]` → `[G_BASIC3D_SCALE]`, exactly what `rows.rs:139-142` already does for develop's `BIPARTITE_3D` and what `:99`/`:117` do for `SPHERE`/`HELIX`. The dropped `G_NO_ITERATIONS` pointed at `layout/spiral.rs:63`, the planar module this row no longer names. Baseline re-pinned: motor hash `1882ccdf…` → `57bf83de…`, tier `1e0/"shape"/"algorithm"` → `1e-15/"tolerance"/"arithmetic"`, and the **reference** hash `94502f44…` left alone. Not weakened: a standing "different algorithm" admission was replaced by a claim of exact agreement, and the run below is what backs it |

**`layout.bipartite_3d` (landed on develop) is untouched:**

```text
$ git diff origin/develop...HEAD --stat -- \
    crates/graph-core/src/layout/basic_3d/bipartite_3d.rs \
    crates/graph-core/src/registry/three_d/bipartite_3d.rs
(empty)
```

No existing layout's *implementation* changed anywhere in the delta — the only edits to
`basic_3d.rs` are the module doc, `pub mod spiral;`, and the new wrapper (verified from the full
diff), and `registry.rs` is purely additive. That is the structural reason no existing stage hash can
have moved.

**No gap, test or expectation was weakened to get green.** Every deleted test body was relocated
verbatim; every `Metadata` block is byte-identical; the one golden that changed
(`hashgate/tests/report.rs:70`) gained a key and lost nothing.

## Check 4 — hash gate and its negative control

```text
$ scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8
...
  layout.basic3d.sphere: 4-way equal on 8/8 seeds
  layout.basic3d.helix: 4-way equal on 8/8 seeds
  layout.basic3d.cube: 4-way equal on 8/8 seeds
  layout.basic3d.spiral: 4-way equal on 8/8 seeds
...
PASS
-> exit 0
```

```text
$ scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q --release -p graph-cli -- hashgate --seeds 8
...
  layout.basic3d.spiral: 4-way equal on 8/8 seeds
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
-> exit 1
```

The control fails as it must, and it is the right control for "do the seeds bite at all". Note what
it is **not**: it perturbs the shared reference model, so it moves every stage at once and names none
of them. `layout.basic3d.spiral` has no control of its own — that is S3-1, and
`sg-spiral3d.md:158-166` says so without dressing it up.

**Did any existing layout's stage hash move? No**, on two independent grounds:

1. **Source.** No existing layout's numeric code is in the delta (above), and `LAYOUTS` grew by one
   appended entry, so every index below 37 — and therefore every `gm_layout_id(i)` the SDK hands out —
   is unchanged.
2. **Run.** All 37 pre-existing stages still report `4-way equal on 8/8 seeds`, native×2 against
   wasm32×2, and the gate exits 0.

Honest limit on (2): hashgate compares the four arms of one run; it does **not** compare against a
cross-branch pinned baseline (`target/gates/` was empty when this review started, and
`capabilities --check` reports "no hashgate record" for 19 rows for exactly that reason). So (2) is
D1/D10 evidence, not a develop-to-branch hash diff. Ground (1) is what settles the question here, and
it is sound because the delta is additive.

The one new *thing* the stage brings to the gate — a 65 536-entry `f64` table and a
`partition_point` binary search, either of which could in principle be target-dependent — is
genuinely exercised: all four arms digest the same.

## Check 5 — the conformance gate

```text
$ scripts/scigraphs-conformance.sh
...
  SPHERE: ok — 111 f64, 1020 f32 of 1020 coordinates, median 4.630e-16 <= 1.000e-15
  SPECTRAL_3D: ok — 1 f64, 1 f32 of 1020 coordinates, median 3.333e-1 <= 1.000e-1
  SPIRAL_3D: ok — 120 f64, 1020 f32 of 1020 coordinates, median 3.342e-16 <= 1.000e-15
  HELIX: ok — 327 f64, 1020 f32 of 1020 coordinates, median 1.372e-16 <= 1.000e-15
  ...
PASS
-> exit 0
```

**`SPIRAL_3D` reaches f32 1020/1020** and stops at the **tolerance** tier (median 3.342e-16 ≤ the
row's 1.000e-15). The reason is written in `docs/measurements/sg-spiral3d.md:99-105`: `x` and `y` are
`libm`'s `sin`/`cos` against numpy's array loops, two implementations that differ by up to 1 `f64`
ulp and vanish in the `f32` narrowing. The `f64` 120/1020 is not the blocker — the motor ships `f32`
end to end. `SPHERE` and `HELIX` sit in exactly the same position and carry the same tier, which is
the consistency check.

**No other row moved.** The judge exits 0 against the pinned baseline table, and the neighbours match
`docs/measurements/scigraphs-conformance.md` row for row: `SPHERE` 111/1020 f64, `HELIX` 327/1020,
`CUBE` 501/1020, `HIERARCHICAL_3D` 374/1020, `BIPARTITE_3D` 480/1020 f64 and 1020/1020 f32 — the
develop-landed row is intact. **No FIX.**

Matrix row 14 and repair 10 are rewritten correctly. Repair 10 in particular retracts the wrong
diagnosis in both halves — SciGraphs never calls `nx.spiral_layout`, so no `resolution` could have
moved the row, and the right diagnosis ("three columns, no `z`") still does not make it a `resolution`
edit.

## Check 6 — house rules, Ponytail markers, negative controls

| rule | result |
|---|---|
| ≤40 lines per function | **pass.** Longest in `spiral.rs` is `columns` at 16 lines (`:203-218`); then `arc_length` 13, `interp` 12, `wanted_at` 9, `parameters` 8, `speed_at` 6, `turns`/`omega`/`grid_step`/`grid_at`/`run` 4 |
| ≤300 lines per file | **pass.** `spiral.rs` 221, `tests.rs` 25, `reference.rs` 177, `structure.rs` 248, `degenerate.rs` 97, `three_d.rs` 94, `three_d/basic.rs` 115, `graph.rs` 124, `spiral3d.rs` 103 |
| ≤4 parameters | **pass.** Max arity 3 (`wanted_at`, `interp`) |
| `Ponytail:` on every heuristic | **pass for the ledger** — `three_d/spiral3d.rs:73-102` carries four markers, each with failing input, direction and escape hatch, and the `n = 0` divergence at `spiral.rs:62-76` is spelled out with the same three parts. **Thin** in two test-only places, which the house rule exempts as heuristics about the code: the 1-ulp / last-bit tolerances at `spiral/tests/reference.rs:122-146,148-177` and the `1e-12` fudge factors at `structure.rs:99,103` and `degenerate.rs:65,67` carry no marker. Test tolerances, not shipped heuristics — no finding |
| negative control on every new gate row | **FAIL — S3-1.** The only new gate row is the hash-gate stage `layout.basic3d.spiral`; it has no per-stage control. No new row was added to any `scripts/orch/rows/*.rows` file (the delta touches none). The remapped conformance row is covered by the gate's existing `--break` control, which targets `SPRING_3D` |
| D1-D10 | **pass.** libm-only; fixed-order sequential cumsum; `partition_point` over a `Vec`, never a `HashMap`; gather form for `t` (`spiral.rs:191-199`); no wall-clock, no RNG (so no seed owed, correctly stated); `usize` used only as a local index, never on the wire; bit-identical native vs wasm32 confirmed by the 4-way hash sweep |
| `cargo fmt --check` | exit **0** |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit **0** |
| tests | `cargo test -p graph-core --lib basic_3d` → **63 passed, 0 failed**; `cargo test -p graph-cli --bins` → **329 passed, 0 failed** |

One unverified gap the branch itself records at `sg-spiral3d.md:145-166`: none of the new tests
carries a **per-test** negative control. The pinning tests are genuinely falsifiable — the `t` and `z`
words at `reference.rs:106,132,157` were reproduced independently from a from-scratch implementation
and match 10/10 — but a handful are self-referential (`structure.rs:20` recomputes the identical
expression `spiral.rs:215` uses; `structure.rs:213` asserts `grid_step == 1/(GRID-1)`). House rule
asks for a negative control per gate *row*, not per assertion, so this is not a finding; it is the gap
`knobs-3d-new.md` step 2 is meant to close.

## Counts

| severity | count |
|---|---|
| BLOCKER | 0 |
| MAJOR | 1 (S3-1) |
| MINOR | 7 (S3-2 … S3-8; S3-7 and S3-8 are outside the delta) |
| unverified | 5 |
| checks run | 6 of 6, all with command output above |
| commands run | 12, exit codes in the table below |
| modules in scope and their finding counts | `layout/basic_3d/spiral.rs` + its 4 test files: **0** (check 1, check 6) · `registry.rs` + `registry/three_d{,/basic,/graph,/spiral3d,/bipartite_3d}.rs`: **4** (S3-4, S3-5; S3-2, S3-3 by reference) · `layout/basic_3d.rs` + tests: **1** (S3-5) · `capabilities/tests/registry{,/routing,/ids}.rs`: **0** · `capabilities/registry/unproven.rs`: **0** · `hashgate/{tests/report,knobs}.rs`: **1** (S3-1) · `snapshot_cmd/{tests,roundtrip/tests}.rs`: **0** · `oracle_python/conformance/{rows,gaps,baseline/table/networkx}.rs`: **1** (S3-6) · `docs/measurements/sg-spiral3d.md` + `scigraphs-conformance.md`: **2** (S3-2, S3-3) · `prompts/jobs/sg-basic3d-spiral-oracle.md`: **1** (S3-7, on develop) · `scripts/orch/rows/p12-t3.rows`: **1** (S3-8, not in the delta) |

## Commands run

| command | exit |
|---|---|
| `scripts/orch/gr cargo build -q --release -p graph-cli` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- capabilities --check` | 1 (72 rows, 36 problems, 0 naming `basic3d`) |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check` | 0 |
| `scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8` | 0 |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q --release -p graph-cli -- hashgate --seeds 8` | 1 (`FAIL: 8 of 8 seeds diverge`) |
| `scripts/scigraphs-conformance.sh` | 0 (`PASS`, 32/32 rows) |
| `scripts/orch/gr cargo test -q -p graph-core --lib basic_3d` | 0 (63 passed) |
| `scripts/orch/gr cargo test -q -p graph-cli --bins` | 0 (329 passed) |
| `scripts/orch/gr cargo fmt --check` | 0 |
| `scripts/orch/gr cargo clippy -q --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo test -q -p graph-cli --lib` | 101 — no library target in `graph-cli`; re-run as `--bins` |

Not run, and not claimed: `scripts/scigraphs-conformance.sh --break`; `hashgate --seeds 1000` and
every other timed gate (the orchestrator owns those).

## What to change

Numbered, all inside the paths the branch already owns except where noted.

1. **`crates/graph-cli/src/hashgate/knobs.rs:127`** — add `layout.basic3d.spiral` **and**
   `layout.bipartite_3d` to `THREE_D_LAYOUT_STAGES` with their `GM_MUTATE_*_NODES` controls and the
   `hashgate-control-*` records, plus the test that fails on any `registry::LAYOUTS` id with no knob.
   `prompts/jobs/knobs-3d-new.md` on develop already writes this job; queue and run it before this
   branch, or record S3-1 as an accepted, named debt with that job in `scripts/orch/queue.txt`.
   *Recommended: run the job — it closes a hole this branch inherits as well as the one it opens.*
2. **`docs/measurements/sg-spiral3d.md:196`** — `70 rows` → `72 rows`.
3. **`docs/measurements/sg-spiral3d.md:292`** — `LAYOUTS: 35 -> 36` → `LAYOUTS: 37 -> 38`.
4. **`crates/graph-core/src/registry/three_d.rs:84`** — "None of these **five** layouts" → "six".
5. **`crates/graph-core/src/registry/three_d.rs:33`** and
   **`crates/graph-core/src/layout/basic_3d.rs:47`** — "the **three** graph-free 3D placements /
   all **three** layouts are `O(n)`" → name the three that were measured, and "each of the **three**
   functions it calls" → four. (`three_d/spiral3d.rs:79-84` already records that the fourth is *not*
   `O(n)` in its constant term; the parent doc should not contradict its own child.)
6. **`crates/graph-cli/src/oracle_python/conformance/gaps.rs:39` and `:45`** — both `at:` fields,
   `basic_3d.rs:43` → `basic_3d.rs:53`. Already stale on develop; this branch edits the adjacent line.
7. *(outside the delta, for the orchestrator)* **`prompts/jobs/sg-basic3d-spiral-oracle.md:3-6`** —
   its *Why* claims `unproven.rs` routes the spiral to `oracle-basic-3d`; on the merged tree it routes
   to `scigraphs-conformance`. And **`scripts/orch/rows/p12-t3.rows:192`** would now fail; it is
   inert because only `quick.rows` runs.

Items 2-6 are ten minutes of prose. Item 1 is the only one with a reason to hold the landing.

VERDICT: FIX