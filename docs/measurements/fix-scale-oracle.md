# fix-scale-oracle — the three scale rows' oracle differential (U12)

Source: `docs/reviews/review-core-post.md:95` (U12) and "decisions needed" items 1 and 5
of `docs/measurements/fix-scale.md`.

U12 was deferred from fix-scale because it needs `harness/` and `graph-cli` paths. This job
builds the differential, runs it, and fixes every mismatch it finds.

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
| `lod.apply_budget` (`lod.py:76-89`) | `lod::hints(..).labelled` | 5 + 1 undecided |
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
communities joined by one edge and a self-loop on a collapsed member
(`simplify.coarse.*`), and adaptive on all four (see the gap rows).

### Last lines of the three commands

```
emit-scale-fixtures                                        exit 0
python3 harness/oracle-scale.py target/scale-fixtures      exit 0
  UNDECIDED lod.budget.tie: the cut falls inside a tie — reference [0, 0, 1, 0, 1, 1]
    against motor [0, 1, 1, 0, 0, 1], and neither order is wrong
  apply_budget: 1 case(s) the reference cannot decide, listed under ties
  {"apply_budget": {"cases": 5, "worst": 0.0, "ties": 1},
   "frustum_cull_spheres": {"cases": 2, "worst": 0.0, "ties": 0},
   "build_coarse_level": {"cases": 4, "worst": 0.0, "ties": 0}}
graph-cli oracle-scale                                     exit 0
  scale.lod.apply_budget: 5 cases, worst 0.000e0, ceiling 0e0: ok
  scale.lod.frustum_cull_spheres: 2 cases, worst 0.000e0, ceiling 0e0: ok
  scale.simplify.build_coarse_level: 4 cases, worst 0.000e0, ceiling 0e0: ok
  closed cases: 3 compared byte for byte: ok
PASS
```

### Negative control

`harness/oracle-scale.py` takes `--break`, which flips the first case's expected answer in
the motor's favour. Every case then compares against the reference and nothing else moves:

```
$ docker run … python3 harness/oracle-scale.py target/scale-fixtures --break
MISMATCH lod.budget.zero: reference [1, 1, 1, 1, 1, 1] against motor [0, 0, 0, 0, 0, 0]
$ scripts/orch/gr cargo run -q -p graph-cli -- oracle-scale
  scale.lod.apply_budget: 4 cases, worst 1.000e0, ceiling 0e0: FAIL
  …
  closed cases: 3 compared byte for byte: FAIL
FAIL                                                              exit 1
```

## Findings

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F1 | major | fixed | `a_self_loop_on_a_node_a_pass_removes_goes_with_it` | `crates/graph-core/src/scale/simplify.rs:236`, `simplify/chain.rs:99` |
| F2 | major | fixed | `a_chain_link_is_re_anchored_when_the_community_pass_hides_an_end` | `crates/graph-core/src/scale/simplify/community.rs:30` (call), `:43` (`reanchor_links`) |
| F3 | none | **false** | `a_community_collapses_onto_its_lowest_index_with_its_external_links` | `crates/graph-core/src/scale/simplify/tests.rs:202` |

### F1 — a self-loop on a node a pass removes stayed drawn (fixed)

`fold_leaves` and `contract_chains` built their removed-edge lists from `edges_between` with
`a != b`, and `simple::build` drops self-loops from the adjacency, so a self-loop on a
folded leaf or on a contracted interior node was never journalled and never cleared: the
edge mask said `1` for an edge whose only node was hidden.

RED: `drawn edges with a hidden end: ["3->[1, 1]", "3->[1, 1]", "5->[2, 2]"]` — exactly the
two self-loops, nothing else in either graph.

The reference decides it. `simplify.py:216-217` computes `ca, cb = inv[edges[:,0]],
inv[edges[:,1]]` and `inter = ca != cb`, then `:218-219` returns with no intra-community
super-edge. A self-loop always has `ca == cb`, so the reference's coarse level never draws
one. GREEN: `simplify.rs:236 self_loops` journals and clears a removed node's self-loops in
both passes; `restore` is unchanged and still exact, and the two reversibility gate rows
were not weakened.

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
differential's `lod.budget`-independent `simplify.coarse.self_loop` case pins that a
self-loop on a collapsed member is in neither the reference's super-edges nor ours.

### The rest of fix-scale's item 5, and where each landed

- the two findings above: fixed, with the reference cited;
- "adaptive on the same": a gap row, below.

## Gap rows

Left alone on purpose; each names what the reference takes that the motor does not expose.

| reference | gap | why it cannot be compared |
|---|---|---|
| `adaptive.select_cut`, `simplify.build_hierarchy` | `scale.adaptive` | both cut a hierarchy of coarse levels that `build_hierarchy` builds with an `infomap` detector; the image does not carry infomap and the motor has no coarse level, only a journal, so the two arms have nothing they could be given in common |
| `simplify.backbone_mask` | `scale.simplify` | the MST / disparity / top-k backbone is not a reversible reduction of the graph and the row does not port it |
| `lod.node_tier`, `lod.edge_tier` | `scale.lod` | thresholds are on-screen pixels of a 1080-tall image; a headless motor has none and takes the tier from the node count |
| `lod.projected_pixel_radius` | `scale.lod` | the reference's two culling functions disagree with each other on the units of `radii`: `frustum_cull_spheres:38-44` compares it in clip space, `projected_pixel_radius:54-55` in world units. No single fixture satisfies both. The differential states which one it feeds, and `scale.rs::clip_radii` refuses a rectangle whose one radius cannot express all three axes |
| leaf folding, chain contraction | `scale.simplify` | no reference function decides them at all; `grep links SciGraphs/engine/scigraphs_engine` is empty, and `simplify.py` folds nothing. The journal's reversibility is graph-core's own gate row |

