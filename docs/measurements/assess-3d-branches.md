# assess-3d — what is left to land from `p12-t4a` and `sg-igraph-clean`

Read-only assessment against `origin/develop` (`bccdf383`). No checkout, no combine, no build.

Refs: `p12-t4a` = `971318dc` (2026-10-01), `sg-igraph-clean` = `06ad8ea1` (2026-10-03),
`develop` = `bccdf383` (2026-10-04). Every "dev sha" below is the last-change sha on
`origin/develop` for that path.

## Method, and two tools I could not run

`opencode.json:29,38` deny `fetch` and the whole `merge` family, which also blocks `merge-base`.
Consequences, stated so they are not mistaken for measurements:

- **No fetch.** The assessment uses the already-present remote-tracking refs. If origin moved
  after the last fetch, the develop tip cited here is stale. UNVERIFIED.
- **No `merge-tree`.** Conflict sets are derived instead: for each branch I intersected
  `diff --name-only <base> <branch>` with `diff --name-only <base> origin/develop`. A path in both
  sets is modified on both sides and is a conflict candidate. This **over-reports** relative to
  `merge-tree` (adjacent-but-disjoint hunks in one file combine cleanly) and does **not** do rename
  or copy detection, so it can under-report a delete-vs-modify clash. Treat the lists below as
  candidates, not as a resolved conflict list.
- **Bases.** `rev-list --boundary` emits several boundary commits (criss-cross history), so the
  newest common ancestor was picked by max committer date over the `rev-list` intersection.
  `p12-t4a` → `bdb04f15`; `sg-igraph-clean` → `02d5c2ad` (a second candidate `146db521` sits 6 s
  earlier). Using the wrong `p12-t4a` base (`713f0771`, which `--boundary` lists last) inflates its
  changed set from 40 to 42 files by adding two bookkeeping files.

Changed-set sizes: `p12-t4a` 40 files, `sg-igraph-clean` 33.
Neither branch is recorded as landed in `docs/reports/STATUS.md:115`.

---

## Branch 1 — `p12-t4a` (971318dc)

Conflict candidates: **26** of 40. Develop has churned 1573 files since the base, so the branch is
stale across the board.

