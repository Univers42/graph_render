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

## Addendum: rule D

Status (rule D): PROCEED-WITH-CONDITIONS, 2026-10-06

Rule D, as proposed: for each forward edge of `v`, in dense edge order, let `S` be the target's
smallest reserved lane if it has one. The edge **shares `S`** when `S` exists and either
`S < lane(v)`, or `lane(v)` is already carried by an earlier edge of `v`. Otherwise it is placed
exactly as under rule C — `lane(v)` to the first such edge (pushed into the target's reservation),
a free lane to a later one (pushed). After all of `v`'s forward edges, `lane(v)` goes back to the
pool if no edge took it. So a line bends **left** into a column already waiting for its target and
never bends right into one. Today's rule is the case "never share on the first edge"; rule C is
"always share".

The model is the same one the earlier ruling used, re-derived from the tree: `rows.rs`
(`crates/graph-core/src/layout/lanes/rows.rs:112-159`) + `assign.rs` (`:226-293`) +
`geometry.rs` (`:66-100`), with `Reserved::push`'s and `Pool::give`'s assertions live on every
draw. It reproduces every hand-built expectation in `crates/graph-core/src/layout/lanes/tests.rs`
exactly under today's rule: the chain (`:67-70`), branch-and-merge `pts == [1.0, 0.5, 1.0, 2.5]`
(`:89-93`), the cycle `pts == [1.0, 1.5, 1.0, 0.5]` and `notes == [2]` (`:105-109`), the
version-ordered cycle `y == [1.0, 0.0, 3.0, 2.0, 4.0]` (`:139`), and the parallel/undirected
case (`:159-165`). Its reproduction of the seeded gate model matches the committed
`server/graph-server/tests/digest/n400.json` edge for edge — 400 nodes, 619 edges, **0**
mismatches in `source`, `target` and `directed` — and it reproduces the job body's own
width/straightness table for all four real histories (below), so it is the model that produced
that table, not a different one. I ran no build, no test and no gate; every `cargo` row in the
conditions is UNKNOWN to me and unknown is failure.

### Rulings

| # | Question | Ruling | Evidence (path:line or command) |
|---|---|---|---|
| 1 | The trace: under rule C, is `a_branch_and_its_merge_take_two_lanes`'s `x` `[0, 1, 1, 0]` or `[1, 0, 1, 0]`? | **`[1, 0, 1, 0]`, and the earlier ruling's `[0, 1, 1, 0]` is wrong. This addendum is the correction; the text above it stands as written and is superseded on this value only.** Rows: `m` 0, `c` 1, `b` 2, `a` 3 (Kahn on `(version, -index)`, versions 1/2/3/4). Row 0 `m` settles on lane 0; `m→b` has no reservation at `b`, pushes 0; `m→c` has none either and `own_taken` is now true, so it takes a fresh lane 1 and pushes it. Row 1 `c` settles on 1; `c→a` finds `a` empty, pushes 1. Row 2 `b` settles on 0; `b→a` finds `a` holding `{1}`, and rule C shares unconditionally, so it takes 1 and pushes nothing; `b`'s lane 0 is then given back. Row 3 `a` settles on its smallest reservation, 1. `x = (a, b, c, m) = [1, 0, 1, 0]`. The orchestrator's trace is right and my earlier value was not: I mis-set the share's effect on `a`'s reservation. Under **D** the same trace gives `[0, 0, 1, 0]` — `b→a` sees `S = 1`, `1 > lane(b) = 0` and `own_taken` is false, so D pushes `lane(b) = 0` and `a` settles on 0 — **today's landed values, unchanged.** | `crates/graph-core/src/layout/lanes/tests.rs:74-94` (the re-pin, now wrong); `crates/graph-core/src/layout/lanes/assign.rs:236-248` (`carry`, the `k == 0` branch rule D replaces); `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md:69-70` (the promise rule C breaks). Trace reproduced by `/tmp/opencode/ruled/trace.py`, 43 lines, printing each row's `own`, `S`, `own_taken` and the lane chosen. |
| 2 | Correctness and the one-reservation invariant under D | **OK, with conditions 1 and 2. The earlier per-edge argument does cover rule D's per-edge mix, and D is strictly safer than C on the one case that matters.** The argument is per edge and D decides per edge: each edge either shares a lane **already in its target's sole reservation** (so it cannot double-reserve, and the shared lane stays reserved for that target continuously from the sharing row to the target's row), or pushes — and a push is either `lane(v)`, which `settle` has just released and no reservation holds, or a lane just removed from the pool, which no reservation holds either. There is no third action, so the invariant holds for any interleaving. **D removes C's one soft spot rather than adding one:** under C an edge can share a lane *larger* than the source's own while the source's own lane is still unreserved, and nothing about the rule prevents an intermediate vertex from taking that own lane; under D that edge pushes `lane(v)` instead, so `lane(v)` is reserved before any lower vertex can take it. Searched rule D and all three guard mutants: every graph on `n = 2, 3, 4` with directed-only edges (3 states per unordered pair), then five states per unordered pair (absent / `i→j` / `j→i` / undirected / both directions), then seven (adding two parallel edges, forward and mixed-direction) — **537,527 graphs × 3 version vectors × 4 rules**, plus **20,000 random** graphs `n ≤ 8`, **4,000 merge-heavy** (fan-in 2–8, parallel, mixed directedness), **3,000 dense** with parallel edges and self-loops `n = 6..18`, **600 large** `n = 20..200` over a chain backbone, **200 layered history-shaped** graphs (4–12 layers × 3–15 per layer, newest-first edge order), and the repo's own fan-in-50 shape (`crates/graph-core/src/layout/lanes/tests/history.rs:104`): **0 failures**, checking per placement that no lane is in two reservations, no reserved lane is in the free pool, no lane is given back above the width, no lane is given back while another reservation holds it, and no vertex's `(lane, row)` lies strictly inside any edge's span in its carrying lane. **The give-back-first build fails as C's did, so the assert stays load-bearing: 4,000 of 4,000 merge-heavy graphs** under D, first failure `lane 0 is reserved for 3 and free` — the same signature as C, not a new one. The geometry needs no change: `geometry.rs:84-91` already bends in and out wherever the carrying lane is not that end's own lane, which is exactly the share-at-once case. I also attacked the case the earlier ruling named, where D shares a lane **equal** to the source's own so `geometry.rs:84` emits no bend and the edge runs straight: it fires on **719** of the gate model's 277,658 edges and is safe, 0 vertex-inside-span hits, for the same reason (that lane is the source's own reservation, whose edges end at the source). | `assign.rs:103-113` (`push`), `:122-141` (`settle`, release-then-give order), `:148-158` (`assert_sole`), `:281-293` (`place`); `geometry.rs:84-91`; `tests/history.rs:104-125` (the fan-in-50 shape). Searches: `/tmp/opencode/ruled/search.py` (`exhaustive`, `rand`, `merge`, `dense`, `fan`, `gb`), `/tmp/opencode/ruled/big.py`, `/tmp/opencode/ruled/giveback.py`, `/tmp/opencode/ruled/straightcase.py`, `/tmp/opencode/ruled/check719.py`, all run 2026-10-06, all exit 0. |
| 3 | The pinning tests under D | **All three named tests keep today's values under D — one is RED against today's code, two are not. One new test is required; I name it and give its values.** `three_lines_forked_from_one_base_share_the_column_waiting_for_it` (the earlier ruling's condition-4 test, `a`/`b`/`c`/`d` at 1/2/3/4, edges `d→a`, `c→a`, `b→a` in that dense order): under D `x == [0.0, 1.0, 1.0, 0.0]`, `y == [3.0, 2.0, 1.0, 0.0]`, width 2, `carried == [0, 0, 0]`, `paths.offsets == [0, 0, 1, 2]`, `paths.pts == [0.0, 1.5, 0.0, 2.5]`, no notes — identical under C, and **RED against today's code** (today: `x == [0.0, 2.0, 1.0, 0.0]`, width 3, `pts == [1.0, 2.5, 2.0, 2.5]`). `a_branch_and_its_merge_take_two_lanes` (`tests.rs:74`): under D `x == [0.0, 0.0, 1.0, 0.0]`, `y == [3.0, 2.0, 1.0, 0.0]`, `offsets == [0, 0, 1, 1, 2]`, `pts == [1.0, 0.5, 1.0, 2.5]` — **today's values, unchanged, and the test does not move.** That is the point of D: the spec's first-parent promise at spec `:30`, `:57` and `:69-70` survives, so the re-pin the earlier ruling's condition 4 demanded **falls away**. `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` (`tests.rs:97`): under D `y == [0.0, 1.0, 2.0]`, `x == [0.0, 0.0, 0.0]`, `notes == [2]`, `offsets == [0, 0, 0, 2]`, `pts == [1.0, 1.5, 1.0, 0.5]` — **today's values, unchanged**, because `b→c` carries `b`'s own lane 0 (`S = 1` is not smaller than `0`, and `own_taken` is false), and `c→a` then shares 1 as it does today. **New test, both arms in one vertex:** `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge` on `a`/`b`/`c`/`d`/`e` at versions 1/2/3/4/5 with edges in dense order `a→b`, `a→c`, `a→e`, `c→e`, `d→b`. Rows: `d` 0, `a` 1, `c` 2, `e` 3, `b` 4. `d→b` pushes lane 0, so when `a` is placed it takes lane 1 and **its first edge `a→b` shares 0** (`S = 0 < 1`); `a→c` then pushes `lane(a) = 1`; `a→e` finds `own_taken` true and takes the fresh lane 2. Both arms of D run in one vertex, and `c→e` is the guard: `S = 2 > lane(c) = 1`, so D pushes 1 where C would share 2. Values: `x == [1.0, 0.0, 1.0, 0.0, 1.0]`, `y == [1.0, 4.0, 2.0, 0.0, 3.0]`, width 3, `carried == [0, 1, 2, 1, 0]`, `paths.offsets == [0, 1, 1, 3, 3, 3]`, `paths.pts == [0.0, 1.5, 2.0, 1.5, 2.0, 2.5]`, no notes. **RED against today's code** (today: `x == [1.0, 0.0, 2.0, 0.0, 2.0]`, width 4, `carried == [1, 2, 3, 2, 0]`, `offsets == [0, 1, 2, 4, 4, 4]`, `pts == [1.0, 3.5, 2.0, 1.5, 3.0, 1.5, 3.0, 2.5]`) **and it distinguishes D from every guard mutant**: dropping the `S < own` guard gives C's answer `x == [1.0, 0.0, 1.0, 0.0, 2.0]`, and removing the `own_taken` clause **crashes** on the one-reservation assert (`lane 1 is in two reservations`, holders 2 and 4). | `crates/graph-core/src/layout/lanes/tests.rs:53-71` (chain), `:74-94` (merge), `:97-110` (cycle), `:121-142` (version order); `assign.rs:236-248`; `geometry.rs:66-100`. Values from `/tmp/opencode/ruled/newtest2.py` (both-arms search over every 3- and 4-edge graph on `n ≤ 7` × every version permutation) and `/tmp/opencode/ruled/pin3.py` (a shape that separates D from `today`, `C`, `D_le`, `D_lt`, `D_noown`), both exit 0. |
| 4 | The gate model under D | **36.5% of edges — heavily reached, and no substitute test is needed.** Over **all 600 seeds** (`gate_node_count(seed) = 2 + seed % 600`, `crates/graph-core/src/stage/topology.rs:19`; 277,658 edges, my `synthetic.rs` port reproducing `n400.json` edge for edge), D shares on **101,226 edges that today's rule does not share — 36.5%**, on **599 of 600 seeds** (the one seed that reaches none has 2 nodes). Broken down by arm: D shares on a **first** forward edge (today's rule can never do this — it always pushes the own lane there, on all 180,300 first edges) on **101,226 edges, 36.5%**; on a later edge because `S < lane(v)`, **52,153 (18.8%)**; on a later edge because `lane(v)` is already carried, **9,255 (3.3%)**. So the branch rule D adds is not a corner: it fires on more than a third of all edges. Today's rule shares on 66,832 (24.1%), C on 198,647 (71.5%), D on 161,915 (58.3%). Mean width over the sweep: today 156.5, C 66.0, D 71.8 (width/n 0.527 / 0.225 / 0.243); seed 7 gives 7 / 2 / 2 and seed 42 gives 23 / 10 / 11. `hashgate --seeds 8` and the stage-scoped `GM_MUTATE_DAG_LANES_NODES=1` control are live gates for this change. **What the gate still does not reach is unchanged and still true:** 0 note-5 edges and 0 cycle breaks across all 600 seeds, because `version` is 0.0 everywhere (`crates/graph-core/src/synthetic.rs:122`) and the model's arcs are already a topological order — so `degradation`'s "tie-break path only" and the earlier ruling's condition 7 stand, and the digest rows are what pin the convention. | `crates/graph-core/src/synthetic.rs:37-51` (`Mulberry32`), `:92-171` (`synthetic_records`, `remix`), `crates/graph-core/src/stage/topology.rs:19,37-46`; `server/graph-server/tests/digest/n400.json` (619 edges, 0 mismatches). Measurements: `/tmp/opencode/ruled/gate.py`, `/tmp/opencode/ruled/branch.py`, `/tmp/opencode/ruled/mutgate.py`, `/tmp/opencode/ruled/eqcheck.py`, all exit 0. |
| 5 | C or D, and the earlier ruling's conditions 1–7 | **D, and it is D by the spec's own words, not by taste.** The spec says the first forward edge pushes `lane(v)` "the first-parent line continues straight" (`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md:69-70`), that the reference design keeps "the first parent kept straight" (`:30`), and that edge order exists "so a source that writes a record's first parent first keeps that line straight" (`:57`). Rule C breaks that promise on the first-parent line of a branch-and-merge — ruling 1's trace shows `m→b→a` no longer straight and the base in the merged branch's lane. Rule D bends **left only**, into a column already reserved for the target, so the first-parent line is preserved unless the target is already spoken for by a *lower* lane, which is the converging-lines case the spec describes at `:71-72`. Condition-by-condition: **(1) give-back after the forward edges — carries over unchanged**, and D needs it *more*: D's `own_taken` flag is the only thing that distinguishes "no edge took it" from "an edge shared it away", so the give must run after the loop. Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0 with debug assertions live. **(2) keep `assert_sole`, add the give-side half — carries over unchanged**, and D adds a third clause worth stating: the assert must also fire when `pool.give` is handed a lane another reservation holds. **(3) land the convention and its evidence in one commit, same id — carries over unchanged**, with one correction: because D leaves `a_branch_and_its_merge_take_two_lanes` and the cycle test alone, the re-pinned set is **one** test, not three. **(4) one hand-built shape pins the rule — changes.** New text: the pinning test is `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge` with the values in ruling 3, and it must be **RED on the parent commit** (today's values in ruling 3) and green on the child. `a_branch_and_its_merge_take_two_lanes` and `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` are **not** re-pinned and must keep their landed values; a build that changes them has implemented C, not D. The `three_lines_forked_from_one_base_share_the_column_waiting_for_it` test from the earlier ruling keeps its values and stays RED-first. **(5) name the pinning test in the `oracle` string, keep `nothing_sits_on_an_edge` — carries over unchanged**, with the name in condition 4 replacing the earlier ruling's. **(6) re-measure the width, do not bundle cause 2 — carries over unchanged**, and D's figures are now measured twice over: my model gives activitywatch 4 / 91.1%, aw-server-rust 4 / 93.2%, graph_render 38 / 88.0%, git/git 197 / 84.3% for width and first-parent straightness, matching the job body's D column (4/91.1, 4/93.2, 38/87.9, 197/84.3) to within 0.1pp on graph_render. **(7) run the rows this verdict could not — carries over unchanged.** | `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md:30,57,69-72`; `crates/graph-core/src/layout/lanes/assign.rs:236-248,281-293`; `crates/graph-core/src/layout/lanes/tests.rs:74-110`; `crates/graph-core/src/registry/lanes.rs:21-54`; `examples/plugins/git/git-log.sh:5` (`%at`, unchanged). Widths: `/tmp/opencode/ruled/real.py` and `/tmp/opencode/ruled/real2.py`, exit 0, over `git log --all --topo-order --format='%H%x1f%P%x1f%ct'` on `/tmp/gitviz/activitywatch.git` (1271 commits), `/tmp/gitviz/aw-server-rust.git` (989), this worktree (2242), `/tmp/gitviz/git.git` (85928). |

