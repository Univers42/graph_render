# Perf PM collide: fewer instructions per candidate and per hit, same bytes

Measured 2026-10-05 on branch `perf-pm-collide-soa` (from `develop` 4e3aa2e4). Host: dlesieur42,
i5-13600KF, 20 threads, 31 GB. Rust from `scripts/orch/gr`, wasm under Node in the same image.

**Not kept.** The keep rule asked for 3 ms off the 8-worker collide `Gather`; the change below
took 0.51 ms off it (33.35 → 32.84 ms), inside the spread of a single run. The instruction
count does fall — 2 758 → 2 608 per query, −5.4% — but at 1M nodes and 8 workers the `Gather` is
not instruction-bound enough for that to be worth 3 ms. Step 4 (the SoA window) is a measured
regression. None of the code is in `develop`: steps 2 and 3, with their test, are archived at
tag `archive/perf-pm-collide-soa` (cf048f1d); only this report landed.

## Why

`perf-pm-stencil.md` §Speed left collide `Gather` as the largest pass at 1M nodes and 8 workers:
33.07 ms of a 123.78 ms tick. A callgrind profile of the gather alone
(`tick --layout particle-mesh --n 100000 --warm 1 --ticks 1 --seed 1 --workers 1`,
`--toggle-collect='*Gather*step_range*'`, image `ge-profile`, line-tables release) put it at
about 2 670 instructions per query, of which roughly 700 went to the filter, 750 to the hit loop
and `resolve`, and 350 to the cell change (`reads`, `point`, `fill`). Two of those were pure
overlap with work already done:

- the hit loop read `self.slot[j]` for every hit to build the jiggle keys, and most hits never
  jiggle;
- `resolve` re-tested `l.is_nan() || l >= c.d2` on every hit, the filter having just decided
  exactly that with the same expression on the same operands.

## Change

| Step | What | Kept |
|---|---|---|
| 2 | The hit loop passes `k`, `j` and the grid to the jiggle path. `grid.order` and the window's `slot` are read inside `push::hit`'s `dx == 0.0` / `dy == 0.0` branches and nowhere else. | archived |
| 3 | `resolve` splits into the overlap test and `push::hit`. `Gather` calls `push::hit` on filtered hits only, so the re-test is gone. The arithmetic is `resolve`'s, moved verbatim: `l = dx * dx + dy * dy`, the two jiggle adds, `dist = sqrt(l)`, `push = (c.reach - dist) / dist * 0.5`, then the caller adds `dx * push` and `dy * push` in hit order. | archived |
| 4 | `at: [[f64; 2]; WINDOW]` → `wx`/`wy`, filter two candidates per iteration. | **dropped** |
| 5 | The hits pushed two at a time so a pair shares one `sqrtpd`/`divpd`. | **dropped** |

On the archived branch:

| File (`crates/graph-core/src/layout/force/particle_mesh/`) | What changed |
|---|---|
| `collide/push.rs` | new: `hit`, the push half of `resolve`, and the two jiggle pass numbers |
| `collide/gather.rs` | `delta`'s hit loop builds the keys lazily and calls `push::hit` |
| `collide.rs` | `resolve` keeps the test and calls `push::hit`; now `#[cfg(test)]`, because the gather no longer calls it and clippy reads an uncalled private fn as dead code |
| `collide/tests.rs` | `the_hit_only_push_is_resolve_bit_for_bit` |

No term, no order and no rounding changed. `push::hit` returns `(dx * push, dy * push)` and the
caller adds them to its own accumulator in hit order, which is the order `resolve` used, so each
of `out.0` and `out.1` still sums its terms in window order. `resolve` and `push::hit` share the
push body, so the tests' branched reference and the gather's filtered path cannot drift apart.

### The asm finding: the two-candidate filter does not pack

The premise of step 4 was that splitting the window by axis and pairing candidates would let LLVM
put two *candidates* in the two SSE lanes. It does not. Read off the release asm
(`objdump -d -l` on a `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` build, the same one
`scripts/orch/profile.sh` builds):

