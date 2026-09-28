# graph-motor — project status (2026-09-28)

Read this first. Then read `prompts/CONTINUE.md`, which tells the next agent how to work. The long
history lives in `docs/reports/HANDOFF.md` and `docs/reports/phase-NN.md`.

## 1. Where the code is

| Branch | Head | Phase | State |
|---|---|---|---|
| `develop` = `claude/sharp-turing-er9nve` | 39d2450 | 0–2 | **merged, gate green** |
| `p2m` | f080394 | 2 close-out | merged into develop (mutants: 591 run, 512 caught, 0 missed, 73 unviable, 6 timeouts) |
| `p3` | a9010f5 (local) / 0b9fa62 (origin) | 3 | close-out in progress: develop merged into p3 (3d5ae48), full gate, ge-check, mutants and `phase-03.md` are running |
| `p3-tidy`, `p3-treemap`, `p3-circular`, `p3-planarity`, `p3-packing` | — | 3 parts | already merged into `p3`; kept for history |
| `p4` | 9389255 | 4 (wasm ABI + JS SDK) | built, reviewed and repaired on its own branch, based on 900cf13 (before p3). **Not merged.** |
| `p7` | 79f4701 | 7 (analysis) | built, reviewed and repaired, based on 900cf13. **Not merged.** |
| `p5` | 4b5ad8a | 5 (Sugiyama) | built, reviewed and repaired, based on b7a068a (p3 substrate). **Not merged.** |
| `p6e` | d5f1fa3 | 6 eigen (spectral, Pivot MDS, JAMA tred2/tql2, LOBPCG) | built, reviewed and repaired, based on b7a068a. **Not merged.** |
| `p6f` | 700f7f4 | 6 force (Barnes–Hut, FA2) | built, reviewed and repaired, based on b7a068a. **Not merged.** |

Every phase branch is pushed to `origin` under its own name. Worktrees exist locally at
`/home/user/wt-<branch>`. They are gone after a container restart; recreate them with
`git worktree add`.

## 2. Merge rule (set by the user)

Merge into `develop` **strictly in sequence**, and only after that phase's own gate is green, it
has passed an independent review, cargo-mutants has run over its diff, and its `docs/reports/phase-NN.md` is
written:

`p2m` ✅ → `p3` (in progress) → `p4` → `p7` → `p5` → `p6e` + `p6f` → then build 8, 9, 10, 11.

To merge each branch:
1. `git merge develop` into the branch (not a rebase; history is published).
2. Resolve conflicts keeping **both** intents.
3. Re-run the full phase gate.
4. Merge into `develop`, then push `develop` and `develop:claude/sharp-turing-er9nve`.

Pitfall: a branch cut before 900cf13 (p4, p7) makes `git diff develop..branch` show
`compute-tiers.md` and `phase-11` as "deleted". That is a diff artefact, not a deletion. `develop`'s
versions win.

## 3. What each unmerged branch still needs at merge time

- **p3.** Finish the close-out: gate at 1000 seeds, ge-check, mutants, `phase-03.md`, and the 5-param
  `seed_triangle` fix. Known risk: `hashgate --seeds 1000` now takes about 30 min per arm, because the
  circle-packing fallback is O(n²). `CHILD_TIMEOUT` was raised from 900 s to 2700 s. Phase 9 must address
  this.
- **p4.** Make the ABI registry-driven (`gm_layout_count`/`gm_layout_id`) so p3's layouts need no ABI
  change. Expose the notes columns (`note.code`, `note.index`) and the Circle r / Box w,h and Polyline
  columns reserved in `docs/contract/wasm-abi.md`. Build `EdgeRecord` without `..`, so the merge
  surfaces p3's `child_first`. Back the `transport.*` ledger rows with hashgate/evidence wiring. Record the
  envelope growth as a deviation; the reviewer's BLOCKER was accepted as "amend the envelope". The wasm
  binary is 265,838 B, 4% over the 250 KB soft ceiling; this is accepted and documented.
- **p7.** Fix the allow-list gate row: its regex must admit the workspace crate `graph-contract` (a
  prompt-row bug). Add `analysis.depth` on top of p3's `hierarchy.rs`. Expose the analysis columns in
  the snapshot, JSON and SDK after p3 and p4 are merged. The `sdk-smoke.mjs` row passes once p4 is in.
- **p5.** Register `layout.dag.sugiyama`. Add the dagre-d3-es arm to `harness/oracle-layouts.mjs`.
  `canonical_json/schema.rs` still lists note codes as [1,2,3]; regenerate it with codes 4 and 5.
  Measured crossings: ours 5,242 vs dagre 7,657 over 236 graphs, so it passes the frozen margin.
- **p6e/p6f.** Wire the new layouts: registry entries, one hashgate stage per layout with its own
  negative control, graph-cli `stress`/`bench`, and the wasm exports through p4's registry-driven ABI.
  Build the approved Python oracle image (debian:trixie-slim, pinned numpy/scipy, networkx 3.6 from
  `/home/user/refs`). Review the "Corrections found during implementation" section of `eigensolver.md`.
  No row goes to `gated` without an oracle differential and a measured ceiling.

## 4. Remaining phases (not started)

- **Phase 8**: post-processing, routing and bundling (FDEB in gather form; MINGLE sequential).
- **Phase 9**: scale and benchmarks, including the per-tier crossover numbers for Phase 11 and the
  circle-packing hashgate cost.
- **Phase 10**: ingest contract, SDK publish. This replaces p4's provisional ingest JSON.
- **Phase 11**: compute tiers (SIMD, then threads, then GPU only if measured necessary). See
  `docs/decisions/compute-tiers.md`.

## 5. Decisions already taken (do not re-ask)

- networkx 3.6 is pinned as a read-only reference. d3-hierarchy 3.1.2, dagre-d3-es 7.0.14,
  d3-force 3.0.0, d3-quadtree 3.0.1, JAMA 1.0.3 and scipy 1.16.2 `lobpcg.py` are all pinned, with
  sha256s in `HANDOFF.md` or `/home/user/ORCH_NOTES.md`, under `/home/user/refs` (read-only).
- Hierarchy policy is "repair + record": a virtual-root forest, and cycle and multi-parent repair.
  A snapshot `notes` section was added as contract 0.3. Note codes are 1–3 in p3; 4 and 5 are
  activated in p5; 6 is reserved.
- Phase 6:
  - FA2 is a port of networkx 3.6. Yifan Hu stays `absent`.
  - Eigen solvers: JAMA tred2/tql2 for n ≤ 256, and scipy LOBPCG (block 4) above that. The file is
    named `lobpcg.rs`.
  - The LOBPCG start block does not depend on the seed. There is no shift-invert.
  - Stress is the Pearson hop/euclid correlation over 32 max-min pivots, with a margin of −0.05 vs d3.
  - The d3 force set is link, manyBody, center and collide; there is no cluster force.
- Compute tiers: tuned scalar, then SIMD, then threads, then GPU only on measurement, and a GPU
  layout gets its own capability id. D10: gather-form kernels.
- The user delegated every non-critical decision to "the recommended option" for autonomous runs.
