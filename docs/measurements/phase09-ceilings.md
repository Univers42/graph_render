# Phase 9 — ceilings: declared vs measured

The before/after table `prompts/phase-09-scale-bench.md` §5 asks for. `graph-cli
capabilities --check --ceilings-measured` reads this file: a row here whose `measured`
cell is not a number is a finding, and an id here the ledger does not have is a finding. A
ledger row this file says nothing about is **counted and printed as still reasoned**, never
counted as measured.

Machine class: the shared container host of this session, seed 0, reference degree 8.
Medians over 3 runs. Not portable, and not stable under load — `BENCHMARKS.md` shows a
4.8× spread on the same code between an idle and a loaded moment.

## Measured this phase

| id | declared | measured | how |
|---|---:|---:|---|
| layout.force.barnes_hut | 200000 | 4000 | largest N whose *wasm32* tick fits 16.67 ms; the native arm reaches 10000 on an idle host. A tick is `run / 112` in both, because the ABI has no per-tick entry |
| scale.lod | 9200000 | 1235726 | `O(n + m)` over an indexed topology; the measured cost is the topology's own columns at N = 10 000, the largest size measured |
| scale.simplify | 9200000 | 1235726 | same shape, plus the O(m log m) simple adjacency; bounded by the same measured columns |
| scale.adaptive | 9200000 | 1235726 | `O(1)`, a pure function of (n, m); the ceiling it inherits is its input's, not its own |

## Corrections this phase found, with both numbers

**The 33 B/node column table in `prompt.md` §5.1 is wrong by 3.7×.** Measured: **123.6
B/node** of columns at N = 10 000 (1 235 726 B over 10 000 nodes), plus a further
**42.9 B/node** of string arena, which the table does not mention at all. A single total
would be ~166 B/node. This is a declared ceiling's *input* being wrong rather than the
ceiling itself, and it is reported here because the ledger's memory rows are derived from
that table. The topology ceiling of 9 700 000 is, on this measurement, closer to
10⁶ · 166 B ≈ 166 MB than to 10⁶ · 33 B ≈ 33 MB — i.e. the declared ceiling is roughly
**five times** what the memory it was derived from allows on a 32-bit-addressed wasm
target. Per the phase's stop-and-ask, this is a finding for a human, not a number to
quietly correct: the ledger's `scale_ceiling` values are left as they are.

**The force layout's declared ceiling of 200 000 is not reachable at 16.67 ms per tick by
either compiled arm.** Measured crossover: 10 000 native (idle host), 4 000 wasm32, 2 000
for the TypeScript oracle. The declared 200 000 is a *usability* ceiling (a layout you may
still start and wait for), not a frame-budget ceiling, and the two are different numbers
that the ledger's single `scale_ceiling` field conflates. The measurement does not
contradict the declaration; it says the field means something the phase prompt's own 16.67 ms
table does not.

## Still reasoned, not measured

Every other row — the Phase 1–8 layout and analysis ceilings — is inherited and unmeasured.
Phase 9 did not run the campaign for them: each needs a run at (and just past) its own
ceiling for its own layout, which is a session per layout, and a row measured on this
host's shared load would be a number nobody could reproduce. `capabilities --check
--ceilings-measured` prints the count of rows still reasoned on every run, so the gap is
visible rather than assumed.
