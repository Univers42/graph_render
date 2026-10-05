# The devil's verdict on `layout.dag.lanes`

Status: PROCEED-WITH-CONDITIONS, 2026-10-05

Rules on the design (`docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`) and the
implementation plan (`docs/superpowers/plans/2026-10-05-dag-lanes-layout.md`) for a new public
capability: a registry entry appended to the wasm-indexed `LAYOUTS`, a capabilities row,
published parameters, and `server/` caps and digest rows. The house rule
(`.claude/rules/devil/risk.md:10-22`) makes the verdict mandatory: this is a public surface
(`risk.md:17`) with a wide blast (`risk.md:19`).

The load-bearing correctness claim survives attack. I could not break it, and the reason it holds
is structural rather than lucky — conditions 1 and 2 below say how that gets pinned. The real
defects are elsewhere: a metadata claim the bench has not earned, a gate model that exercises
almost none of the algorithm, and an oracle whose comparison surface is narrower than the
layout's output.

## Rulings

| # | Question | Ruling | Evidence (path:line or command) |
|---|---|---|---|
| 1 | Correctness — "no vertex sits on an edge that runs past it" | **OK, with condition 1.** I re-implemented `rows.rs` + `assign.rs` + `geometry.rs` from the plan verbatim and searched for a counterexample of at most 6 vertices. None exists, and the guarantee is structural: each lane is in at most one reservation, a reserved lane is never in the free pool, and a lane is freed only at the row of the vertex whose reservation held it. Exhaustive over **all** graphs on n <= 4 with every directed/undirected/absent state on every ordered pair (16,781,328 graphs), plus 200k random graphs to n = 7, plus 4,000 merge-heavy adversaries (fan-in <= 8, parallel edges, mixed directedness) — 0 failures, including an explicit per-placement assertion that no lane is in two reservation lists and no reserved lane is in the pool. The same harness reproduced all four of the plan's hand-computed assertions exactly (`a_chain`, `a_branch_merge` `pts == [1.0,0.5,1.0,2.5]`, the cycle case `pts == [1.0,1.5,1.0,0.5]`, and the parallel/undirected case), so it models the planned code rather than a different algorithm. | plan `assign.rs:554-555` ("A lane is in at most one reservation at a time, so `next` is indexed by lane") is the claim; `assign.rs:574-587` `settle` and `assign.rs:669-681` `place` are what make it true. Spec `:83-85` states the consequence. The plan's own test `no_vertex_sits_on_an_edge_it_does_not_end` (plan `:227-249`) covers 8 seeds at n = 2000 — a good but narrow subset of what I swept. |
| 2 | Determinism (D1–D10) | **OK, with conditions 2 and 3.** Both heaps are total orders: `Ready`'s `Ord` is `version.total_cmp` then index (plan `rows.rs:371-377`), and the pool is `BinaryHeap<Reverse<u32>>` (plan `assign.rs:536`). No `HashMap`/`HashSet` anywhere; every container is `Vec` or `BTree*`. No wall clock, no `mul_add`, no randomness. I checked the one float claim that could have broken D1: the plan's oracle key `total_key` (plan `:983-986`) is order-isomorphic to `f64::total_cmp` — 0 disagreements against an independently formulated IEEE-754 `totalOrder` reference over 2,402,800 comparisons, and it reproduces every property std documents (`-0.0 < 0.0`, `-NaN < -inf`, `inf < +NaN`). `u32` on the wire throughout; the `as f32` casts are on `u32` lanes and rows only. **But** the coordinates lose their half-row offset above n = 2^23 — see condition 3. | `prompt.md` §6; plan `rows.rs:371-391`; plan `assign.rs:524-560`; `crates/graph-contract/src/geometry.rs:159-164` (`Paths` is `u32` offsets). |
| 3 | The hand oracle's independence | **CONDITION 4 — the weakest link.** It does use different structures (`BTreeSet` ready set vs `BinaryHeap`; `BTreeMap<u32,BTreeSet<u32>>` vs the intrusive `head`/`next`/`min` lists), and it rebuilds its input from `seeded_model` rather than reading the snapshot back, which is the right shape. **Shared steps it will not catch:** (a) both sides iterate forward edges in **dense edge index order** — the oracle's `out.entry(early).or_default().push((e, late))` (plan `:1026`) and `Forward::of`'s counting sort (plan `:605-619`) encode the same convention twice, so a wrong edge order passes; (b) both sides implement "vertex takes the smallest reserved lane, else the smallest free lane, else a new one" and "first forward edge keeps the vertex's lane, a later one shares the target's smallest reserved lane" — the oracle's `mine.first()` / `set.insert(own)` (plan `:1034-1040`) restates `settle` and `carry` (plan `:574-587`, `:638-650`) step for step, including the `k == 0` special case; (c) the `BTreeSet` `first()` and the layout's `min[v]` are different code for the same fact, but a wrong *choice of minimum* is invisible to both. The oracle is a transcription check on the data structures, not an independent derivation of the convention. Worse, the comparison surface is narrower than the layout's output: `compare` is specified in prose only (plan `:1048-1055`), and the `oracle` metadata string claims "compared bit for bit per seed" without saying which columns. | plan `:1024-1045` (oracle `rows`/`assign`) vs plan `:415-495` and `:597-682` (layout `rows`/`assign`); `crates/graph-cli/src/snapshot_cmd/hand_oracles.rs:1-8` states the independence claim; `hand_oracles.rs:41-56` (`circular`) is the shape the plan copies. |
| 4 | The public surface | **OK, with conditions 5 and 6.** Appending last is genuinely safe: `layouts.rs` is 308 lines, its header states it is append-only because the wasm module maps a layout by *index* (`registry/layouts.rs:1-5`), and `dot` is the last entry — so the append repoints nothing. `MAX_SCALE_CEILING` is `TOPOLOGY_CEILING` = 9,200,000 (`capabilities/registry.rs:24`), so a declared 1,000,000 is inside the bound. **`scale_ceiling: 1_000_000` is a Ponytail that is not labelled as one in the `ponytail` field** — every comparable entry carries an explicit `Ponytail (scale_ceiling)` clause saying it is an estimate (`registry/grid.rs:38-40`, `:83-84`), and the plan's `ponytail` string (plan `:847-852`) has three Ponytails and none of them is the ceiling. **`degradation` is accurate but incomplete**: it says width grows "up to n on an antichain" and mentions the seeded model, which is right, but it does not say what a consumer sees. **`ponytail` omits the escape hatch on all three clauses**, which the house rule requires (AGENTS.md: "heuristics get one (failing input, direction, escape hatch)"); the spec's own draft has it on the lane clause (spec `:119`, "Escape hatch: none needed") and the plan drops it. | `crates/graph-core/src/registry/layouts.rs:1-5,308`; `crates/graph-cli/src/capabilities.rs:111-116` (`MAX_SCALE_CEILING`); `crates/graph-cli/src/capabilities/registry.rs:24`; `crates/graph-core/src/registry/grid.rs:38-40`; plan `:827-853` (the metadata the build job writes); plan `:1091-1094` (the bench that would justify the ceiling). |
| 5 | The gate model | **CONDITION 7 — the most consequential finding.** A layout that draws the gate model n lanes wide is a legitimate registry entry, and `degradation` already declares it. But the gate model is a far weaker test of *this* layout than it looks, and the metadata understates it. I reproduced `synthetic_records` + `remix` exactly (`synthetic.rs:92-171`, `stage/topology.rs:37-46`) and ran the planned algorithm over **all 600 gate seeds** (`gate_node_count(seed) = 2 + seed % 600`): width settles at about 0.6 n (353 lanes at n = 601), consistent and bounded — fine. **The number that matters: across all 600 seeds the drawing carries ZERO note-5 edges and never breaks a cycle.** The seeded model's directed edges always point from a higher index to a lower one (`synthetic_edges` builds `i -> earlier(i)`), and with every `version` = 0.0 (`synthetic.rs:122`) Kahn's order is index order, so no arc ever runs against the grain. So the hash gate never executes the cycle-breaking branch, the `EdgeReversed` note path, the `lowest_unplaced` forward scan, or the `Ready` heap's `version` comparison — it only ever exercises the tie-break-by-index path. `version` is the field the layout's own Ponytail is about, and the gate model holds it at a constant. The gate proves the layout is deterministic and self-consistent; it does not prove the convention. The per-stage knob the plan adds (`GM_MUTATE_DAG_LANES_NODES`) inherits the same blind spot. | `crates/graph-core/src/synthetic.rs:122` (`version: 0.0`), `:132-160` (`i -> earlier(i)`, `directed: kind == EdgeKind::Relation`); `crates/graph-core/src/stage/topology.rs:19` (`gate_node_count`), `:37-46` (`remix` redraws `kind` and `child_first` but **not** `directed`, so only the edges that stay `Relation` are directed); `crates/graph-core/src/weights.rs:25` (`REFERENCE_DEGREE = 8`); plan `:1102` ("Real-history inputs are measured by the git plugin job"). |
| 6 | Scope — does anything under `crates/` name a data source? | **OK.** I ran the plan's own gate command's regex against every `rust` block in the plan: **0 hits**. The forbidden words appear in the plan only in the *prose* that states the rule (plan `:31-33`) and in the git commands the worker runs (plan `:763-772`, `:915`, `:1079`, `:1102-1103`) — never in code destined for `crates/`. The one place a name could leak is the test helper `fn history(n, seed)` (plan `:207`), whose doc says "A seeded history-shaped DAG"; `history` is not on the forbidden list and describes a shape, not a source, so it passes as written — but it is the single word most likely to be edited into a source name later, so condition 8 makes the gate row permanent rather than a one-time check. The measurement doc (`docs/measurements/dag-lanes.md`) names its inputs, which the spec explicitly permits (spec `:10-13`, `:147-156`). | plan `:31-34` (the rule and its gate); plan `:33` (`git grep -n -i -E '\bgit\b|commit|repositor|activitywatch' -- crates/graph-core/src/layout/lanes* crates/graph-core/src/registry/lanes.rs crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`); plan `:205-224` (the `history` helper); spec `:10-13`. |