- **base filter**, one candidate per iteration, `at` interleaved: `movupd` (one 16-byte candidate)
  → `movapd`/`subpd` → `mulpd` → `movapd`/`unpckhpd`/`addsd` → `xor`/`ucomisd`/`seta` → the hit
  store. About **12–13 instructions per candidate**, and the lanes hold one candidate's x and y.
- **step 4's filter**, `wx`/`wy` and two candidates per iteration: `movsd` + `movhpd` (two loads
  now, one per axis, where the interleaved window paid one `movupd`) → `movapd`/`subpd` →
  `mulpd` → `movapd`/`unpckhpd`/`addsd` → `xor`/`ucomisd`/`seta`, and per lane a `movzbl` + the
  `mov` store plus its `add`/`lea` bookkeeping. About **19 instructions per candidate**.

There is no `addpd` and no `cmpltpd` in the step-4 loop: `unpckhpd` and `addsd` are still there,
once per candidate. Writing the two candidates as explicit `[f64; 2]` lanes through a `pair()`
helper was tried as well and produced the same shape. The reassociation pass runs after SLP and
puts x and y back into one vector, because x and y are the two values the horizontal sum
consumes; the source shape does not survive it. So the pairing is not reachable from safe Rust on
this target, which is plain x86-64: SSE2, no `target-cpu`, so no wider lane is available either.

Step 5 is dropped for the same reason and was not built: `push::hit` computes `l` from one hit's
`(dx, dy)`, the same two values a horizontal add consumes, so two hits cannot be made to share one
`sqrtpd`/`divpd` without the same reassociation undoing it. It could not have been kept in any
case — the job asks it to hold on top of steps 2–4, and step 4 is gone.

## Byte identity

| Check | Result |
|---|---|
| `graph-cli snapshot --layout force.particle_mesh` at n = 333, 1000, 4096, 12345, 20000, 50000, 77777, 100000: sha256 on this tree against its own base | identical at all 8 (`diff` rc 0) |
| `wasm-threads.mjs hash --serial <this base's serial.wasm>` (this tree's threaded build against the base tree's serial build), BH and PM, workers {1,2,3,4,7}, seeds 0–7 | rc 0, every row `equal`, 0 differing |
| `wasm-threads.mjs hash --break` (negative control) | rc 1, 64 differing, as expected |
| `graph-cli hashgate --seeds 8` | rc 0, PASS |
| `graph-cli hashgate --seeds 8` with `GM_MUTATE_REFERENCE_DEGREE=9` (negative control) | rc 1, 8 of 8 seeds diverge |
| `cargo test --workspace --no-fail-fast` | rc 0, all suites green |
| `cargo clippy --workspace --all-targets -- -D warnings` | rc 0 |
| `cargo fmt --all --check` | rc 0 |
| `cargo build -p graph-core --target wasm32-unknown-unknown` | rc 0 |

Raw: `$GM_SCRATCH/bench/pm-collide-soa/{xtree-base,xtree-branch,hash-base,hash-break}.out`.

### Negative controls

The control the job names — the filter's second lane reading `wx[j]` instead of `wx[j + 1]` —
targets step 4's code, which was dropped, so it was run against the single-lane filter:

- **the filter reads the wrong candidate.** `overlaps` used `py - self.at[(j + 1) % len][1]` in
  place of `py - qy`, the single-lane form of the same mistake. Failing test:
  **`the_filtered_gather_is_the_branched_one_bit_for_bit`** (and
  `the_grid_finds_every_overlap_the_pairwise_scan_finds`), `slot 0`, left
  `(13826141961753500315, 13839223781857575560)` against right
  `(4608182409034528193, 4617727353762584509)`.
- **the lazy jiggle keys read the wrong slot.** The keys' second element became
  `grid.order[self.slot[j + 1] as usize]`. Failing test:
  **`the_filtered_gather_is_the_branched_one_bit_for_bit`** (and
  `the_grid_finds_every_overlap_the_pairwise_scan_finds`).

One control that did *not* fire, worth recording: swapping the two jiggle keys,
`(order[q], order[k])`, leaves every test green. `jiggle` folds its pair order-independently
(`rng.rs::fold` sorts into `lo_hi`, pinned by `jiggle_is_order_independent_in_i_and_j`), so the
key *order* is not observable even though the job asks for `(order[k], order[q])` in that order.
The archived code keeps that order anyway; it costs nothing.

