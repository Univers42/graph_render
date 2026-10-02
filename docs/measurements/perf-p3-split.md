# Perf P3, rung 1 — `Threads` writes into the caller's column

Measured 2026-10-02 on branch `perf-p3-split` at `1d9a3d8`, host dlesieur42 (20 cores), image
`ge-rust`, release profile. Four sibling jobs shared the host throughout; every row below
prints the `/proc/loadavg` reading taken immediately before it, which is the only reason to
believe any of the small differences here. The stage, the seed, the pass list and the tick
count are the `bench --tiers` defaults: `layout.force.barnes_hut`, seed 0, passes
link/charge/collide, `TICKS` = 112 (`crates/graph-cli/src/bench/tiers/markdown.rs:122`).

## Problem

`Threads::run` used to give every worker **its own** `Vec`, then assemble the column after the
join:

```rust
let mut span = vec![O::Out::default(); range.len()];   // one allocation per worker
kernel.step_range(range, &mut span);
...
out.extend_from_slice(&parts.concat());                 // one more, then a full copy
```

The Barnes-Hut passes' `Out` is `(f64, f64)` — 16 bytes
(`crates/graph-core/src/layout/force/barnes_hut/step.rs:68,99,132`) — so one column at 1 000 000
nodes is 16 MiB. Per `run` call that was `workers` allocations of the whole column between them
plus `concat`'s 16 MiB and `extend_from_slice`'s second 16 MiB: **two full copies and two extra
allocations of the column per pass**, three passes per tick.

## Change

One `out.clear()` + `out.resize(kernel.len() as usize, O::Out::default())`, then inside
`std::thread::scope` each range peels its span off the tail with `split_at_mut(range.len())` and
is spawned against it (`crates/graph-cli/src/exec_native.rs:48-63`). No `unsafe`, no per-worker
`Vec`, no `concat`, no `extend_from_slice`. The scope's own join is still the one barrier per
step — dropping the explicit `handles`/`join()` loop does not add one, because `thread::scope`
joins every unjoined thread when its closure returns.

The legality of the disjoint borrow is `partition`'s contract and nothing else: its ranges are
ascending, contiguous and cover `0..n` exactly
(`crates/graph-core/src/exec/partition.rs:117-133`), so peeling `range.len()` off the tail each
iteration hands worker `i` a span that begins exactly where `range.start` is and leaves the
final peel empty. A `debug_assert!(rest.is_empty())` at the end of the scope states that as a
check rather than a comment.

Nothing about the arithmetic moved: every worker still receives the same `(range, span)` it
received before, `step_range` still sees `out[i - range.start]`, and no sum is split or
reordered. The bench's own `equal to scalar: true` column is the byte-identity witness, and it
was true in every row of both runs below.

## Before / after

Same command for both, `workers` 2/4/7, median of the bench's repeats:

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench \
  --layout layout.force.barnes_hut --n 100000 --tiers scalar,threads --workers 2,4,7
