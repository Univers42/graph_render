# Job lanes-merge-verdict (agent devil, docs only: rule on narrowing `layout.dag.lanes` before code)

Why: `layout.dag.lanes` landed on 2026-10-06 (`docs/decisions/dag-lanes.md`, PROCEED-WITH-
CONDITIONS). Its "Commit DAGs" measurement (`docs/measurements/dag-lanes.md`) shows it draws
real histories much wider than the reference tool does:

| input | lanes width | `git log --graph` width |
|---|---|---|
| git/git | 364 | 106 |
| aw-server-rust | 14 | 3 |
| graph_render | 40 | 26 |

A model of the layout written by the orchestrator explains most of the gap. The model is Kahn
with a ready max-heap on `(version, -index)`, then the lane rules of
`crates/graph-core/src/layout/lanes/assign.rs:1-13`. It reproduces the measured widths: 2, 6, 14,
40 and 366, against the 364 measured on git/git.

**Cause 1 (the motor).** `assign.rs:236-249` `carry`: a vertex's **first** forward edge always
carries the vertex's own lane down to its target. It does so even when the target already has a
reserved lane. So every branch forked from one old base holds its own lane all the way down to
that base. The reference tool joins such a line into the column already waiting for the base, one
row later.

The proposed rule: an edge shares its target's smallest reserved lane whenever the target has one.
This applies to the first edge too, as it already does to later ones. Otherwise the vertex's own
lane goes to the first edge that needs a lane, and any further edge takes a free lane. A vertex
whose lane no edge took frees it after its row. Widths in the model, with committer time as
`version`:

| input | today | proposed |
|---|---|---|
| git/git | 282 | 189 |
| graph_render | 40 | 37 |
| aw-server-rust | 8 | 4 |
| activitywatch | 6 | 4 |

The reference tool's `--date-order` gives 181, 4 and 4 on the inputs it was run on.

**Cause 2 (the plugin, not the motor).** The plugin feeds author time (`%at`) as `version`.
Rebased commits keep old author times, so a version-first order interleaves branches. Committer
time (`%ct`) is what `--date-order` uses. Switching it is a change under `examples/plugins/git/`
only, and it needs no verdict. It is listed so the numbers above are read correctly.

The geometry needs no change. `crates/graph-core/src/layout/lanes/geometry.rs:1-3` already bends
an edge into its carrying lane half a row after its earlier end when that lane is not the
vertex's own (`:85`). Later edges use that path today.

You rule on Cause 1. You write no code.

Read, in full:
- `docs/decisions/dag-lanes.md` (the first verdict and its nine conditions);
- `crates/graph-core/src/layout/lanes/assign.rs`, `geometry.rs` and `lanes.rs`;
- `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs` (the hand oracle, which states the
  same convention: `carry`, `:194-205`);
- `docs/measurements/dag-lanes.md`, sections "Commit DAGs" and "What the gate model does not
  reach".

Rule on each of these. For each, one line: OK, or a condition the build job must meet.
1. **Correctness.** Does "no vertex sits on an edge that runs past it" still hold? An edge that
   shares at once leaves the vertex at row `r` and bends into the shared lane at `r + 0.5`.
   - The vertex's own lane can be taken by a vertex at row `r + 1`.
   - The shared lane is in exactly one reservation, its target's.

   Try to break it with at most 6 vertices; if you find a counterexample, BLOCK and give it.
2. **The one-reservation invariant** (condition 1 of the first verdict, the `debug_assert`s in
   `assign.rs`). Does the new rule keep every lane in at most one reservation, and no reserved
   lane free?
3. **Changing a landed layout's output under the same id.** `layout.dag.lanes` has been on develop
   for hours. Its consumers are:
   - the hash gate's own runs;
   - two digest pins (`server/graph-server/tests/digest/manifest.json`);
   - the roundtrip hand oracle;
   - the git plugin's bench.

   Is a new id required, or is changing the convention under the same id acceptable before any
   external consumer exists? Say what must change in the same landing: oracle, digests, tests,
   the `degradation`/`ponytail` text, the measurement.
4. **The hand oracle.** It must state the new rule too, so it cannot independently catch a wrong
   rule. Name the test that pins the new behaviour on a hand-built shape (a branch forked from a
   base another line already waits for), with its exact expected lanes. It must fail against
   today's code.
5. **The gate model.** Does the seeded model ever reach the new branch? It does if a target is
   already reserved when its first incoming forward edge is processed. If it never does, say which
   test covers it instead.

Score the four axes 1–5 (blast radius, reversibility, cost on failure, confidence) and name the
worst. Then give one verdict: PROCEED, PROCEED-WITH-CONDITIONS (numbered conditions, each
checkable by a command or a test name), or BLOCK (what to resolve).

Write it to `docs/decisions/dag-lanes-merge.md`:
- a title;
- `Status: <verdict>, 2026-10-06`;
- the five rulings as a table `| # | Question | Ruling | Evidence (path:line or command) |`;
- the scores;
- the conditions.

Nothing else.

You may run `git grep`, `sed -n` and read any file. Do not run builds, tests, benches or gates
other than the one below.

Paths you may touch: `docs/decisions/dag-lanes-merge.md`. Nothing else.

Done when:
- `scripts/orch/gate.sh target/rows-lanes-merge-verdict scripts/orch/rows/docs.rows` writes a
  `summary.txt` with every row PASS;
- `grep -E '^Status: (PROCEED|PROCEED-WITH-CONDITIONS|BLOCK), 2026-10-06$' docs/decisions/dag-lanes-merge.md`
  prints one line.

Return: the branch tip, the verdict, each condition, and every deviation from this brief.
