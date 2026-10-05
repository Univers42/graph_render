# The devil's ruling on narrowing `layout.dag.lanes`: the first forward edge shares

Status: PROCEED-WITH-CONDITIONS, 2026-10-06

Rules on one change to a layout that has been on develop for hours, proposed in
`prompts/jobs/lanes-merge-verdict.md` and motivated by the "Commit DAGs" table of
`docs/measurements/dag-lanes.md:187-194`: a vertex's **first** forward edge always carries the
vertex's own lane down to its target (`crates/graph-core/src/layout/lanes/assign.rs:236-249`),
even when the target already holds a reservation, so every branch forked from one old base keeps
its own lane all the way down. The proposal: **an edge shares its target's smallest reserved lane
whenever the target has one, first edge included**; otherwise the vertex's own lane goes to the
first edge that needs one, any further edge takes a free lane, and a vertex whose lane no edge
took frees it after its row. Cause 2 (the git plugin feeding author time, `examples/plugins/git/
git-log.sh:5` `%at`) is a change under `examples/plugins/git/` only and needs no verdict here.

I re-implemented `rows.rs` + `assign.rs` + `geometry.rs` from the tree and attacked the proposal
with it. The model is the landed code, not a paraphrase of it: it reproduces every hand-built
expectation in `crates/graph-core/src/layout/lanes/tests.rs` exactly (`tests.rs:67-70` chain,
`:89-93` branch-and-merge `pts == [1.0, 0.5, 1.0, 2.5]`, `:105-109` cycle `pts == [1.0, 1.5,
1.0, 0.5]` and `notes == [2]`, `:139-141` the version-ordered cycle `y == [1.0, 0.0, 3.0, 2.0,
4.0]`, `:159-165` parallel/undirected), and its reproduction of `seeded_model` matches the
committed `server/graph-server/tests/digest/n400.json` edge for edge, endpoints and `directed`
included. I ran no build, no test and no gate: this job's brief forbids it, so every `cargo` row
in the conditions is UNKNOWN to me and unknown is failure.

## Rulings

