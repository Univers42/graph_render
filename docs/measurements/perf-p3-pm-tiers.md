# Perf P3-PM — `layout.force.particle_mesh` under the thread tiers

Measured 2026-10-02 on branch `perf-p3-pm-tiers`, host dlesieur42 (**nproc 20**, shared:
load average 11–33 from other jobs, printed next to each run), image `ge-rust`, release
build. The predecessor is `docs/measurements/perf-p2-pm.md`, which measured this layout
single-threaded (361.13 ms a 1M tick).

## What changed

**The mesh tick was already threaded in its gathers; nothing could reach the control.**
`ParticleMeshRun::step_with` built its `How` with `split: Split::None` hard-coded
(`crates/graph-core/src/layout/force/particle_mesh.rs:107`), so the `Split` that the three
passes already read — `how.split.splits(Split::Link)` (`:133`), `Split::Charge`
(`particle_mesh/charge.rs:50`), `Split::Collide` (`particle_mesh/collide.rs:260`) — was
unreachable from any caller. The stage's bytes were fixed at the one schedule.

- **`ParticleMesh::run_under(topology, params, runner, workers, split)`** is the new entry
  point, copied from `BarnesHut::run_under` (`barnes_hut.rs:163`) with its reason
  (`barnes_hut.rs:156-162`) attached: separate rather than a defaulted argument because a
  control a caller can forget to pass is not a control. `run_with` delegates with
  `Split::None`, so `Stage::run` and every existing caller are unchanged.
- **`ParticleMeshRun` gained `step_under(runner, workers, split, ticks)`**; `step_with`
  delegates to it with `Split::None`. I took the `step_under` half of the choice the body
  left open, not the `step_with`-takes-the-split half: `step_with` has two callers outside
  the paths this job may edit (`crates/graph-cli/src/bench/tick.rs:131`, and the mesh's own
  tests), so widening it would have been a deviation.
- **Bench routing.** `ParticleMesh::ID` is in `ROUTES` after `YifanHu::ID`, with two
  `run_once` arms, a `particle_mesh` helper passing `control.split`, and a `stage_sentence`
  that names the serial deposit and the two FFTs.
- **Hash-gate routing.** `ParticleMesh::ID` is in `THREADED_STAGES` after `YifanHu::ID`, and
  `geometry` routes it under `setting.split_sum` beside the two force layouts.

**No kernel changed.** The deposit, the two FFTs, the collide sort, `center` and
`integrate` are the same code they were, in the same tick order.

**The serial bytes did not move.** `hashgate --seeds 8` (the 4-way base arms) exits 0 with
the digests it had before, and `cargo test --workspace` is green including the goldens and
the registry digest for `layout.force.particle_mesh`. The 65 golden digests are untouched
because `run_with` passes `Split::None`, which is what `step_with` did before.

## Gates