### A divergence I am ruling on, not blocking

`acyclic.rs:52-71` puts an `EdgeReversed` note on **every** non-loop edge whose source ranks after
its target, directed or not. The lanes plan notes only **directed** edges (plan `rows.rs:485-494`,
`cols.directed[e] && ...`). This is a real difference and I checked whether it is load-bearing: on
the plan's own `parallel_and_undirected_edges_are_routed_without_notes` case, the undirected edge
`a-b` **is** drawn head-to-tail (`row[a] = 1 > row[b] = 0`) and carries no note. The note code's
own contract text says "a Sugiyama edge was drawn head to tail to break a cycle"
(`graph-contract/src/notes.rs:53-54`), and the only consumers are the name table in the renderer
(`packages/graph-render/src/snapshot/decode.ts:88`) and `snapshot_cmd/dag.rs:36`, which is Sugiyama's
own invariant checker and never reads a lanes snapshot. So no consumer breaks.

**Ruling: acceptable, and it must stay acceptable by staying deliberate.** The divergence is
defensible — an undirected edge has no head to tail to violate, and the geometry helper
(`geometry.rs:747-749`) reverses the point order so the drawing is still correct. But it is a
divergence from the house convention on a public note code, and the plan never mentions it. That is
condition 6.

### The failure nobody mentioned

