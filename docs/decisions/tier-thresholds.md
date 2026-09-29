# Tier thresholds — the table `exec/select.rs` reads, and where it came from

Status: **provisional and inert.** Every node threshold is `4_294_967_296` and the worker cap
is `1`, so `exec: "auto"` selects `Scalar` at every graph size. This file records why that
is the honest table today and what has to be measured before any number is written into it.

`compute-tiers.md` rule 4 and `phase-11-compute-tiers.md` step 5: the thresholds are derived
from measurements, never guessed, and `select(n, m, caps, thresholds)` is a **pure function**
of its four arguments — no clock (D8), no hardware detection inside graph-core.

## The committed table

| field | value | meaning |
|---|---:|---|
| `simd_nodes` | `4_294_967_296` | nodes past which SIMD measured faster than scalar |
| `threads_nodes` | `4_294_967_296` | nodes past which threads measured faster than the best single-threaded tier |
| `threads_max` | `1` | the most workers threads may use, whatever the host reports |

These are `graph_core::exec::Thresholds::MEASURED`, and `select.rs`'s test
`the_committed_table_promotes_nothing_until_it_is_measured` asserts the consequence at every
node count the `u32` index space can hold.

**Two independent guards, so neither alone is load-bearing.** `4_294_967_296` is
`u32::MAX + 1`, one past the largest node count a `u32` index can hold, so no graph reaches
it (the comparison is `>=`); and `threads_max: 1` makes the threads branch unreachable at
any size even if the node threshold were lowered by mistake.

## Why nothing is promoted yet

Phase 9 measured where the **scalar** motor stops fitting the frame budget
(`docs/measurements/phase09-crossover.md`): native holds 16.67 ms per tick to N = 10 000,
wasm32 to N = 4 000, the TypeScript oracle to N = 2 000. It did **not** measure a SIMD arm
or a threaded arm, so this phase has no crossover for either.

A guessed number here would be the one thing rule 4 forbids: a threshold with no measurement
behind it is a claim in the ledger that nothing checked. So the table promotes nothing, and
a host that wants a tier asks for it by name — `exec: "threads"` is always allowed where the
host can run it, and is **refused** with an error, never silently downgraded, where it
cannot (`select.rs`'s `a_named_unavailable_tier_is_refused_rather_than_downgraded`).

## What each row needs before it can be written

A row is a measured crossover, and the measurement has three parts.

1. **The equality proof.** Every CPU tier is hash-equal to scalar over the gate's seeds, per
   stage: `hashgate --seeds N --tiers all` runs ten arms (native ×2, wasm32 ×2, a scalar
   reference, and one Barnes-Hut arm per worker count in {1, 2, 3, 4, 7}) and refuses to
   pass unless all ten agree. Without this a "faster" tier has only proved it is differently
   wrong.
2. **The speedup, with the losing sizes reported.** Not just the crossover: the sizes where
   the tier **loses** are the reason the threshold exists at all, and a table that reported
   only the wins would place every threshold too low.
3. **The machine class.** Phase 9's note applies verbatim: these numbers are a sample of one
   machine, one build, one seed. A row is stated as "measured on *X*, at *N*".

## The three rules that constrain any future row

1. **`auto` never selects a GPU id** (`compute-tiers.md` rule 5). A GPU result is only
   per-device reproducible, so it is reached by naming `layout.force.barnes_hut.gpu` and
   nothing else. `select.rs`'s `auto_never_selects_the_gpu_however_large_the_graph_or_wide_the_host`
   asserts this at the smallest graph that exists and the largest the index space can hold.
2. **Every gated worker count is one `select` could choose.**
   `hashgate/tier.rs` and `select.rs` share the vocabulary, and
   `every_gated_worker_count_is_one_select_could_choose` asserts a `threads_max: 7` selects
   a width the gate actually runs. A threshold promoting an untested width would be a row
   with no equality proof behind it.
3. **A speedup under 3 % at every N is noise** (`minimalism-ladder.md`, and the phase
   prompt's stop-and-ask 4). Report it; do not write a row; do not ship the tier.

## Ponytail (thresholds)

Thresholds come from measurements on one machine class. **Failing input:** a host whose core
count or SIMD width differs from the one the table was measured on. **Direction:** a
slower-than-possible tier is picked, which is cosmetic and never changes output bytes —
tiers 1a/1b/2 are bit-identical, so a wrong threshold costs time and nothing else.
**Escape hatch:** the explicit `exec` option, per call.

This is the marker the phase prompt requires on the thresholds, and it is the only marker in
this phase: `partition` and the range kernels are exact, and a marker on exact code would
teach the reader to discount it.
