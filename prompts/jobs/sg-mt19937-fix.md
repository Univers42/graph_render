# Job sg-mt19937, review round 1 (same session, same worktree): FIX, 1 MAJOR, test only

The review (graph-render-3e, 2026-10-02) verified the MT19937 port bit for bit against numpy 2.3.3 and
an independent Python port (init_genrand, twist, tempering, res53, cube node 8). No code change is wanted
outside tests, comments and docs.

MAJOR — `cube/tests/interior.rs:68-76` `a_neighbouring_seed_moves_the_interior_and_no_corner` is vacuous.
Its expected value uses x5.0 where the reach is 4.0, and it compares an f32 widened to f64 (`x[8]`) to
an f64, so it passes with the SAME seed (3.2923626... against 4.1154533...). Rebuild it with reach 4.0,
compare in f32, and paste a run where it FAILS when the seed is the same (then restore the neighbouring
seed and paste it passing).

MINOR, text only:
1. `rng.rs:70-71` and `docs/measurements/sg-mt19937.md:11` are false: `random_sample` IS genrand_res53
   ((a*2^26+b)/2^53); `Generator(MT19937 legacy-seeded).random() == RandomState.rand()` is True.
2. `cube.rs:~147` and `interior.rs:47-49`: "(2u-1)*reach vs -reach+2*reach*u differ in last bits" is false
   at reach 4.0 (2e6 draws, 0 mismatches), so the uniform test cannot tell the two orders apart; say so.
3. `docs/measurements/scigraphs-conformance.md:137,283` and `sg-mt19937.md:68`: the 501 f64-exact CUBE
   coordinates are the +-5.0 corner coordinates of all 24 fixtures, not "fixtures with no interior"
   (the n<=8 fixtures hold only 117).
4. `cube/tests.rs:123,143`: "not the reference's" and "statistical" are stale; the interior is now bit-exact.
5. `gaps.rs` `G_RANDOM_ITER` `at` should name `random.rs:59` (`run_seeded`), not `:43` (`run`). Check the line.
6. `scigraphs-conformance.md:171` "Procrustes medians ~1e-16": RANDOM's is 2.28e-15.
7. The report lacks the RED run (sg-common step 2), the step-1 before-metrics line, the fmt/clippy/test
   last lines and the `--break` output: add them to `docs/measurements/sg-mt19937.md` from real runs.

Done when: quick.rows green (hashgate-8 and its negctl), `scripts/scigraphs-conformance.sh` exit 0 and
`--break` exit 1, the MAJOR test's fail-then-pass pasted, and a return block with every real exit code.
