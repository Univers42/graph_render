# Job merge-igraph-clean (agent build: land sg-igraph-clean over develop, one 3-D FR and KK each)

Read `scripts/orch/common.md` first. Your worktree's branch starts at `sg-igraph-clean`. Released only
after `merge-p12-t4b` is on develop (`git merge-base --is-ancestor`; if not, stop and report).

Why. Two branches added the same two layouts under two ids:

| | merge-p12-t4b (on develop) | sg-igraph-clean (this branch) |
|---|---|---|
| ids | `layout.force.fruchterman_reingold.3d`, `layout.force.kamada_kawai.3d` | `layout.force.fruchterman_reingold_3d`, `layout.force.kamada_kawai_3d` |
| code | `run_3d` inside `force/fruchterman_reingold.rs` and `force/kamada_kawai.rs`, shims in `registry/arms_3d.rs` | `force/{fruchterman_reingold_3d,kamada_kawai_3d,fr_kernel,kk_kernel}.rs` |
| start | a cube (its metadata says it differs from igraph) | the spec's ball / sphere start |
| evidence | `harness/check-3d-ledger.sh`, `oracle-igraph3d` stress | SciGraphs rows `IGRAPH_FR`, `IGRAPH_KK`; `oracle-igraph.py` stress; clean-room provenance in `docs/measurements/sg-igraph-dims.md` |

Decision (orchestrator, 2026-10-02): one implementation and one id per layout. The ids are the
`.3d` ones (the family convention of the five t4b arms: `drl.3d`, `forceatlas2.3d`, `yifan_hu.2z`,
and the harness already names them). The code is sg-igraph-clean's (spec start, clean-room
provenance, conformance rows).

The clean room. Never open `/goinfre/dlesieur/refs/igraph-0.11.9`, any igraph source, or branch
`sg-igraph-dims`. You may read both branches above. Do not change the 3-D kernels' arithmetic: this
job moves and renames, it does not re-port.

Do:
1. Before merging, record on develop: `hashgate --seeds 8` per-stage hashes of every id except the
   two `.3d` FR/KK ids (paste them). Merge `origin/develop` (`merge -m updated`, never rebase).
2. Resolve: `ID` constants of sg-igraph-clean's modules become the `.3d` strings; the `_3d` registry
   entries and their `Metadata` go; `arms_3d::run_fr_3d` / `run_kk_3d` call sg-igraph-clean's
   modules; t4b's `run_3d` in the 2-D modules goes when nothing else calls it (retarget the doc links
   in `drl.rs`, `forceatlas2.rs`, `kamada_kawai.rs`). One `Metadata` per id, every field true for the
   code it now runs (the start is no longer a cube). Every id list (`capabilities/tests/registry/ids.rs`,
   `unproven.rs`, `snapshot_cmd/tests.rs`, `hashgate/tests/report.rs`, `oracle_python/igraph.rs`,
   conformance `rows.rs`, `motor/fit.rs`, `gaps.rs`, `harness/oracle-igraph.py`,
   `harness/check-3d-ledger.sh`) names each layout once. `LAYOUTS` loses two entries: keep the order
   of the rest.
3. After: the hashes of step 1 are unchanged (paste both). The two `.3d` FR/KK hashes now equal
   sg-igraph-clean's own `_3d` hashes before the merge (paste both).
4. `docs/measurements/merge-igraph-clean.md`: the before/after tables and the resolution list.

Proof: quick.rows green; `capabilities --check` shows no new problem; `codegen --check` exit 0;
`scripts/scigraphs-conformance.sh` exit 0 with `IGRAPH_FR`/`IGRAPH_KK` re-pinned only if their
motor arm moved (it should not), `--break` exit 1; `harness/check-3d-ledger.sh` exit 0.
