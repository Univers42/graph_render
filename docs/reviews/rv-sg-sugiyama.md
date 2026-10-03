# Review — `sg-sugiyama` (SciGraphs conformance: SUGIYAMA)

Reviewed: branch `sg-sugiyama` at `38f4384`, delta `git diff origin/develop...HEAD`
(18 files). Rules, severity and output shape per `prompts/jobs/review-core-post.md`;
scope per `prompts/jobs/sg-sugiyama.md`. Only path written: this file. No code changed.

Every finding below carries a command with its output or a reference `file:line`.

---

## Findings

| id | severity | file:line | defect | failing input | evidence | proposed fix |
|---|---|---|---|---|---|---|
| 1 | MAJOR | `crates/graph-core/src/registry/grid.rs:97`, `:109` | The registered `layout.dag.sugiyama` `Metadata` still names the greedy feedback-arc-set. `oracle` says "acyclic after FAS" and `ponytail` ends "Ponytail (FAS): greedy, not minimum". The product path takes `ArcOrder::NodeIndex` and never runs a FAS. | any graph with a cycle — the registry is the public capability contract a caller reads before choosing the layout | `crates/graph-core/src/layout/sugiyama/acyclic.rs:46` (`Self::oriented(topology, ArcOrder::NodeIndex)`) vs `git show origin/develop:.../acyclic.rs` where `Acyclic::of` called `greedy_fas_order`; `docs/decisions/sugiyama-heuristics.md` §"Cycle breaking is by node order" already records this as overstated and out of the repair job's envelope | reword `oracle` to "acyclic after orienting every non-loop edge forward along the dense node order" and replace the FAS sentence in `ponytail` with one about the node-order breaker. Needs a job whose paths include `registry/grid.rs`. |
| 2 | MAJOR | `crates/graph-core/src/registry/grid.rs:99` | `complexity: "O(n+m) per phase"` is now false. `Arcs::grouped()` sorts (`acyclic.rs:126`) and is called three times per pipeline, so `assign_layers`, `budget_plan` and `materialize` are each O(m log m), where develop's were O(n+m) linear passes. | every `layout.dag.sugiyama` run | `crates/graph-core/src/layout/sugiyama/layering.rs:48`, `:129`, `:219` each build the arc list; `crates/graph-core/src/layout/sugiyama/acyclic.rs:126` `pairs.sort_unstable()` | say `O(m log m)` per layering phase (the arc list is sorted once per phase), or hoist one `grouped()` into `layered()` and pass it down, which also removes two of the three sorts. |
| 3 | MINOR | `crates/graph-core/src/layout/sugiyama/layering.rs:219` | `materialize` now iterates arcs in ascending `(tail, head)` order instead of ascending edge index. Dummy numbering and `up`/`down` insertion order both follow that iteration, and `init_order` walks `down[v]` then `up[v]` in list order — so the seed order of every layer changes, and with it the drawing. `layout.dag.sugiyama`'s public output therefore moved on any graph with a multi-span edge whose edge list is not already `(tail, head)`-sorted. No artefact on this branch records that. | every `hashgate` model: `synthetic_edges` draws a second `i -> earlier()` edge with probability 0.5 and an `extras` block of random pairs (`crates/graph-core/src/synthetic.rs:134-146`) | mechanism: `crates/graph-core/src/layout/sugiyama/ordering.rs:76-78` vs `SciGraphs/.../hierarchical.py:432-441`; `git diff origin/develop...HEAD -- layering.rs` (`materialize`); no stored per-stage digest exists to have caught it — `crates/graph-cli/src/hashgate/compare.rs:48-50` reports arm equality only, and `hashgate --seeds 8` printed `layout.dag.sugiyama: 4-way equal on 8/8 seeds` with no per-stage baseline | one line in `docs/decisions/sugiyama-heuristics.md` recording that `layout.dag.sugiyama`'s output moves on multigraphs, and pin a `--dag` fixture with a parallel arc so the crossing gate can see it (see finding 9 for why it currently cannot). |
| 4 | MINOR | `crates/graph-core/src/layout/sugiyama/acyclic.rs:126` | `grouped()` sorts arcs by the node **id**, while the reference sorts by **rank**: `sorted(arcs, key=lambda a: (rank[a[0]], rank[a[1]]))`. The two agree only when `rank == index`, i.e. only for `ArcOrder::NodeIndex`. Under `ArcOrder::Feedback` the port's arc order differs from `_acyclic_arcs`'s. | any graph run through `ArcOrder::Feedback` — reachable only from `acyclic/tests.rs:61` | `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:305,311` vs `acyclic.rs:123-126` | one sentence on `ArcOrder::Feedback` (`acyclic.rs:181-187`) saying the arc list is ordered by node index, not by rank, and that this is the one place the port is not verbatim. |
| 5 | MINOR | `crates/graph-core/src/layout/sugiyama/scaled.rs:75` | `run_scaled` refuses `scale <= 0` and non-finite. The reference accepts any `scale` and at `scale = 0` returns all-zero positions (`(x-lo)/width*2-1) * 0` and the `max_layer` branch aside, everything collapses to 0). The port errors instead. | `run_scaled(t, 0.0)` — legal for `_sugiyama_layout(G, 0)` | `scaled.rs:75-80` vs `SciGraphs/.../hierarchical.py:684`; the divergence is pinned as intended by `crates/graph-core/src/layout/sugiyama/scaled/tests.rs:75-77`. Not exercised by the row: the dispatcher always passes `SCALE = 5.0` (`conformance/motor.rs:102`) | either accept `scale == 0.0` (rejecting only negative and non-finite), or add a Ponytail line on `Frame`/the guard naming the input it refuses that the reference does not. The crate's own convention (`run` refuses `layer_spacing <= 0`) argues for keeping it and documenting it. |
| 6 | MINOR | `crates/graph-core/src/layout/sugiyama/acyclic/feedback.rs:127` | `greedy_fas_order`'s outer `while remaining > 0` has no guard for the heap draining while `remaining > 0`; the inner `while let` would simply end and the outer loop would spin forever. Unreachable given the re-push invariant, and untested. | none found — stated as a robustness gap, not a live defect | `feedback.rs:143-151`: the inner loop has no `else`/fallthrough | `unreachable!("every live vertex has a current-key entry")` after the inner loop, as `crates/graph-core/src/layout/sugiyama/layering.rs:152` already does. |
| 7 | MINOR | `crates/graph-core/src/layout/sugiyama/scaled/tests.rs` | No empty-topology test for `run_scaled`. By reading, the empty case is correct — `Frame::of(&[])` falls to `(0.0, 0.0)` / `width = 1.0` (`scaled.rs:41-49`), both loops are empty, and that is what `np.zeros((0, 3))` means — but nothing pins it. | `index_model(&[], &[])` | `scaled.rs:38-53`, `:88-94` vs `SciGraphs/.../hierarchical.py:654-655`; `crates/graph-core/src/layout/sugiyama/acyclic/tests.rs:75` pins the empty case for the other stage | add `an_empty_topology_draws_nothing` to `scaled/tests.rs`, mirroring the acyclic one. |
| 8 | MINOR | `crates/graph-core/src/layout/sugiyama/stages/dump.rs:73` | `ids.iter().position(...)` per edge endpoint is O(n·m), against the file's own claim of being a cheap dump. Test-only and on the three file fixtures (largest n=77, m=254), so harmless today. | a large `fixtures/*.json` graph added to `FIXTURES` | `dump.rs:68-76` | build a `name -> index` map once, or drop the file fixtures in favour of `graph_core::seeded_model` as `GATE_SEEDS` already does. |
| 9 | MINOR | `docs/measurements/sg-sugiyama.md:183` | "the crossing counts are byte-identical" — the *before* column has no recorded run. The identity is real, but for a structural reason the doc does not state: the whole `--dag` corpus is structurally blind to this branch's arc-dedup change. | — | re-measured by me, after column: `scripts/orch/node-slim.sh node harness/oracle-layouts.mjs --dag` → `graphs=236 (fixtures=6) sum(ours)=5242 sum(dagre)=7657 1.10x(dagre)=8422.7 verdict=ok`, `status: pass`, exit 0 — identical to `sg-sugiyama.md:171-181`. Blindness measured: 0 parallel pairs in all six fixtures (`chain 0, diamond 0, cyclic 0, multi-span 0, wide-layer 0, disconnected 0`), and `synthetic_dag` (`mod.rs:228-235`) emits only `i < j` with no duplicates | reword to "unchanged, and structurally cannot move: no fixture and no synthetic DAG has a parallel arc" — which is also the sentence that should have been written next to finding 3. |
| 10 | MINOR | `docs/measurements/sg-sugiyama.md:133` | The stage table cites `python3 diff.py py-stages.json target/sugiyama-stages.json`; the command block at `:72-74` writes `py.json`, and `diff.py` is not committed. A third party cannot re-run the table. | — | `sg-sugiyama.md:72-74` vs `:133`; `git status --short` → clean, no `diff.py` in the tree | paste the six lines of `diff.py` next to the python appendix, as the appendix already is, or commit it. |
| 11 | MINOR | `docs/reviews/review-layout-tree.md:59-61` | The FAS "match[es] the reference verbatim" claim is now true of code the pipeline never runs (`ArcOrder::Feedback` is `#[cfg_attr(not(test), allow(dead_code))]`). It is not wrong — see check 2 — but it reads as a statement about the layout. | — | `crates/graph-core/src/layout/sugiyama/acyclic.rs:186`; `docs/measurements/sg-sugiyama.md:152-154` | annotate the three claims with "verified against `ArcOrder::Feedback`, the reference's directed branch, which `apply_graph_layout` never reaches". |
| 12 | INFO | `crates/graph-cli/src/oracle_python/conformance/motor.rs:242` | `row_line` is 53 lines, over the 40-line house limit. Pre-existing; not touched by this branch (absent from `git diff origin/develop...HEAD`). | — | `CLAUDE.md` "House limits"; measured by brace-depth scan | none on this branch; file it against the conformance job that owns `motor.rs`. |