| file | class | evidence |
|---|---|---|
| `crates/graph-core/src/registry.rs` | MISSING (in this form) | dev sha `c89e8155` moved `LAYOUTS` to `registry/layouts.rs:47` (`[Capability; 47]`); t4a patches `registry.rs:97` (`[Capability; 33]`). The append target is gone. |
| `crates/graph-core/src/registry/spectral.rs` | PARTIAL | dev `fc816c3f` already has `SPECTRAL_3D_LAYOUT` / `PIVOT_MDS_3D_LAYOUT` (`spectral.rs:144-157`). t4a adds the same two entries under different ids plus `*_3D_CEILING` aliases (`t4a registry/spectral.rs:78,80`). |
| `crates/graph-core/src/registry/closed_form.rs` | PARTIAL + MISSING | dev `f9a74776`. `SPIRAL_3D` / `BIPARTITE_3D` duplicate develop's `layout.basic3d.spiral` (`layout/basic_3d/spiral.rs:58`) and `layout.bipartite_3d` (`basic_3d/bipartite_3d.rs:58`). `RANDOM_3D` has no develop counterpart. |
| `crates/graph-core/src/registry/tests.rs` | MISSING | dev `f066f41a`. Both new tests (`t4a tests.rs:42-92`) pin `LAYOUTS[..28]` / `ids[28..]`, an index assumption invalid on develop's 47-entry table. |
| `crates/graph-cli/src/capabilities/registry/layout_row.rs` | MISSING | dev `44b6650a`. `SCIPY_ORACLE_LAYOUTS` 2→4 is a genuinely new row set. |
| `crates/graph-cli/src/oracle_python/spectral.rs` | MISSING | dev `d970ac61`. Adds rows `layout.spectral.3d`, `layout.mds.pivot.3d` and `CEILING_SPECTRAL_3D`. |
| `crates/graph-cli/src/oracle_python/closed_form.rs` | MISSING | dev `d90aeec1`. Adds `CEILING_3D_COORDS=1e-7`, `CEILING_3D_RANDOM=0.5`, 3 ceiling rows. |
| `harness/oracle-spectral.py` | MISSING | dev `cc5487da`; pattern `3d` on `origin/develop -- harness/oracle-spectral.py` = **0 hits**. Branch threads `dims` through `pivot_spectrum`, the component-coordinate helpers and the degeneracy rule. |
| `harness/oracle-closed-form.py` | MISSING | dev `cc5487da`. Adds `random_3d`, `spiral_3d`, `bipartite_3d` and a shared xyz `block()` reader. |
| `crates/graph-core/src/layout/spectral/space.rs` (+ `space/tests.rs`) | MISSING | absent from develop (no last-change sha). New eigenspace/space module for 3-D. |
| `crates/graph-core/src/layout/spectral/tests/dims_3d.rs` | MISSING | absent from develop. |
| `crates/graph-core/src/layout/spiral/spiral_3d.rs` (+ `tests.rs`) | PARTIAL / duplicate | path absent from develop, but the algorithm is develop's `layout.basic3d.spiral`. |
| `crates/graph-core/src/layout/random.rs` | MISSING (the real increment) | dev `11442d22`; develop has **no** random-3D arm at all. `ID_3D = "layout.random.3d"` at `t4a random.rs:30`. |
| `crates/graph-core/src/layout/{bipartite,spiral,coords,pivot_mds,spectral,spectral_stage}.rs` | PARTIAL / duplicate | dev `1f09ccfd`,`5e5c4aab`,`5e5c4aab`,`42bcd892`,`c89e8155`,`42bcd892` — all carry 3-D work for the same layouts, living in `registry/three_d/` rather than `layout/{spiral,bipartite}.rs`. |
| `crates/graph-core/src/linalg/lobpcg/{ops,tests}.rs` | MISSING | dev `d130fe29`. Branch adds a 3-D block-conjugate-gradient path; the `diag_block` example is its evidence. |
| `crates/graph-core/examples/diag_{block,lobpcg,seed}.rs` | MISSING | absent from develop. Diagnostic examples for the above. |
| `crates/graph-cli/src/oracle_python.rs`, `oracle_python/tests.rs`, `tests/cli_ledger.rs`, `capabilities/tests/{mod,ledger,registry,stages}.rs`, `snapshot_cmd/tests.rs`, `snapshot_cmd/roundtrip/tests.rs`, `hashgate/tests/report.rs` | PARTIAL (stale literals) | dev `997ba55a`,`9b0564ec`,`c020ff08`,`fc816c3f`,`44b6650a`,`f066f41a`,`fc816c3f`,`c89e8155`,`c89e8155`,`c89e8155`. Branch pins `snapshots` counts off a 145-base tree; develop is at 185. Regenerate, do not hand-combine. |
| `docs/measurements/p12-t4a.md`, `scripts/orch/rows/p12-t4a.rows` | MISSING | absent from develop, nothing supersedes them. `scripts/orch/queue.txt:32` still points p12-t4a at `quick.rows`. |
| `docs/measurements/scigraphs-coverage.md` | PARTIAL / branch is fresher | dev `997ba55a`; branch rewrites develop's 2-D-only claims (28→33 layouts, "12"→"17"). Develop's own copy at line 66 already flags the t4a arms as "not on this tree". |

---

## Branch 2 — `sg-igraph-clean` (06ad8ea1)

Conflict candidates: **18** of 33.

