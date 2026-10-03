# Perf P3-link-once — each edge's force computed once, then gathered

Measured 2026-10-02 on branch `perf-p3-link-once` at `2fac8f2`, against `27c3828`
(`perf-p3-pm-serial`, the base `perf-p3-pm-collide` measured from). Host dlesieur42, 20 cores,
load average 24–30 for the whole run, so the decision rests on callgrind's instruction counts.

`LinkPass` (`docs/decisions/link-gather.md`) gathers each node's share of its incident edges, so
every simple edge's force (a square root and a division) was evaluated twice, once per endpoint:
`barnes_hut::link::halves` was 294.4 M of a 1M particle-mesh tick (`perf-p3-pm-serial.md`).

## Design

- **`LinkForces`** (`barnes_hut/step.rs`): one output per simple edge, `link::force(sim, e)`, the
  old `halves` up to the bias split. Each output is its own edge's, so the pass partitions by edge.
- **`LinkPass`** reads that output and applies `link::share(f, b, hi)`: `(-fx·b, -fy·b)` for the
  higher endpoint, `(fx·(1−b), fy·(1−b))` for the lower one, which are the old `halves`
  expressions on the same `(fx, fy)`. A node's row is walked in the same order, so its sum sees
  the same terms in the same order.
- **The scratch** is `Sim::link_forces`, `m × 16 B`, taken and put back each tick, so its
  capacity is kept: 1 549 780 simple edges at 1M nodes, 24.8 MB. It is listed in
  `Sim::scratch_capacities`, which `session/tests/live.rs::the_scratch_buffers_are_reused_across_ticks`
  holds constant over 200 ticks.
- `halves` stays as the serial reference's split, compiled for tests only.
- `THREADED_PASSES` lists the new pass: `["link forces", "link", "charge", "collide"]`.

Both `layout.force.barnes_hut` and `layout.force.particle_mesh` run this pass. No new layout id,
no golden update.

## Identity

`graph-cli snapshot --seed S --nodes N --layout L --out-bin …`, the profile builds of `27c3828`
and `2fac8f2`; `cmp` silent on every pair. SHA-256, first 8 hex digits:

| seed | n | `force.particle_mesh` | `force.barnes_hut` |
|---:|---:|---|---|
| 1 | 100 000 | `3d47b7d1` | `47296918` |
| 3 | 20 000 | `6c50077d` | `02da6ea5` |
| 4 | 1 000 | `baeff1b6` | `3d184107` |
| 5 | 5 000 | `453c138c` | `6ac01f26` |

The particle-mesh hashes are the ones in `perf-p3-pm-serial.md` and `perf-p3-pm-fft.md`.

Negative control, `cargo test -p graph-core --lib layout::force` with `share`'s branches swapped
(`if !hi`, the higher endpoint given the lower share): exit 101, 195 passed, 6 failed:

| failing test | what it pins |
|---|---|
| `barnes_hut::tests::kernels::every_passs_own_control_bites_by_the_gate_s_fifth_seed` | the per-pass controls against the serial tick |
| `barnes_hut::tests::the_jacobi_link_stability_fixture_stays_finite_and_bounded_devil_c9` | link stability |
| `session::tests::m1a::the_default_session_reproduces_the_frozen_layout_on_every_seed` | the session against the frozen layout |
| `session::tests::m1c::the_command_script_replays_to_identical_bytes` | the replay bytes |
| `yifan_hu::tests::neighbours_end_up_nearer_than_the_average_pair` | quality |
| `yifan_hu::tests::the_output_on_a_path_of_40_is_pinned` | pinned output |

## Instructions, a 1M tick at eight threads

`valgrind --tool=callgrind --separate-threads=yes` on `graph-cli tick --layout particle-mesh
--n 1000000 --warm 1 --seed 1 --workers 8` (image `ge-profile`), `--ticks 1` and `--ticks 3`; per
tick = the difference over two ticks, self cost summed over every thread
(`callgrind_annotate --auto=no` per thread file).

| | `27c3828` | `2fac8f2` |
|---|---:|---:|
| `link::halves` | 294.4 M | — |
| `link::force` (inside `LinkForces`) | — | 122.4 M |
| `LinkForces` | — | 13.9 M |
| `LinkPass` | 116.7 M | 98.1 M |
| **the link passes** | **411.1 M** | **234.4 M (−43.0 %)** |
| `collide::Gather` | 1 215.5 M | 1 199.1 M |
| the tick | 2 953.7 M | 2 762.1 M (−6.5 %) |
| thread 1, serial | 83.6 M | 83.6 M |
| serial share | 2.83 % | 3.03 % |
| Amdahl at 8 threads | 6.68× | 6.60× |

`Gather`'s 16.4 M is the compiler's, not this slice's: its code and its input are the same in both
builds (the snapshots are equal), and no other function moved by more than 1.3 M.

## Wall clock

The same builds, `tick --layout particle-mesh --n 1000000 --warm 2 --ticks 6 --seed 1
--workers W`, three rounds, the arm order alternating per round, inside `ge-profile`. Tick median
in ms per round, then the median of the three:

| workers | base, per round | base | new, per round | new | change |
|---:|---|---:|---|---:|---:|
| 1 | 596.8, 605.2, 601.7 | 601.7 | 595.0, 578.8, 574.9 | 578.8 | −3.8 % |
| 8 | 340.6, 389.2, 318.7 | 340.6 | 269.0, 291.8, 363.2 | 291.8 | −14.3 % |

The fastest tick of the 18 per cell: 517.2 → 469.6 ms at one worker (−9.2 %), 253.2 → 210.5 at
eight (−16.8 %). The load average was 24–30 on 20 cores, so the eight-worker row ran on about two
free cores and these ticks are slower than `perf-p3-pm-serial.md`'s; only the direction is claimed.

## Gates

**Not run** on this branch yet: the merge floor runs when it lands through `queue.sh land`, and
hashgate `--seeds 1000`, the Python oracles and mutants run once on develop.

## What it does not do

- **24.8 MB more resident at 1M** (`m × 16 B`). The gather it replaces needed none.
- **The bias multiply still runs at both endpoints**: two multiplies per endpoint, cheaper than
  storing both shares (another `m × 16 B`).
- **The serial merge** (`step::merge`, 8.5 M) is unchanged.
