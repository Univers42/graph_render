# Job test-speed-geometry (agent build: cut the landing gate's slowest test binary)

Read `scripts/orch/common.md` first. Test code only: no file under `crates/*/src/` changes, and no
Cargo file changes (they are fingerprinted and change every build).

Fact (2026-10-02, landing gate logs): `crates/graph-core/tests/geometry_invariants.rs` takes about
760 s of the about 1000 s `cargo test --workspace` row; `tests/cli_force.rs` takes about 177 s. The
binary holds three `#[test]`s, each a serial `for seed in 0..SEEDS` (SEEDS = 200) loop, and
`every_registered_layout_emits_no_nan_or_inf_and_circle_radii_are_positive` runs every
`registry::LAYOUTS` entry per seed in one test. libtest runs tests in parallel (20 cores here), so
one long test is one core.

1. Measure first: `scripts/orch/gr cargo test -p graph-core --test geometry_invariants -- --report-time -Z unstable-options`
   if the toolchain accepts it, else time each test alone with `--exact`. Paste the per-test times.
2. Split each long sweep into several `#[test]`s over disjoint seed ranges (or, for the all-layouts
   test, one test per layout) that together cover exactly the same `(seed, layout)` pairs and keep
   every assertion and message. No threads spawned inside a test (`RUST_TEST_THREADS` must keep
   capping the binary).
3. If you split per layout, add a guard test asserting the per-layout list equals the ids of
   `registry::LAYOUTS` in order, so a new registry row cannot go unchecked. RED: drop one layout
   from the list locally and show the guard fails; do not commit that.
4. Same for `crates/graph-cli/tests/cli_force.rs` only if one test there dominates (measure; if
   it does not, say so and leave it).

Paste before and after: the binary's `finished in` line, with and without `RUST_TEST_THREADS=4`.
Coverage must not shrink: list the `(test, seed range, layouts)` table before and after.

Paths: `crates/graph-core/tests/geometry_invariants.rs`, optionally
`crates/graph-cli/tests/cli_force.rs`, `docs/measurements/test-speed-geometry.md`.

Done when: `cargo test --workspace --no-fail-fast` green, fmt and clippy `-D warnings` green, the
binary's wall time at least halved at the default thread count, and the measurement doc holds the
before and after numbers.
