# Job scigraphs-gap (agent build, docs only)

Why: the user asked for every SciGraphs layout in the motor AND in the studio picker
(`prompts/RESUME.md`, "Queued, not started"). The picker reads the motor's registry, so a layout is in
the studio once it is registered on develop. Nobody has measured the gap since p12-t1 landed.

Facts:
- `SciGraphs/` is initialized in this worktree. Its layouts: `SciGraphs/core/scigraphs_core/mesh/layouts/`
  (`dispatcher.py` maps names to functions; `networkx_layouts.py`, `igraph_layouts.py`, `hierarchical.py`,
  `basic.py`, `circle_packing.py`, `forceatlas.py`, `yifan_hu.py`, `simulation.py`, `interactive.py`).
  Graphviz engines are listed there too.
- The motor's layouts: `LAYOUTS` in `crates/graph-core/src/registry.rs` and `registry/*.rs`; the ledger
  view is `scripts/orch/gr cargo run -q -p graph-cli -- capabilities` (read its rows by id).
- In flight on other branches (count them as "in flight", not "missing"): `p12-igraph` (DRL, LGL,
  DavidsonHarel, Graphopt, Kamada-Kawai, Fruchterman-Reingold), `sim` (live force session, not a new
  layout id). Planned: `p12-t2` (force.spring = networkx spring_layout 2D, circular.hierarchy =
  SciGraphs CIRCULAR_HIERARCHY), `p13-gv1` (twopi, circo, patchwork, osage), `p13-gv2` (neato, fdp,
  sfdp, dot), 3D layouts after the contract-3d verdict. See `git log --oneline origin/develop..origin/p12-igraph`.
- User decisions: Graphviz engines are reimplemented natively and must match Graphviz's own output
  (the Graphviz source is an algorithm reference and a docker-only oracle, EPL-1.0: read it, never
  translate it line by line). 3D is approved subject to a devil verdict on the contract change.

Do:
1. `docs/measurements/scigraphs-coverage.md`: one row per SciGraphs layout name (from `dispatcher.py`,
   with `file:line`): its SciGraphs function, dimension (2D/3D), the motor registry id if any, status
   (`on develop` / `in flight: <branch>` / `planned: <job>` / `missing`), the reference the motor would
   port from (networkx, igraph, Graphviz, SciGraphs itself), and the oracle that would gate it. End
   with the counts per status.
2. For the `missing` rows that no planned job covers, add a draft brief per coherent group as
   `prompts/jobs/p12-t3.md` (and more if needed), in the style of this brief: why, facts with
   `file:line`, do, allowed paths, done-when. Each layout needs registry metadata (oracle, complexity,
   `scale_ceiling`, `degradation`, `ponytail`), a hashgate entry, and an oracle differential.
3. Draft `prompts/jobs/p12-t2.md` for force.spring and circular.hierarchy the same way (its first
   brief was lost with `/sgoinfre`).

Paths you may touch: `docs/measurements/scigraphs-coverage.md`, `prompts/jobs/p12-t*.md`. Nothing else.

Done when: every SciGraphs layout name appears once in the table with a `file:line`; every status is
backed by a registry id, a branch log line, or a brief path; the drafts exist.
