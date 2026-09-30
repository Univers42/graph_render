# graph-motor — project status (2026-09-30)

Read this first, then `prompts/CONTINUE.md` (how to work on this host) and `prompts/RESUME.md`
(the newest handoff, HANDOFF 2026-09-30 00:40). Long history: `docs/reports/HANDOFF.md`,
`docs/reports/phase-NN*.md`. The tree and `git` are the final authority over all three.

Every fact below was checked on 2026-09-30 against `origin/develop` = **f261baf** (251 commits).

## 1. Branches

Check any row yourself: `git log --oneline origin/develop..origin/<branch>` — **0 lines = merged**,
n lines = n commits not yet on develop.

### Merged into develop

| Branch | Head | Landed | What |
|---|---|---|---|
| p0–p2 | `p2m` f080394 | develop 39d2450 | foundation, topology, geometry contract + stage registry |
| p3 | 2538f0a | inside p6f, merge 45a653f | the deterministic one-shot layouts |
| p4 | 2c49e04 | merge efe2452 | wasm transport + JS SDK, registry-driven ABI |
| p5 | 4caf184 | merge 7788d85 | Sugiyama layered DAG |
| p6e | 7788d85 | merge 6ae96a1 | spectral, pivot MDS, JAMA tred2/tql2, LOBPCG |
| p6f | 67543b4 | merge 45a653f | Barnes–Hut, FA2 |
| p7 | ab396f5 | merge ffb837a | the ANALYSIS stage |
| p8 | b980ae8 | head is a develop commit | post/routing/bundling (+ `p8-p8-bundle`, `p8-route`, `p8-styles`) |
| p9 | d9912ad | head is a develop commit | scale stage + benches |
| p10 | 1e3a6d9 | merge e612991 | ingest contract, `gm_build_contract`, SDK publish |
| p11 | 8543d98 | merge c84c869 | compute tiers: `exec/` runners, hashgate Threads/Tiers arms, 11 knobs |
| p12-t1 | 0c72dc5 | merge b182b7e | yifan_hu multilevel, closed-form fixtures + oracle |
| studio | 984f433 | head is a develop commit | the Vite/React studio, S1–S5 + parity |
| `limits` | 09db63b | head is a develop commit | the split of the 19 files over 300 lines |
| `gui`, `abi-post`, `studio-post` | 03ade38 / 77ab863 / ef4e9da | heads are develop commits | studio app, wasm ABI for POST/ANALYSIS, the POST+ANALYSIS panels |
| `orchfix` | 15edd32 | merge 3486ca5 | oc-live/oc-job/oc-status liveness on the service session list |

Also merged and kept for history: `followups` (746f3a8), `negctl` (7f0cab6), `lesmis` (4f82f36),
`fa2fix` (c6b6ea8), `repair-evidence` (8c1d3c3), `reports` (79aef00), `train` (24d92bc),
`integ` (8292407), `p3-tidy`, `p3-circular`, `p3-treemap`, `p3-planarity`, `p3-packing`.

### Pushed, not merged

| Branch | Head | Commits ahead | What |
|---|---|---|---|
| `followups2` | 3ddd308 | 1 | phase-7 follow-ups: graph-wasm analysis/post registries, hashgate knobs/stages |
| `sim` | 5ae4210 | 1 | force-session M1 (live simulation for the Obsidian-style drag) |
| `p12-igraph` | 8238039 | 1 | igraph layouts DRL, LGL, DavidsonHarel, Graphopt, KK, FR |
| `studio-force` | 15ce426 | 1 | `LiveForce` port, worker loop, Forces panel (waits on force-wasm) |
| `studio-s7` | 9af62bc | 3 | S7 perf: 10k/20k case, `stats()` counters, `STUDIO_PERF_BREAK` negctl |
| `studio-ux` | 855a876 | 2 | **DROPPED by the user 2026-09-30. Do not plan work on it.** |

`studio-ux` and studio plan item **S8** were dropped on 2026-09-30. The branches stay on `origin`;
they are history, not work.

## 2. Merge rule (user, 2026-09-29 — replaces the old strict-sequence rule)

**Merge first, repair on develop.** A branch is merged when the floor is green **on the merged
tree**, then one full gate runs on `develop` and its red rows become repair tasks. No per-branch
1000-seed hashgate, no per-branch mutants gate before the merge — that gating became perfectionism
and blocked the train for hours.

The floor (`scripts/orch/rows/quick.rows`):