| file | class | evidence |
|---|---|---|
| `crates/graph-core/src/layout/force/fr_kernel.rs` | MISSING | absent from develop. Develop inlines the equivalent in `force/fruchterman_reingold.rs` (dev `997ba55a`). Branch version is const-generic `layout<const D>` (`fr_kernel.rs:41`). |
| `crates/graph-core/src/layout/force/kk_kernel.rs` + `{springs,gradient,step}.rs` | MISSING | absent from develop. Re-derives develop's `force/kamada_kawai/solve.rs` + `start.rs` (dev `8bcde7e9`, `MAX_DIM = 3`). |
| `crates/graph-core/src/layout/force/fruchterman_reingold_3d.rs` (+ `tests.rs`) | PARTIAL / duplicate | path absent, but the capability is already registered as `layout.force.fruchterman_reingold.3d` (`registry/arms_3d.rs:224` → `fruchterman_reingold.rs:79 run_3d`, dev `997ba55a`). |
| `crates/graph-core/src/layout/force/kamada_kawai_3d.rs` (+ `tests.rs`) | PARTIAL / duplicate | path absent, capability already registered as `layout.force.kamada_kawai.3d` (`arms_3d.rs:230` → `kamada_kawai.rs:77 run_3d`, dev `c89e8155`). |
| `crates/graph-core/src/layout/force/{fruchterman_reingold,kamada_kawai}.rs` | CONFLICTING rewrite | branch `+15/-119` and `+13/-149` against dev `997ba55a` / `c89e8155`. Same 2-D ids, different internals. |
| `crates/graph-core/src/layout/force/mod.rs` | PARTIAL | dev `f5f7a068`; adds four module declarations and two re-exports. |
| `crates/graph-core/src/registry.rs` | MISSING (in this form) | dev `c89e8155` moved `LAYOUTS` to `registry/layouts.rs:47` (`[47]`); branch patches `registry.rs:79` (`[38]`). |
| `crates/graph-core/src/registry/igraph.rs` | PARTIAL | dev `ab8dc721`; branch adds `Metadata` for the two new arms (`igraph.rs:54`, `:80`). |
| `crates/graph-cli/src/oracle_python/igraph.rs` | PARTIAL | dev `8bcde7e9`; branch adds the two `_3d` rows and a `columns_3d` branch. Develop already carries `DIM` / `initial_3d` (`harness/oracle-igraph.py:175,187`) but no `_3d` key. |
| `harness/oracle-igraph.py` | PARTIAL / **corrects develop** | dev `ba3e5fab`. Branch adds `REFERENCES` entries at `dim=3` (`:59-60`) and `start_layout(initial, dim)` (`:115` refuses a `_3d` id with no z column). |
| `crates/graph-cli/src/oracle_python/conformance/motor/fit.rs` (+ `fit/tests.rs`) | MISSING | absent from develop. SciGraphs `_igraph_fit_positions` (`igraph_layouts.py:24-42`) reimplemented; `FITTED` = FR_3d, KK_3d, drl, lgl. |
| `crates/graph-cli/src/oracle_python/conformance/gaps.rs` | PARTIAL / **corrects develop** | dev `59f3c682`. Refutes develop's `G_IGRAPH_SEED` note — python-igraph installs stdlib `random` as its RNG, so the obstacle is licence, not unreachability. Adds `G_IGRAPH_FIT`, `G_KK_NON_FINITE`, `G_DRL_NO_3D`. |
| `crates/graph-cli/src/oracle_python/conformance/{motor,rows}.rs`, `conformance/baseline/table/networkx.rs`, `capabilities/registry/unproven.rs`, `capabilities/tests/registry/ids.rs`, `oracle_python/tests.rs`, `snapshot_cmd/tests.rs`, `snapshot_cmd/roundtrip/tests.rs`, `hashgate/tests/report.rs` | PARTIAL (id + count literals) | dev `8f3cef95`,`8f3cef95`,`42bcd892`,`c89e8155`,`f462c696`,`9b0564ec`,`c89e8155`,`c89e8155`,`c89e8155`. Re-keys to `_3d` ids; count literals stale. |
| `docs/decisions/layouts-igraph.md`, `docs/layouts/layout.force.{fruchterman_reingold,kamada_kawai}.md` | PARTIAL / branch is superset | dev `82380394`. All three are **append-only, 0 deletions** (+14, +14, +33). Develop already carries `docs/layouts/` entries for both layouts. |
| `docs/measurements/scigraphs-conformance.md` | PARTIAL / branch is fresher | dev `8f3cef95`; +50/-13, drops superseded matrix rows and the claim about igraph RNG seeding. |
| `docs/measurements/sg-igraph-dims.md` | MISSING | absent from develop. Existence and last-change sha only — body not read (branch-name exclusion). |