### The failure nobody mentioned

**D's `S < lane(v)` guard is the whole difference between C and D, and nothing but a
hand-written test can tell them apart.** Dropping it (`D_le`) does not break a single invariant —
0 failures across all 537,527 exhaustive draws, 20,000 random, 4,000 merge-heavy and 3,000 dense
graphs — and on the gate model it is **byte-identical to rule C on 600 of 600 seeds**, changing
127,625 edges (46.0% of all edges). So the guard that decides between the two rules is invisible
to every structural check the layout has: `assert_sole`, `nothing_sits_on_an_edge`, the oracle and
the hash gate all pass on both. The `assert_sole` argument is also not enough on its own —
removing the `own_taken` clause (`D_naive`) crashes on `lane 1 is in two reservations` in 1,304
of 6,000 merge-heavy draws, which is the one mutant the invariant *does* catch, but only because it
re-introduces a double push; the guard's *purpose* (a lane left unreserved while an edge runs in
it) is invisible until it is already corrupt. **Consequence: condition 4's named test is the only
evidence that D is D.** A build that implements "share whenever the target has a reservation" and
calls it D passes every gate in the tree and every row of condition 7, and ships C's drawing with
D's commit message. That is the same laundering risk the earlier ruling's condition 4 named, and it
is *worse* under D, not better: under C the guard does not exist, so there is nothing to get wrong;
under D it is one comparison whose absence no check notices.

