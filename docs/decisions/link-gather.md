# The link pass as a range kernel: a scatter becomes a gather

Status: accepted, Phase 11 (branch `p11`). Implements `phase-11-compute-tiers.md` step 2 for
the one pass of `layout.force.barnes_hut` that was not already a gather.

Prompt: `prompt.md` §6 **D10** — kernels are in gather form, so any division of the outputs
is a legal schedule and no division splits one element's sum. A force layout's link pass
computes **one delta per edge and adds it into two nodes**, which is a scatter: two elements
written by one unit of work, possibly into two different ranges. D10 allows two repairs — a
**gather**, or a **fixed-order reduction**. This records which one was taken and why the
chosen one is bit-identical rather than close.

## The two repairs, and which one this port takes

| repair | shape | cost | verdict |
|---|---|---|---|
| **gather**, partition by node | node `i` sums its own share over its own incident edges | each edge's terms are evaluated at **both** endpoints, so link's arithmetic is done twice | **taken** |
| fixed-order reduction, partition by edge | each edge writes `out[e]`; one serial ascending-`e` merge scatters into the two nodes | the scatter survives, and a second buffer of edge length is needed | rejected |

The reduction is legal — D3's "fixed-order reduction" is exactly what it is, and the
per-node sums would still be in ascending edge order. It is rejected because it does not
remove the thing the contract exists to remove: a serial pass that writes across the slices,
whose cost is `O(m)` whatever the worker count, and whose output buffer is not the same
column the other two passes write. The gather leaves the pass with the same shape as
`charge` and `collide` — one input, one output column, ascending index — which is what lets
all three share one scratch buffer and one merge.

## Why the gather is the same computation, term for term

1. **The per-edge quantity is a pure function of start-of-step state.** `halves` reads the
   projected positions `x + vx`, `alpha`, the seed, the tick number and the frozen per-edge
   `(distance, strength, bias)`. It writes nothing, and nothing it reads is written by the
   pass — devil C7's Jacobi snapshot is already in force. So every worker sees the same
   inputs whichever slice its node fell into.

2. **The one shared difference is recomputed identically from either endpoint.**
   `displaced(sim, hi, lo)` is always the higher endpoint minus the lower, so the two
   endpoints never re-derive it independently. IEEE-754 round-to-nearest subtraction is
   antisymmetric — `fl(a - b) = -fl(b - a)` exactly — and for `a == b` both directions give
   `+0.0`, not `±0.0`, so the `jiggle` branch that replaces it is taken identically from both
   ends. Pinned by `link/tests.rs`'s two tests.

3. **Each node's sum sees its terms in the order the single-threaded loop saw them.** The
   old loop walked edges `0..m` ascending and accumulated into `dvx[node]`, so node `i`'s
   terms arrived in ascending *simple-edge index*. The kernel walks `graph.rows.row(i)`,
   and `SimpleGraph::rows` is built by `row_csr` over ascending simple-edge indices, so the
   row is in that same order. Same terms, same order, same start from `0.0`: the sums are
   the same floats, not merely close.

4. **The two shares are the same two floats.** The old code wrote `dvx[hi] -= fx * b`; the
   kernel adds `(-fx) * b` for the same `fx`. Negation is exact in IEEE-754, so `-(fx * b)`
   and `(-fx) * b` are the same value, sign included.

Together: `LinkPass` is the old loop with the node range as its index, not a second
implementation. The tests hold it there —
`barnes_hut/tests/kernels.rs`'s `link_gives_the_same_deltas_at_every_worker_count_as_the_loop_it_replaces`
compares the kernel against `link::scatter` (the old inline body, kept as a named function
for exactly this) and against every worker count including 3, 7, 8 and 64.

## What it costs, stated

Every simple edge's terms are evaluated twice, once per endpoint. That is the price of the
gather and it is not hidden: it is inside the timed stage in
`docs/measurements/phase11-threads.md`, so the threads tier's reported speedup is the speedup
*with* that doubling.

## What would make this wrong, and what catches it

- **A changed edge order** (a CSR sorted by value, or a row built in a different pass order)
  would reorder a node's sum and change the bytes. `csr.rs` already documents that a sorted
  CSR breaks the differential; the kernel's row order is now load-bearing for the layout too,
  and the equality test above is what would fail.
- **A merge that scatters** would put two elements in one worker's range. `Split::Link` is
  the control for exactly that shape: it makes the merge read the *next* node's delta, and
  `GM_MUTATE_SPLIT_SUM=link` goes red on `layout.force.barnes_hut` and on nothing else.
- **A per-pass control that passes vacuously.** Collide's own control bites at the gate's
  fifth seed and not before (at two or three nodes link and many-body have already pushed the
  pair past `2 * collideRadius`, so collide has nothing to resolve). That is measured in
  `barnes_hut/tests/kernels.rs::every_passs_own_control_bites_by_the_gate_s_fifth_seed`, the
  same trap `cli_force.rs` records for the theta knob — and the gate now **refuses** such a
  row (exit 2, "could not run") rather than passing it, so `tier-thresholds.md` carries the
  refusal and not a vacuous exit 0.

## Ponytail (the choice of gather)

Recomputing each edge at both endpoints is a trade, not a proof. **Failing input:** a graph
so edge-dominated that link's doubled arithmetic is visible in the tier's wall time — a
hub-and-spoke model at large `m`, where the pass is `O(m)` and the doubling is not amortised
against the many-body pass's `O(n log n)`. **Direction:** the threads tier loses time there,
never bytes: every tier is hash-equal to scalar per stage. **Escape hatch:** the explicit
`exec` option per call, or the node threshold in `tier-thresholds.md`.

No marker is owed on the kernel itself. `LinkPass` is exact — same terms, same order, same
floats — and the marker belongs on the decision to prefer it to the reduction, which is
where the judgement is.