**The coordinates silently lose their half-row offset above n = 2^23, and `scale_ceiling` is the
only thing standing between this layout and the bug.** `row_spacing` and the bends are computed as
`(r as f32 + 0.5) * row_spacing` (plan `geometry.rs:739`, `:743`). In `f32`, `r + 0.5` is exact only
while `r < 2^23 = 8,388,608`; at `r = 8,388,608` the sum rounds to `8388608.0` and at
`r = 8,388,609` to `8388610.0`. So from row 2^23 on, a polyline's half-row bend lands **exactly on
an adjacent vertex row**, and the polyline can run along a row instead of between two — the layout's
own core promise, quietly voided, with no error and no note. I measured it: the first collision is
at `r = 8,388,608` exactly, and it reproduces for every spacing in the published `tunable!` range
(0.0625 ... 1024).

The declared `scale_ceiling` of 1,000,000 is **below** 2^23 and therefore safe, with 7,388,608 rows
of headroom — which is the only reason this is a condition and not a BLOCK. But nothing in the code
enforces it. `run()` refuses only a spacing that is not finite and above 0 (plan `lanes.rs:310-329`);
it has no `n` bound, so `TOPOLOGY_CEILING` = 9,200,000 (`capabilities/registry.rs:24`) is reachable
and the top 811,392 rows of that range are inside the tree's own limits and outside `f32`'s. Nothing
warns. (By contrast `x = lane * lane_spacing` and `y = row * row_spacing` themselves are exact well
past 2^24 for every published spacing — I checked adjacent-index collapse up to 9,200,000 and found
none. The defect is specifically the `+ 0.5`.)

## Scores