1. `scripts/orch/gr cargo fmt --all --check`
2. `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings`
3. `scripts/orch/gr cargo test --workspace --no-fail-fast`

Per merge: `git merge develop` into the branch (never rebase — history is published), resolve
keeping **both** intents, run the floor on the merged tree, merge into `develop`, push.
A fresh worktree needs `npm ci` before `cargo test` (`wt-new.sh` does it). A worktree cut before a
shared file was restructured shows that file as "deleted" in `git diff` — a diff artefact, develop's
version wins.

Still true: UNKNOWN = FAIL, SKIP is not a pass, a gate that did not run is "not run" and never
"green". Evidence records are fingerprinted to the tree.

## 3. Host

`dlesieur42` since 2026-09-30: **20 cores, 31 GB RAM** (was a 4-CPU host before). No `/goinfre`,
no `/sgoinfre`. Host-local, unversioned, lost on every host change — `scripts/orch/scratch.sh` puts
it under `$GM_SCRATCH` = `$HOME/goinfre`:

- `wt/` — worktrees, one per job, made by `scripts/orch/wt-new.sh <branch> [base]`
- `refs/` — the pinned read-only references (`networkx-3.6`, `scipy-1.16.2`, `jama-1.0.3`,
  `matplotlib-3.10.0`, `npm/`), rebuilt by `scripts/orch/fetch-refs.sh`
- `orch/{bin,logs,locks}` — helper symlinks, job logs, gate locks
- `mcp-out/` — browser screenshots from the `pw` MCP

Lost with the previous host: the `/sgoinfre/students/dlesieur/orch` prompts and rows. Rows now live
in `scripts/orch/rows/` (only `quick.rows` today) and job briefs in `prompts/jobs/`. Docker is the
only toolchain: `scripts/orch/{gr,node-slim.sh,ge-check.sh,gate.sh,mutants.sh}`.

## 4. Work queue, in order

1. **First tasks on develop** (unchanged, still not run): wasm32 build of graph-core,
   `hashgate --seeds 8` and one negctl (`GM_MUTATE_REFERENCE_DEGREE=9` must be non-zero), then one
   full gate on develop. Red rows become repair tasks.
2. **The merge train**: `followups2` → `sim` → `p12-igraph`, one at a time, each under the floor.
3. **Live forces**: `force-wasm` cut from `sim` (the session ABI), then `studio-force`, then merge
   `studio-force` into `studio`, rebuild `app/public/graph_wasm.wasm`, then `studio` → develop.
4. **`studio-s7`**: merges into develop once `quick.rows` and the studio rows are green.
5. **No-cargo lanes** (no gate, run in parallel): SciGraphs layout coverage in the motor *and* the
   studio picker; the thread-tier audit; the 3D devil verdict (`docs/decisions/contract-3d.md`);
   the Graphviz oracle; CI.

## 5. Decisions already taken (do not re-ask)

- **2026-09-30 — free-model outage means the queue waits.** No paid fallback model. Probe the free
  models before a launch; if they answer `provider.quota` or hang, the job queue waits.
- **2026-09-30 — the Graphviz engines must match Graphviz's own output.** Full port set, gated
  against a Graphviz oracle (no Graphviz binary, library or server on this host).
- **2026-09-30 — CI approved**: a GitHub Actions merge-floor workflow with an arm64 hashgate arm.
- Compute tiers: tuned scalar, then SIMD, then threads, then GPU only on measurement; a GPU layout
  gets its own capability id. D10: gather-form kernels. CPU only, multi-threaded (user).
- Hierarchy policy is "repair + record": a virtual-root forest, cycle and multi-parent repair. A
  snapshot `notes` section is contract 0.3; note codes 1–3 in p3, 4–5 in p5, 6 reserved.
- Phase 6: FA2 is a port of networkx 3.6, Yifan Hu stays `absent`. JAMA tred2/tql2 for n ≤ 256,
  scipy LOBPCG (block 4) above; the LOBPCG start block does not depend on the seed; no shift-invert.
  Stress is the Pearson hop/euclid correlation over 32 max-min pivots, margin −0.05 vs d3. The d3
  force set is link, manyBody, center, collide — no cluster force.
- References are pinned read-only (networkx 3.6, the four d3 packages, JAMA 1.0.3, scipy 1.16.2
  `lobpcg.py`); sha256s in `HANDOFF.md`. Docs write the path as `/home/user/refs/...` — same files.
- The user delegated every non-critical decision to "the recommended option" for autonomous runs.