The second, quieter hazard: **D shares left, which means the target's smallest reservation is what
an edge converges into, so lane numbers on a merge depend on the order in which *other* vertices'
edges arrived.** That is already true of C and of today's rule (the spec's convention, `:71-72`),
but D makes it bite on first edges too, so a merge's drawn lane is now decided by rows above it
rather than by the merging vertex. The evidence is clean — the same model, same seeds, same lane
choice — but it means the digest rows are sensitive to `version` changes in a way today's rule
is not, which is exactly why condition 6 keeps `%at → %ct` in a separate landing.

### Scores

| axis | score | why |
|---|---|---|
| blast radius | 3 | One function (`carry`) and one call site (`place`), exactly as under C, but a shipped public layout id whose **output** changes on 88.9% of the gate model's edges (246,756 of 277,658 differ from today's rule). Two digest pins, the 100-seed roundtrip oracle, the git plugin's bench, and every drawing a consumer has cached. No data, no schema, no security surface; `crates/graph-core/src/registry/layouts.rs:1-5` says an index-keyed consumer is not re-pointed because the append stays last. **Lower than C's 3 only in expectation, not in fact:** under D two of the three landed tests do not move, so the *test* blast radius is smaller while the *drawing* blast radius is the same size. |
| reversibility | 4 | High. `git revert` of one commit restores the old convention and the old `pts`; the digests are re-emitted from the tree, so nothing is hand-maintained state. It drops to 2 the moment the plugin's `%at → %ct` change (cause 2) lands on top, which is why condition 6 keeps them in separate landings. Unchanged from the earlier ruling — D and C differ in one comparison, not in reversibility. |
| cost on failure | 3 | A wrong drawing is cosmetic and visible — **unless** the evidence is re-emitted over it. Under D that risk is *higher* than under C in one specific way and lower in another: higher, because the guard that separates D from C breaks no invariant and so launders silently (the failure nobody named above); lower, because D leaves the spec's first-parent promise intact, so a wrong D is a wrong C only in the narrow "bent the wrong way" sense and the first-parent straightness measurement barely moves. The narrow guard is condition 4's named test, which must be shown RED first. |
| confidence | 4 | The algorithm is well-evidenced: the model is the landed code (six hand tests reproduced exactly, `seeded_model` reproduced against the committed `n400.json` edge for edge with 0 mismatches, and the job body's own four-history table reproduced to 0.1pp), and 537,527 exhaustive plus ~28,000 random, merge-heavy, dense, large, layered and fan-in-50 graphs give 0 counterexamples under D and every guard mutant. Held off 5 by two gaps I could not close: **I ran no build, no test and no gate** (this brief forbids it), so every `cargo` row below is UNKNOWN; and **D's `S < lane(v)` guard survives every structural check in the tree**, so the *convention* rests on one hand-written test rather than on an invariant, which is a narrower base than C had. |

**Worst axis: confidence (4).** The rule is the best-understood thing in this change; the unrun
build, and the one guard that no check can see, are what is left.

### Verdict

**PROCEED-WITH-CONDITIONS.** Rule D is the right rule: it is what the spec already promises, it
keeps the first-parent line straight where rule C breaks it, it is *safer* than C on the one case
where C's share leaves the source's own lane unreserved, and it costs 4 lanes of width on
git/git against C (197 vs 189, still 46% below today's 282) while giving back the first-parent
straightness C loses (84.3% vs 83.9%). The correctness claim survives a wider search than the
earlier ruling ran, and the gate model exercises D's new branch on more than a third of its edges.
Each condition below is an acceptance criterion for the build job, checkable by a command or a
test name; every `cargo` row is one I could not run.

1. **Implement rule D's guard verbatim, in `carry`, with both clauses.** In
   `crates/graph-core/src/layout/lanes/assign.rs:236-248` replace the `first`/`k == 0` branch with
   the two-clause test: share the target's smallest reserved lane `S` when `S` exists **and**
   (`S < lane(v)` **or** `lane(v)` is already carried by an earlier forward edge of `v`);
   otherwise push `lane(v)` if no earlier edge took it, else take the smallest free lane and push
   it. The give-back runs **after** the forward-edge loop, keyed on "no edge took `lane(v)`" —
   which under D means the `own_taken` flag, not `out.is_empty()`. Freeing `lane(v)` before the
   loop puts a lane in two reservations on **every** merge-heavy graph (4,000 of 4,000 measured).
   Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0 with debug assertions live;
   `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge`
   observed RED against the give-back-first build and green against this one.
2. **Keep `assert_sole` and add the give-side half, with three clauses.** `assign.rs:148-158` is
   what catches condition 1 and D adds the only path that hands a lane to the pool while the
   vertex still owns it. The push-side assert must keep both clauses it has (lane in no other
   reservation; lane not in the free pool) and the give-side assert must be added: a lane passed
   to `pool.give` is in **no** reservation. Measured: removing D's `own_taken` clause crashes this
   assert on `lane 1 is in two reservations` in 1,304 of 6,000 merge-heavy draws, so it is
   load-bearing and must not be relaxed to make a test pass. Checkable:
   `scripts/orch/gr cargo test -p graph-core lanes` exit 0 and
   `scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings` exit 0.
3. **The named test is the evidence, and it is the only evidence that D is D.** Add
   `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge` to
   `crates/graph-core/src/layout/lanes/tests.rs` on `a`/`b`/`c`/`d`/`e` at versions 1/2/3/4/5 with
   edges in dense order `a→b`, `a→c`, `a→e`, `c→e`, `d→b`, asserting exactly:
   `x == [1.0, 0.0, 1.0, 0.0, 1.0]`, `y == [1.0, 4.0, 2.0, 0.0, 3.0]`, width 3,
   `paths.offsets == [0, 1, 1, 3, 3, 3]`, `paths.pts == [0.0, 1.5, 2.0, 1.5, 2.0, 2.5]`, no
   notes. It must fail on the parent commit (today: `x == [1.0, 0.0, 2.0, 0.0, 2.0]`, width 4,
   `carried == [1, 2, 3, 2, 0]`) and pass on the child, and it must be shown to fail against the
   guard-dropped build, which draws C's `x == [1.0, 0.0, 1.0, 0.0, 2.0]` and passes every
   structural check in the tree. **Weakened to a bound or a `width` assertion, the build has no
   evidence that it implemented D.** Checkable: the test fails on the parent commit, passes on the
   child, and fails against the build with `S < lane(v)` removed.
4. **The other three tests keep their landed values; do not re-pin them.**
   `a_branch_and_its_merge_take_two_lanes` (`tests.rs:74`) keeps `x == [0.0, 0.0, 1.0, 0.0]`,
   `y == [3.0, 2.0, 1.0, 0.0]`, `offsets == [0, 0, 1, 1, 2]`, `pts == [1.0, 0.5, 1.0, 2.5]`;
   `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` (`tests.rs:97`) keeps
   `x == [0.0, 0.0, 0.0]`, `y == [0.0, 1.0, 2.0]`, `notes == [2]`, `offsets == [0, 0, 0, 2]`,
   `pts == [1.0, 1.5, 1.0, 0.5]`; and
   `three_lines_forked_from_one_base_share_the_column_waiting_for_it` keeps `x == [0.0, 1.0, 1.0,
   0.0]`, `y == [3.0, 2.0, 1.0, 0.0]`, width 2, `offsets == [0, 0, 1, 2]`, `pts == [0.0, 1.5, 0.0,
   2.5]`. **A build that changes any of these three has implemented rule C, not D** — the earlier
   ruling's condition 4 re-pin of the first test (`x == [0, 1, 1, 0]`) is **withdrawn**: it was
   wrong, and this addendum supersedes it. Checkable: the three tests pass unmodified on the child
   commit; `git diff -- crates/graph-core/src/layout/lanes/tests.rs` shows the two re-pinned
   assertions untouched.
5. **Land the convention and its evidence in one commit, same id.** `assign.rs:1-13` (the module
   doc, which currently states "Its first forward edge carries its own lane on" and must state
   rule D), `assign.rs:233-249`, `hand_oracles/lanes.rs:7-9` and `:194-205` (the oracle restates
   the rule step for step and must be restated for D, or the bit-for-bit comparison is a
   transcription check on the wrong convention), `registry/lanes.rs:21-54` (`oracle`,
   `degradation`, `Ponytail (lane choice)`), the new test, both digest rows **re-emitted through
   `GM_SVC_DIGEST_EMIT=1` and never hand-edited**, and `docs/measurements/dag-lanes.md`. No new
   id: `git grep -l 'layout.dag.lanes'` finds no consumer outside Rust,
   `examples/plugins/git/bench.mjs:29` and the two `manifest.json` rows. Checkable:
   `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` exit 0; a plain
   `cargo test -p graph-server digest` afterwards must not rewrite the manifest; `git diff --stat`
   names every file in this list.
6. **Name the pinning test in the `oracle` string, and keep `nothing_sits_on_an_edge`.** The
   oracle shares the rule by construction (`hand_oracles/lanes.rs:194-205` mirrors
   `assign.rs:236-248` step for step), so it cannot be the evidence for the convention; the
   evidence is condition 3's test plus
   `hand_oracles/lanes/compare.rs:151-174`, which reads lanes and rows rather than `carry`. Name
   `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge` in
   the `oracle` string. Checkable: `scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds
   100` exit 0 and `layout.dag.lanes on its stated conventions on 100/100 seeds`; the test name
   appears in `crates/graph-core/src/registry/lanes.rs`.
7. **Re-measure the width, and do not bundle cause 2 with it.** `examples/plugins/git/git-log.sh:5`
   feeds `%at`; D's lane choice on a merge depends on which rows above it reserved first, so it is
   more sensitive to a `version` change than today's rule. Land the `%ct` switch in a separate
   commit and re-run `docs/measurements/dag-lanes.md`'s "Commit DAGs" commands (`:150-171`),
   updating the `lanes width` column and the `Ponytail (lane choice)` clause's figure together.
   The figures to expect under D, width and first-parent straightness, are activitywatch 4 /
   91.1%, aw-server-rust 4 / 93.2%, graph_render 38 / 88.0%, git/git 197 / 84.3% — measured by my
   model on `git log --all --topo-order` output with committer time, and they must be re-measured
   on the real build, not copied. Checkable: the commands in
   `docs/measurements/dag-lanes.md:150-171` pasted with real exit codes, and
   `grep -c '%ct' examples/plugins/git/git-log.sh` still 0 in the motor's commit.
8. **Run the rows this verdict could not.** Everything requiring a build is UNKNOWN to me:
   `cargo build -p graph-wasm --target wasm32-unknown-unknown`, `cargo test -p graph-core lanes`,
   `roundtrip --seeds 100`, `hashgate --seeds 8` plus its `GM_MUTATE_DAG_LANES_NODES=1` control
   (non-zero), `capabilities --check`, and the digest suite before and after re-emission. Ruling 4
   says the gate model reaches D's new branch on 36.5% of its edges, so `hashgate` is a real gate
   here and not a formality. Checkable: the exit codes in the build job's return block; an unrun
   row is a failure, not a pass.