| axis | score | why |
|---|---|---|
| blast radius | 3 | A new layout appended to a shared registry; `registry/layouts.rs:1-5` warns an insertion repoints every index-keyed consumer with no compile error, so an append is the safe form and the index risk is retired. What remains is one row in `capabilities` and one published param pair — plus whatever a consumer draws with a 353-lane-wide `layout.dag.lanes` on the gate model. Not one function, not the whole system. |
| reversibility | 4 | High. Removing the last `LAYOUTS` entry and the module un-indexes it; nothing persisted depends on it yet, and the roundtrip oracle is added in the same change. It becomes irreversible only when `server/` caps and digest rows and the studio picker ship — that is the point to stop and re-read this verdict. |
| cost on failure | 3 | A wrong drawing is cosmetic and visible, not silent corruption — but condition 7 is the exception: the gate model never runs the reversed path, so a defect *there* is silent until a real history hits it. Note 5 and a head-to-tail polyline are recoverable by re-running; a wrong `scale_ceiling` on a published row misleads every consumer that reads it. |
| confidence | 3 | The algorithm itself is now well-evidenced — I reproduced it exactly and searched it hard for the correctness claim. What I could **not** verify is anything requiring a build: I did not run `cargo test`, the hash gate, the roundtrip oracle, or the bench, because this job's brief forbids it. So the gate rows in plan Task 4, the wasm32 build, the `codegen`/`capabilities --check` rows and the 1M bench are all **UNKNOWN**, and unknown is failure. |

**Worst axis: confidence (3), driven by condition 7 rather than by the algorithm.** The layout is
the best-understood part of this change; the *gate* is what is unproven, and the gate is what
certifies it.

## Verdict

**PROCEED-WITH-CONDITIONS.** The design is sound and the correctness claim survives attack. Nothing
here needs re-submitting. Each condition below is an acceptance criterion for the build job, and
each is checkable by a named command or test.

1. **Pin the one-reservation invariant in code, not only in a doc comment.** Add to `assign.rs` a
   `debug_assert!` (or a `#[cfg(debug_assertions)]` check) inside `Reserved::push` and
   `Reserved::settle` asserting that the lane being pushed is in no other reservation and that no
   reserved lane is in `pool.free`. `assign.rs:554-555` asserts this in prose and the whole
   correctness argument rests on it; a future edit that breaks it corrupts `next` silently, because
   `next` is indexed *by lane* and a lane in two lists is a cycle `settle` would walk forever.
   Checkable: `scripts/orch/gr cargo test -p graph-core lanes` (debug assertions active), plus
   `scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings` exit 0.
2. **Widen `no_vertex_sits_on_an_edge_it_does_not_end` past the shapes it currently covers.** It
   runs 8 seeds of one generator (plan `:227-249`). Add the cases I had to build to attack the
   claim: a graph with **parallel edges between the same pair**, a **merge with fan-in >= 50** from
   sources at *different* rows, and a **reversed (head-to-tail) edge** spanning several rows — the
   last one is the shape the gate model never produces (condition 7), so it is the one most likely
   to be wrong. Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0 with the new
   cases named in the output, and each new case must be shown to fail against a deliberately
   broken `settle` (a mutation, not a comment).
3. **Refuse, or document, `n >= 2^23` in `run()`.** Either (a) `lanes::run` returns `StageError`
   when `topology.node_count() >= 1 << 23`, with a `Ponytail` note saying the half-row offset is
   not representable in `f32` above that row; or (b) `degradation` states plainly that above 2^23
   rows the bends collapse onto vertex rows and the drawing is wrong. (a) is recommended — it costs
   one comparison and turns a silent wrong drawing into a refusal. If (a), add a test at the
   boundary: `n = (1 << 23) - 1` draws, `n = 1 << 23` is refused. Checkable: `scripts/orch/gr cargo
   test -p graph-core lanes` exit 0; the refusal test must be shown to fail before the guard is
   added.
4. **Make the oracle's independence claim true, or narrow the claim.** `registry/lanes.rs`'s
   `oracle` string (plan `:837-839`) says "compared bit for bit per seed"; `compare` exists only as
   prose (plan `:1048-1055`) and is not written. Before the row ships: (a) write `compare` and
   state in the metadata **which columns** are compared (node `x`/`y`, polyline `offsets` and `pts`,
   notes) and at **what node counts**; (b) add one negative control per column — perturb a *node
   row* (`y`), not only `x`, and perturb a *note* — since `with_x(&snapshot, 0, 0.5)` (plan `:940`)
   proves only the `x` comparison is live. A bit-for-bit claim on one column is a transcription
   check, and the metadata should not claim more. Checkable: the control tests must each fail
   against an unperturbed snapshot and pass against a perturbed one; `capabilities --check` exit 0.
