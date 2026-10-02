# Job p13-gv2-dot-rank (agent build, dot pass 1 of 3: text width + rank)

You start on branch `p13-gv2-dot-rank`, cut from `p13-gv2-dot` (commit f098188). That branch holds the
first dot job's partial port: `crates/graph-core/src/layout/graphviz/dot.rs` and `dot/{fast,decomp,acyclic,tests}.rs`,
and the measurements `docs/measurements/p13-gv2-dot.md`. Read that file in full first, especially
"Blocker 1", "Blocker 2" and "The remaining passes, as a draft". The parent brief is
`prompts/jobs/p13-gv2-dot.md` and its model `prompts/jobs/p13-gv1-twopi.md`. Their rules (EPL-1.0: read the
reference, never translate line by line; determinism D1–D10; house limits) all apply.

Decisions taken by the orchestrator (2026-10-01) on the first job's two questions:
1. **Node width.** Pin a text-width table measured from the oracle image. Write the decision and its evidence
   in `docs/decisions/graphviz-oracle.md` (a new section, additive). The data lives once in graph-core, in a
   new module `crates/graph-core/src/layout/graphviz/text_width.rs`, because osage has the same cause
   (`docs/measurements/p13-gv1-osage.md`); do not wire osage to it in this job, just name it as the follow-up.
   Measure on `ge-graphviz-oracle` with one-node graphs (`width=0 margin=0`) for every character the fixture ids
   use (read `emit-graphviz-fixtures` and `fixtures/adversarial-ids.json`), alone and in runs, and find the
   rule that reproduces **every** label of the 1000-seed fixture set exactly (the first job measured
   `9.04536 + (k-1) × 10.1758` for `n` repeated `k` times; check whether digits share one advance). The node
   width is then `max(0.75 in, text + 2 × 0.11 in)`. A unit test pins the table against at least the four
   measured rows in the measurements file. If no rule reproduces every label, stop and say which labels fail.
2. **Split.** dot is three jobs: this one (rank), then mincross, then position + frame + differential. Do
   only this one.

Do:
1. Split `dot/fast.rs` (350 lines, over the 300-line house limit) into child modules first.
2. `text_width.rs` as above.
3. Items 1–3 of the draft: `simplex.rs` (`rank2`: feasible tree, cut values, the pivot loop with its search
   cut-off, `TB_balance`; written generically enough that the position pass can run it again over the
   auxiliary graph with `LR_balance`), `rank.rs` (`dot1_rank` for one component, the no-op stages named as
   no-ops), `class2.rs` (chains for long edges, merged parallel edges, `virtual_weight`).
4. Tests: the ranks of the six closed cases and of 20 fixture seeds equal the ranks implied by the oracle's
   `-Tplain` y coordinates (y = rank × 72 pt × (nodesep-free) — derive the rank from y, do not assume). Record
   the comparison over all 1000 seeds in the measurements file: how many seeds agree on every node's rank.
5. No `LAYOUTS` entry, no differential, no rows file yet: the layout has no x coordinates until pass 3.

Paths you may touch: `crates/graph-core/src/layout/graphviz/{dot.rs,dot/,text_width.rs,mod.rs}` (mod.rs: one
additive line), `docs/measurements/p13-gv2-dot.md`, `docs/decisions/graphviz-oracle.md` (additive section),
and a probe under `target/`. Nothing else.

Done when: fmt, clippy `-D warnings`, `cargo test -p graph-core --lib` and the wasm32 build of graph-core
exit 0; the rank agreement over 1000 seeds is recorded with the command that produced it; the return block
pastes each command's real exit code and ends with the draft brief for `p13-gv2-dot-mincross`.
Leave everything uncommitted; the orchestrator commits.

## Decisions (orchestrator, 2026-10-02, on the first run's return block)

- Node width: keep `max(0.75 in, text + 2*0.11 in)` in this job. The measured
  `node = 1.37952 * label_box + 0.30669` in goes into `p13-gv2-dot-position`, where x is computed;
  record the four oracle widths there, not here.
- The first run returned without the rank pass. This run does it: the rank pass, the six closed cases,
  the 20 fixture seeds, and the 1000-seed rank agreement count with its command, all per "Do" and
  "Done when" above. The work already in the worktree stays; build on it.