| # | Question | Ruling | Evidence (path:line or command) |
|---|---|---|---|
| 1 | Correctness — does "no vertex sits on an edge that runs past it" still hold? | **OK, with conditions 1 and 2.** No counterexample at 6 vertices or below, and the guarantee stays structural rather than lucky. The rule only ever hands an edge a lane that is *already* in its target's single reservation, so the carrying lane is reserved for that target continuously from the sharing row down to the target's row; and a vertex's own lane is returned to the pool only when no edge took it, in which case nothing runs in it below that vertex's row (`assign.rs:122-141` releases `keep` without giving it back; the give site is `assign.rs:290-292`). The brief's two hazards survive both: the shared lane is in exactly one reservation (its target's), so no vertex at row `r + 1` can take the freed own lane while an edge runs there — the edge has left that lane by `r + 0.5`. I also attacked the case the brief does not name, where the shared lane happens to **equal** the source's own lane, so `geometry.rs:84` emits no bend and the edge runs straight down: it is safe for the same reason (that lane is the source's own reservation, whose edges end at the source), and my check covers it. Searched: every graph on `n <= 4` with every absent/directed/undirected state on every ordered pair, three version vectors each (1,596,537 draws), plus 130,000 random graphs — `n <= 7` general, merge-heavy `n = 5..14`, dense with parallel edges and self-loops `n = 8..20` — and the repo's own fan-in-50 shape (`tests/history.rs:104`). **0 failures under both rules**, checking per placement that no lane is in two reservations, no reserved lane is in the free pool, no lane is freed above the width, and no vertex's `(lane, row)` lies strictly inside any edge's span in its carrying lane. The geometry needs no change: `geometry.rs:84-91` already bends in and out wherever the carrying lane is not that end's own lane, which is exactly the share-at-once case; on the condition-4 shape the new drawing's `pts` is `[0.0, 1.5, 0.0, 2.5]`, both bends present and no interior point on a vertex row. | `assign.rs:236-249` (the rule that changes), `:281-293` (`place`, where the give-back moves), `geometry.rs:1-3` and `:84-91` (the bend already covers it), `assign.rs:12-13` (the promise), `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes/compare.rs:151-174` (`nothing_sits_on_an_edge`, the one check of this promise that does not restate the rule) |
| 2 | The one-reservation invariant (first verdict condition 1, the `debug_assert`s) | **OK, with condition 3 — and the failure nobody named is an ordering trap, not a rule defect.** Sharing never pushes, so it cannot double-reserve; a push is either the vertex's own lane, which no reservation holds after `settle`, or a lane just taken out of the pool, which no reservation holds either; and the give-back runs only when no edge took the lane, with `settle`'s own release-then-give order (`assign.rs:129-132`) keeping "free" and "reserved" disjoint. 0 invariant violations across every search in ruling 1. **The trap:** the obvious build gives the vertex's own lane back *before* running its forward edges, because that reads as "free it after its row". That build puts a lane in two reservations and frees a reserved lane — **40,000 failures in 40,000 merge-heavy graphs**, `lane 0 in two reservations (1,5)`, `lane 2 is free twice`. It is caught by the existing `assert_sole` (`assign.rs:148-158`), which is why that assert must survive the edit, and it is caught in a debug build only. The landed `today` rule has no give-back after the edges at all (its first edge always takes the lane, so `out.is_empty()` is the only give), which is why this code path is new and untested. | `assign.rs:103-113` (`push`), `:122-141` (`settle`), `:148-158` (`assert_sole`), `:281-293` (`place`), `:39-43` and `:94-98` (the debug-only `held`/`free_flag` counters the first verdict's condition 1 installed) |
| 3 | Changing a landed layout's output under the same id | **OK — same id, no new one, but the landing is not allowed to be partial.** No external consumer exists: `git grep -l 'layout.dag.lanes'` returns only Rust sources, `examples/plugins/git/bench.mjs:29`, `crates/graph-cli/src/snapshot_cmd/roundtrip/seed.rs:73` and the two `manifest.json` rows — no `packages/`, no `app/`, no published fixture. The id is also the safe kind of change to make: `registry/layouts.rs:1-5` states the table is append-only because the wasm module maps a layout by *index*, and `lanes` is last, so nothing is re-pointed. What is **not** safe is landing the motor without its evidence. In the same commit: `assign.rs:1-13` and `:233-249` (the rule, twice), the oracle `hand_oracles/lanes.rs:7-9` and `:194-205` (`compare.rs` needs no change), `registry/lanes.rs:21-54` (`oracle` names the convention's test, `degradation`, and the `Ponytail (lane choice)` clause whose width figure is now stale), the two digest rows, the affected tests, and `docs/measurements/dag-lanes.md`. The digest rows must be **re-emitted, never hand-edited**: `manifest.rs:11-14` makes `emit_the_manifest` the only writer and needs `GM_SVC_DIGEST_EMIT=1`. A red digest row is the expected, correct outcome of this change and must not be edited into green. | `crates/graph-core/src/registry/layouts.rs:1-5` and `:308` (append-only, index-keyed), `server/graph-server/tests/digest/manifest.json:52,111` (the two pins, `a143ef0c…` and `da49438b…`), `server/graph-server/tests/digest/manifest.rs:5-9,11-14` (re-emit, do not edit), `crates/graph-core/tests/geometry_invariants.rs:191-193` (invariant-based, passes either way), `examples/plugins/git/bench.mjs:29,70` (prints the width, so the bench is affected) |
| 4 | The hand oracle | **CONDITION 4 — it cannot catch a wrong rule, so a named test must.** The oracle restates the convention with different data structures, which is the right shape, but `carry` there (`hand_oracles/lanes.rs:197-212`) mirrors `assign.rs:236-249` step for step, including the `k == 0` special case — condition 4 of the first verdict already said so, and this change makes it worse, because the rule that decides *every* edge's lane would now be wrong on both sides at once and the bit-for-bit comparison would still pass. The pinning test is `three_lines_forked_from_one_base_share_the_column_waiting_for_it`: vertices `a` (base, version 1.0), `b` (2.0), `c` (3.0), `d` (4.0) and edges in dense order `d -> a`, `c -> a`, `b -> a` — three lines forked separately off one base, the base already waited for when the second and third fork. Expected, and **fails against today's code**: `y == [3.0, 2.0, 1.0, 0.0]`, `x == [0.0, 1.0, 1.0, 0.0]`, `carried == [0, 0, 0]`, width **2**, `paths.offsets == [0, 0, 1, 2]`, `paths.pts == [0.0, 1.5, 0.0, 2.5]`, no notes. Today draws width **3** (`x == [0.0, 2.0, 1.0, 0.0]`, `carried == [0, 1, 2]`, `pts == [1.0, 2.5, 2.0, 2.5]`), so the `x` assertion is RED before the change. Two landed tests change with the rule and must be re-pinned to the new numbers deliberately, not loosened: `a_branch_and_its_merge_take_two_lanes` (`tests.rs:74`, `x` becomes `[0,1,1,0]`, `offsets` `[0,0,1,2]`) and `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` (`tests.rs:97`, `x` becomes `[0,0,1]`, `offsets` `[0,0,1,2]`). The one check that survives a wrong rule is `nothing_sits_on_an_edge` (`compare.rs:151-174`), which reads lanes and rows rather than `carry`; keep it and name it in the `oracle` string. | `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs:194-205` (the restated `carry`), `crates/graph-core/src/layout/lanes/assign.rs:236-249` (what it mirrors), `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes/compare.rs:151-174` (`nothing_sits_on_an_edge`), `:90-123` (`interior_points`, unaffected), `crates/graph-core/src/layout/lanes/tests.rs:74-94,97-110` |
| 5 | The gate model — does it ever reach the new branch? | **OK, and heavily: no substitute test is needed for reachability.** The new branch fires when a target is already reserved when its first incoming forward edge is processed, and on the gate model that is the common case, not a corner: over **all 600 seeds** (`gate_node_count(seed) = 2 + seed % 600`, 277,658 edges) a first forward edge **shares** on **131,289 edges, 47.3%**; today's rule has 64.9% of edges take the vertex's own lane instead. Mean lane count falls 156.5 -> 66.0 (seed 7: 7 -> 2; seed 42: 23 -> 10). So `hashgate --seeds 8` and the stage-scoped `GM_MUTATE_DAG_LANES_NODES=1` control are live gates for this change. What the gate still does **not** reach is unchanged and still true: I re-measured **0 note-5 edges and 0 cycle breaks across all 600 seeds**, because `version` is 0.0 everywhere (`synthetic.rs:122`) and the model's arcs are already a topological order — so `degradation`'s "tie-break path only" and condition 7 stand as written, and the digest rows (not the hash gate) are what pin the convention. | `crates/graph-core/src/synthetic.rs:122` (`version: 0.0`), `:132-160` (`i -> earlier(i)`), `crates/graph-core/src/stage/topology.rs:19` (`gate_node_count`), `:37-46` (`remix` redraws `kind` and `child_first` but **not** `directed`, which is why the model reproduces `n400.json`), `crates/graph-core/src/registry/lanes.rs:33-41` (`degradation`) |

