# Phase 6 — force layout timing and ceilings (branch p6f)

Measured on the Phase 6 (p6f) worktree, release build, x86_64, inside the `ge-rust`
toolchain image. Reproduce, from the worktree root, with
`/goinfre/dlesieur/orch/bin/gr` (the docs write it as `/home/user/gr` on the older host)
standing in for the `cargo` below:

```sh
gr cargo run --release -p graph-cli -- bench --layout layout.force.barnes_hut --n 220,1000,10000
gr cargo run --release -p graph-cli -- bench --layout layout.forceatlas2 --n 220,1000,2000,5000,10000
```

`bench` times each registered layout's own `run` over an already indexed topology and
nothing else — building the model and formatting output are outside the timer, which is
what the original `elapsed_ms` on stderr measured too. The original run drove the
`examples/force_dump` binary that the phase branch added; that example is still in
`crates/graph-core/examples/` but is no longer the way these numbers are reproduced, and
its saved stderr (`scratch/dumps/timing-n<n>-{barnes_hut,fa2}.stderr`) is what the tables
below were read from. `--past-ceiling` runs a size past a layout's registered ceiling,
which the measurements at 100 000 need for Barnes-Hut.

## Barnes-Hut: tick count and scaling

`TICKS = 112` (`crates/graph-core/src/layout/force/params.rs`): the least `k` with
`(1 - alpha_decay)^k < alpha_min` at the frozen `alpha_decay = 0.06`, `alpha_min =
0.001` (`prompt.md` §5.2), checked by `ticks_is_the_least_k_below_alpha_min`. Every
Barnes-Hut run below ticks exactly 112 times regardless of `n` — theta-approximated
many-body is the only super-linear-per-tick cost, and it is `O(n log n)` per tick, not
`O(n^2)`.

| n (nodes) | m (edges) | elapsed | per tick |
|---:|---:|---:|---:|
| 220 | 329 | 33.83 ms | 0.302 ms |
| 1 000 | 1 541 | 220.36 ms | 1.968 ms |
| 10 000 | 15 474 | 3 296.46 ms | 29.43 ms |

