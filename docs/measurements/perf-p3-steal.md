# Perf P3 steal: the pass stops waiting for helpers that never took it (parked, not landed)

Parked. The code is kept at tag `archive/perf-p3-steal` (`59869720`); the branch is deleted.

Measured 2026-10-03 on branch `perf-p3-steal` (from develop `7c7d656`, which carries the pool's chunk
claiming). Host: dlesieur42, i5-13600KF (6 P-cores with HT + 8 E-cores). Node from `scripts/orch/gr`.

Verdict: **parked.** The change is correct and its gates are green, but the problem it was built
for is no longer there, and the quietest measurement points the wrong way. It stays on its branch.

## Why it was tried

Before chunk claiming, the coordinator spent 32% of a 1M tick blocked on its helpers
(`perf-p3-scale.md`). The pool also waited for every addressed helper to *wake*, even one the OS
had not scheduled yet, so one descheduled helper stalled every pass.

## Design

| Piece | Where | What changed |
|---|---|---|
| opt-out | `graph-wasm/src/pool.rs` `State.running`, `Running` | a helper counts itself into a pass under the lock that shows it the job, and out on drop (return or unwind). The coordinator clears the job once its own claim loop ends and waits only while `running > 0`; a helper that wakes later finds no job and skips the pass |
| panic | `Running::drop`, `Joining::drop` | natively, a helper that unwinds counts itself out and sets `failed`; the coordinator panics at the end of the pass instead of hanging |
| tests | `pool/tests.rs`, `pool/harness.rs` | every element written once at length 1009 (workers 2, 3, 7); a registered helper that never serves does not hold the pass; the pass waits for a helper still inside it; a helper panic fails the pass |

## Gates

| Check | Result |
|---|---|
| `cargo fmt --check`, `clippy -D warnings`, `cargo test -p graph-wasm` | 0, 0, pass (151 tests) |
| mutation: `Joining::drop` without its wait loop | the slow-helper test dies with SIGSEGV (a use-after-free), as it should; restored |
| mutation: the old pool with the phantom-helper test | the test fails after its 20 s timeout, as it should; restored |
| `harness/wasm-threads.mjs hash`, threads vs serial | 0 differing; `--break` 54 differing |
| `harness/wasm-threads.mjs session` | 0 differing; `--break` 36 differing |
| `hashgate --seeds 8` | PASS, 4-way equal on 8/8 seeds |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | FAIL, 8 of 8 seeds diverge (exit 1, as expected) |

## Profile: the coordinator no longer waits

`node --cpu-prof` over `tick --n 1000000 --workers W --ticks 5`, threads builds of `7c7d656` (`base`)
and of this branch (`steal`), run alternated at load 22–24. Self time of `Condvar::wait` and
`Condvar::notify_all`, in ms over the whole run (graph build included):

| arm | workers | coordinator wait | coordinator total | coordinator `notify_all` |
|---|---:|---:|---:|---:|
| base | 8 | 61 | 5667 | 4 |
| steal | 8 | 28 | 4193 | 8 |
| base | 16 | 80 | 6068 | 13 |
| steal | 16 | 126 | 7195 | 231 |

With chunk claiming in place, the coordinator waits on its helpers for about 1–2% of its time in
both arms. There is no straggler wait left for this change to remove. At 16 workers the steal arm
spent 231 ms in `notify_all` against 13 ms. A likely mechanism: the coordinator no longer waits
for late helpers to park, so the next broadcast meets helpers still waking from the last one. V8's
futex emulation takes one global lock for both. One run each, so this is a lead, not a measurement.

## Bench: wasm threads, `tick` mode

`harness/wasm-threads.mjs tick --wasm <arm> --n 1000000,400000 --workers 8,12,16 --ticks 7`, three
rounds, arms alternated (`target/ab-steal/run.sh`). Load rose from 20.8 to 41.0 over the run (ten
OpenCode jobs and two landers), and a cell moved 3–4× between rounds, so only round 1 (load 20.8–22.8)
is read. Median ms/tick:

| n | workers | base | steal | change |
|---:|---:|---:|---:|---:|
| 1000000 | 8 | 146.7 | 154.2 | +5.1% |
| 1000000 | 12 | 128.4 | 170.8 | +33.0% |
| 1000000 | 16 | 171.4 | 187.7 | +9.5% |
| 400000 | 8 | 75.4 | 75.3 | −0.1% |
| 400000 | 12 | 86.9 | 94.9 | +9.2% |
| 400000 | 16 | 75.9 | 77.6 | +2.2% |

Rounds 2 and 3 favour steal in 9 of 12 cells, but under load that doubled the base arm's own times
from round to round.

## What it does not do

- It does not make a tick faster on this host. The one quiet round has it slower in 5 of 6 cells.
- Caveat: every number here was taken on a shared host (load 20–41). A re-run on a quiet host could
  move the bench either way. The profile's wait share is the firmer fact, because it is a ratio
  within one run.
- The native helper-panic hang it fixes only affects native tests: on wasm32 a panic is an abort,
  and `harness/wasm-threads-helper.mjs` kills the process. That fix alone could land as a smaller
  change if a test ever hangs on it.
- The next serial pieces of the 1M tick are elsewhere: collide's counting sort in `Grid::build`
  (about 9 ms per tick on the coordinator, `collide5apply` self time) and the barnes_hut link force
  (`barnes_hut::link::force`, 36–50 ms over the run).
