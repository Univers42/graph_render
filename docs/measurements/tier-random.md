# `layout.random` — the measured "no tier"

`docs/measurements/tiers-audit.md` row 5 called `layout.random` "sequential by nature" and
shipped `none` as a **judgement**. This file replaces that judgement with a measurement. The
answer did not change; what backs it did.

**The deliverable is this document, not code.** No `run_with`, no `StepRange`, no threaded
hashgate arm, no new knob. The layout is one sequential PRNG stream: node *i*'s pair is draws
`2i` and `2i+1`, and the state at draw `2i` depends on every draw before it. There is no edge
work, no reduction, and no second O(n) term to overlap the stream against. The layout *is* the
stream.

## What the plain bench path measures, and what it does not

```sh
scripts/orch/gr cargo run --release -p graph-cli -- bench \
  --layout layout.random --n 220,10000,100000 --repeat 5
```

| n | one run (ms) | five invocations (ms) |
|---:|---:|---|
| 220 | 0.00 | 0.00, 0.00, 0.00, 0.00, 0.00 |
| 10000 | 0.06 | 0.06, 0.06, 0.06, 0.06, 0.06 |
| 100000 | 0.72 | 0.72, 0.73, 0.74, 0.72, 0.71 |

**`--repeat` is ignored on this path.** `bench.rs:205-207` opens one `Instant` pair per
(size, layout) and prints one run; `row()` never reads `plan.repeat`. Only the campaign path
(`--out` / `--crossover`, dispatched at `bench.rs:150-151`) takes a median, in
`campaign.rs:183-192`. The five invocations above are five *processes*, not five repeats, so
the spread column is reproducibility of the number the tool prints, not a distribution of the
measurement.

`n=100000` is under this layout's own ceiling — `CLOSED_FORM_CEILING = 1_000_000`
(`registry/closed_form.rs:16`, applied at `:32`) — so nothing is refused. For scale, the
campaign path at the ceiling itself: 3.24 ms median of 9 at n=1000000, with 985 ms of
`index_model` in front of it. The layout is 0.3% of the work of indexing the graph it draws.

## The same cells as medians

The campaign path resolves `--layout` for real (`campaign.rs:142` → `super::resolve`), so no
code change was needed to get medians for this layout — only the `--tiers` sweep is
Barnes-Hut-hardcoded (`bench/tiers/sweep.rs:113-119`, dispatched at `bench.rs:146-148`).

```sh
scripts/orch/gr cargo run --release -p graph-cli -- bench \
  --layout layout.random --n 220,10000,100000 --repeat 9 --out /tmp/tier-random.md
```

| n | build ms (median of 9) | run ms (median of 9) | five invocations of the median |
|---:|---:|---:|---|
| 220 | 0.07 | 0.001 | 0.001, 0.001, 0.001, 0.001, 0.001 |
| 10000 | 3.61 | 0.026 | 0.026, 0.026, 0.026, 0.026, 0.026 |
| 100000 | 56.11 | 0.256 | 0.256, 0.257, 0.257, 0.257, 0.261 |

`--repeat 31` at n=100000 gives 0.256, 0.257, 0.261 — the median is not a function of the
repeat count here, which is what a 2.6 ns-per-node body with nothing to warm up looks like.
The plain path's 0.72 ms is 2.8× the campaign's 0.256 ms median at the same n, and that gap is
**not run-to-run noise**: every one of the five plain-path invocations landed in 0.71..0.74 and
every one of the five campaign medians in 0.256..0.261, so it is a systematic property of
timing the *first* call in a process (`bench.rs:205-207` times one call; `campaign.rs:187` takes
a median over `repeat`). **What causes it was not measured** — first-touch page faults on the
freshly mapped `Vec<f64>` pages and allocator arena growth are the obvious candidates for a
body this short, but this file does not claim that. Quote the campaign number for a comparison
and the plain number for "what the tool prints"; they are two different measurements.

Host: nproc 20 · load average 3.36 3.34 2.33 at the start, 3.20 3.35 2.45 at the end · release
build · one machine class, shared.

## Why the ceiling is below 2×, stated once and then left alone

