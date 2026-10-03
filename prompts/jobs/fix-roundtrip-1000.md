# Job fix-roundtrip-1000 (agent build, branch fix-roundtrip-1000, worktree ~/goinfre/wt/fix-roundtrip-1000)

Symptom: develop-full row `roundtrip-1000|0|scripts/orch/gr cargo run -q -p graph-cli -- roundtrip
--seeds 1000` (debug build, develop e124184a) ran 2 h at 100% of one core, 12.9 MiB RSS, empty log,
and was killed. `roundtrip --seeds 4` takes 6 s warm. 1000 seeds linear would be ~25 min. Either the
cost per seed grows with the seed, or one seed hangs.

Exact tasks:
1. Read the roundtrip command: `git grep -n roundtrip -- crates/graph-cli/src` (entry, seed → graph size
   and layout mapping, where output is printed — buffered to the end would explain the empty log).
2. Measure, each under `timeout 1800`: `scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip
   --seeds N` for N = 8, 32, 128, 256; then debug for N = 32, 128. Table: N, wall time, exit.
3. If one seed hangs: find it by bisection (add a `--seed-from`/single-seed option only if none exists),
   then find the stage/layout that loops, at file:line. If the per-seed cost grows: name the term.
4. Fix the root cause in the shared function (motor or graph-cli), not per caller. Determinism rules
   (`prompt.md` §6): `libm`, BTreeMap/IndexMap, no new graph-core deps. If the only finding is "debug
   is too slow", change the row in `scripts/orch/rows/develop-full.rows` to `cargo run --release -q ...`
   and give the measured release wall time for 1000 seeds.
5. Print per-seed progress to stderr (one line per seed) so a stall is visible in the log.
6. Leave one check: a test, or a bounded-time assertion, that fails on the old behaviour.
7. Write `docs/measurements/fix-roundtrip-1000.md`: commands, before/after table, root cause file:line.

Paths allowed: `crates/graph-cli/**`, `crates/graph-core/**` (only the root cause),
`scripts/orch/rows/develop-full.rows`, `docs/measurements/fix-roundtrip-1000.md`.

Done when: `roundtrip --seeds 1000` (release) exits 0 with its wall time pasted, and
`scripts/orch/gr -e GM_MUTATE_NODE_Z=1 cargo run -q -p graph-cli -- roundtrip --seeds 8` exits non-zero.
