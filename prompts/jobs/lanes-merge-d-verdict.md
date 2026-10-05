# Job lanes-merge-d-verdict (agent devil, docs only: an addendum ruling on rule D for `layout.dag.lanes`)

Why: `docs/decisions/dag-lanes-merge.md` (landed 2026-10-06) rules PROCEED-WITH-CONDITIONS on
rule C:
- an edge shares its target's smallest reserved lane whenever the target has one;
- otherwise the vertex's own lane goes to the first edge that needs a lane, and a later edge takes
  a free lane;
- a vertex whose lane no edge took frees it after its forward edges.

Its condition 4 re-pins `a_branch_and_its_merge_take_two_lanes`
(`crates/graph-core/src/layout/lanes/tests.rs:74-94`) to `x == [0, 1, 1, 0]`. The orchestrator's
trace of rule C on that test gives `[1, 0, 1, 0]` instead, and breaks the spec's promise:
- Vertices: a 1.0, b 2.0, c 3.0, m 4.0. Edges in order: m→b, m→c, b→a, c→a. Rows: m 0, c 1, b 2,
  a 3.
- Row 0: m takes lane 0. m→b: b holds nothing, so m's own lane 0 goes to b. m→c: c holds
  nothing and the own lane is used, so a new lane 1 goes to c.
- Row 1: c settles on 1. c→a: a holds nothing, so c's own lane 1 goes to a.
- Row 2: b settles on 0. b→a: a holds {1}, so under rule C the edge shares 1, and b's lane 0 is
  freed.
- Row 3: a settles on 1. So `x` (a, b, c, m) is `[1, 0, 1, 0]`.

The first-parent line m→b→a no longer runs straight, and the base sits in the merged branch's
lane. The spec promises the first parent kept straight
(`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md:30`, `:57`, `:69-70`), and the
reference drawing keeps the base under b.

**Rule D**, proposed instead. For each forward edge of `v`, in edge order:
- Let `S` be the target's smallest reserved lane, if it has one.
- The edge **shares `S`** when `S` exists and either:
  - `S < lane(v)`, or
  - `lane(v)` is already carried by an earlier edge of `v`.
- Otherwise it is placed exactly as under rule C:
  - `lane(v)` goes to the first such edge, pushed into the target's reservation;
  - a later one takes a free lane and pushes it.
- After all of `v`'s forward edges, `lane(v)` is given back if no edge took it (rule C's
  condition 1).

So a line bends left into a column that is already waiting for its target. It never bends right
into one. Today's rule is the case "never share on the first edge"; rule C is "always share".

The orchestrator's model replays Kahn with the ready max-heap on `(version, -index)` and the
three rules. It uses committer time as `version` (the logs the plugin will feed once cause 2
lands). The last column is the share of first-parent edges whose two ends sit in one lane.

| input | n | today: width, straight % | C | D |
|---|---:|---|---|---|
| activitywatch | 1271 | 6, 91.1 | 4, 91.0 | 4, 91.1 |
| aw-server-rust | 989 | 8, 93.2 | 4, 93.2 | 4, 93.2 |
| graph_render | 2187 | 40, 87.8 | 37, 86.0 | 38, 87.9 |
| git/git | 85928 | 282, 84.3 | 189, 83.9 | 197, 84.3 |

Read, in full:
- `docs/decisions/dag-lanes-merge.md` (your earlier ruling: re-use its model and its search);
- `docs/decisions/dag-lanes.md`;
- `crates/graph-core/src/layout/lanes/assign.rs`, `geometry.rs`, `tests.rs`;
- `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`;
- the spec's "Algorithm" section (`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`).

Rule on each of these. For each, one line: OK, or a condition the build job must meet.
1. **The trace above.** Under rule C, is `a_branch_and_its_merge_take_two_lanes`'s `x` equal to
   `[0, 1, 1, 0]` or `[1, 0, 1, 0]`? Give the trace. If the earlier ruling's value is wrong, say
   so: the addendum is the correction. Do not edit the earlier text.
2. **Correctness and the one-reservation invariant under D.** Your earlier argument (rulings 1
   and 2) is per edge: each edge either shares a lane already in its target's sole reservation,
   or pushes the vertex's own lane or a fresh lane. Does it therefore cover any per-edge mix of
   the two actions, and so rule D? Run your search on D: the exhaustive `n <= 4` set, the random
   sets, and the fan-in-50 shape. Also run the give-back-first build of D: it must fail as C's
   did, so the assert stays load-bearing.
3. **The pinning tests under D**, each with its exact values, each RED against today's code where
   the rule changes the result:
   - `three_lines_forked_from_one_base_share_the_column_waiting_for_it`: your shape; the model
     gives `x == [0, 1, 1, 0]` under D, as under C.
   - `a_branch_and_its_merge_take_two_lanes`: the model gives today's values unchanged under D.
   - `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` (`tests.rs:97-110`): the model
     gives today's values unchanged under D (b→c carries b's own lane 0, because `S = 1` is not
     smaller).
   - one new test where D shares on a **first** edge whose target already waits in a smaller
     lane, while a later edge of the same vertex still pushes a fresh lane, so both arms of D
     run in one vertex. Name it and give its exact `x`, `y`, `paths.offsets`, `paths.pts`.
4. **The gate model under D.** Over all 600 seeds, on what share of edges does D share where
   today's rule would not? If it is zero, name the test that covers it instead.
5. **C or D.** Which one does the build implement? For each of the earlier ruling's conditions
   1-7, say whether it carries over unchanged, changes (give the new text), or falls away.

Score the four axes 1-5 (blast radius, reversibility, cost on failure, confidence) and name the
worst. Then give one verdict on rule D: PROCEED, PROCEED-WITH-CONDITIONS (numbered conditions,
each checkable by a command or a test name), or BLOCK (what to resolve).

Append to `docs/decisions/dag-lanes-merge.md`, after its last line, and do not change any line
above it:
- a section `## Addendum: rule D`;
- a line `Status (rule D): <verdict>, 2026-10-06`;
- the five rulings as a table `| # | Question | Ruling | Evidence (path:line or command) |`;
- the scores;
- the conditions, complete: the build job reads this section alone for its acceptance criteria.

You may run `git grep`, `sed -n`, read any file, and run your own models with `python3` from
`/tmp` (no repository file). Do not run builds, tests, benches or gates other than the one below.

Paths you may touch: `docs/decisions/dag-lanes-merge.md` (append only). Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-lanes-merge-d-verdict scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS;
- `grep -E '^Status \(rule D\): (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), 2026-10-06$' docs/decisions/dag-lanes-merge.md`
  prints one line;
- `git diff -U0 docs/decisions/dag-lanes-merge.md | grep -c '^-[^-]'` prints 0.

Return: the branch tip, the verdict, each condition, and every deviation from this brief.