A chunk-seeding scheme is bit-identical and still does not pay: advance the state serially
once per chunk to reach its seed, then generate each chunk from its own `Mulberry32`. The
serial prologue is 2n draws — the *whole* stream — and each worker then re-draws its own
chunk. So the serial fraction is 1/2 by construction, Amdahl's ceiling is exactly 2.0× at
infinite workers, and every finite width is below it. There is no third O(n) term to
overlap: `run` (`random.rs:25-31`) draws, unzips, and calls `point_geometry`. The layout is
the stream.

And the sizes do not rescue it. `docs/measurements/phase11-threads.md` brackets the crossover
between n=220 and n=10000 (0.52×..0.75× at n=220, 1.34× at 2 workers and 2.97× at 7 at
n=10000). At n=220 this layout's entire body is 0.001 ms — a millisecond of arithmetic
divided by seven threads, which cannot pay for seven threads. The prologue is the dominant
term *by construction*, so the ratio is bounded below 1 at every width the gate runs.

## The gate arms: none, by design

`hashgate --seeds 8 --tiers all` adds `native scalar` plus one `native threads N` arm per
`WORKER_COUNTS = [1, 2, 3, 4, 7]` (`hashgate/tier.rs:45`, arms at `:82-91`) = 10 arms,
compared line-for-line against arm 0 (`compare.rs:54-56`) and printed as `{ways}-way equal`
(`hashgate.rs:227-231`).

| run | result | `layout.random` |
|---|---|---|
| `hashgate --seeds 8` | exit 0, `4-way equal on 8/8 seeds` | in the tally |
| `hashgate --seeds 8 --tiers all` | exit 0, `10-way equal on 8/8 seeds` | **vacuously** |
| `GM_MUTATE_NODE_COUNT=7 hashgate --seeds 8` | exit 1, `FAIL: 8 of 8 seeds diverge` | `4-way equal on 0/8 seeds` |

Those 10 arms exist for Barnes-Hut. `hashgate.rs:169` is
`let bytes = if id == BarnesHut::ID { ..run_under(..&Threads..) } else { bytes }` — every
other stage reuses the scalar run's bytes (`:181-183`). So the 10-way row for this layout is
equal **by construction, because no threaded arm recomputed it**, and that is the correct
state for it. **The base 4 arms are the whole claim for this stage**: one program, twice per
target, identical bytes. If the 10-way row ever goes red for `layout.random`, something added
the id to `hashgate.rs:169`, and the red is correct behaviour.

## "No tier" is not "no coverage"

`layout.random` is in `LAYOUTS` (`registry.rs:167`), so the gate hashes it every run — at base
and under `--tiers all`, in the tally both times, 8/8 seeds. The control that proves it is
**`GM_MUTATE_NODE_COUNT`** (`hashgate/knob.rs:46-47`, variant `NodeCount`, in `Knob::ALL` at
`:109`): it adds nodes to the gate's one shared model, so the stream consumes more draws and
every coordinate moves. Measured above: exit 1, `layout.random` at 0/8.

Two honest caveats on that control:

- It is **native arm only** (`knob.rs:46`), so the two wasm32 arms stay equal to each other
  and only the native ones diverge. That is the expected shape for an "is this stage hashed"
  control, not a partial failure.
- It moves **every** stage at once, because it perturbs the shared model. It answers "is the
  stage hashed", not "which stage diverged" — that is what the per-stage knobs exist for
  (`knob.rs:18-37`), and this layout is the one job in the tier queue that needs none.
  `GM_MUTATE_SPLIT_SUM` (`knob.rs:85-86`, `100`) answers a different question — "did the
  threaded arm recompute it" — and is inert for this layout, which is the point.

**No new knob.** A layout with no tier has nothing to recompute, so it has nothing for a
threaded-arm control to bite on.

## Ponytail (the "no")

A negative result is still a sample of **one machine class, one build and one model seed** on
a shared host. Failing input: a host whose core count, memory bandwidth or load differs from
this one, or a `--layout` that the `--tiers` sweep silently ignores (it does — `bench.rs:146-148`;
the medians above came from the campaign path for that reason). Direction: over-reporting the
cost of a tier, which costs an afternoon. Escape hatch: `exec::select` / `resolve`
(`exec/select.rs:139`, `:157`) bypass every threshold — and can never change a byte, because
every tier `select` can return is hash-equal to scalar per stage.