## Instructions per query

`valgrind --tool=callgrind --branch-sim=yes --dump-instr=yes '--toggle-collect=*Gather*step_range*'`,
100 000 queries, image `ge-profile`:

| build | Ir | Ir/query | branches | Br/query |
|---|---:|---:|---:|---:|
| base `4e3aa2e4` (the frozen binary) | 275 827 182 | 2 758 | 28 189 598 | 282 |
| steps 2 + 3 (archived) | 260 802 998 | **2 608** | 24 811 677 | **248** |
| steps 2 + 3 + 4 (dropped) | 277 398 206 | 2 774 | 24 148 436 | 241 |

−5.4% instructions and −12% branches for steps 2 + 3; +0.6% instructions for step 4 on top of
the base, which is why it is out. The job quotes 267.2 M as the base; measured here on this
branch's own base on this host it is 275.8 M, and the comparison above is against the latter.

The retained saving is almost entirely the branch count: 34 fewer branches per query, which is the
`l.is_nan() || l >= c.d2` re-test that step 3 removed — one branchless-but-present comparison per
hit before, none after.

## Speed

```
scripts/orch/gr /w/target/wf/pm-collide-soa/{base,final}/graph-cli \
  tick --layout particle-mesh --n 1000000 --ticks 7 --workers 8 --passes
```

Arms alternated base, branch, base, … three rounds each
(`$GM_SCRATCH/bench/pm-collide-soa/passes.sh`); load average 2.89–5.46 on 20 threads, 17–18 GB
available, no peer gate running. 1 000 000 nodes. Median of the three runs:

| pass | base ms | branch ms | Δ |
|---|---:|---:|---:|
| collide `Gather` | 33.35 | **32.84** | **−0.51** |
| **tick, median of 7** | **97.74** | **97.90** | +0.16 |

Run by run — `Gather`: base 32.41 / 33.72 / 33.35, branch 32.84 / 33.18 / 32.68. Tick median:
base 91.67 / 98.10 / 97.74, branch 97.90 / 101.34 / 95.85.

**Not kept: the rule asked for ≥ 3 ms off `Gather` and the median moved 0.51 ms, which is inside
the 33 ms pass's own run-to-run spread of 1.3 ms. The tick median did not fall either.** At 1M
nodes and 8 workers the `Gather` is not paying for the instructions it retires: five percent of
2 758 is 150 instructions per query, and each is a cheap, well-predicted SSE compare rather than a
miss. The job kept steps 2 and 3 because its fallback said "revert the code, keep the new test",
and the new test exercises `push::hit`, so the two could not both hold. The rule's intent is that
code which does not pay does not land, so steps 2 and 3 were reverted as well and archived with
their test at `archive/perf-pm-collide-soa`.

Raw: `$GM_SCRATCH/bench/pm-collide-soa/passes-{base,branch}-r{1,2,3}.out`, log in `passes.log`.

## What this does not do

- `Caveat:` the host was loaded (2.9–5.5) by two other OpenCode jobs for every run, and a peer
  `perf-pm-velocity` bench held `bench.lock` for part of it. The arms alternated and each run took
  the lock, so no two 1M benches overlapped, but the runs are not independent samples of a quiet
  machine; only medians are claimed and the spread is the reason the 0.51 ms is not called a win.
- `Caveat:` the bench's 7 ticks run from the start layout, which is denser than a settled one, so
  it has more hits per candidate than a converged tick. The instruction saving here is per hit
  and per candidate alike, so a settled tick would show a smaller share of the same total, not a
  different sign.
- It does not touch the filter. That is the pass's larger half and step 4 was the only attempt on
  it; the asm finding above says the obvious SoA pairing does not pay on SSE2 without
  `target-cpu`, and widening that is a target-feature decision outside this job.
- It does not add `target-cpu` or `target-feature`. `mul_add` is still never written and no
  contraction is possible at this target, which is what keeps native and wasm32 bit-identical.
- No browser number: the wasm tick bench (`wasm-threads.mjs tick`) was not run on this tree.
