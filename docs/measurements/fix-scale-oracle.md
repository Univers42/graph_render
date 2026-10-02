# fix-scale-oracle — the three scale rows' oracle differential (U12)

Source: `docs/reviews/review-core-post.md:95` (U12) and "decisions needed" items 1 and 5
of `docs/measurements/fix-scale.md`.

U12 was deferred from fix-scale because it needs `harness/` and `graph-cli` paths. This job
builds the differential, runs it, and fixes every mismatch it finds.

**Round 2** is the independent review's FIX list on round 1. The findings are `R1`–`R7`
below; `F1`–`F3` are round 1's, unchanged. Every code item is RED first, then GREEN.

## The differential

Three commands, in the house pattern copied from the spectral one:

```
$ scripts/orch/gr cargo run -q -p graph-cli -- emit-scale-fixtures
$ docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \
      python3 harness/oracle-scale.py target/scale-fixtures
$ scripts/orch/gr cargo run -q -p graph-cli -- oracle-scale
```

The Python arm imports `SciGraphs/engine/scigraphs_engine/{lod.py,simplify.py}` and calls
them unchanged; nothing in `harness/oracle-scale.py` re-implements one. Fixtures are data
emitted once (`target/scale-fixtures/scale.jsonl`, 12 lines) and read by both arms. Every
float in a fixture is an exact binary fraction and `serde_json` writes an `f64` with the
shortest string that round-trips to the same bits, so the camera and the geometry reach
the reference unchanged.

The outputs are masks and index lists, so agreement is **exact equality**, not a
tolerance: each ceiling is `0.0`, and the result carries `tolerance: false` plus a `closed`
section, so `oracle_python::closed_cases` refuses anything but byte equality.

### What is compared

| reference function | motor arm | cases |
|---|---|---|
| `lod.apply_budget` (`lod.py:76-89`) | `lod::hints(..).labelled` | 6 (3 byte-equal, 3 tied) |
| `lod.frustum_cull_spheres` (`lod.py:35-47`) | `lod::hints(..).visible` | 2 |
| `simplify.build_coarse_level` (`simplify.py:163-240`) | the community pass's `Step::links` | 4 |