### Counts

| area | BLOCKER | MAJOR | MINOR | INFO |
|---|---|---|---|---|
| registry `Metadata` (findings 1-2) | 0 | 2 | 0 | 0 |
| `layout/sugiyama` (findings 3-8) | 0 | 0 | 6 | 0 |
| docs (findings 9-11) | 0 | 0 | 3 | 0 |
| pre-existing (finding 12) | 0 | 0 | 0 | 1 |
| **total** | **0** | **2** | **9** | **1** |

Module coverage (every module named, with its finding count):
`acyclic.rs` 3 (3, 4, 6) · `acyclic/feedback.rs` 2 (6, and check 2 below) ·
`acyclic/tests.rs` 0 · `layering.rs` 2 (3, and 2 via `distinct()`) · `layering/tests.rs` 0 ·
`coords.rs` 0 (unchanged; read for check 3) · `ordering.rs` 0 (unchanged; read for check 3) ·
`routing.rs` 0 (unchanged) · `scaled.rs` 2 (5, 7) · `scaled/tests.rs` 0 · `stages.rs` 0 ·
`stages/dump.rs` 1 (8) · `mod.rs` 0 · `conformance/motor.rs` 1 (12, pre-existing) ·
`conformance/motor/tests.rs` 0 · `conformance/gaps.rs` 0 · `conformance/rows.rs` 0 ·
`conformance/baseline/table/structured.rs` 0 · `registry/grid.rs` 2 (1, 2, out of this
branch's envelope) · `docs/decisions/sugiyama-heuristics.md` 0 ·
`docs/measurements/{sg-sugiyama,scigraphs-conformance}.md` 3 (9, 10, and 11 in the
review it supersedes).

---

## Check 1 — did `layout.dag.sugiyama`'s public output move?

**Yes, on two counts, and the brief's condition is satisfied but only one of the two is
recorded.**

**(a) Cycle breaking: greedy FAS → node order.** `Acyclic::of` now takes
`ArcOrder::NodeIndex` (`acyclic.rs:46`); develop's took `greedy_fas_order` (read from
`git show origin/develop:crates/graph-core/src/layout/sugiyama/acyclic.rs`). This moves the
output on every graph with a cycle. It **is** covered by the crossing gate —
`fixtures/dag/cyclic.json` has one backward-spelled edge (measured) — and the gate is green
(after column re-measured, above). The decision doc says why
(`sugiyama-heuristics.md` §"Cycle breaking is by node order, not by the greedy
feedback-arc-set (repaired 2026-10-02)"). The registry row does **not** (finding 1).

**(b) Layering over distinct arcs, not edges.** This changes more than the dedup: it
re-orders dummy allocation and `up`/`down` construction (finding 3). The decision doc says
why the dedup happened and measures the conformance gain; it does **not** say that
`layout.dag.sugiyama`'s registered output moved as a result. Neither does the registry row.

**`hashgate --seeds 8`:**

```
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8   -> exit 0
  native run 1  digest 97c9f2fc3c829a7a85a737eb000cd96d883d40044fc45f290561f4bce364fb2f
  native run 2  digest 97c9f2fc3c829a7a85a737eb000cd96d883d40044fc45f290561f4bce364fb2f
  wasm32 run 1  digest 97c9f2fc3c829a7a85a737eb000cd96d883d40044fc45f290561f4bce364fb2f
  wasm32 run 2  digest 97c9f2fc3c829a7a85a737eb000cd96d883d40044fc45f290561f4bce364fb2f
  layout.dag.sugiyama: 4-way equal on 8/8 seeds
PASS
```

PASS, exit 0 — so D10 (native ≡ wasm32, bit-identical) holds on this branch, and it matches
the record in `docs/measurements/sg-sugiyama.md:254`. **But `hashgate` cannot answer the
question it was asked here.** It prints one whole-pipeline digest and then per-stage
*arm-equality* lines (`crates/graph-cli/src/hashgate/compare.rs:48-50`); it stores no
per-stage digest baseline, so "did the stage hashes move" is not something it measures in
either direction. The gate models are, on top of that, exactly the inputs that changed
(parallel and backward edges, `synthetic.rs:134-146`). I did not run `hashgate --seeds
1000`; that is the orchestrator's row.

**Dagre crossing counts:** `scripts/orch/node-slim.sh node harness/oracle-layouts.mjs --dag`
→ exit 0, `chain 0/0, diamond 0/0, cyclic 0/0, multi-span 0/0, wide-layer 36/36,
disconnected 0/0`, `sum(ours)=5242 sum(dagre)=7657 1.10x(dagre)=8422.7 verdict=ok`,
`status: pass`. Matches `docs/measurements/sg-sugiyama.md:171-181` exactly. **Green**, so
the brief's condition holds. But see finding 9: the corpus has no parallel arcs, so its
greenness is structural, not evidential, for change (b).

---

## Check 2 — the FAS rewrite vs `hierarchical.py`, line by line

`crates/graph-core/src/layout/sugiyama/acyclic/feedback.rs` against
`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:244-296`
(`_greedy_fas_order`), plus `acyclic.rs:52-72` (`Acyclic::oriented`) against
`hierarchical.py:298-311` (`_acyclic_arcs`).

| reference | port | verdict |
|---|---|---|
| `:252-253` `succ`/`pred` exclude self-loops, one entry per neighbour | `feedback.rs:41-50` `if s != t` then `sort_unstable` + `dedup` | match |
| `:254-255` degrees over that neighbour list | `feedback.rs:75-76` `s.len()` / `p.len()` | match |
| `:256` `rank = {n: i for i, n in enumerate(nodes)}` | `acyclic.rs:197` `(0..node_count()).collect()` | match under the fixture contract (`conformance/fixtures.rs`), stated at `feedback.rs:15-17` |
| `:258-259` `heap = [(in-out, rank[n], n)]`, `heapq.heapify` (min-heap) | `feedback.rs:56-59` `heap_key = (in-out, v)`; `:80-82` into `BinaryHeap<Reverse<..>>` | match — `Reverse` makes `pop()` the minimum, so the **largest** `out - in` is peeled first (ELS) |
| `:260` `ready = [n for n in nodes if not out or not in]`, then `:281 ready.pop()` (LIFO) | `feedback.rs:77-79` ascending filter; `:128 state.ready.pop()` | match, including the LIFO discipline |
| `:266-271` peel: decrement `in_degree[v]`, `if not in_degree[v]: ready.append(v)`, heappush **unconditionally** | `feedback.rs:98-107` same three steps, push outside the `== 0` | match |
| `:272-277` the mirror for `out_degree[u]` | `feedback.rs:108-117` | match |
| `:280-285` `(right if not out_degree[node] else left).append(node)` evaluated **before** `peel` | `feedback.rs:132-137` same test, same position | match |
| `:288-293` pop until `node in alive and key == in_degree[node] - out_degree[node]`, then `left.append`, `peel`, `break`; stale entries skipped by re-keying | `feedback.rs:143-151` identical, `Reverse((key, node))` destructured | match |
| `:295-296` `right.reverse(); return left + right` | `feedback.rs:153-154` same | match |
| `_acyclic_arcs` `:307-310` skip `u == v`; orient `rank[u] < rank[v]` else swap | `acyclic.rs:59` `s != t && rank[s] > rank[t]` → reverse, never drop | match — and self-loops are exempted at the orientation, not just the degree count (`acyclic.rs:59`) |
| `:306` `arcs` is a `set` | `acyclic.rs:100-114` `grouped()` coalesces equal `(tail, head)` runs | match (new) |
| `:311` `sorted(arcs, key=lambda a: (rank[a[0]], rank[a[1]]))` | `acyclic.rs:126` `pairs.sort_unstable()` on `(tail, head, edge)` | **differs under `ArcOrder::Feedback`** — see finding 4 |

**Verdict: the rewrite still matches.** Heap key, tie-break and self-loop handling — the
three properties `docs/reviews/review-layout-tree.md:56-64` certified — are all still exactly
what that review said, and I re-derived each from the reference rather than trusting the
review. The only divergence is the arc *sort key*, which the old code did not have (the
arc-set concept is new) and which is confined to the branch the product path does not take.

Tie-break detail worth naming: `v` appears once in the port's key where the reference has
`rank[n]` then `n` (`hierarchical.py:258`). They coincide because `rank == index` under the
contract; `feedback.rs:15-17` states the reason rather than leaving it implicit.

---

## Check 3 — `scaled.rs` / `run_scaled` vs `hierarchical.py:679-685`; is `stages/dump.rs` inert?

**`run_scaled` against `hierarchical.py:679-685`: matches, including both degenerate
branches.**

| reference | port |
|---|---|
| `:679-680` `lo = min(x.values()) if x else 0.0` / same for `hi` | `scaled.rs:39-45` fold `f64::min`/`f64::max` from `±INFINITY`, non-finite → `(0.0, 0.0)` |
| `:681` `width = (hi - lo) or 1.0` — a Python `or`, so a zero span is *replaced* | `scaled.rs:49` `if span == 0.0 { 1.0 }` — not a divide-by |
| `:685` `((x[i] - lo) / width * 2 - 1) * scale` | `scaled.rs:56-58`, same operand order |
| `:684` `((layer_of[i] / max_layer) * 2 - 1) * scale if max_layer else 0.0` — the conditional covers the **whole** expression | `scaled.rs:63-68` `if self.max_layer == 0 { return 0.0 }` before the quotient |
| `:662` `max_layer = max(layer_of) if layer_of else 0`, over the ordering graph including dummies | `scaled.rs:85` `layering.layer_of.iter().max().unwrap_or(0)` |
| `x` covers **every** ordering-graph vertex, dummies included (`lo`/`hi` are its extremes) | `coords.rs:29-31` sizes `x` to `layering.layer_of.len()`; `scaled.rs:83-87` takes `lo`/`hi` over that whole vector and `scaled_paths` (`:121-122`) uses the dummy X |
| `:685` `z = 0.0` | `Geometry::planar` + `NodeGeometry::Point` |

The load-bearing claim — that this cannot be a post-pass in `conformance/motor.rs`, because
`Geometry` carries no dummy coordinates — holds: `coords.rs:30` confirms `Coords` is sized
by the ordering-graph vertex count, not the node count. So the entry point belongs beside
the stages, as placed (`mod.rs:23,27`), and the motor override (`motor.rs:69,100-104`) is a
routing change only. Pinned independently of the reference run by
`scaled/tests.rs:30-38` (zero-width chain, both ends of both axes), `:40-48` (non-degenerate
fan-out), `:50-57` (`max_layer == 0`) and `:59-70` (node-order orientation, values read off
the reference).

Divergence: `scale <= 0` and non-finite are refused where the reference accepts them —
finding 5, MINOR, deliberate and pinned.

**`stages/dump.rs` is test instrumentation only. Confirmed three ways.**

1. Reachability: `mod.rs:24-25` is `#[cfg(test)] mod stages;`, and `stages.rs:67-68` is
   `#[cfg(test)] mod dump;`. Neither is in a non-test build, and nothing outside them calls
   `stages()` — its only caller is `dump.rs:179`.
2. I/O: `grep` for `std::fs|println!|eprintln!|dbg!|std::io|write!` over
   `crates/graph-core/src/layout/sugiyama/` returns `mod.rs` and `stages/dump.rs` only.
   Both are inside `#[cfg(test)]` blocks (`mod.rs:150-151` `mod measurement`, pre-existing
   and untouched by this branch; `dump.rs:163` `#[test] #[ignore]`). The product path —
   `run` (`mod.rs:83`) and `run_scaled` (`scaled.rs:74`) — contains no I/O of any kind.
3. Its writes go to `target/`, and it is `#[ignore]`d: `scripts/orch/gr cargo test -p
   graph-core sugiyama` → `test result: ok. 25 passed; 0 failed; 2 ignored`, exit 0. The two
   ignored tests are the two dumps.

`stages.rs` itself is a faithful re-run of `layered()` (`mod.rs:42-50`) plus the two
intermediate values `layered` drops — `reversed` and `crossings` — which is what the stage
diff needs. It duplicates the six stage calls rather than reaching through `layered`;
`stages.rs:41-43` says why.

---

## Check 4 — complexity and house limits

**No O(n²) anywhere on the path, and nothing introduced by this branch.**

- `feedback.rs`: `unique_neighbours` is O(m log m) (one sort per row);
  `Peeling::peel` walks each adjacency list once per removed vertex, so the whole loop is
  O(m) plus O(m log n) of heap pushes. `greedy_fas_order` (`:123-152`) is O(m log n) — no
  full scan per vertex.
- `acyclic.rs:57-66` is one ascending pass over edges. `grouped` (`:115-135`) is one
  O(m log m) sort plus a linear walk — the module doc at `:112-114` says so and it is true.
- `layering.rs`: `longest_path_layers` is a single Kahn pass; `reduce_slack` (`:76-91`) is
  ≤4 passes over nodes with a per-node median, i.e. O(4·(n + m log m)). `budget_plan`'s inner
  `spans.iter().filter(..)` (`:140-151`) `return`s on its first hit, so it is O(m) overall,
  not nested. `materialize` is one linear pass.
- The superlinear-in-m item the branch adds is the **triplication** of the sort:
  `distinct()` at `layering.rs:48`, `distinct()` at `:129`, `grouped()` at `:219` — three
  full `O(m log m)` sorts per pipeline where develop had none. Constant factor, not a
  complexity class change, but it is finding 2's other half.
- Unchanged and outside this delta: `ordering.rs:146-173` `transpose` is
  O(rounds · Σ|adjacent pair| · deg), and `bilayer_crossings` is exact. Not a quadratic
  step. `layout.dag.sugiyama`'s registered `scale_ceiling` is 200 000
  (`registry/grid.rs:88`), and `scale_ceiling` is enforced by `graph-cli bench`, not by the
  layout — so at 1 M nodes nothing in the layout refuses, but no quadratic step appears if
  one tries.
- Finding 8 (`dump.rs:73`, O(n·m)) is the only superlinear loop in the delta and is
  `#[cfg(test)] #[ignore]`d.

**House limits.** Every file in the delta is under 300 lines: `acyclic.rs` 208,
`acyclic/feedback.rs` 156, `acyclic/tests.rs` 79, `layering.rs` 241, `layering/tests.rs` 68,
`mod.rs` 293, `scaled.rs` 131, `scaled/tests.rs` 78, `stages.rs` 68, `stages/dump.rs` 185,
`motor.rs` 294, `motor/tests.rs` 298, `gaps.rs` 97, `rows.rs` 238. No function in the delta
exceeds 40 lines — the longest are `greedy_fas_order` (34, `feedback.rs:123`), `slide`
(33, `layering.rs:93`, at the 4-parameter limit), `run_scaled` (30, `scaled.rs:74`),
`budget_plan` (27), `dump.rs::load` (27), measured by brace-depth scan. One over-limit
function exists in the touched area, `motor.rs:242` `row_line` at 53 lines, and it is
finding 12: pre-existing and not in this diff.

---

## Check 5 — `scripts/scigraphs-conformance.sh`

```
scripts/orch/gr cargo build --release -p graph-cli            -> exit 0
scripts/scigraphs-conformance.sh                              -> exit 0
  ...
  SUGIYAMA: ok — 597 f64, 1020 f32 of 1020 coordinates, median 1.259e-16 <= 1.000e-15
PASS
```

**Matches the measurement doc exactly.** `docs/measurements/scigraphs-conformance.md:156`
records `f64 x 597/1020`, `f32 x 1020/1020`, max ULP 2.57e+08, max gap 1.91e-07,
procrustes median 1.26e-16 / 4.63e-16, tier `tolerance`, cause `arithmetic`; the run above
reports 597/1020 and 1020/1020 with median 1.259e-16 against a 1.000e-15 ceiling. The
baseline pin moved with it and is correct: `conformance/baseline/table/structured.rs:13`
`fb980d82…c8240cd` (was `859ea6d8…674b4`), `1e-15` (was `1e0`), tier `shape` → `tolerance`,
cause `algorithm` → `arithmetic`; the reference-side sha is untouched.

`gaps: &[]` (`rows.rs:230`) is defensible on both gaps. `G_SCALE_FIXED_LAYER` is gone
because `run_scaled` takes `scale` and the motor hands it the dispatcher's `5.0`
(`motor.rs:102`) — the gap's premise, "`run` takes no scale", is no longer true of the arm.
`G_NO_ITERATIONS` was dropped as a false record: `_sugiyama_layout(G, scale)` takes no count
(`hierarchical.py:638`), and the throttle the reference *does* hardcode
(`:670-672`, 8/4 iterations, 4/2/1/0 transpose rounds) is implemented verbatim at
`ordering.rs:20-27`. The constant survives for the six rows that still need it.

**Not run:** `scripts/scigraphs-conformance.sh --break` (the negative control) and
`capabilities --check`'s 1000-seed requirement — orchestrator rows.
`scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` → exit 1, but all 36
problems are `"run the gate"` records missing on a fresh worktree
(`hashgate ran 8 seeds, need 1000`, `no oracle-diff record`, …); the only `layout.dag.sugiyama`
lines are `gated, but hashgate ran 8 seeds, need 1000` and `gated, but no roundtrip record`,
i.e. the same missing-gate state. Nothing sugiyama-specific.

---

## Check 6 — the worker's open question: should the registry `Metadata` be reworded off the greedy FAS?

**Yes, and there are three fields, not one.** Answered from the code:

- `registry/grid.rs:97`, `oracle`: "...per-seed structural invariants (**acyclic after
  FAS**, monotone layers, contiguous dummy chains) checked by graph-cli roundtrip". The
  invariant that is actually checked is "acyclic after orienting every non-loop edge forward
  along `ArcOrder::NodeIndex`" (`acyclic.rs:46,59`). "FAS" names a stage the product path
  does not run.
