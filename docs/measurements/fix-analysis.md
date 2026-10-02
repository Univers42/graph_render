# fix-analysis: review repairs in analysis (2026-10-02)

Branch `fix-analysis`, worktree `$GM_SCRATCH/wt/fix-analysis`. Reviews: `docs/reviews/review-core-post.md`
(R, M, U ids) and `docs/reviews/review-core-base.md` (F-34). Paths below are under `crates/graph-core/src/`
unless they start with `graph-cli/`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| R5 | BLOCKER | fixed: a zero-distance reached peer counts, as networkx 3.6 `closeness.py:127-133` | `closeness_counts_a_zero_distance_peer_as_reached_like_networkx` | `analysis/centrality.rs:77`; test `analysis/centrality/tests.rs:168` |
| R6 | BLOCKER | fixed in graph-core (D9): closeness clamped to `f32::MAX` (Ponytail); eigenvector stops unconverged on a non-finite norm; Brandes path counts are `PathCount` (finite past 2^1024, bit-identical below 2^512) | `closeness_past_f32_range_is_finite`, `eigenvector_whose_step_overflows_f64_is_finite_and_unconverged`, `betweenness_past_f64_path_counts_is_finite_and_exact` | `analysis/centrality.rs:87`, `:142`, `analysis/centrality/brandes.rs:137`; tests `analysis/centrality/tests.rs:178`, `:188`, `:220` |
| R18 | MAJOR | fixed: strictly-positive precondition stated and `debug_assert`ed (igraph 0.11.9 `betweenness.c:436-437` refuses the same input) | `betweenness_panics_in_debug_on_a_zero_weight_edge` | `analysis/centrality/brandes.rs:28`; test `analysis/centrality/tests.rs:238` |
| R19 | MAJOR | fixed: eigenvector `debug_assert`s non-negative weights | `eigenvector_panics_in_debug_on_a_negative_weight_graph` | `analysis/centrality.rs:103`; test `analysis/centrality/tests.rs:254` |
| R20 | MAJOR | fixed: the row states the real cost, O(n (n + m log n)) with Θ(n) buffers per source | none (row text) | `graph-cli/src/capabilities/analysis.rs:106`, `analysis/centrality/brandes.rs:21` |
| R21 | MAJOR | fixed: `debug_assert!` → `assert!`, refused in release too | `two_roots_with_no_virtual_root_are_refused` (run `--release`) | `analysis/depth.rs:155`; test `analysis/depth/tests.rs:251` |
| R25 | MAJOR | fixed: a self-loop lands twice in its node's row, as networkx `G.degree` (`louvain.py:265`) | `modularity_counts_a_self_loop_twice_like_networkx`, `louvain_counts_a_self_loop_twice_like_networkx` | `analysis/communities.rs:185`; tests `analysis/communities/tests.rs:120`, `:132` |
| M27 | MINOR | fixed: a source past the last node panics by name before petgraph indexes it | `dijkstra_refuses_a_source_past_the_last_node_by_name`, `bellman_ford_refuses_a_source_past_the_last_node_by_name` | `analysis/paths.rs:61` (called at `:25`, `:48`); tests `analysis/paths.rs:148`, `:154` |
| M28 | MINOR | doc-only: module doc and row say what depth does check | none | `analysis/depth.rs:9`, `graph-cli/src/capabilities/analysis.rs:149` |
| M29 | MINOR | fixed: `check_roots` refuses a detected root too, by index | `a_detected_root_past_the_last_node_is_refused_with_its_index` | `analysis/depth.rs:179` (called at `:152`, `:174`); test `analysis/depth/guards.rs:29` |
| M30 | MINOR | fixed: `Depth::is_unreached` added; the row names the raw `u32::MAX` and the escape hatch | `an_unreached_node_reads_as_unreached_and_a_reached_one_does_not` | `analysis/depth.rs:138`, `graph-cli/src/capabilities/analysis.rs:164`; test `analysis/depth/guards.rs:35` |
| M36 | MINOR | fixed: `powi(2)` → `share * share`. No numeric RED exists (`powi(2)` and `x*x` are bit-identical); the evidence is the grep below | none | `analysis/communities.rs:69` |
| F-34 | MAJOR | fixed-by M36 | none | `analysis/communities.rs:69` |
| U9 | unverified | = R18 | `betweenness_panics_in_debug_on_a_zero_weight_edge` | `analysis/centrality/brandes.rs:28` |
| U10 | unverified | no caller outside graph-core; the in-tree callers guard or are tests. The source check is M27's | as M27 | `analysis/paths.rs:61` |
| U11 | unverified | false: a convention, now documented. networkx 3.6 on path a–b–c gives b = 1.0 undirected and 2.0 on the symmetrised digraph with `normalized=False`; ours is 2.0, also pinned by `graph-wasm/src/analysis/tests/json.rs:44` | none | `analysis/centrality/brandes.rs:22` |
| U23 | unverified | = R21 | `two_roots_with_no_virtual_root_are_refused` | `analysis/depth.rs:155` |
| U24 | unverified | verified: the `depth/hierarchy.rs` tests ran green (25 depth tests, debug) | `a_repaired_hierarchy_reaches_every_node_so_unreached_never_appears` | `analysis/depth/hierarchy.rs:78` |
| U25 | unverified | deferred to fix-scale (not this job's path). Premise holds: a scratch probe on n0–n1 plus triangle 1-2-3 printed `louvain = [0, 0, 1, 1]`; the probe was removed | none | n/a |
| U26 | unverified | false: `find` merges each community into one entry, so the unstable sort has no ties. The test stays | `neighbor_weights_holds_one_entry_per_community_so_the_sort_has_no_ties` | `analysis/communities/tests.rs:102` |

## RED → GREEN (last lines, filtered runs)

- R5, R6, R18, R19: RED `test result: FAILED. 9 passed; 6 failed; ... 1041 filtered out`; GREEN
  `test result: ok. 18 passed; 0 failed; ... 1041 filtered out`.
- R25: RED, louvain `left: [0, 0, 0] right: [0, 1, 1]`, modularity `left: -0.0625 right: -0.125`,
  `test result: FAILED. 5 passed; 2 failed; ... 1054 filtered out`; GREEN `test result: ok. 7 passed; 0 failed; ... 1054 filtered out`.
- R21, `--release`: RED `test did not panic as expected` / `test result: FAILED. 0 passed; 1 failed; ... 1061 filtered out`;
  GREEN `test result: ok. 1 passed; 0 failed; ... 1063 filtered out`.
- M30: RED `error[E0599]: no method named is_unreached found for struct Depth` (4 errors); GREEN ok.
- M29: RED `panic did not contain expected string ... "root 2 of 2"` / `test result: FAILED. 1 passed; 1 failed`; GREEN ok.
- M27: RED, petgraph `bellman_ford.rs:249:13` and `csr_petgraph.rs:173:15` panics,
  `test result: FAILED. 0 passed; 2 failed; ... 1064 filtered out`; GREEN `test result: ok. 5 passed; 0 failed; ... 1062 filtered out`.
- Depth, debug, all: `test result: ok. 25 passed; 0 failed; ... 1039 filtered out`.
- M36: before, `git grep -n powi` named `analysis/communities.rs:66` as the only non-test graph-core use;
  after, `git grep -n -E '\.powi\('` over the motor names only layout test files.

## Done-when commands (run from the worktree, `CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4`)

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.42s`` |
| `timeout 2400 scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 20 test binaries, 1652 passed, 0 failed |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.51s`` |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | **1** | `capabilities --check: 69 rows, 36 problems` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `PASS` (`4-way equal on 8/8 seeds`, every `analysis.*` stage included) |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (expected non-zero) | `FAIL: 8 of 8 seeds diverge` |
| `scripts/orch/gr cargo build --release -p graph-cli` | 0 | ``Finished `release` profile [optimized] target(s) in 21.51s`` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` (32/32 rows reached a reference) |

`capabilities --check` is red on evidence alone, not on a row. All 36 problems are `gated, but no <record> record:
run the gate`, over 18 distinct ids. These are exactly the 18 `gated` rows in `capabilities --json`, and none is
`analysis.*`. All ten analysis rows are `implemented`. Exit 0 needs hashgate at 1000 seeds or more
(`graph-cli/src/capabilities/verdict.rs:13`), every negative control, oracle-diff, roundtrip, spectral,
layouts and osage, recorded on this tree. That is the full develop gate, which runs once on develop, not per
branch (CLAUDE.md, merge floor of 2026-09-29). `scripts/orch/rows/p12-t2.rows:75` records the same state.

The first conformance run exited 1 at `scripts/scigraphs-conformance.sh:121`: `Permission denied` on
`target/scigraphs-conformance-judge.log`. The containers had created `target/` as root in this fresh
worktree. The log file was created through `gr` and owned by uid 1000, and the unchanged script was re-run:
exit 0. The script assumes a host-writable `target/`.

## Decisions needed

- R6: graph-core no longer emits a non-finite value from closeness, eigenvector or betweenness. A JSON-face
  `is_finite` refusal belongs to `graph-wasm/src/analysis/report.rs` (fix-wasm-abi).
- R18: a release build still returns networkx's value on a zero weight (`[0, 2, 0, 0]`). Refusing it in
  release needs a `Result` on `betweenness`, which is a public-surface change.
- M28: refusing one root with `Some(n)` as its virtual root would invert the pinned test
  `a_lone_root_sits_at_depth_zero_even_when_the_source_names_a_virtual_root`, so it is tolerated, as p3
  tolerates it. Moving the check into the `Roots` impl needs `layout/hierarchy.rs`.
- U25: Louvain's representative, fix-scale's path.
- `capabilities --check` exit 0 needs the full develop gate after merge (above).
