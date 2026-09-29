# Sugiyama layout — heuristics, deviations and the oracle differential

Status: **decided** (Phase 5). Scope: `crates/graph-core/src/layout/sugiyama/*`,
`fixtures/dag/*.json`, `docs/measurements/phase05-crossings.md`,
`crates/graph-contract/src/notes.rs` (note codes 4-5).

## Pipeline and citations

Ported from `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:1-693`. Four
stages, one module each:

1. **`acyclic`** — cycle breaking: greedy feedback-arc-set (Eades, Lin & Smyth,
   "A Fast and Effective Heuristic for the Feedback Arc Set Problem", 1993).
2. **`layering`** — layer assignment: longest-path layering with slack reduction, then
   dummy-vertex chains for edges spanning more than one layer, budget-limited.
3. **`ordering`** — crossing reduction: median heuristic + transpose, with an iteration
   and transpose-round throttle (Gansner, Koutsofios, North & Vo, "A Technique for
   Drawing Directed Graphs", 1993 — the `dot` algorithm), counted exactly via bilayer
   cross counting (Barth, Jünger & Mutzel, "Simple and Efficient Bilayer Cross
   Counting", 2002).
4. **`coords`** — X assignment: the priority method (Sugiyama, Tagawa & Toda, "Methods
   for Visual Understanding of Hierarchical System Structures", 1981).
5. **`routing`** — geometry: node points, edge polylines through their dummy chains.

Dense-index tie-breaks are used everywhere ties can occur (FAS candidate order, layer
compression, ordering initialization, median ties, priority-move ties), per the
determinism constraints (`prompt.md` §6, D1-D10).

## Ponytail markers — every heuristic, consolidated

The four approximate stages each carry a `Ponytail:` doc comment in their module (never
on the exact bilayer-crossing counter or the exact longest-path/slack-reduction code,
which are not heuristics):

- **`acyclic.rs`** — greedy FAS is a heuristic, not a minimum feedback-arc-set solver.
  Failing input: an adversarial tournament graph where this local peeling order cannot
  reach the true minimum. Direction: more edges reversed than strictly necessary —
  cosmetic (each one notes `dag.edge_reversed`), never a wrong graph, since a reversed
  edge is drawn head to tail, not dropped.
- **`layering.rs`** (on `Layering::build`'s budget) — `DUMMY_BUDGET` (200,000,
  `hierarchical.py:7`) is a resource cap, not a correctness rule; over budget the
  longest-span arcs draw straight and note `dag.dummy_budget_exceeded`, never silently.
- **`layering.rs`** (on `assign_layers`) — parallel edges are kept separate rather than
  deduped like the reference's `(u, v)` set; see "Parallel-edge deviation" below.
- **`ordering.rs`** — median+transpose is a local search, not a minimum-crossing solver
  (the crossing count itself is exact, computed by an accumulator-tree/Fenwick
  bilayer counter). Failing input: a layer whose optimum needs a non-adjacent swap this
  pass never tries. Direction: more crossings than optimal, never a wrong edge. Escape
  hatch: the margin in `docs/measurements/phase05-crossings.md`.
- **`coords.rs`** — the priority method is a small, cheap heuristic, not Brandes &
  Köpf's edge-straightening method (the reference's own comment at
  `hierarchical.py:611-613` says as much). Failing input: a long dummy chain that
  Brandes-Köpf would draw straighter. Direction: a visually kinked long edge, never a
  wrong position — every vertex still sits inside its own layer's row. No separate
  escape hatch: only the crossing count downstream of `ordering` is gated.

## Deviations

### Parallel edges are not deduped

The reference collapses layering to a `(u, v)` edge set before computing dummy chains;
this port keeps every parallel arc as its own entry through `layering::assign_layers`
and `budget_plan`, so a multigraph with `k` parallel edges between the same two
far-apart layers gets `k` independent dummy chains instead of one shared chain. This
costs more dummy budget on a multigraph and nothing else: geometry is still correct
(every edge still routes tail to head through its own chain), determinism is unaffected
(dense-index order over edges, not deduped pairs), and no fixture here is dense enough
in parallel edges for the budget difference to matter. Marked Ponytail-adjacent in
`layering.rs`'s `assign_layers` doc rather than fixed, since fixing it would mean
building and threading a `(u, v) -> Vec<edge>` map through the whole layering stage for
a cosmetic budget saving no test here needs.

### Disconnected components need no virtual root

Phase 3's `hierarchy.rs` gives disconnected forests a hidden virtual root so every tree
layout has one root to recurse from. Sugiyama layering does not need the equivalent:
`assign_layers`' longest-path walk and `ordering::init_order`'s breadth-first seeding
both run over the whole arc set with no assumption of a single root, so each weakly
connected component gets its own layer-0 start and its own BFS order independently,
correctly, with no virtual node and no geometry for one to leak into the output. The
`disconnected` fixture (a chain and a separate diamond, no edge between them) is the
regression test for this: 0 crossings, and layers respect each component's own edges
after reversal, with nothing tying the two components' layer indices together.

### `canonical_json/schema.rs` is now stale (documented, not fixed)

`crates/graph-contract/src/canonical_json/schema.rs`'s `note_codes` doc comment and
JSON-schema literal (`"enum": [1, 2, 3]`, "4-6 are reserved for later phases") were
written before this phase activated codes 4 and 5 in `notes.rs`, and are now out of
date: no test cross-checks the schema generator against the real `NoteCode::ALL`, so
`cargo test --workspace` stays green while the generated schema quietly under-declares
the valid set. `schema.rs` is not in this phase's CREATE/MODIFY envelope (it is neither
a new `sugiyama` module nor `package.json`/`package-lock.json`), so it is left as is and
recorded here as a gap for whichever phase next touches schema generation, rather than
opportunistically patched.

A second file has the identical gap: `docs/decisions/snapshot-notes.md` (the Phase-3 ADR
for the notes section) still documents codes 4-6 as a block of "reserved, not `NoteCode`
variants" (its lines on "4 (reserved) dag.dummy_budget_exceeded", "5 (reserved)
dag.edge_reversed", "The reserved codes 4-6 are not `NoteCode` variants, so no writer can
emit one", and "every code is 1-3 (4-6 are refused as `Reserved`...)"), which is now
false for codes 4 and 5. It is also out of this phase's envelope (not one of the four
`docs/decisions/sugiyama-heuristics.md` / `docs/measurements/phase05-crossings.md` /
fixture / `sugiyama/*.rs` files this phase may create or modify), so it is named here,
not edited, for whichever phase next touches note-code docs.

