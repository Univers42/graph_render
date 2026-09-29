# Phase 11 progress — compute tiers

Branch `p11`, from `develop` (p3…p9 merged; p10 in progress on its own branch).
Written 2026-09-29. This is a progress report, not the phase's §12 report: it says what
landed in this slice, what is deliberately not there yet, and what the next slices are.

## What landed

### The range-kernel contract, as a trait and a runner

`crates/graph-core/src/exec/partition.rs` — `StepRange` (the kernel), `Runner` (who runs
it), `Serial` (the reference), and `partition(n, workers)`.

- `StepRange` is generic over its output (`type Out`), not fixed to `f32`: the bundling
  points are `f32` and the force simulation is `f64`, and one contract with two widths beats
  the same rule written twice.
- `StepRange: Sync` is a **supertrait**, so "the workers share the start-of-step state" is a
  type error to violate rather than a convention.
- `out` is the **range's own sub-column**: element `i` of `range` is written at
  `out[i - range.start]`. An executor can therefore hand each worker a fresh buffer and
  assemble the column after the join, with no `unsafe` and no shared borrow.
- `Runner::run` clamps `workers` below 1 up to **one**, not zero. `partition(n, 0)` is
  correctly "no slices", but that is the wrong answer to "compute the outputs": a host
  reporting no threads gets the serial bytes, not a column of zeros. (A test caught this
  while the slice was being written — the first version returned zeros.)
- A **short arm** is still refused; only the "exactly 4 arms" rule became "at least 2".

### The native executor

`crates/graph-cli/src/exec_native.rs` — `Threads`, a `std::thread::scope` runner. Unit
struct: the worker count arrives with the call, so there is one place for the number to live.
Below two workers it takes the serial path, because the single-threaded path is what every
byte-identity claim is stated against.

### A real kernel: Barnes-Hut's many-body pass

`crates/graph-core/src/layout/force/barnes_hut/step.rs` — `Pass`, the first `StepRange` in
the motor, over the per-node tree walk.

- The **whole layout** is not partitionable and the code says so: `TICKS` ticks, and tick
  `t + 1` reads what tick `t` wrote. The tick loop stays one thread's straight-line code and
  the barrier lands at the end of the pass, once per tick. What is partitionable is the
  per-node gather inside a pass.
- The tree build and the bottom-up aggregate stay single-threaded and **ahead of the ranges**,
  as the phase prompt's step 2 prescribes. Both are in `charge::prepare`, shared by the
  serial and threaded paths so there is no second copy to drift.
- `Quadtree::visit_in(&self, stack, prune)` is the enabling change: `visit` needed `&mut`
  only for its reused stack, so without a caller-owned stack the walk was a `&mut` borrow of
  shared state and no two workers could take it at once. `visit` is now `visit_in` plus its
  own buffer.
- `BarnesHut::run_with(topology, params, runner, workers)` is the runner-parameterised
  stage; `Stage::run` is that call with `&Serial, 1`, so **the stage and one worker are the
  same program by construction** — a test asserts it rather than assuming it.

### The N-way hash gate

`hashgate --tiers base|all` (`base` is the old four arms; `all` is ten).

| arm | what it is |
|---|---|
| `native run 1`, `native run 2` | the compiled-in path, twice, in separate processes |
| `wasm32 run 1`, `wasm32 run 2` | the real ABI under Node, twice |
| `native scalar` | the in-process scalar arm, built by the same code as `hashgate-arm` |
| `native threads {1,2,3,4,7}` | the Barnes-Hut stage over that many `std::thread`s |

- **The odd counts are pinned in code** (`WORKER_COUNTS = [1, 2, 3, 4, 7]`), not left to a
  benchmark script. 3 and 7 leave uneven slices; an even split hides a range-boundary bug
  behind symmetry. A test asserts 8 is *absent* — the table is the gate's, not the host's.
- `compare::diverged` takes any number of arms (≥ 2). The report prints "N-way equal", and
  the record (`target/gates/hashgate.json`) carries `arms` and `arm_names` — the per-stage
  counts mean nothing without the width they were measured at.
- A test walks **every one of the ten arms**, breaks it in turn, and asserts the divergence
  is caught: a rule that compared only the first four would pass a broken fifth arm.

### The negative control, `GM_MUTATE_SPLIT_SUM`

The phase prompt's "a negative control that splits one node's force sum across two threads
must go red". It is a **compiled-in parameter** (`BarnesHut::run_under(..., split_sum)`),
not a `cfg` or an environment read inside graph-core, so:

- a unit test calls it directly and asserts the layout moves;
- the gate row reaches it through the same argument.

Verified in this slice, not just asserted:

```
hashgate --seeds 2 --tiers all                     → exit 0, 10-way equal on 2/2
GM_MUTATE_SPLIT_SUM=1 hashgate --seeds 2 --tiers all → exit 1
    layout.force.barnes_hut: 10-way equal on 0/2 seeds
    (every other stage: 2/2)
```

The control moves the force stage **and only the force stage**, which is the property a
control needs: a knob that moved every stage would back none of them.

### Tier selection

`crates/graph-core/src/exec/select.rs` — `select(n, m, caps, thresholds) -> Tier` as a pure
function, plus `resolve` for an explicit request. `auto` never selects a GPU; an explicit
request for an unavailable tier is **refused**, not downgraded. `Thresholds::MEASURED` is
inert (see `docs/decisions/tier-thresholds.md`).

## Two bugs the tests caught while writing this

Worth recording, because both were invisible to a reading of the code and both were found
by a test that was written to check something else:

