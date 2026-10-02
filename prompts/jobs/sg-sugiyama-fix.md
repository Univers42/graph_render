# Job sg-sugiyama, review round 1 (same session, same worktree): FIX, 2 MAJOR, 10 MINOR

The review is `docs/reviews/rv-sg-sugiyama.md` on branch `rv-sg-sugiyama`. Read it with
`git show origin/rv-sg-sugiyama:docs/reviews/rv-sg-sugiyama.md`. It confirms the FAS rewrite against
`hierarchical.py`, the `run_scaled` port, and D10 (hashgate 8 is 4-way equal). The registry row and
some evidence are wrong. Fix every item, then return a new block. Your old block does not count:
this round must end in a commit past the current head (38f4384, which already merges develop).

This round widens your paths: `crates/graph-core/src/registry/grid.rs` (items 1-3 only),
`docs/decisions/sugiyama-heuristics.md`, `docs/reviews/review-layout-tree.md` (item 12 only),
`fixtures/dag/` and `harness/oracle-layouts.mjs`'s fixture list (item 5 only).

MAJOR
1. `registry/grid.rs:97`: `oracle` says "acyclic after FAS", but the pipeline takes
   `ArcOrder::NodeIndex` and never runs a FAS. Name the node-order orientation it actually performs.
2. `registry/grid.rs:99`: `complexity: "O(n+m) per phase"` is false. `Arcs::grouped()` sorts
   (`acyclic.rs:126`) and is called three times per pipeline. Prefer hoisting one `grouped()` into
   `layered()` (`layering.rs:42-50`) and passing it down, which keeps one sort. Then write the true
   bound (`O(m log m)` once, plus `O(n+m)` per phase). Only a pure refactor is allowed here: the
   output must not move (prove it in step "Proof" below).
   Also in `grid.rs:109`, replace the FAS sentence in `ponytail` with the node-order breaker's, and
   keep the "note 5 is cosmetic" part.

MINOR
3. `docs/decisions/sugiyama-heuristics.md`, §"Parallel edges are deduped": add that dummy
   allocation and the `up`/`down` order now follow `(tail, head)` order, not edge index. That moves
   `layout.dag.sugiyama`'s coordinates on multigraphs (review finding 3).
4. `acyclic.rs:186`: add one sentence on `ArcOrder::Feedback`. The arc list is ordered by node index,
   not by rank as `_acyclic_arcs` does, and the two agree only when rank == index (finding 4).
5. `docs/measurements/sg-sugiyama.md:183`: reword "byte-identical". State why the `--dag` corpus
   cannot see the dedup change: no fixture has a parallel arc. Add a `fixtures/dag/` fixture with a
   parallel arc and register it in the `--dag` list, so the crossing gate can see the change. Paste
   the `--dag` run with the new fixture; it must pass.
6. `docs/measurements/sg-sugiyama.md:133`: make the stage table reproducible. Paste `diff.py` into
   the doc, and fix the `py-stages.json` / `py.json` mismatch with `:72-74`.
7. `acyclic/feedback.rs:151`: put an `unreachable!` with a reason after the heap loop, so a drained
   heap with `remaining > 0` panics instead of spinning (finding 6).
8. `scaled/tests.rs`: add an empty-topology case for `run_scaled` that pins `(0, 3)`-shaped empty
   output (finding 7).
9. `stages/dump.rs:73`: build a `name -> index` map once, instead of `position` per endpoint
   (finding 8; `BTreeMap` or a sorted Vec, never `HashMap`).
10. `scaled.rs:75`: the reference accepts `scale == 0` and returns all zeros. Either accept
    `0.0` (preferred: pin it with a test against the reference value) or add a `Ponytail:` line
    naming the refused input (finding 5).
11. `docs/reviews/review-layout-tree.md:59-61`: annotate the three certified FAS claims. They hold
    for `ArcOrder::Feedback`, which the product pipeline does not take (finding 11).
12. `conformance/motor.rs:242`: `row_line` is 53 lines. Do not touch it here, and name it in the
    report as owned by another job.

Proof (paste each command and its last lines):
- `hashgate --seeds 8` exit 0, and its `GM_MUTATE_REFERENCE_DEGREE=9` control non-zero.
- The item-2 refactor moved no output: the `layout.dag.sugiyama` 4-way digest equals the review's
  `97c9f2fc3c829a7a85a737eb000cd96d883d40044fc45f290561f4bce364fb2f` (item 10 may move only `run_scaled`
  at `scale == 0`, which no gate seed uses).
- `capabilities --check` and `codegen --check` exit 0.
- `scripts/scigraphs-conformance.sh` exits 0 with no row moved, and `--break` exits 1.
- The merge floor: fmt, clippy `-D warnings`, `cargo test --workspace --no-fail-fast`.