| gate | exit | what it says |
|---|---:|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | no new `#[allow]`; `run_under` has five parameters as `BarnesHut::run_under` does, and that one carries no suppression |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | every suite green |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all` | 0 | `layout.force.particle_mesh: 10-way equal on 8/8 seeds`, `PASS` |
| `scripts/orch/gr -e GM_MUTATE_SPLIT_SUM=1 cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all` | 1 | `FAIL: 8 of 8 seeds diverge` |

The last row is the negative control, and the stage it proves is now compared rather than
self-compared. With `GM_MUTATE_SPLIT_SUM=1` the three force stages all report
`10-way equal on 0/8 seeds`:

```
layout.force.barnes_hut:      10-way equal on 0/8 seeds
layout.force.yifan_hu:        10-way equal on 0/8 seeds
layout.force.particle_mesh:   10-way equal on 0/8 seeds
```

Before this job `layout.force.particle_mesh` was absent from `THREADED_STAGES`, so its
"10-way equal" was the **vacuous** case `tiered.rs:4-7` names: the arm hashed the scalar
run's bytes and compared them with themselves. It now names the stage in the divergence
report, which is only possible because the control reaches the passes.

Both tests that carry the claim were RED first and are green now
(`crates/graph-cli/src/hashgate/tests/arm_lines.rs`,
`crates/graph-cli/src/bench/tiers/tests/layout.rs`). The bench-side test was observed red
with `no tier route (Scalar)`, the message `run_once` gives an unrouted id.

## The tiers

`scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
layout.force.particle_mesh --tiers scalar,threads --n 100000,1000000 --past-ceiling`
(the flags are the ones `bench --help` documents; `--repeat` is left at its default 5, so
each cell is a median of 5, not a single timing). Host load `19.70 17.03 13.37` at the
start, `20.86 18.77 16.43` at the end.

| n | tier | workers | median ms | speed-up | equal to scalar |
|---:|---|---:|---:|---:|---|
| 100 000 | scalar | 1 | 3 659.15 | — | true |
| 100 000 | threads | 2 | 3 739.46 | 0.98× | true |
| 100 000 | threads | 4 | 3 456.80 | 1.06× | true |
| 100 000 | threads | 7 | 2 554.29 | 1.43× | true |
| 1 000 000 | scalar | 1 | 71 141.11 | — | true |
| 1 000 000 | threads | 2 | 53 927.76 | 1.32× | true |
| 1 000 000 | threads | 4 | 36 994.69 | 1.92× | true |
| 1 000 000 | threads | 7 | 31 095.65 | 2.29× | true |

**Every arm is byte-equal to its own layout's scalar arm at the same size**, which is the
claim the `equal` column makes: a tier that is faster and different is a faster wrong
answer.

**The host is too busy to read a trend past four workers, and the three runs disagree.**
Run twice more at `--n 100000,1000000`, same command:

| run | load start → end | 1M scalar | 1M t2 | 1M t4 | 1M t7 | 100k t7 |
|---|---|---:|---:|---:|---:|---:|
| A | 11.46 → 22.27 | 67 210.84 | 49 512.27 | 35 760.85 | 33 638.80 | 2 242.47 |
| **B (above)** | 19.70 → 20.86 | 71 141.11 | 53 927.76 | 36 994.69 | 31 095.65 | 2 554.29 |
| C | 19.47 → 33.06 | 81 699.49 | 71 280.92 | 57 410.41 | 73 337.54 | 2 611.54 |

Run C's load climbed by 14 across the run and its 1M `threads 7` cell came out **slower**
than its `threads 4` cell, on five timings spread 56.7 s–101.9 s. Run A's 1M `threads 2` is
1.36× and run B's is 1.32×, which agree; run C's 1.15× does not. The 1M rows are one cell
each on a host running other jobs at load 19–33 on 20 cores, so **the 1M speed-up is
1.3–1.4× at two workers and 1.4–1.9× at four, with no trustworthy seven-worker number**.
Nothing here justifies a threshold row.

## What it does not do

- **The deposit and the two FFTs stay serial.** The mesh's `P × P` convolution is a
  butterfly network over the whole field, not a per-node gather, so there is nothing for a
  runner to partition; `charge::apply` reads `how.split.splits(Split::Charge)` for its
  *gather* (the per-node CIC deposit and read-back), and the `solve` it wraps is the serial
  part inside that pass. The stage sentence
  (`crates/graph-cli/src/bench/tiers/markdown.rs`) says so, so a reader of the table cannot
  attribute the whole tick's speed-up to the runner. **The bench prints no per-pass
  timings** — `bench --tiers` measures the whole stage by design
  (`crates/graph-cli/src/bench/tiers.rs:5-9`) — so the serial share of a tick is **UNKNOWN
  from this run**, not estimated here.
- **It does not thread `center` or `integrate`.** Both are whole-array passes over the node
  set, called between the gathered ones, exactly as in Barnes-Hut's tick.
- **It does not change the mesh cap.** `P = clamp(next_pow2(ceil(√n)), 128, 1024)`: past
  about 1M nodes a cell holds more than one node on average and the short-range charge is
  the smoothed part of the force (carried over from `perf-p2-pm.md`).
- **It does not move the goldens or the registry hash.** `layout.force.particle_mesh`'s own
  digest and the 65 goldens are as they were; `Split::None` is what `Stage::run` passes, so
  only the tiers move.
- **It is not a live session.** `ParticleMeshRun` still runs the frozen stage; the studio's
  live loop stays on Barnes-Hut.
- **The speed-ups are one machine class, one build, one model seed, on a loaded shared
  host.** A threshold read off this table picks a slower tier than the best available, or a
  faster one than the host can deliver; both cost time and never bytes, because every tier
  in the table is hash-equal to scalar per stage.

## Reproduce

```sh
scripts/orch/gr cargo test -p graph-cli --bins -- the_mesh the_split_control_moves the_recompute_list
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all
scripts/orch/gr -e GM_MUTATE_SPLIT_SUM=1 cargo run -q -p graph-cli -- hashgate --seeds 8 --tiers all
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.particle_mesh \
  --tiers scalar,threads --n 100000,1000000 --past-ceiling
```