### Note codes 4 and 5 activated here, as pre-authorized

`dag.dummy_budget_exceeded` (4) and `dag.edge_reversed` (5) were reserved codes in
`notes.rs` (`RESERVED_NOTE_CODES`) with no `NoteCode` variant, so `layering::Route` and
`acyclic`'s FAS reversal could not emit them until this phase turned them on: added as
real `NoteCode` variants, removed from `RESERVED_NOTE_CODES` (leaving only `6`,
`post.route_fallback`, Phase 8), and covered by a dedicated round-trip test
(`dag_note_codes_activate_and_round_trip_both_faces` in `notes/tests.rs` and
`notes/tests/json.rs`) confirming both the binary and JSON faces accept the two new
codes and everything else is unaffected. This activation was pre-authorized for Phase 5
specifically to avoid a separate integration step for a change confined to one file.

### Isomorphism invariance does not hold, by design — documented, not tested to pass

`prompts/ONBOARDING.md` §6.5 lists isomorphism invariance (relabeling nodes without
changing structure gives identical geometry up to the label permutation) as a
metamorphic property worth checking when there is no oracle. Sugiyama layout does not
have this property here, deliberately: every tie-break in the pipeline (FAS candidate
order, layer-compression order, `init_order`'s BFS seed order, median ties in
`ordering`, priority-move ties in `coords`) is broken by dense index, per the
determinism requirement that the same graph always draws the same way (D1-D10). A
structure-preserving relabeling changes which node dense index sits on which tie, so it
can legally produce a *different valid* layering/ordering/coordinate assignment for an
isomorphic graph — same structural invariants (layers respect edges, dummy chains are
contiguous, no crossing-count regression), different geometry. This is expected
behavior for a heuristic pipeline built for reproducibility over relabel-invariance, not
a bug, and is not asserted as a passing test; determinism (same graph, same input order,
byte-identical output) is what is tested (`run_is_deterministic`, and the module's
determinism test run twice per the branch-local gate).

### The measurement lives in the oracle harness, not in `scratch/`