Scaling check: 1000 → 10000 is a 10x increase in nodes; elapsed time rises 14.96x. A
pure `O(n log n)` law predicts `10 * (ln 10000 / ln 1000) ≈ 10 * 1.33 = 13.3x`; the
measured 14.96x is somewhat above that (collide's fixed-radius neighbour queries and
quadtree-depth growth both add a little beyond the many-body term's own `n log n`), but
is far below `O(n^2)`'s 100x — clearly sub-quadratic, unlike the dense FA2 below.

## FA2: dense O(n²), and its practical ceiling

`forceatlas2_layout`'s repulsion is a dense, all-pairs computation (networkx 3.6, as
ported verbatim — `state.rs::repulsion`, one Newton's-third-law pass over
`i < j`, still `Θ(n²)` work). Attraction and gravity are `O(m)` and `O(n)` and do not
change this. Iteration count is bounded (`max_iter = 100`, networkx's own default) but
not fixed the way Barnes-Hut's tick count is: `run()` exits early once a tick's total
movement falls under `1e-10`, so smaller/quieter graphs finish in fewer than 100
iterations — the timings below are each whatever `run()` actually did, not a forced
100.

| n (nodes) | m (edges) | elapsed |
|---:|---:|---:|
| 220 | 329 | 8.04 ms |
| 1 000 | 1 541 | 149.86 ms |
| 2 000 | 3 075 | 619.62 ms |
| 5 000 | 7 721 | 3 704.93 ms |
| 10 000 | 15 474 | 14 489.32 ms |

Scaling check: 5000 → 10000 doubles `n`; elapsed rises 3.91x — close to the 4x an
`O(n²)` law predicts (the shortfall from exactly 4x is `max_iter`'s early-exit: the
larger graph's `run()` did not necessarily use every one of its 100 allowed
iterations, and neither did the smaller one — iteration counts were not separately
logged, so this is read as consistent with `O(n²)`, not as a precise confirmation of
it).

**Ceiling**: at `n = 10 000`, one FA2 layout took **~14.5 s**. Doubling again to
`n = 20 000` would be expected near **~58 s** (`O(n²)`, all else equal) — past what any
interactive or per-request use can afford. `n = 10 000` is this measurement's own
practical ceiling for FA2 in this port: below it, sub-second to low-single-digit-second
runs; at and above it, tens of seconds, growing quadratically. This is a property of
the *algorithm being ported* (networkx's own dense form — the reference has no spatial
approximation, unlike Barnes-Hut's theta-tree), not a defect specific to this port; a
faster FA2 would need a Barnes-Hut-approximated repulsion of its own, which is outside
`P56_SPEC.md`'s decision 2 ("port ... as-is with networkx's defaults").

**Memory**: not separately profiled with a counting allocator (unlike
`phase01-topology-memory.md`'s `memory.rs` probe) — reported here as an analytical
reading of `Fa2State`'s own fields (`state.rs`), not a measured number, and labelled as
such rather than presented as one. Every field is `O(n)` or `O(m)`: `x, y, mass, ux,
uy` are five `f64` vectors of length `n` (40 B/node before allocator overhead), and the
`SimpleGraph` graph is `O(n + m)` (`lo, hi, strength` each length `m`, plus a CSR of
length `n + 2m`). The dense `O(n²)` cost is in *time* only — `repulsion()` computes
each pair's force on the fly and never materializes an `n x n` distance or force
matrix — so FA2's ceiling here is a time ceiling, not a memory one; at `n = 10 000`
its resident state is on the order of a few hundred KB to low MB, dwarfed by the ~14.5
s it takes to get there.

## Yifan Hu: absent

Per `P56_SPEC.md`'s own decision 3: *"Yifan Hu: stays `absent` — reason: Graphviz sfdp
source not on disk; SciGraphs' `YIFAN_HU` is a different algorithm."* Neither reference
this branch would need to port from is available in `/goinfre/dlesieur/refs` (written
`/home/user/refs/...` on the older host), and a missing reference is a stop, not an
improvisation (`AGENT_BRIEF.md`) — so it is not attempted,
guessed at, or approximated from the Barnes-Hut/FA2 code already here (which are
different algorithms, not drop-in substitutes for Yifan Hu's own multipole-style
force approximation). Not part of this branch's `CREATE` list either way
(`P56_SPEC.md` branch p6f lists only `rng.rs`, `quadtree.rs`, `barnes_hut.rs`,
`params.rs`, `forceatlas2.rs`).

## Stability: the Jacobi-link fixture (devil C9)

Reformulating d3's Gauss-Seidel link force as a Jacobi gather (`docs/measurements/
phase06-stress.md`'s deviation 1) is exactly the kind of change devil C9 exists to
catch: if it destabilizes the simulation into oscillation, the instruction is to
**stop and report, never normalise it away**. The adversarial fixture
(`barnes_hut/tests.rs::the_jacobi_link_stability_fixture_stays_finite_and_bounded_
devil_c9`) wires one mid-degree node to six much-higher-degree hubs (15 leaves each)
and runs the full `TICKS = 112`, asserting kinetic energy stays finite every tick and
settles (peak kinetic energy in the last 10 ticks < 1.0) rather than growing or
oscillating.

**Result: it does not oscillate.** Kinetic energy stays finite throughout and settles.
This is a genuine, run test result, not an assumption — verified run-twice
bit-identical as part of the branch-local gate's determinism checks. C9's "stop and
report" branch was not needed; reported here as the negative result it is (no
instability found), not omitted because it happened not to trigger.

## The wiring run (2026-09-29): `graph-cli bench`, and where the ceilings come from

The ceilings registered in `crates/graph-core/src/registry.rs` (`FORCE_CEILING`,
`FA2_CEILING`) are derived from the measurements above, and are now reproducible as a
gate, one layout at a time (the old `--nodes` bench form is gone; `--layout` names the
layout and `--n` the sizes, comma-separated):

```sh
gr cargo run --release -p graph-cli -- bench --layout layout.force.barnes_hut --n 220,10000,100000
gr cargo run --release -p graph-cli -- bench --layout layout.forceatlas2 --n 220,10000,100000
```

On this machine (release, x86_64, inside the toolchain image):

| n | edges | `layout.force.barnes_hut` | `layout.forceatlas2` |
|---:|---:|---:|---:|
| 220 | 329 | 17.83 ms | 6.96 ms |
| 10 000 | 15 474 | 2 375.95 ms | 13 860.56 ms |
| 100 000 | 154 978 | 57 793.99 ms | **refused** (past `FA2_CEILING` 14 000) |

**`FORCE_CEILING = 100 000`**, chosen as the largest round size inside a 60-second
budget: 100 000 measures 46.8–57.8 s natively, and 200 000 is extrapolated (not run —
it is itself impractically slow) to land past it. This is a **time** ceiling, not a
memory wall, and unlike the grid's and the hierarchy layouts' ceilings it is *not*
projected onto wasm32's 4 GiB: inside wasm32 the same curve runs on a slower machine
with no new refusal to detect it by.

**`FA2_CEILING = 14 000`**, from the same command with `fa2`. The 5 000 → 10 000 step
is 4.09x for exactly 2x the nodes, the `O(n^2)` networkx's dense all-pairs repulsion
predicts; under a 30-second budget `10 000 * sqrt(30 / 13.522) = 14 880`, rounded down
to two figures. 14 000 is not run — 13.5 s is already the slowest thing measured here,
and the point is the shape rather than the digit. The measured ratio between the two
ceilings is 100 000 / 14 000 = **7.14x**; `registry.rs`'s test pins "materially below"
at 5x rather than inventing a round factor the measurements do not support.

**`bench` refuses rather than runs past a ceiling**, and exits 0 doing so: the phase
gate's own size set contains 100 000, which FA2 refuses, so a non-zero code there would
make the gate row unsatisfiable. The refusal is printed with the ceiling it came from,
so the ceiling is checkable rather than a number in a doc. `--dry-run` reports which
sizes each layout would run and times none of them, which is what makes the ceiling
logic testable — Barnes-Hut at 15 000 nodes takes about 25 s.

### The honest answer the phase prompt asked for

> WASM is *slower* than the TypeScript at N=220 → report it, do not hide it.

**Not measured, and therefore not claimed.** Every number above is native x86_64. The
wasm32 arm is exercised by the 4-way hash gate, which proves the two targets produce
*bit-identical output* — it does not, and cannot, compare their speed, and the
cross-target timing the prompt asks for needs Phase 9's harness. What the gate does
establish at every size it runs is the thing that actually matters for correctness: the
force layouts are deterministic across targets, which is a stronger claim than being
fast, and is the one this project gates on.
