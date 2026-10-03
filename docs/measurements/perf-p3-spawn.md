# Perf P3-spawn — what `Threads` pays to spawn per pass, and no pool yet

Measured 2026-10-02 on host dlesieur42 (20 cores, load average 25–30). Plan P3 builds a
persistent native pool only if spawning costs at least 3 % of a tick (`prompts/perf-plan.md`).

`Threads::run` (`crates/graph-cli/src/exec_native.rs`) opens a `std::thread::scope` per pass,
spawns one thread per range and joins them all.

## Passes per tick

callgrind's calls into `pthread_create` on the `perf-p3-link-once.md` runs (1M nodes, particle
mesh, `--workers 8`), the difference between `--ticks 3` and `--ticks 1` over two:

| build | spawns per tick | passes per tick |
|---|---:|---:|
| `27c3828` | 160 | 20 |
| `2fac8f2` (link once) | 168 | 21 |

## The cost of one pass's spawn and join

A scope that spawns `w` threads with an empty body and joins them, 3 000 times, built with
`rustc -O` in `ge-rust`, run on the host:

```rust
let mut out = vec![0u64; w];
for r in 0..reps {
    let t = Instant::now();
    std::thread::scope(|s| {
        for (i, slot) in out.iter_mut().enumerate() {
            s.spawn(move || *slot = std::hint::black_box((i + r) as u64));
        }
    });
    samples.push(t.elapsed().as_nanos() as u64);
}
```

| workers | min µs | p50 µs | p90 µs | p99 µs |
|---:|---:|---:|---:|---:|
| 2 | 22.0 | 29.0 | 34.4 | 64.7 |
| 4 | 38.5 | 57.6 | 68.8 | 221.5 |
| 8 | 71.7 | 101.7 | 115.1 | 1 077.4 |

## Decision

21 passes × 101.7 µs = **2.1 ms per tick** at eight workers. The 1M tick at eight workers is
120–150 ms on a quieter host (`perf-p3-pm-serial.md`) and 211–363 ms at this load
(`perf-p3-link-once.md`), so spawning is 0.6–1.8 % of it: **below the 3 % bar, no pool.**

The bar is crossed when the eight-worker tick falls under 71 ms (2.1 ms / 3 %). At the plan's
25 ms target it would be 8.5 %, so the pool is owed before that target can be claimed.

Caveat: the bench runs an empty body, so it measures the spawn, the scheduling delay and the
join, not the cache warm-up a fresh thread pays on its first touch of its span; and at this load
the scheduling delay dominates, so on a quiet host the cost is lower and the decision holds.
