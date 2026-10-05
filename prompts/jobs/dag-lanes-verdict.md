# Job dag-lanes-verdict (agent devil, docs only: rule on `layout.dag.lanes` before any code)

Why: `layout.dag.lanes` adds a public capability:
- a registry entry appended to the wasm-indexed `LAYOUTS`;
- a capabilities row;
- published parameters;
- server caps and digest rows.

The house rule (`.claude/rules/devil/risk.md`) requires a verdict before code. You rule on the
design and on its implementation plan. You write no code.

Read, in full:
- `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md` (the design);
- `docs/superpowers/plans/2026-10-05-dag-lanes-layout.md` (the code the build job will write).

Facts (develop, 2026-10-05; re-check each on the tree you start from):
- `crates/graph-core/src/registry/layouts.rs` is 308 lines. Its header says it is append only,
  because `crates/graph-wasm` maps a layout by its index. The last entry is
  `layout.dag.dot`'s `Capability`.
- `crates/graph-core/src/layout/sugiyama/acyclic.rs:52-71` notes `EdgeReversed` on **every**
  non-loop edge whose source ranks after its target, directed or not. The lanes plan notes only
  **directed** edges (`lanes/rows.rs` `notes`). Rule on whether that difference is acceptable
  or must match.
- The hash gate runs every registered layout on `seeded_model(seed, n, REFERENCE_DEGREE)`:
  undirected edges, every `version` 0 (`crates/graph-core/src/records.rs`). On that model, lanes
  rows fall back to index order, and the width can reach n.
- `crates/graph-cli/src/capabilities/registry/layout_row.rs:48` `ROUNDTRIP_LAYOUTS` lists the
  layouts whose oracle is a hand oracle on `roundtrip`.
- Coordinates are `f32`. A row `r` and `r + 0.5` are exact in `f32` while `r < 2^23`.

Rule on each of these. For each, one line: OK, or a condition the build job must meet.
1. **Correctness.** The claim "no vertex sits on an edge that runs past it" (spec, "Why no edge
   crosses a node") against `assign.rs` in the plan: each lane in at most one reservation, and a
   reserved lane never free. Try to break it with a counterexample of at most 6 vertices; if you
   find one, BLOCK and give it.
2. **Determinism (D1–D10, `prompt.md` §6).** Both heaps, the `f64::total_cmp` key, the `as f32`
   casts, and no `HashMap`.
3. **The hand oracle's independence.** It uses BTree structures instead of heaps, and is
   compared bit for bit. Does it restate the convention, or copy the code's shape closely enough
   to share its bugs? Name any shared step.
4. **The public surface.** Appending last is safe for the wasm index. `scale_ceiling` is
   1,000,000, labelled before the bench measures it. `degradation` and `ponytail` say what they
   must.
5. **The gate model.** Is a layout that draws the seeded undirected graph n lanes wide a
   legitimate registry entry, or must its metadata or the hash gate treat it specially?
6. **Scope.** Does anything under `crates/` in the plan name a data source (git, a repository, a
   commit)? The user forbids that (2026-10-05).

Score the four axes 1–5 (blast radius, reversibility, cost on failure, confidence) and name the
worst. Then give one verdict: PROCEED, PROCEED-WITH-CONDITIONS (numbered conditions, each
checkable by a command or a test name), or BLOCK (what to resolve).

Write it to `docs/decisions/dag-lanes.md`:
- a title;
- `Status: <verdict>, 2026-10-05`;
- the six rulings as a table `| # | Question | Ruling | Evidence (path:line or command) |`;
- the scores;
- the conditions.

Nothing else.

You may run `git grep`, `sed -n` and read any file. Do not run builds, tests, benches or gates
other than the one below.

Paths you may touch: `docs/decisions/dag-lanes.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-dag-lanes-verdict scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS.
- `grep -E '^Status: (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), 2026-10-05$' docs/decisions/dag-lanes.md`
  prints one line.

Return: the branch tip, the verdict, each condition, and every deviation from this brief.