### One case the reference cannot answer

`lod.budget.tie` (degrees `[1,2,3,1,2,3]`, budget 3) has its cut strictly inside a class of
equal `order_key`. `np.argsort` is **not** stable — verified in the image at n = 6, 8, 15,
16, 17, 24 and 40 — so the reference's order inside the tie is the sort's, while the motor
breaks ties by ascending dense index (`prompt.md` §6, D2, which wins over the reviewer's
guess per fix-common). The harness detects the situation from the two masks themselves
(`straddled`: the smallest kept key is not above the largest dropped one), records the case
under `ties` with **both** masks, and does not count it as a compared case. The divergence
is therefore in the artifact rather than inferred, and
`the_tie_case_cuts_inside_a_class_of_equal_degrees` pins the shape that makes the case
undecidable so it cannot silently become a decided one.

## Non-vacuity

Two guards, because a differential that compares nothing reads exactly like one that
passes:

- `scale.rs::decidable` **refuses** to emit a cull fixture with a node within 1e-6 of a
  decision boundary (the rectangle grown by the radius — the reference's `abs(w) + 1e-9` at
  `lod.py:40` moves each boundary about 4e-9 world units, so a closer node could fall
  either way and the case would prove nothing);
- `the_camera_is_the_rectangle_and_its_radius_crosses_the_edge` re-runs `lod.py:35-47`
  natively against the emitted matrix, so a matrix that culled everything or nothing would
  fail here rather than pass silently.

## Docs

`docs/measurements/phase09-lod.md:24-36` no longer says a budget of zero lets the most
important visible node keep its label. Since R4 a budget of 0 means **no limit** on both
sides (`lod.py:79-80`, `lod.rs:225-228`), and the never-empty guarantee applies to budgets
of 1 and above (`lod.py:86-88`). Both pinning tests are cited by path.
`docs/measurements/fix-scale.md` items 1 and 5 record the resolution.

The three scale rows' `oracle` text now names the differential and what it checks, and
`oracle_record` moved from `oracle-diff` to `oracle-scale`.

## Merge floor

| command | exit | last line |
|---|---|---|
| `gr cargo fmt --all --check` | 0 | — |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile` |
| `gr cargo test --workspace --no-fail-fast` | 0 | 1681 passed, 0 failed, 12 ignored |
| `gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished dev profile` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` (32/32 rows reached a reference) |
| `gr cargo run -p graph-cli -- codegen --check` | 0 | `up to date docs/contract/ingest-schema.json` |
| `gr cargo run -p graph-cli -- capabilities --check` | 1 | `69 rows, 36 problems` — **all** 36 are `<row>: gated, but no <X> record: run the gate` on rows this job did not touch (18 `layout.*`, 16 `topology.*`, 2 `transport.*`), because no gate has been run in this worktree: `target/gates/` held only the `oracle-scale.json` this job wrote. **0** problems name a `scale.*` row |

## Decisions taken

1. **`fixtures/scale/**` is not used.** It is in the job's paths, but `fixtures` is inside
   `fingerprint::FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21-36`) and every
   emitted fixture records the tree fingerprint, so a committed fixture could never
   validate itself: committing it moves the fingerprint, which invalidates the
   `fingerprint` field it carries, which requires re-emitting, which moves the tree again.
   The fixtures therefore go to `target/scale-fixtures`, like all twelve other
   differentials. Reason: `fingerprint.rs` is outside this job's paths and the loop is not
   escapable. Easiest to undo: change the two `default_value`s in `cli.rs`.
2. **`fixtures` are emitted to `--cases`, not `--seeds`.** The graphs are hand-built shapes
   chosen for a question the reference can answer, so there is no sweep to widen;
   `--cases` defaults to `CASES` (12) and is capped at it.
3. **The ledger reads `not backed: oracle-scale ran 12 seeds, need 1000`.**
   `verdict::seeds_of` (`capabilities/verdict.rs:72-80`) applies a flat `MIN_SEEDS = 1000`
   to every oracle record. The `oracle-igraph` differential already runs 100 seeds and
   reads the same way, so this is the house state for a non-sweep differential and the rows
   stay `Implemented`. `verdict.rs` is outside this job's paths, so it was left alone.

## Deviations

- `crates/graph-cli/src/capabilities.rs` (the `scale_rows()` closure and the three rows)
  and `crates/graph-cli/src/capabilities/tests/registry.rs` (the `r.stage == "scale"` arm)
  are in `capabilities/`, but `scale_rows()` is physically inline in `capabilities.rs`.
  The body says "the scale rows' oracle text … names the new differential", so these are
  the files it means. No other row in either file was touched.
- `crates/graph-cli/src/oracle_python/scale/cases.rs` is a new child module of
  `scale.rs`. `scale.rs` reached 418 lines, over the 300-line house limit, and this is the
  same split `lod.rs`+`lod/` and `simplify.rs`+`simplify/` already use. `scale.rs` is 273
  lines, `cases.rs` 158, `tests.rs` 240; the longest function in any of them is 28 lines.
- `crates/graph-cli/src/oracle_python.rs` gained one line: the ledger body's `tolerance`
  is now read from the result and **defaults to `true`**. Additive — no existing
  differential's result carries the key, so all twelve records are byte-identical — and it
  is what lets `oracle-scale` record `tolerance: false` and have the ledger print
  `byte-equal/12 seeds` instead of claiming a measured ceiling.