- `registry/grid.rs:109`, `ponytail`: "**Ponytail (FAS)**: greedy, not minimum; extra
  reversed edges (note 5) are cosmetic". The note-5 part is still exactly right — a reversed
  edge is drawn head to tail, `acyclic.rs:13-16` — but the failure mode is now the node
  order's, not a greedy peel's, so the "greedy, not minimum" framing describes a solver that
  is not running. The right sentence is the one already in `acyclic.rs:18-22`.
- `registry/grid.rs:99`, `complexity`: `"O(n+m) per phase"` — **this one the worker did not
  ask about, and it is the more falsifiable of the three.** Finding 2.

The branch is right not to have touched `registry/grid.rs`: it is outside the paths
`prompts/jobs/sg-sugiyama.md:33-35` grants, and `docs/decisions/sugiyama-heuristics.md`
§"Cycle breaking is by node order" records the deferral explicitly ("That file was outside
the repair job's envelope and is left for the job that owns it"). So this is a documented
deferral, not an omission — but it is a deferral that leaves the public capability contract
describing the wrong algorithm, and it should be a named follow-up rather than a paragraph
in a decision doc.

---

## What to change, in order

1. `crates/graph-core/src/registry/grid.rs:97` — reword `oracle`'s "acyclic after FAS" to
   the node-order orientation the pipeline actually performs.
2. `crates/graph-core/src/registry/grid.rs:109` — replace the FAS sentence in `ponytail`
   with the node-order breaker's, keeping the "note 5 is cosmetic" part.
3. `crates/graph-core/src/registry/grid.rs:99` — `complexity` to `O(m log m)` per layering
   phase, or hoist one `grouped()` into `layered()` (`layering.rs:42-50`) and pass it down.
4. `docs/decisions/sugiyama-heuristics.md`, §"Parallel edges are deduped" — add one line
   that the re-ordering of dummy allocation and `up`/`down` moves `layout.dag.sugiyama`'s
   coordinates on multigraphs, which is finding 3 and is currently undocumented.
5. `docs/measurements/sg-sugiyama.md:183` — reword "byte-identical" to state the structural
   reason the corpus cannot see the change (finding 9), and add a `--dag` fixture with a
   parallel arc so the gate can.
6. `docs/measurements/sg-sugiyama.md:133` — make the stage table reproducible: commit
   `diff.py` or paste it, and fix the `py-stages.json` / `py.json` filename mismatch.
7. `crates/graph-core/src/layout/sugiyama/acyclic.rs:186` — one sentence on
   `ArcOrder::Feedback`: the arc list is ordered by node index, not by rank (finding 4).
8. `crates/graph-core/src/layout/sugiyama/acyclic/feedback.rs:151` — `unreachable!` after
   the heap loop (finding 6).
9. `crates/graph-core/src/layout/sugiyama/scaled/tests.rs` — empty-topology case (finding 7).
10. `crates/graph-core/src/layout/sugiyama/stages/dump.rs:73` — index map instead of
    `position` per endpoint (finding 8).
11. `crates/graph-core/src/layout/sugiyama/scaled.rs:75` — either accept `scale == 0.0` or
    add the Ponytail line naming the input the port refuses and the reference does not
    (finding 5).
12. `docs/reviews/review-layout-tree.md:59` — annotate the three certified claims with the
    fact that they hold for `ArcOrder::Feedback`, the branch the pipeline does not take
    (finding 11).

Items 1-3 need a job whose paths include `crates/graph-core/src/registry/grid.rs`; they are
outside `prompts/jobs/sg-sugiyama.md:33-35`, which is why they are findings here and not
edits.

VERDICT: FIX
