# fix-scale: review findings in `crates/graph-core/src/scale/`

Source: `docs/reviews/review-core-post.md` (R4, R7, R22, R23, R24, M31–M35, U12, U13, U25).
Branch `fix-scale`, from develop `45aaa8a`. Paths below are relative to `crates/graph-core/src/scale/`.

## Verdicts

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| R4 | BLOCKER | fixed | `lod::mask_tests::a_label_budget_of_zero_means_no_limit` | test `lod/mask_tests.rs:10`; fix `lod.rs:215` (`label_mask`: 0 is no limit, per `SciGraphs/engine/scigraphs_engine/lod.py:76-79`) |
| R7 | BLOCKER | fixed | `simplify::invariant_tests::a_chain_never_contracts_onto_a_folded_leaf` | test `simplify/invariant_tests.rs:40`; fix `simplify/chain.rs:25` |
| R22 | MAJOR | fixed | `lod::mask_tests::the_label_rank_reads_each_degree_once_and_orders_by_degree_then_index` | test `lod/mask_tests.rs:28`; fix `lod.rs:238` (`rank_by_degree`, counting sort) |
| R23 | MAJOR | fixed | `simplify::scaling_tests::{a_large_star_folds_its_leaves_in_linear_time, a_large_path_contracts_its_chain_in_linear_time, a_skipped_chain_is_walked_once}` | tests `simplify/scaling_tests.rs:46`, `:58`, `:71`; fix `simplify.rs:210` (`edges_between` over CSR rows), `simplify/chain.rs:15` (`seen`), `simplify/chain.rs:98` |
| R24 | MAJOR | fixed | `simplify::scaling_tests::many_communities_collapse_in_linear_time` | test `simplify/scaling_tests.rs:90`; fix `simplify/community.rs:17`, `:50`, `:80` (one edge pass) |
| M31 | MINOR | false | `lod::mask_tests::the_rank_degree_counts_a_self_loop_twice_like_the_incident_list` | test `lod/mask_tests.rs:78`; doc `lod.rs:284` |
| M32 | MINOR | doc-only | – | `lod.rs:126-128` (`edge_stride` 0 is read as 1) |
| M33 | MINOR | fixed | `lod::mask_tests::a_nan_viewport_field_culls_nothing_on_its_side` | test `lod/mask_tests.rs:47`; fix `lod.rs:99` (`Viewport::or_unbounded`) |
| M34 | MINOR | fixed | `simple::tests::a_row_offset_is_as_wide_as_the_neighbour_column_length` | test `simple.rs:126`; fix `simple.rs:20` (`offsets: Vec<usize>`) |
| M35 | MINOR | fixed-by R23 | – | `simplify.rs:210` |
| U12 | unverified | deferred | – (R4's RED is the evidence the defect class exists) | the oracle differential against `lod.py` needs `harness/` and `graph-cli` paths: decision 2 |
| U13 | unverified | false | – | `simplify.rs:37-44`: `Plan` holds three booleans and no target count, so there is no target to miss |
| U25 | unverified | fixed | `simplify::invariant_tests::a_community_never_collapses_onto_a_folded_leaf` | test `simplify/invariant_tests.rs:78`; fix `simplify/community.rs:50` |
| N1 (new) | – | fixed | `simplify::invariant_tests::a_two_node_component_keeps_its_lower_index` | test `simplify/invariant_tests.rs:57`; fix `simplify.rs:176` |
| N3 (new) | – | fixed-by R24 | `simplify::invariant_tests::a_community_step_links_only_its_own_external_edges` | test `simplify/invariant_tests.rs:89`; fix `simplify/community.rs:80` |

N1: a two-node component folded both leaves, each onto the other. N3: before R24's
rewrite, every community step listed every link of the pass.

The R24 review example, singleton communities, never reached the rescan, because a
singleton returned early. The RED uses communities of three.

## RED, then GREEN

Every RED was run against the unfixed code with the new test in place, using `scripts/orch/gr cargo test -p graph-core --lib <filter>`.

| id | RED (failing run) | GREEN |
|---|---|---|
| R4 | `left: [0, 0, 1, 0, 0, 0, 0, 0]` `right: [0, 0, 1, 1, 1, 1, 1, 0]`; `test result: FAILED. 0 passed; 1 failed` | `test result: ok. 7 passed; 0 failed` |
| R22 | `mask_tests.rs:33:5` `left: 14134` `right: 800` (degree reads); `FAILED. 7 passed; 1 failed` | `test result: ok. 8 passed; 0 failed` |
| M33 | `left: [0, 0, 0, 0, 0, 0, 0, 0]` `right: [0, 0, 1, 1, 1, 1, 1, 0]`; `FAILED. 2 passed; 1 failed` | `test result: ok. 9 passed; 0 failed` |
| R7 | `invariant_tests.rs:12:9` `node 0 -> hidden 1` `left: 0` `right: 1`; `FAILED. 0 passed; 1 failed` | `test result: ok. 7 passed; 0 failed` |
| N1 | `invariant_tests.rs:12:9` `left: 0` `right: 1`; `FAILED. 1 passed; 1 failed` | `test result: ok. 8 passed; 0 failed` |
| R23 | `not finished within 20s: a pass is super-linear` ×3; `FAILED. 0 passed; 3 failed ... finished in 21.53s` | `test result: ok. 30 passed; 0 failed ... finished in 3.31s` |
| R24, U25, N3 | N3 `left: [(0, [(0, 3), (3, 6)]), (3, [(0, 3), (3, 6)]), (6, [(0, 3), (3, 6)])]`; U25 `node 0 -> hidden 0`; R24 `not finished within 20s`; `FAILED. 11 passed; 3 failed ... finished in 20.85s` | `test result: ok. 33 passed; 0 failed ... finished in 5.98s` |
| M34 | `simple.rs:131:9` `left: 4` `right: 8`; `FAILED. 3 passed; 1 failed` | `test result: ok. 34 passed; 0 failed ... finished in 4.51s` |
| M31 | passes on the unfixed code, so it is recorded false and the test stays | `test result: ok. 4 passed; 0 failed` |

The R23 path test passed on the old code at N = 100 000 (14.08 s). At N = 300 000 it
fails. The wall-clock bound carries a `Ponytail:` in the header of `simplify/scaling_tests.rs`.

## Done-when commands

Every command was run from the worktree with `CARGO_BUILD_JOBS=6`.

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.26s`` |
| `timeout 2400 scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` (doc-tests; 1653 passed and 0 failed in total, graph-core lib `1057 passed; 0 failed; 6 ignored`) |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.51s`` |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` / `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (expected non-zero) | `4-way equal on 0/8 seeds` / `FAIL: 8 of 8 seeds diverge` |
| `scripts/orch/gr cargo build --release -p graph-cli` | 0 | ``Finished `release` profile [optimized] target(s) in 17.54s`` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` (31 rows `ok`, no row moved; `GRAPHVIZ_DOT` is "not run: no motor layout for this name", the same as on the first run) |

The first conformance run exited 1, but it was an environment failure, not a verdict:

- The run failed with `scripts/scigraphs-conformance.sh: line 121: target/scigraphs-conformance-judge.log: Permission denied`.
- The cause: in a fresh worktree, the first container run creates `target/` as root, so the host shell cannot write the judge's log there.
- The judge never ran.
- `scripts/orch/gr chown 1000:1000 target`, which changes the top directory only, made the log writable. The script was then run again in full.

## Decisions needed

1. `docs/measurements/phase09-lod.md:26-28` still says "a budget of zero still lets the most
   important visible node keep its label". That is stale after R4, and the file is outside this slice's paths.
2. U12: a lod/adaptive oracle differential against
   `SciGraphs/engine/scigraphs_engine/lod.py`. It needs `harness/` and `graph-cli` paths.
3. M33 is fail-open: a NaN viewport field reads as unbounded and is not refused. Refusing it
   needs an error path in `hints`, which is an API change.
4. M34's real overflow input, 2^31 distinct pairs (tens of GiB), cannot be built in a test. The
   test pins the offset width instead.
5. Seen but not fixed. Each is a behaviour of the passes, not one of the findings:
   - A self-loop on a folded leaf, or on a contracted interior node, stays drawn.
   - An external edge between two collapsed members stays drawn. The existing test pins it with `s.edges[6] == 1`.
   - A chain step's links can name a node that a later community step hides.
6. `scripts/scigraphs-conformance.sh:121` writes into `target/`, which is root-owned in a fresh
   worktree. `wt-new.sh`, or the script, could create `target/` as the host user.
7. `rank_by_degree` keeps a slot vector of O(max degree), at most 2m + 2 entries.