5. **Label `scale_ceiling` as a Ponytail, in the `ponytail` field, and re-derive it from the bench
   number.** `LANES_CEILING = 1_000_000` (plan `:830`) is written before the bench that would
   justify it, and plan `:1093-1094` says to lower it only *if* 1M exceeds 1 s. Add a
   `Ponytail (scale_ceiling)` clause in the same form as `registry/grid.rs:38-40`, stating that the
   figure is a projection and naming the measurement it came from; if the bench has not run when the
   row ships, the clause must say so. Note the bench refuses `n > scale_ceiling`
   (`crates/graph-cli/src/bench.rs:203-211`), so a too-low ceiling silently stops the bench from
   ever testing the range it was supposed to certify. Checkable: the string is present in
   `crates/graph-core/src/registry/lanes.rs`, and `scripts/orch/gr cargo run -q -p graph-cli --
   capabilities --check` exit 0.
6. **State the note-5 divergence in `degradation`, and add an escape hatch to all three Ponytail
   clauses.** `acyclic.rs:52-71` notes every non-loop edge drawn head-to-tail; lanes notes only
   directed ones (plan `rows.rs:485-494`), so an undirected edge can be drawn head-to-tail
   unnoted (demonstrated on the plan's own `parallel_and_undirected_edges_are_routed_without_notes`
   case). That is a deliberate divergence on a public note code and the plan never says so. Add one
   clause to `degradation` naming it. Separately, the `ponytail` string (plan `:847-852`) has
   failing input and direction on each clause but **no escape hatch** on any of the three; the house
   rule requires all three parts. Checkable: `crates/graph-core/src/registry/lanes.rs` contains the
   divergence clause and the words "escape hatch"; `capabilities --check` exit 0.
7. **The gate model does not test this layout's distinguishing behaviour — say so, and add the
   coverage the hash gate cannot give.** Over all 600 gate seeds the drawing carries **zero** note-5
   edges and never breaks a cycle: `version` is 0.0 for every synthetic node (`synthetic.rs:122`),
   and the seeded model's directed edges run `i -> earlier(i)` (`synthetic.rs:132-160`), which is
   already a topological order, so Kahn never reverses an arc, never runs `lowest_unplaced`, and
   never compares two `version`s. So `hashgate --seeds 8` on this layout certifies determinism and
   nothing else. Required: (a) `degradation` says the gate model exercises the tie-break path only,
   and the cycle-breaking and `version`-ordering paths are covered by unit tests instead; (b) the
   two unit tests that do cover them must be named in the capabilities row's `oracle` string —
   `a_directed_cycle_is_broken_at_the_lowest_index_and_noted` and
   `equal_versions_fall_back_to_index_order` (plan `:153`, `:165`) are the *only* evidence for the
   cycle path, and they are one hand-built 3-vertex graph and one 3-vertex graph with no edges;
   (c) extend at least one of them to a seeded graph with distinct `version`s and a real cycle, so
   the `Ready` heap's `version.total_cmp` comparison is executed at least once outside the
   hand-written examples. Checkable: `scripts/orch/gr cargo test -p graph-core lanes` exit 0, and
   `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` exit 0 **with**
   `GM_MUTATE_DAG_LANES_NODES=1` non-zero.
8. **Keep the no-data-source grep as a permanent gate row, not a one-time manual check.** 0 hits
   today across every `rust` block in the plan, but the rule is a standing property of the tree and
   the helper named `history` (plan `:207`) is one edit away from naming something. Add the plan's
   grep (plan `:33`) as a row in the phase's rows file so it runs on every gate, not once during
   implementation. Checkable: the row is in the rows file and prints nothing.
9. **Run the gate rows this verdict could not.** Everything in plan Task 4 Step 1 is UNKNOWN to me:
   I ran no build, no test, no hash gate, no roundtrip and no bench, because this job's brief
   forbids it. The build job must run them and paste the real exit codes — in particular
   `cargo build -p graph-core --target wasm32-unknown-unknown` (exit 0), `roundtrip --seeds 100`
   (exit 0), `hashgate --seeds 8` (exit 0) and its `GM_MUTATE_REFERENCE_DEGREE=9` and
   `GM_MUTATE_DAG_LANES_NODES=1` controls (both non-zero). An unrun gate row is a failure, not a
   pass. Checkable: the exit codes in the build job's return block.

## What this verdict does not cover

The `server/` caps and digest rows (plan Task 4 Step 4) belong to `graph-render-4f`, and this
verdict says nothing about them beyond: a red caps or digest row is *expected* for a new layout and
must not be edited from this branch. The real-history lane-width measurement (spec `:147-163`) is
the git plugin job's, and the motor's ignorance of its inputs is condition 8's subject.