`crates/graph-core/src/registry/arms_3d.rs` is **not** in either changed set. It is absent from both
bases, so develop's copy survives any combine; the two-dot `D` entries for it are a direction
artifact, not a deletion by the branch.

---

## Overlaps

### `p12-t4a` vs develop's spectral3d / pivot3d / arms_3d

| | `p12-t4a` | develop |
|---|---|---|
| spectral 3-D | `layout.spectral.3d` (`registry.rs:259`) | `layout.spectral3d` (`registry/spectral.rs:145`) |
| pivot MDS 3-D | `layout.mds.pivot.3d` (`registry.rs:264`) | `layout.mds.pivot3d` (`registry/spectral.rs:153`) |
| run function | `spectral_stage::spectral_3d`, `spectral_stage::pivot_mds_3d` | **the same two functions** |
| ceiling | `SPECTRAL_3D_CEILING = SPECTRAL_CEILING`, `PIVOT_MDS_3D_CEILING = PIVOT_MDS_CEILING` (`t4a registry/spectral.rs:78,80`) | `scale_ceiling: SPECTRAL_CEILING`, `PIVOT_MDS_CEILING` — the same values |

**Same algorithm, same run function, different id string**, for both. Four of t4a's five arms are
duplicates by content; only `layout.random.3d` (`t4a random.rs:30`) is new. One numeric difference
is not merely naming: t4a's `CLOSED_FORM_3D_CEILING = CLOSED_FORM_CEILING` (1 000 000) disagrees
with develop's `BASIC_3D_CEILING = MAX_BENCH_NODES` (`registry/three_d.rs:119`) for the same
spiral/bipartite content.

### `sg-igraph-clean` vs develop's `KAMADA_KAWAI_3D_LAYOUT` / `FRUCHTERMAN_REINGOLD_3D_LAYOUT`