## Scores

| axis | score | why |
|---|---|---|
| blast radius | 3 | One function (`carry`) and one call site (`place`), but a shipped public layout id whose **output** changes on every input with a merge: two digest pins, the 100-seed roundtrip oracle, the git plugin's bench, and every drawing a consumer has cached. No data, no schema, no security surface, and `registry/layouts.rs:1-5` says an index-keyed consumer is not re-pointed because the append stays last. Not one function; not the whole system. |
| reversibility | 4 | High. `git revert` of one commit restores the old convention and the old `pts`; the digests are re-emitted from the tree, so nothing is hand-maintained state. It drops to 2 the moment the plugin's `%at -> %ct` change (cause 2) lands on top: two independent width changes in two files then need two reverts to separate, which is why condition 6 keeps them in separate landings. |
| cost on failure | 3 | A wrong drawing is cosmetic and visible — **unless** the evidence is re-emitted over it, and that is the real cost here: the oracle restates the rule, so a wrong rule passes both digest pins and all 100 oracle seeds and is laundered into committed evidence. The narrow guard is condition 4 (one hand-built shape with its lanes written down, RED first) plus keeping `nothing_sits_on_an_edge`. |
| confidence | 4 | The algorithm is well-evidenced: the model is the landed code (five hand tests reproduced exactly, and `seeded_model` reproduced against the committed `n400.json`), and 1.6M exhaustive plus 130k random graphs give 0 counterexamples under both rules. Held off 5 by two gaps I could not close: **I ran no build, no test and no gate** (this brief forbids it), so every `cargo` row below is UNKNOWN; and the width figures in the job body are a *model's* prediction — it reports 366 lanes on git/git against 364 measured, a 0.5% gap that means the rule is validated but the widths are still predictions. |

**Worst axis: confidence (4).** The rule is the best-understood thing in this change; the unrun
build and the unverified width numbers are what is left.

## Verdict

**PROCEED-WITH-CONDITIONS.** The narrowing is sound, the correctness claim survives a wider
search than the first verdict ran, and the gate model turns out to exercise the new branch on
half its edges. Nothing here needs re-submitting. Each condition is an acceptance criterion for
the build job, checkable by a command or a test name; every `cargo` row is one I could not run.