Each arm is given only what its function takes. The budget's `order_key` is the node
degree and one node per block, so `cumulative <= budget` answers the same question on both
sides. The cull runs over a square orthographic camera `scale.rs::ortho` builds from the
very `Viewport` the motor was given. The coarse level is handed the motor's own louvain
partition as its `labels`, so what is compared is the collapse and not the detection
(Phase 7's `analysis.communities` row gates louvain).

Coverage required by the job, and where it landed: budget 0 / 1 / 2 / n on a hub whose five
leaves tie (`lod.budget.{zero,one,two,all}`), a budget over a path whose middle four tie
(`lod.budget.path`), a viewport that culls half the nodes (`lod.cull.half`) and one whose
nodes straddle two sides (`lod.cull.straddle`), simplify on a star, a path, two
communities joined by one edge and a self-loop on a leaf
(`simplify.coarse.*`), and adaptive on all four (see the gap rows).

### Last lines of the three commands

```
emit-scale-fixtures                                        exit 0
python3 harness/oracle-scale.py target/scale-fixtures      exit 0
  TIED lod.budget.two: the cut falls inside [1, 2, 3, 4, 5] — reference [1, 1, 0, 0, 0, 0]
    against motor [1, 1, 0, 0, 0, 0], and the reference's order inside it is its own
  TIED lod.budget.path: the cut falls inside [1, 2, 3, 4] — reference [0, 1, 1, 1, 0, 0]
    against motor [0, 1, 1, 1, 0, 0], and the reference's order inside it is its own
  TIED lod.budget.tie: the cut falls inside [1, 4] — reference [0, 0, 1, 0, 1, 1]
    against motor [0, 1, 1, 0, 0, 1], and the reference's order inside it is its own
  apply_budget: 3 case(s) whose cut is inside a class of equal degrees, compared against
    the rule rather than the reference's order
  {"apply_budget": {"emitted": 6, "cases": 6, "exact": 3, "ties": 3, "worst": 0.0},
   "frustum_cull_spheres": {"emitted": 2, "cases": 2, "exact": 2, "ties": 0, "worst": 0.0},
   "build_coarse_level": {"emitted": 4, "cases": 4, "exact": 4, "ties": 0, "worst": 0.0}}
graph-cli oracle-scale                                     exit 0
  scale.lod.apply_budget: 6 cases, worst 0.000e0, ceiling 0e0: ok
  scale.lod.frustum_cull_spheres: 2 cases, worst 0.000e0, ceiling 0e0: ok
  scale.simplify.build_coarse_level: 4 cases, worst 0.000e0, ceiling 0e0: ok
  closed cases: 3 compared byte for byte: ok
PASS
```

## Findings

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F1 | major | fixed (round 1; wording corrected, `R7`) | `a_self_loop_on_a_node_a_pass_removes_goes_with_it` | `crates/graph-core/src/scale/simplify.rs:236`, `simplify/chain.rs:99` |
| F2 | major | fixed (round 1) | `a_chain_link_is_re_anchored_when_the_community_pass_hides_an_end` | `crates/graph-core/src/scale/simplify/community.rs:30` (call), `:43` (`reanchor_links`) |
| F3 | none | **false** | `a_community_collapses_onto_its_lowest_index_with_its_external_links` | `crates/graph-core/src/scale/simplify/tests.rs:202` |
| R1 | major | fixed | `a_case_is_a_tie_when_its_cut_is_inside_a_class_of_equal_degrees_and_nothing_else`, `the_tie_case_cuts_inside_a_class_of_equal_degrees` | `harness/oracle-scale.py` (`tie_class`, `tie_verdict`), `crates/graph-cli/src/oracle_python/scale/tests.rs:191,213` |
| R2 | major | fixed | `a_case_the_harness_never_compared_fails_its_layout`, `a_deliberately_broken_result_is_never_a_pass` | `crates/graph-cli/src/oracle_python.rs:240` (`judge`), `:262` (`covered`), `:277` (`unbroken`) |
| R3 | major | fixed | `a_link_whose_ends_land_in_one_representative_is_dropped` | `crates/graph-core/src/scale/simplify/community.rs:53`, `simplify/invariant_tests.rs:26,207` |
| R4 | major | evidence only | — | below |
| R5 | — | accepted, no `scale.*` problem | — | below |
| R6 | minor | doc-only | `the_registry_covers_every_oracle_function_once_its_ids_are_unique` (amended) | `crates/graph-cli/src/capabilities.rs:100-160`, `capabilities/tests/registry.rs:258` |
| R7 | minor | doc-only | — | `docs/measurements/phase09-lod.md:24`, `crates/graph-cli/src/oracle_python/scale/cases.rs:5,74,128`, `harness/oracle-scale.py:6` |

### R1 — a tied case was counted under `ties` and never checked (fixed)

`straddled(key, mask)` asked whether the **reference's** mask straddled, and a case that
straddled was filed under `ties` with the motor's mask never read. So the three case counts
moved with the host's `np.argsort`: on a host where the sort happens to order a tied class
as D2 does, `budget.two` and `budget.path` were counted as byte-compared and passed with any
mask; on another they were not counted at all.

The tie class is now decided from the degrees, the block counts and the budget alone
(`tie_class`): the descending order, the longest prefix that fits, and whether the class of
the key at the cut is wholly on one side of it. A tied case is still compared against the
motor, in three parts (`tie_verdict`) that between them pin it — the same kept count, exact
agreement on every node **outside** the class, and inside it the motor's own stated order
(D2, ascending dense index), which is exactly that the kept members are the class's lowest
`n` indices.

RED (the three counts the review named, now stated as an invariant of the table):

```
test scale::tests::a_case_is_a_tie_when_its_cut_is_inside_a_class_of_equal_degrees…
    ... panicked at crates/graph-cli/src/oracle_python/scale/tests.rs:264:10: an array
```

GREEN:

```
test oracle_python::scale::tests::a_case_is_a_tie_when_its_cut_is_inside_a_class_of_equal_degrees_and_nothing_else ... ok
test oracle_python::scale::tests::the_tie_case_cuts_inside_a_class_of_equal_degrees ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 328 filtered out
```

That the rule finds three ties and not one is the finding: `lod.budget.two` cuts inside the
star's five-way tie at degree 1 and `lod.budget.path` inside the path's four-way tie at
degree 2, and both agreed with the reference only by luck.

### R2 — `graph-cli` read neither the counters nor the break (fixed)

`judge` asked only "were there cases, and is the worst under the ceiling". It now also
requires `covered()`: for a differential that states how many cases it emitted, every one of
them has to have been compared, and the two halves of the count (`exact` + `ties`) have to
sum to it — so a case read and skipped, or counted twice, fails. And `unbroken()` fails the
run outright when the harness recorded a `broken` case, because a `--break` run's mismatches
are the control's own, not a verdict. Both keys are absent from every other differential's
result, so no other record changes.

RED:

```
  scale.lod.apply_budget: 6 cases, worst 0.000e0, ceiling 0e0: ok
  scale.lod.frustum_cull_spheres: 2 cases, worst 0.000e0, ceiling 0e0: ok
  scale.simplify.build_coarse_level: 4 cases, worst 0.000e0, ceiling 0e0: ok
  scale.lod.apply_budget: 5 cases, worst 0.000e0, ceiling 0e0: ok
thread ... panicked at crates/graph-cli/src/oracle_python/tests.rs:61:9:
{"cases":5,"emitted":6,"exact":3,"ties":3,"worst":0.0} counts a case twice or not at all
test result: FAILED. 0 passed; 1 failed
```

```
thread ... panicked at crates/graph-cli/src/oracle_python/tests.rs:41:5:
assertion failed: !unbroken(&json!({ "broken": "lod.budget.tie" }))
test result: FAILED. 0 passed; 1 failed
```

GREEN:

```
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 327 filtered out
```

### R3 — a link could name one node twice (fixed)

`reanchor_links` maps both ends of every already-journalled link through `representative`,
so a chain link whose two ends landed in one community became `(r, r)`. `classify_edges`
never writes one — it pushes only when `ra != rb` — and neither does the reference's coarse
level, whose super-edges are the `inter` ones (`simplify.py:216-217`), so the journal held
something no front can draw.

RED, on K4 `{0,1,2,3}` plus the run `0-4-5-1`, whose chain ends `0` and `1` share the
clique's community, under all eight pass combinations:

```
thread ... panicked at crates/graph-core/src/scale/simplify/invariant_tests.rs:31:13:
assertion `left != right` failed: link 0-0 of Step { kind: Chain, representative: 0,
  nodes: [4, 5], edges: [6, 7, 8], links: [(0, 0)] } names one node twice
test scale::simplify::invariant_tests::a_link_whose_ends_land_in_one_representative_is_dropped ... FAILED
```

GREEN: `reanchor_links` retains `a != b` after re-anchoring, and `assert_links_survive`
now asserts it for all three kinds, so any future pass that reintroduces one fails in every
test that checks a journal.

```
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 1065 filtered out
```

### R4 — hash gate evidence

The change moves no registered layout, post, analysis or scale output: `reanchor_links`
drops a link that only existed because of a re-anchor, and the hash gate's stages are
unaffected. Both runs pasted, whole:

```
$ scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
  post.style.quadratic: 4-way equal on 8/8 seeds
  post.style.bezier: 4-way equal on 8/8 seeds
  transport.wasm.columnar: 4-way equal on 8/8 seeds
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 8/8 seeds
PASS
exit 0

$ scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
  post.style.quadratic: 4-way equal on 8/8 seeds
  post.style.bezier: 4-way equal on 8/8 seeds
  transport.wasm.columnar: 4-way equal on 8/8 seeds
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
exit 1
```

### R5 — `capabilities --check`

It exits 1 in a fresh worktree, and it did before this job: `target/gates/` holds only the
`oracle-scale.json` this job wrote, so every row another job owns has no gate record here.

```
$ scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check
…
capabilities --check: 69 rows, 36 problems
exit 1

$ … | grep 'scale\.'
 0 problems name a scale.* row
```

Accepted for this job only because of that zero, which is what the item asks for.

### R6 — the ledger's own rows (doc-only)

`scale.lod` and `scale.simplify` now name the differential's function names — the keys of
the `oracle-scale` record's `functions` map, which are the ceiling ids of
`oracle_python::SCALE` — so `verdict::oracle_diff` can read each one. `scale.adaptive` is a
gap row and now names **no** record: `adaptive.py`'s cut has nothing on this side to be
compared with, and a record it is not compared by would be a claim. The doc comment at
`capabilities.rs:100-110` no longer says "has no oracle differential". The registry test's
`scale` arm names the exception instead of assuming it.

### R7 — docs and citations (doc-only)

- `cases.rs`: the doubled doc on `graph` and its dead `[cases]` link are gone.
- `cases.rs`: `simplify.coarse.self_loop`'s loop moved onto a **real leaf**. Node `1` was in
  the triangle `0-1-2`, so "a self-loop on a leaf" named a node that was not one; the case is
  now `(0,1), (1,1), (0,2), (2,5)` plus the triangle `3-4-5`, where `1`'s only neighbour is
  `0`. Louvain's partition, the `map` and the link set are byte-identical to before
  (`labels [0,0,0,1,1,1]`, `map [[0,0],[1,3]]`, `links [[0,3]]`), so nothing else moved.
- `harness/oracle-scale.py:6` names `target/scale-fixtures`, which is where it writes.
- `phase09-lod.md:24-33` cites `lod.py:81-85` for the sort, the cumulative cut and the
  mask, `86-88` for the never-empty guard, and the submodule pin `b7ccee6`.
- The F1 test's doc no longer cites `simplify.py` for the leaf and chain passes: the
  reference's **only** self-loop rule is `build_coarse_level`'s (`simplify.py:216-217`) and
  it covers the coarse level alone; `grep` finds no `fold`, `chain` or `contract` function in
  `SciGraphs/engine/scigraphs_engine/`, so for those two passes the argument is the journal's
  own and the row says so.
- The merge floor is pasted as commands and their last lines, below, and a `--break` run now
  **refuses** to write into the fixtures directory without `--out`: the run that writes there
  is the one `oracle-scale` records from.

## F1–F3 (round 1, unchanged)

### F1 — a self-loop on a node a pass removes stayed drawn (fixed)

`fold_leaves` and `contract_chains` built their removed-edge lists from `edges_between` with
`a != b`, and `simple::build` drops self-loops from the adjacency, so a self-loop on a
folded leaf or on a contracted interior node was never journalled and never cleared: the
edge mask said `1` for an edge whose only node was hidden.

RED: `drawn edges with a hidden end: ["3->[1, 1]", "3->[1, 1]", "5->[2, 2]"]` — exactly the
two self-loops, nothing else in either graph.

The reference decides the community case the same way: `simplify.py:216-217` computes
`ca, cb = inv[edges[:,0]], inv[edges[:,1]]` and `inter = ca != cb`, then `:218-219` returns
with no intra-community super-edge, and a self-loop always has `ca == cb`. That is the
coarse level only — for leaf folding and chain contraction there is no reference function
at all, so there the argument is the journal's own (R7). GREEN: `simplify.rs:236
self_loops` journals and clears a removed node's self-loops in both passes; `restore` is
unchanged and still exact, and the two reversibility gate rows were not weakened.

### F2 — a chain step's links named a node a later community step hid (fixed)

`chain.rs:45` wrote `links: [(lo, hi)]` where both ends were drawn at chain time; the
community pass could then hide one and nothing rewrote the journal.
`assert_links_survive(&s, Kind::Chain)` existed but was only ever called with community
collapse off.

RED: `link 2-5 of Step { kind: Chain, representative: 2, nodes: [3, 4], … links: [(2, 5)] }`
on the graph `8 nodes, (0,1),(1,2),(2,0),(2,3),(3,4),(4,5),(5,6),(6,7),(7,5)`, whose louvain
partition is `[0,0,0,1,1,2,2,2]`. GREEN: `community.rs:43 reanchor_links` re-anchors every
already-journalled step's links through `out.representative` after the representative
fixup, so the link becomes `(0,5)`. `restore` reads no links and is unchanged.

### F3 — "an external edge between two collapsed members stays drawn" (false)

`s.edges[6] == 1` (`simplify/tests.rs:202`) is edge `(2,5)`, which joins **different**
communities, so it is the cross-community edge, not an edge inside one community. The
reference draws it too, as the single super-edge between the two representatives — the
differential's `simplify.coarse.two` case compares exactly that pair list and agrees. The
test was right and the reviewer's wording was ambiguous; nothing changed. The genuinely
intra-community edges are already removed (`s.edges[0] == 0` on the next line) and the
differential's `simplify.coarse.self_loop` case pins that a self-loop on a collapsed leaf is
in neither the reference's super-edges nor ours.

## Negative control

Two controls, both on the tie, because a tie is the case the comparison cannot settle by
equality alone.

`--break=<case>` drops **one kept node** from that case's recorded motor mask. On
`lod.budget.tie` that is the whole content of R1's first check, and it goes red:

```
$ docker run … ge-python-oracle python3 harness/oracle-scale.py target/scale-fixtures \
      --break=lod.budget.tie --out target/scale-break
  MISMATCH lod.budget.tie: kept 2 nodes against the budget's 3 — reference [0, 0, 1, 0, 1, 1]
    against motor [0, 1, 1, 0, 0, 0]
  {"apply_budget": {"emitted": 6, "cases": 5, "exact": 3, "ties": 2, "worst": 1.0}, …}

$ scripts/orch/gr cargo run -q -p graph-cli -- oracle-scale --dir target/scale-break
  scale.lod.apply_budget: 5 cases, worst 1.000e0, ceiling 0e0: FAIL
  scale.lod.frustum_cull_spheres: 2 cases, worst 0.000e0, ceiling 0e0: ok
  scale.simplify.build_coarse_level: 4 cases, worst 0.000e0, ceiling 0e0: ok
  closed cases: 3 compared byte for byte: FAIL
  "lod.budget.tie" was deliberately broken: not a verdict
FAIL
warning: oracle-scale: not recorded — the existing record passed and this run did not, so the
  passing record stands (re-run the gate to replace it)
exit 1
```

Two independent reasons there: the mismatch itself, and R2's `covered` (5 compared of 6
emitted) — plus `unbroken`, which is the third and would fire on a break that happened to
stay green. The break dir is its own: without `--out` the harness refuses, so a control can
never overwrite the result `oracle-scale` records from.

```
$ docker run … python3 harness/oracle-scale.py target/scale-fixtures --break=lod.budget.tie
--break writes a deliberate mismatch: give it --out DIR of its own
exit 1
```

## Gap rows

Left alone on purpose; each names what the reference takes that the motor does not expose.

| reference | gap | why it cannot be compared |
|---|---|---|
| `adaptive.select_cut`, `simplify.build_hierarchy` | `scale.adaptive` | both cut a hierarchy of coarse levels that `build_hierarchy` builds with an `infomap` detector; the image does not carry infomap and the motor has no coarse level, only a journal, so the two arms have nothing they could be given in common |
| `simplify.backbone_mask` | `scale.simplify` | the MST / disparity / top-k backbone is not a reversible reduction of the graph and the row does not port it |
| `lod.node_tier`, `lod.edge_tier` | `scale.lod` | thresholds are on-screen pixels of a 1080-tall image; a headless motor has none and takes the tier from the node count |
| `lod.projected_pixel_radius` | `scale.lod` | the reference's two culling functions disagree with each other on the units of `radii`: `frustum_cull_spheres:38-44` compares it in clip space, `projected_pixel_radius:54-55` in world units. No single fixture satisfies both. The differential states which one it feeds, and `scale.rs::clip_radii` refuses a rectangle whose one radius cannot express all three axes |
| leaf folding, chain contraction | `scale.simplify` | no reference function decides them at all: `simplify.py` has no `fold`, `chain` or `contract` function, and folds nothing. The reference's one self-loop rule (`simplify.py:216-217`) covers the **coarse level only**. The journal's reversibility is graph-core's own gate row |

### The three cases whose cut is inside a tie

`lod.budget.two` (the star's five leaves, budget 2), `lod.budget.path` (the path's middle
four, budget 3) and `lod.budget.tie` (degrees `[1,2,3,1,2,3]`, budget 3) all have their cut
strictly inside a class of equal `order_key`. `np.argsort` is **not** stable, so the
reference's order inside the class is the sort's, while the motor breaks ties by ascending
dense index (`prompt.md` §6, D2, which wins over the reviewer's guess per fix-common).
Whether a case is one of them is read from the degrees and the budget alone, and each is
then compared in three parts — kept count, exact agreement outside the class, and the
motor's own order inside it. The divergence itself is still in the artifact: the result's
`ties` section carries both masks and the class.

## Non-vacuity

Three guards, because a differential that compares nothing reads exactly like one that
passes:

- `scale.rs::decidable` **refuses** to emit a cull fixture with a node within 1e-6 of a
  decision boundary (the rectangle grown by the radius — the reference's `abs(w) + 1e-9` at
  `lod.py:40` moves each boundary about 4e-9 world units, so a closer node could fall
  either way and the case would prove nothing);
- `the_camera_is_the_rectangle_and_its_radius_crosses_the_edge` re-runs `lod.py:35-47`
  natively against the emitted matrix, so a matrix that culled everything or nothing would
  fail here rather than pass silently;
- `every_case_is_one_line_naming_a_reference_the_ceilings_know` and
  `a_case_is_a_tie_when_its_cut_is_inside_a_class_of_equal_degrees_and_nothing_else` pin the
  table's own shape and classification, and `judge`'s `covered` fails a run that did not
  compare every case it emitted.

## Merge floor

```
$ scripts/orch/gr cargo fmt --all --check
(no output)
exit 0

$ scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.94s
exit 0

$ scripts/orch/gr cargo test --workspace --no-fail-fast
test result: ok. 1076 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 11.11s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
  … 1685 passed, 0 failed summed over the 27 binaries
exit 0

$ scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s
exit 0

$ scripts/scigraphs-conformance.sh
  CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15
PASS
exit 0

$ scripts/orch/gr cargo run -q -p graph-cli -- codegen --check
  up to date  docs/contract/ingest-schema.json
exit 0

$ scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check
capabilities --check: 69 rows, 36 problems
exit 1   — 0 of the 36 name a scale.* row (R5)
```

## Out of scope, stated not fixed

`capabilities/verdict.rs` applies `MIN_SEEDS = 1000` to every record
(`seeds_of`, `verdict.rs:72-80`), so the `oracle-scale` record's twelve cases can never back
a `gated` row: the three scale rows read `not backed: oracle-scale ran 12 seeds, need 1000`.
That is a separate ledger job, and `verdict.rs` is outside this one's paths. `scale.adaptive`
now names no record at all, so its `oracle_diff` cell is the empty-record reason instead.

## Decisions taken

1. **`fixtures/scale/**` is not used.** It is in the job's paths, but `fixtures` is inside
   `fingerprint::FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21-36`) and every
   emitted fixture records the tree fingerprint, so a committed fixture could never
   validate itself: committing it moves the fingerprint, which invalidates the
   `fingerprint` field it carries, which requires re-emitting, which moves the tree again.
   The fixtures therefore go to `target/scale-fixtures`, like all twelve other
   differentials. Reason: `fingerprint.rs` is outside this job's paths and the loop is not
   escapable. Easiest to undo: change the two `default_value`s in `cli.rs`.
2. **Fixtures are emitted to `--cases`, not `--seeds`.** The graphs are hand-built shapes
   chosen for a question the reference can answer, so there is no sweep to widen;
   `--cases` defaults to `CASES` (12) and is capped at it.
3. **A tied case counts as compared, and `closed` counts only the byte-equal ones.** R1 asks
   for both: the motor is checked on every tied case, so the count of checked cases is what
   the ledger prints, while `closed_exact` and the `closed` section keep meaning "the two
   arms returned the same bytes". `apply_budget` is therefore `cases: 6` (3 exact, 3 ties)
   and `closed.apply_budget: 3`.
4. **`scale.adaptive` names no `oracle_record`.** Leaving it on `oracle-scale` with an empty
   `functions` list would have it read as a differential it is not compared by; `""` makes
   the gap the reason, and the registry test names the exception rather than assuming it.

## Deviations

- `crates/graph-cli/src/capabilities.rs` (the `scale_rows()` closure and the three rows) and
  `crates/graph-cli/src/capabilities/tests/registry.rs` (the `r.stage == "scale"` arm) are
  in `capabilities/`, but `scale_rows()` is physically inline in `capabilities.rs`. The body
  says "the scale rows' oracle text … names the new differential", so these are the files it
  means. No other row in either file was touched.
- `crates/graph-cli/src/oracle_python/scale/cases.rs` and `scale/tests.rs` are new child
  modules of `scale.rs` (round 1). `scale.rs` is 322 lines, over the 300-line house limit.
  The round-2 growth is `covered`/`unbroken` in `oracle_python.rs` (a shared file, not a
  scale module) and the tie helpers in the two child modules, not `scale.rs`; the fix is to
  move `ortho`/`clip_radii`/`decidable`/`degrees` into a `scale/camera.rs` child, which is
  a split of existing code and no new behaviour, so it is left to the job that owns the
  split rather than smuggled in here.
- `crates/graph-cli/src/oracle_python.rs` gained two lines beyond round 1: `pass &=
  unbroken(&result)`, one line in `judge` for `covered`, and the two functions. Both keys
  (`broken`, `emitted`/`exact`) are absent from every other differential's result, so all
  twelve other records stay byte-identical.