Same algorithm, mechanically re-derived into const-generic form (`kk_kernel.rs:45 descend<const D>`,
`fr_kernel.rs:41 layout<const D>` replacing develop's runtime-`dim` pair). The numeric structure
agrees on both sides — `start_temp = sqrt(n)/10`, linear cooling `temp -= start_temp/niter`, and the
KK 3-D start radius `0.36*sqrt(n)` (`t4a`-unrelated; `kamada_kawai_3d.rs:69-70` vs develop
`kamada_kawai/start.rs:18-22`). It is nevertheless a second implementation, not a port:

- **ids differ**, so no runtime collision but two registered ids per algorithm:
  `layout.force.fruchterman_reingold.3d` vs `..._3d`; likewise for kamada_kawai. They also sit at
  different `LAYOUTS` indices, and the wasm module maps layouts by index, so a combine reorders the
  index space.
- **FR noise source differs**: develop uses a Mulberry32 RNG (`fruchterman_reingold.rs`
  `start_positions`); the branch uses a counter hash (`fr_kernel.rs:64`, `coincident_nudge:154`).
  The pinned 2-D ids would then produce different coordinates.
- **No budget gate**: develop calls `budget::quadratic(...)` (`kamada_kawai.rs:86`); the branch's
  `kamada_kawai_3d.rs` has zero `budget` calls.
- The branch has **no** drl 3-D, no fa2 3-D, no yifan_hu 2z — develop's other three arms
  (`arms_3d.rs:236`, `:242`, `:217`).

Develop's five arms (`registry/arms_3d.rs:216-247`) each delegate to the kernel's own entry point,
and `arms_3d.rs:188` states plainly that none of them is a second implementation.

### Which side has the oracle differential

Nobody is complete; the two branches are missing opposite halves.

- **develop**: has the 3-D *arms* but **no** 3-D oracle for spectral/pivot
  (`3d` on `origin/develop -- harness/oracle-spectral.py` → 0 hits).
- **`p12-t4a`**: has the spectral / closed-form 3-D oracle differential, absent from develop.
- **`sg-igraph-clean`**: has the igraph `dim=3` oracle differential, absent from develop.
- The two branches' oracle work does not overlap each other — spectral/closed-form vs igraph.

---

## Merge conflicts

`p12-t4a` — 26 candidates, every one of which develop also touched:

```
crates/graph-cli/src/capabilities/registry/layout_row.rs
crates/graph-cli/src/capabilities/tests/ledger.rs
crates/graph-cli/src/capabilities/tests/mod.rs
crates/graph-cli/src/capabilities/tests/registry.rs
crates/graph-cli/src/capabilities/tests/stages.rs
crates/graph-cli/src/hashgate/tests/report.rs
crates/graph-cli/src/oracle_python.rs
crates/graph-cli/src/oracle_python/tests.rs
crates/graph-cli/src/snapshot_cmd/tests.rs
crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs
crates/graph-cli/tests/cli_ledger.rs
crates/graph-core/src/layout/bipartite.rs
crates/graph-core/src/layout/coords.rs
crates/graph-core/src/layout/pivot_mds.rs
crates/graph-core/src/layout/random.rs
crates/graph-core/src/layout/spectral.rs
crates/graph-core/src/layout/spectral/tests.rs
crates/graph-core/src/layout/spectral_stage.rs
crates/graph-core/src/layout/spiral.rs
crates/graph-core/src/registry.rs
crates/graph-core/src/registry/closed_form.rs
crates/graph-core/src/registry/spectral.rs
crates/graph-core/src/registry/tests.rs
docs/measurements/scigraphs-coverage.md
harness/oracle-closed-form.py
harness/oracle-spectral.py
```

`sg-igraph-clean` — 18 candidates:

```
crates/graph-cli/src/capabilities/registry/unproven.rs
crates/graph-cli/src/capabilities/tests/registry/ids.rs
crates/graph-cli/src/hashgate/tests/report.rs
crates/graph-cli/src/oracle_python/conformance/baseline/table/networkx.rs
crates/graph-cli/src/oracle_python/conformance/gaps.rs
crates/graph-cli/src/oracle_python/conformance/motor.rs
crates/graph-cli/src/oracle_python/conformance/rows.rs
crates/graph-cli/src/oracle_python/igraph.rs
crates/graph-cli/src/oracle_python/tests.rs
crates/graph-cli/src/snapshot_cmd/tests.rs
crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs
crates/graph-core/src/layout/force/fruchterman_reingold.rs
crates/graph-core/src/layout/force/kamada_kawai.rs
crates/graph-core/src/layout/force/mod.rs
crates/graph-core/src/registry.rs
crates/graph-core/src/registry/igraph.rs
docs/measurements/scigraphs-conformance.md
harness/oracle-igraph.py
```

Touched by **both** branches — resolve once, consistently:
`crates/graph-core/src/registry.rs`,
`crates/graph-cli/src/hashgate/tests/report.rs`,
`crates/graph-cli/src/oracle_python/tests.rs`,
`crates/graph-cli/src/snapshot_cmd/tests.rs`,
`crates/graph-cli/src/snapshot_cmd/roundtrip/tests.rs`.

Near-miss worth naming even though the files differ: `p12-t4a` edits
`crates/graph-cli/src/capabilities/tests/{mod,registry}.rs` while `sg-igraph-clean` edits
`crates/graph-cli/src/capabilities/tests/registry/ids.rs`. All three enumerate 3-D ids and will
disagree on naming even where no hunk collides.

---

## Recommendation

### The decision that gates both branches: DECISION

**Which 3-D id scheme is canonical?** Three spellings now exist for the same layouts: develop's
`layout.force.kamada_kawai.3d` / `layout.spectral3d` / `layout.bipartite_3d`; `p12-t4a`'s
`layout.spectral.3d` / `layout.mds.pivot.3d`; `sg-igraph-clean`'s `..._3d`.

- **Option A — develop wins.** Keep develop's ids; re-key both branches' oracle rows and metadata
  onto them; drop t4a's four duplicate arms and the other branch's two `_3d` modules and kernels.
  - For: develop already registers all of these; `arms_3d.rs:188` states the arms are not second
    implementations; develop keeps the FR Mulberry32 seed the 2-D hashgate pins, and keeps the
    `budget::quadratic` gate the branch drops.
  - This is the smaller diff and the only option that keeps the existing gates.
- **Option B — branch naming wins.** Retire develop's `arms_3d.rs` entries for FR/KK in favour of
  the const-generic kernels.
  - Against: it drops the `budget` gate, changes FR's noise source (invalidating the pinned 2-D
    coordinates), and buys no third algorithm.