```

### n = 100 000, 112 ticks

| arm | before (ms) | after (ms) | change | equal to scalar |
|---|---:|---:|---:|:---:|
| scalar | 14 278.31 | 13 792.04 | −3.4 % | true |
| threads 2 | 11 429.63 | 11 066.86 | −3.2 % | true |
| threads 4 | 8 247.47 | 7 798.19 | −5.4 % | true |
| threads 7 | 6 546.57 | 5 650.44 | −13.7 % | true |

Load average 1 min, before run: `9.42 8.71 9.76`; after run: `9.28 10.07 11.16`. Whole-run wall
clock 3:26.07 → 3:13.78.

**The scalar row is the noise floor, and it is not free.** The scalar arm does not execute any of
the changed code — it takes the `workers < 2` branch — yet it moved 3.4 % between the two runs on
a host running four other jobs. Read the threaded rows against that: threads 2 (−3.2 %) is inside
the noise, threads 4 (−5.4 %) is barely outside it, **threads 7 (−13.7 %) is the only row that
clears the noise by a factor of four**. That is also the row where the removed work is largest,
since `concat` and `extend_from_slice` both cost the whole column per pass regardless of worker
count but the seven per-worker allocations grow with it — and the more cores the more the copies
were competing with the walks for bandwidth.

### n = 1 000 000, 112 ticks, `--past-ceiling`

<!--P3-1M-TABLE-->

Both rows above and below come from one run each, at the ceiling and one step past it, so they
are the numbers this tree printed on this host on this day. They are **not** a threshold-grade
sweep: `docs/decisions/tier-thresholds.md:46-58` wants the losing sizes in the same table, and
neither run has them.

## Spawn overhead

An empty kernel — `step_range` writes nothing — over the bench's 112 steps at 7 workers, in the
executor as it now stands:

```sh
scripts/orch/gr cargo test --release -p graph-cli exec_native -- --nocapture --test-threads=1 spawn_cost
```

```
spawn: 112 empty steps on 7 workers = 16.053 ms (143.3 us/step)
```

Load average at the run: `8.01 9.95 11.14`. The kernel's `Out` is the pass's own `(f64, f64)`
over 100 000 nodes, so the 16 MiB `resize` memset and the seven spawns are both in that number,
which is what the real tier pays too.

**Share of one 100 000-node tick.** The Barnes-Hut tick calls the runner three times — charge,
collide, link (`crates/graph-core/src/layout/force/barnes_hut/{charge,collide,link}.rs:45,45,68`)
— so a tick pays 3 × 143.3 µs = **0.43 ms** of spawn-and-resize. One scalar tick at 100 000
nodes in this tree's own before-run is 14 278.31 / 112 = **127.5 ms**, so the share is
**0.43 / 127.5 = 0.34 %**.

That is far below the 3 % at which `prompts/perf-plan.md:89` makes a persistent pool the next
rung, so no pool is proposed here. The 0.34 % is also the ceiling on what the spawn itself can
ever be worth: even a free pool buys three tenths of a percent of a 100k tick, and less than that
as `n` grows and the walks get longer. **The copies were worth removing; the spawns are not worth
a pool.**

## Reproduce

```sh
scripts/orch/gr cargo test -p graph-cli exec_native                      # 9 tests
scripts/orch/gr cargo clippy -p graph-cli --all-targets -- -D warnings
scripts/orch/gr cargo fmt --all --check
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.barnes_hut \
  --n 100000 --tiers scalar,threads --workers 2,4,7
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.barnes_hut \
  --n 1000000 --past-ceiling --tiers scalar,threads --workers 2,4,7     # ~60 min
scripts/orch/gr cargo test --release -p graph-cli exec_native -- --nocapture --test-threads=1 spawn_cost
cat /proc/loadavg                                                     # next to every run
```

The "before" arm is this branch's parent commit: `git stash`-free, it is the same file with the
per-worker `Vec` and the `concat`, and both runs were taken on the same host within two hours of
each other rather than from `docs/measurements/phase11-threads.md`, whose numbers come from a
different build on a different day.

## What it does not do

- **No persistent pool.** The spawn is 0.34 % of a 100k tick; the remaining rungs of
  `prompts/perf-plan.md` P3 (parallel tree build, `select.rs` in production, the browser models)
  are untouched and unmeasured here.
- **No change to `partition`.** `partition(n, workers)` still returns a `Vec<Range<u32>>`, so one
  small allocation survives per `run` call (`workers × 8` bytes — 56 bytes at 7 workers, against
  the 16 MiB column). `graph_core::exec::ranges` is the allocation-free iterator and exists for
  exactly this, but it is not re-exported from `crates/graph-core/src/exec/mod.rs:24` and
  graph-core is out of this job's paths. Handed on as a one-line export plus `ranges(n, workers)`
  at `partition.rs:123`.
- **No change to the worker count, the barrier count, or the bytes.** One `thread::scope` per
  `run` call, one implicit join inside it — the same synchronisation as before, minus the handle
  `Vec`.
- **No claim about the serial path.** The scalar arm's 3.4 % move is host noise, not an effect of
  this change; the scalar branch of `run` is the same three lines it was.
- **No threshold promotion.** One host, one run per row, load average printed but uncontrolled.
  `Thresholds::MEASURED` still promotes nothing, and `docs/measurements/phase11-threads.md`,
  `docs/measurements/tiers-audit.md:53` and the `<!-- MEASURED-TABLE -->` block in
  `docs/decisions/tier-thresholds.md:39-44` all still carry the phase-11 numbers, which this
  change does not invalidate but does supersede at 100k and 1M.

## Caveat

Every wall-clock row here is one run on a shared host whose load average never dropped below 7.8
on 20 cores. The two large effects — the 13.7 % at 7 workers/100k and the 1M table — are outside
that noise; the 3.2 % and 5.4 % rows are not, and the scalar row is quoted precisely so the
reader can see how much of the change is visible above it. A clean-host interleaved A/B of two
binaries would separate the 4-worker row properly; that is the next measurement, not a claim of
this one.