1. **The give-back runs after the forward edges, never before.** In `place`
   (`crates/graph-core/src/layout/lanes/assign.rs:281-293`), an edge that shares leaves the
   vertex's own lane unused, and that lane must be returned to the pool only once every forward
   edge has been processed. Freeing it first — the reading of "frees it after its row" that looks
   natural — puts a lane in two reservations on **every** merge-heavy graph I drew (40,000 of
   40,000). Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0 with debug
   assertions live, and the new test from condition 4 observed RED against that build.
2. **Keep `assert_sole` and add the give-side half.** `assign.rs:148-158` is what catches
   condition 1, and this change adds the only path that hands a lane to the pool while the
   vertex still owns it. Add the matching check that a lane passed to `pool.give` is in no
   reservation. Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0 and
   `scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings` exit 0.
3. **Land the convention and its evidence in one commit, same id.** `assign.rs:1-13` and
   `:233-249`, `hand_oracles/lanes.rs:7-9` and `:194-205`, `registry/lanes.rs:21-54`
   (`oracle`, `degradation`, `Ponytail (lane choice)`), the re-pinned tests, both digest rows
   re-emitted through `GM_SVC_DIGEST_EMIT=1` (never hand-edited), and the measurement. No new id:
   `git grep -l 'layout.dag.lanes'` finds no consumer outside Rust, `bench.mjs` and the manifest.
   Checkable: `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` exit 0; a plain
   `cargo test -p graph-server digest` run afterwards must not rewrite the manifest; `git diff
   --stat` names every file in this list.
4. **One hand-built shape pins the rule, in numbers, RED first.**
   `three_lines_forked_from_one_base_share_the_column_waiting_for_it` in
   `crates/graph-core/src/layout/lanes/tests.rs`, on `a`/`b`/`c`/`d` with versions 1.0/2.0/3.0/4.0
   and edges `d -> a`, `c -> a`, `b -> a` in that dense order: `x == [0.0, 1.0, 1.0, 0.0]`,
   `y == [3.0, 2.0, 1.0, 0.0]`, width 2, `paths.offsets == [0, 0, 1, 2]`,
   `paths.pts == [0.0, 1.5, 0.0, 2.5]`, no notes. Today's code draws `x == [0.0, 2.0, 1.0, 0.0]`,
   width 3, `pts == [1.0, 2.5, 2.0, 2.5]`. Re-pin
   `a_branch_and_its_merge_take_two_lanes` and
   `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` to the new values in the same
   commit. Checkable: the new test fails on the parent commit and passes on the child; both
   re-pinned tests pass with the exact values written above, not with a loosened bound.
5. **Say in `registry/lanes.rs`'s `oracle` string which test pins the convention, and keep
   `nothing_sits_on_an_edge`.** The oracle shares the rule by construction
   (`hand_oracles/lanes.rs:194-205`), so it is a transcription check and cannot be the evidence
   for the convention; the evidence is condition 4's test plus
   `hand_oracles/lanes/compare.rs:151-174`, which is derived from lanes and rows rather than
   from `carry`. Checkable: `scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds 100`
   exit 0 and `layout.dag.lanes on its stated conventions on 100/100 seeds`.
6. **Re-measure the width, and do not bundle cause 2 with it.** `examples/plugins/git/git-log.sh:5`
   feeds `%at`; the reference tool's `--date-order` uses committer time, so the widths in the job
   body are measured under a different `version` than the reference's. Switching to `%ct` is a
   change under `examples/plugins/git/` and needs no verdict, but if it lands in the same commit
   the motor's improvement and the plugin's are indistinguishable in the table. Land them apart and
   re-run `docs/measurements/dag-lanes.md`'s "Commit DAGs" commands, updating the `lanes width`
   column and the `Ponytail (lane choice)` clause's figure together. Checkable: the commands in
   `docs/measurements/dag-lanes.md:150-171` pasted with real exit codes, and
   `grep -c '%ct' examples/plugins/git/git-log.sh` still 0 in the motor's commit.
7. **Run the rows this verdict could not.** Everything requiring a build is UNKNOWN to me:
   `cargo build -p graph-wasm --target wasm32-unknown-unknown`, `cargo test -p graph-core lanes`,
   `roundtrip --seeds 100`, `hashgate --seeds 8` plus its
   `GM_MUTATE_DAG_LANES_NODES=1` control (non-zero), `capabilities --check`, and the digest suite
   before and after re-emission. Ruling 5 says the gate model reaches the new branch on 47.3% of
   its edges, so `hashgate` is a real gate here and not a formality. Checkable: the exit codes in
   the build job's return block; an unrun row is a failure, not a pass.