1. **`Threads::run` doubled its output.** It resized the caller's buffer to the kernel's
   length and *then* appended the workers' parts, so every node appeared twice. The
   equality test caught it as a length mismatch, not as a value difference.
2. **The threaded arm printed its lines seed-major** while the comparison reads stage-major
   (`line i` is stage `i / seeds`, seed `i % seeds`). The gate refused it as malformed at
   line 1 — "not comparable", exit 2, rather than the "equal" the arm claimed. The test
   `the_threaded_arm_prints_its_stages_in_the_same_order_as_the_scalar_one` now holds the
   two arm builders to the same shape.

## What is not here, and why

| slice | state |
|---|---|
| SIMD (tier 1b) | **not started.** Needs a measurement first (per the ADR's order) and a second wasm artifact. The kernel it would vectorise is now a `StepRange`, which is the precondition; nothing is vectorised yet. |
| Browser workers (tier 2, SDK) | **blocked on stop-and-ask 1** (SAB + COOP/COEP vs independent instances). Not attempted without that answer. |
| `harness/tier-equality.mjs` | not written. It needs the two wasm artifacts, which need the SIMD slice. |
| GPU (tier 3) | **blocked on stop-and-ask 2**, and the numbers that would justify asking do not exist yet. |
| `collide` and `link` as range kernels | not started. `collide` is the same shape as `charge`; `link` writes *both* endpoints of an edge, so it partitions by edge and sums into two nodes, and needs its own argument before it is a kernel. |
| `post.bundle.fdeb` as a range kernel | not started. Its own doc already calls the iteration gather-form and partitionable by point. |
| Capability ledger rows | not added. `exec.simd` / `exec.threads.native` should be added when the speedup measurement lands, not before — a gated row needs both the equality proof and the measured speedup, and only the first exists. |
| Benchmarks | **not run.** `docs/measurements/phase11-{simd,threads,gpu}.md` do not exist. The executor is proven equal and unmeasured for speed; `tier-thresholds.md` is inert because of exactly this. |

## Next slices, in order

1. **Measure the threaded tier** (`docs/measurements/phase11-threads.md`): Barnes-Hut's
   many-body pass, scalar vs 1/2/3/4/7 workers, at N = 220, 10 000, 100 000, 1 000 000, in
   release, `--repeat 5`, medians. **Report the sizes where threads lose** — at N = 220 they
   almost certainly do, and that is the reason the threshold exists. If the best speedup is
   under 3 % at every N, that is stop-and-ask 4: report it and do not ship the tier.
2. **Then, and only then**, write the `threads_nodes` and `threads_max` rows into
   `tier-thresholds.md` from those numbers, and add the `exec.threads.native` ledger row.
3. **`collide` as a range kernel**, then `link` (with its two-endpoint argument written
   down), so the whole tick is partitionable rather than one pass of it.
4. **SIMD**: autovectorisable SoA first, in the same `step_range` bodies, checked with the
   ten-arm gate. `select.rs` already has the SIMD tier and the SDK already has the `exec`
   option's vocabulary, so the selection side is done — only the kernels and the second
   wasm artifact are missing.
5. **Browser workers**, after stop-and-ask 1.
6. **GPU**, only after stop-and-ask 2 and only with the crossover numbers in hand.

## Files touched, against the phase's authorization envelope

Within the envelope: `crates/graph-core/src/exec/{mod,partition,select}.rs` (new),
`crates/graph-cli/src/exec_native.rs` (new), `crates/graph-core/src/lib.rs`,
`crates/graph-core/src/layout/force/*.rs` (the SIMD-friendly-loop allowance; here the
range-kernel contract lands on the same loops), `crates/graph-cli/src/{main,hashgate}.rs`.

**Child modules, a deviation in the same shape as Phase 6's** (which split
`barnes_hut.rs` into `barnes_hut/*` and recorded it): the house's 300-line cap forces
`exec/{partition,select}/tests.rs`, `exec_native/tests.rs`, `hashgate/tier.rs` +
`hashgate/tier/tests.rs`, and `layout/force/barnes_hut/{step,charge/tests}.rs`. None is a
new *area* — every one is the tests or the implementation of a file the prompt names.

**`hashgate/compare.rs`, `hashgate/{knob,report}.rs` and `hashgate/tests/*`** are children
of `hashgate.rs`, which the prompt lists. The prompt does not name them individually.

Not created, though the envelope allows them: `crates/graph-sdk-js/src/exec/*` (browser
workers, stop-and-ask 1), `exec/gpu/*` (stop-and-ask 2), `harness/tier-equality.mjs` (needs
the two wasm artifacts), `docs/measurements/phase11-*.md` (needs the measurements),
`graph-wasm`'s `+simd128` variant (needs the SIMD slice).

## Gates run in this slice

| check | result |
|---|---|
| `gr cargo fmt --all --check` | 0 |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `gr cargo test --workspace --no-fail-fast` | 0 — 950 passed, 0 failed, 11 ignored |
| `hashgate --seeds 2 --tiers all` (in-process, from this tree) | 0 — 10-way equal on 2/2, all 12 stages |
| `GM_MUTATE_SPLIT_SUM=1 hashgate --seeds 2 --tiers all` | **1** — the new control, force stage only |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 2` | **1** — the pre-existing control still goes red |

**Not run, and therefore not claimed:** the 1000-seed hashgate, the full `gate.sh`, and
`cargo mutants`. Per the working rules those are the orchestrator's, under the host-wide
lock. The 2-seed run above is an exploration, not a gate row.
