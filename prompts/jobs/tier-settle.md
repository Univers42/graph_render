# Job tier-settle (agent build, one layout id)

Why: `prompts/RESUME.md` queues "Every layout gets the Phase 11 thread tier and a bench-driven
optimization pass. Order: easiest first." This is the easiest one that is not already done, and
the best value in the set. `layout.force.yifan_hu` runs the **same three `StepRange` kernels**
`layout.force.barnes_hut` runs, through the same `Sim::tick`, and gets none of the speed-up:
`crates/graph-core/src/layout/force/barnes_hut/settle.rs:29-30` hard-codes

```rust
How { runner: &Serial, workers: 1, deltas: &mut deltas, split: Split::None }
```

The kernels are gather-shaped and byte-identical by construction already
(`crates/graph-core/src/layout/force/barnes_hut/step.rs:11-17`). The difference between a serial
layout and a threaded one is one literal and two parameters. `docs/measurements/tiers-audit.md`
row 1 is the analysis; this is the work.

Facts:

- **The layout**: `crates/graph-core/src/layout/force/yifan_hu.rs:45` `Stage::run` → `:81`
  `multilevel`, which per level calls `settle` at `:84` and `:87`
  (`use super::barnes_hut::{golden_seed, settle};` at `:15`). Coarsening (`force/yifan_hu/coarsen.rs:20`)
  and prolongation (`yifan_hu.rs:94-102`) are **greedy and sequential** — coarsen must finish
  before the quotient exists, and each refinement starts from its parent's positions
  (`yifan_hu.rs:86-87`). They stay serial and must say so in one line.
- **The precedent to copy**: `BarnesHut::run_with(topology, params, runner, workers)`
  (`crates/graph-core/src/layout/force/barnes_hut.rs:148`, runner param at `:151`) delegating to
  `run_under(.., split)` (`:164`), which builds `How` at `:174-180` and ticks at `:180`. `YifanHu`
  gets `run_with` in the same shape; `Stage::run` keeps the `&Serial, 1` call, exactly as
  `barnes_hut.rs:116` does.
- **The runner**: `Threads`, `crates/graph-cli/src/exec_native.rs:34`, `impl Runner` at `:36`. It
  lives in graph-**cli**; graph-core must never name it. Take `runner: &impl crate::exec::Runner`
  from the caller. `settle` gains the same two parameters and threads them into `How`.
- **No new kernel.** `Split::None` at `settle.rs:30` is the only behavioural literal, and turning
  it into a parameter is what makes the existing knob live here (below). `THREADED_PASSES`
  (`barnes_hut.rs:134`) names the three passes this layout already runs.
- **Determinism**: CLAUDE.md "Determinism", `prompt.md` §6. Nothing about this change is allowed
  to alter a byte — the merges (`barnes_hut/charge.rs:49`, `link.rs:76`, `collide.rs:69`) stay
  ascending-index loops and `Sim` stays tick-sequential (`step.rs:5-9`).
- **Sizing**: `registry/force.rs:141-142` — `O(n log n) × (112 + 48 × levels)`. The settle ticks
  dominate, which is why this is the highest-value S row.

Do:

1. `YifanHu::run_with(topology, params, runner, workers)`; `Stage::run` calls it with `&Serial, 1`.
   `settle(graph, params, start, (ticks, alpha))` gains `runner` and `workers` and puts them in
   `How`. `Split` gains a parameter too, defaulting from the caller so `barnes_hut::settle`'s
   existing two call sites still compile unchanged if you prefer that shape — either is fine, pick
   the smaller diff and say which you picked.
2. Say in one line, where the module already discusses levels, that coarsen and prolong are serial
   by nature and why. One line, in the module doc. No new doc page.
3. **Extend the bench tier sweep to route by layout.** `bench.rs:146-148` returns
   `tiers::entry(plan)` before any layout resolution and `bench/tiers/sweep.rs:113-119` always
   calls `BarnesHut::run_under`. Without this the acceptance criterion below **cannot be run at
   all**, so treat it as step 1, not as a nicety. Keep `BarnesHut` the default so every existing
   row in `docs/measurements/phase11-threads.md` still reproduces byte-for-byte, and keep
   `refuse_unproved_widths` (`bench/tiers.rs:219-229`) refusing any `--workers` outside
   `hashgate::WORKER_COUNTS = [1,2,3,4,7]` (`hashgate/tier.rs:45`) — 8 is refused and must stay
   refused.