The Node side is `harness/oracle-layouts.mjs --dag`, the dagre-d3-es arm the phase
authorizes; its input is `target/dag-crossings.json`, written by graph-core's ignored
`dump_crossing_measurements` test, so nothing measured is committed as a working file.
The raw evidence of the recorded run is condensed into
`docs/measurements/phase05-crossings.md`. `package-lock.json` stays in the tree because
the arm's `npm ci` needs it to install the pinned `dagre-d3-es`.

### The first crossing-measurement technique was wrong, and was replaced before being trusted

The original plan read dagre-d3-es's own crossing count off its drawn geometry: lay a
graph out with `layout()`, bucket every node center and edge bend point into a layer by
exact `y`, sort by `x`, brute-force count interleaved segments per adjacent rank pair.
This was implemented, then checked against a hand count on the `wide-layer` (K4,4)
fixture before being trusted — any two-layer straight-line drawing of the complete
bipartite K4,4 has exactly `C(4,2) * C(4,2) = 36` crossings, unconditionally, since all
16 edges exist regardless of ordering. The geometry reconstruction reported 4. A
diagnostic dump of `edge.points` showed why: they are spline control points from
dagre's curve-fitting render step, not points that sit on a rank's `y` at all (a K4,4
edge's first bend point landed at `y: 6.25` against real rank `y`s of `5` and `65`), so
bucketing by exact `y` scattered real segments into spurious one-point buckets and
silently dropped nearly all of them from the count.

Replaced with a method that reads dagre's own crossing count directly instead of
re-deriving one from geometry never meant to expose it that way: re-run the same
internal stages `layout()` itself runs (`acyclic.run`, a scratch-root attachment
matching what dagre's own nesting-graph does for a flat non-compound graph so
network-simplex ranking never sees a disconnected input, `rank`, dropping the scratch
root, `normalizeRanks`, `normalize.run` for dummy chains, `order`), then calling dagre's
own `crossCount` on the resulting layering — the exact function dagre's ordering
heuristic minimizes internally, and the same bilayer-counting algorithm family
(Barth, Jünger & Mutzel) as our own `ordering::bilayer_crossings`. Re-checked against
K4,4: 36, matching the hand count and our own layout. Full write-up and the real
measured numbers: `docs/measurements/phase05-crossings.md`.

This correction happened before the margin in that file was evaluated against any
number, and changes only how "dagre's count" is computed, not the frozen margin
thresholds themselves or anything in `ordering.rs`. It is recorded here because it
reversed the phase's first (wrong) `status: blocked` reading on `wide-layer` to the
correct, verified `status: pass`.

### rustfmt vs. the 300-line-per-file cap

rustfmt's default "small heuristics" force any multi-field struct literal or
multi-call iterator chain onto several lines regardless of whether it fits under
`max_width = 100`, and there is no project `rustfmt.toml` overriding this. `layering.rs`
production code alone was 302 lines with zero tests before any trimming. Brought under
300 in every one of the six files (299/298/294/216/153/291 lines respectively) by, in
order of preference: structural merges that cut real code (folding `preds_succs` into
`assign_layers`; folding `arc_spans`/`dummy_budget_threshold`/`routed_dummy_count` into
one `budget_plan`; moving the `PlaceCtx` context struct's fields onto `ChainBuilder`
itself so a free function became a method with fewer parameters — all reducing the
number of struct literals rustfmt would otherwise expand); packing doc-comment prose
into single long lines, since rustfmt's `wrap_comments` defaults to `false` and never
reflows `///`/`//!` text regardless of length; and removing blank lines between
top-level items. One empirical quirk hit and worked around without full diagnosis:
removing the doc comment from `layering.rs`'s `Route::Direct` variant caused the
sibling `Chain { first, count }` variant's struct-literal-shaped payload to reformat
from one line to four; restoring `Direct`'s doc comment reverted it, so every `Route`
variant keeps a short doc.

## Fixtures

`fixtures/dag/{chain,diamond,cyclic,multi-span,wide-layer,disconnected}.json` — a linear
chain (no dummies), a diamond, a 3-cycle exercising FAS reversal, two multi-span edges
exercising dummy chains, a complete bipartite K4,4 exercising real crossing reduction
(the forced-36-crossings case above), and two disconnected components (a chain and a
diamond) exercising the no-virtual-root reasoning above. Each file's own `"about"` field
states what it is for.

## Result

`status: pass`. Measured numbers, per-fixture table and the ≥200-seed synthetic sweep:
`docs/measurements/phase05-crossings.md`. No constant in `ordering.rs` was tuned to
reach this result.