Recommended: **A**.

### `p12-t4a` (971318dc) → LAND-PART

Land — the oracle layer and the unique kernels, none of which is on develop:

- `harness/oracle-spectral.py`, `harness/oracle-closed-form.py` — the 3-D oracle differentials.
- `crates/graph-cli/src/oracle_python/spectral.rs`, `oracle_python/closed_form.rs` — the ceiling rows.
- `crates/graph-core/src/linalg/lobpcg/{ops.rs,tests.rs}` and `crates/graph-core/examples/diag_*.rs`.
- `crates/graph-core/src/layout/spectral/space.rs` (+tests), `layout/spectral/tests/dims_3d.rs`.
- The `ID_3D` arm in `crates/graph-core/src/layout/random.rs` — the only layout develop lacks.
- `docs/measurements/p12-t4a.md`, `scripts/orch/rows/p12-t4a.rows`.

Do not land: `registry.rs`, `registry/spectral.rs`, the `SPIRAL_3D` / `BIPARTITE_3D` rows of
`registry/closed_form.rs`, `registry/tests.rs`, `layout/spiral/spiral_3d.rs` — all superseded under
Option A. `registry.rs` cannot apply at all: the `LAYOUTS` table it patches moved to
`registry/layouts.rs:47`. Every `snapshots` count literal must be regenerated from the post-combine
table, not hand-resolved.

### `sg-igraph-clean` (06ad8ea1) → LAND-PART

Land — develop has no equivalent:

- `crates/graph-cli/src/oracle_python/conformance/motor/fit.rs` (+tests) — the `_igraph_fit_positions`
  arm, and the only 3-D igraph oracle differential available.
- The `dim=3` `REFERENCES` entries and `start_layout(initial, dim)` in `harness/oracle-igraph.py`,
  re-keyed to develop's dotted ids.
- The RNG-seeding correction in `crates/graph-cli/src/oracle_python/conformance/gaps.rs`, which
  refutes a develop note that is factually wrong.
- The three append-only doc supersets (`docs/decisions/layouts-igraph.md`,
  `docs/layouts/layout.force.{fruchterman_reingold,kamada_kawai}.md`) and
  `docs/measurements/{scigraphs-conformance.md,sg-igraph-dims.md}`.

Do not land: `fr_kernel.rs`, `kk_kernel*`, `fruchterman_reingold_3d.rs`, `kamada_kawai_3d.rs` and
their tests, the `force/{fruchterman_reingold,kamada_kawai}.rs` rewrites, and the two `_3d` registry
entries — all superseded by develop's `run_3d` arms under Option A.

### Open item on the `dim=3` contradiction

develop `harness/oracle-igraph.py:32` records that `layout_kamada_kawai(dim=3)` **refuses**, with
`Invalid start position matrix size in 3d Kamada-Kawai layout`. The branch supplies a 3-column start
via `start_layout` and measures at `dim=3`. These are reconcilable: develop's failure is a
start-matrix *arity* error from a 2-column start, not a refusal of `dim=3` — but neither claim was
re-run here. Settle it by measurement before landing the igraph 3-D rows.

### What I did not do

No branch was checked out, combined, built or tested; no reference oracle was executed. Every class
above is a static reading of blobs on `origin/develop` versus the two branch tips.