4. **Add the id to the hashgate's threaded match.** `crates/graph-cli/src/hashgate.rs:169` is
   `let bytes = if id == BarnesHut::ID { ... } else { bytes }`; every other stage reuses the scalar
   run's bytes (`:181-183`). Until `YifanHu::ID` is in that match, every `--tiers all` arm hashes
   the same bytes for this stage and "equal" is vacuous. Add the branch, and pass
   `setting.split_sum` into it the way `:175` does.
5. A test with a negative control: with the id in the match, `GM_MUTATE_SPLIT_SUM=1` must make the
   threaded arms diverge from the scalar one **on the force stage only**, and `--seeds 2` with
   `GM_MUTATE_SPLIT_SUM=collide` must exit 2 "could not run" (`barnes_hut.rs:85-93` sets
   `min_seeds() == 5`). A control that never turns the gate red is not a control.

Hashgate arm that must stay equal: `--tiers all`, which adds `native scalar` plus one
`native threads N` arm per `WORKER_COUNTS` (`hashgate/tier.rs:82-91`) — 10 arms total, compared
line-for-line against arm 0 (`compare.rs:54-56`), printed as `{ways}-way equal`
(`hashgate.rs:227-231`). "4-way" is the width of `native threads 1..4`; `7` is there too and
matters because an even split hides a range-boundary bug (`exec/partition.rs:9-12`).

Knob that must turn it red: **`GM_MUTATE_SPLIT_SUM`** — no new knob. It already exists
(`hashgate/knob.rs:82-100`, declared in `Knob::ALL` at `:116`), already carries a
`barnes_hut::Split` on `Setting` (`:180`, parsed at `:246`), and already makes "a gathered pass's
merge read a neighbouring node's delta — the shape a wrong partition of the outputs would take"
(`knob.rs:85-86`). It reaches a stage **only**
through `BarnesHut::run_under(..., setting.split_sum)` (`hashgate.rs:175`), so for this layout it
becomes live by threading `Split` into `settle` and passing it at the new `hashgate.rs` branch.
Adding a second knob here would be a second answer to the same question.

Bench command (only after step 3; it does not work before):

```sh
scripts/orch/gr cargo run --release -p graph-cli -- bench \
  --layout layout.force.yifan_hu --n 220,10000 \
  --tiers scalar,threads --workers 2,4,7 --repeat 5 \
  --out docs/measurements/tier-settle.md
```

Note the deviations from the template in `prompts/jobs/tiers-audit.md:24`: `--tiers` needs a value;
`--workers 8` is refused by `refuse_unproved_widths` (`bench/tiers.rs:219`); `1` is omitted because
the sweep's scalar arm already is the one-worker run (`bench/tiers.rs:46-50`). Do not "fix" that
refusal — a width no hashgate arm ran cannot become a threshold row.

Gate:

```
hashgate --seeds 8 --tiers all            -> 0, and layout.force.yifan_hu: 10-way equal on 8/8
GM_MUTATE_SPLIT_SUM=1 hashgate --seeds 8 --tiers all -> non-zero, force stage diverges
GM_MUTATE_SPLIT_SUM=collide hashgate --seeds 2 --tiers all -> exit 2, "could not run"
```

Done when: `bench --tiers` times `layout.force.yifan_hu`, the hashgate recomputes it under every
width in `WORKER_COUNTS`, all arms are byte-equal at every width, the split-sum control turns it
red and only on the force stage, the bytes at `workers = 1` are identical to today's
`layout.force.yifan_hu` gate lines, and `docs/measurements/tier-settle.md` carries the measured
table with the `Ponytail:` note `docs/measurements/phase11-threads.md` ends on. UNKNOWN = FAIL: if
the sweep step is not done, nothing here is measurable and the job is not done.

Paths you may touch: `crates/graph-core/src/layout/force/yifan_hu.rs`,
`crates/graph-core/src/layout/force/barnes_hut/settle.rs`,
`crates/graph-cli/src/bench/tiers/`, `crates/graph-cli/src/hashgate.rs`,
`docs/measurements/tier-settle.md`, and the matching tests. Nothing else.
