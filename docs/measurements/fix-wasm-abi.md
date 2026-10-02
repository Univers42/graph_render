# fix-wasm-abi: the graph-wasm and SDK findings of `review-core-base.md`

Job brief: `prompts/jobs/fix-wasm-abi.md` over `prompts/jobs/fix-common.md`. Branch `fix-wasm-abi`,
merged with `origin/develop` at `a20fdfe` (perf-p5) before the gates below ran.

Paths are relative to `crates/graph-wasm/src/` unless they start with `crates/`, `docs/` or
`harness/`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-01 | BLOCKER | doc-only (refusing it is "decisions needed") | `the_abi_doc_states_what_an_omitted_child_first_means`; pins `an_omitted_child_first_reads_parent_first_and_a_present_one_is_read`, `a_child_first_that_is_not_a_boolean_is_refused` | `ingest/tests/child_first.rs:35`, `:17`, `:24` |
| F-16 | MAJOR | deferred: the ceiling is a product number (the studio's source scales to 1M nodes); "decisions needed" | — | `ingest.rs:49` |
| F-17 | MAJOR | false: C7/C10 already make a view invalid after `gm_run`/`gm_post_run`/`gm_release`, and the SDK enforces it with an epoch | `a_released_or_unrun_handle_is_refused_by_name_never_served_stale` | `handle.rs:181` |
| F-18 | MAJOR | false: a released handle is refused with `InvalidHandle`, never served | `a_released_or_unrun_handle_is_refused_by_name_never_served_stale` | `handle.rs:181` |
| F-30 | MAJOR | false: as F-17; `ptr` and `len` are read inside one SDK epoch | `a_released_or_unrun_handle_is_refused_by_name_never_served_stale` | `handle.rs:181` |
| F-31 | MAJOR | fixed: a present but empty column reads `(0, 0)` like an absent one (C3), handle and session | `a_present_but_empty_column_reads_zero_zero_like_an_absent_one`; `an_empty_sessions_columns_read_zero_zero` | `views/tests/wire.rs:22`; `session/tests/refusals.rs:214` |
| F-32 | MAJOR | false: an absent column is a documented answer (C3), not a refusal, so clearing the code is correct; absent and empty are told apart by geometry kind | `on_an_empty_graph_absent_and_empty_are_told_apart_by_geometry_kind_not_by_length` | `views/tests.rs:142` |
| F-33 | MAJOR | fixed: `gm_abi_version() -> u32` (returns `1`), documented; the SDK refuses another number naming both | `the_sdk_and_the_doc_state_this_modules_abi_version`; `every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order`; SDK `a module reporting another ABI version is refused, naming both numbers` | `errors/mirrors.rs:74`, `:64`; `crates/graph-sdk-js/test/abi-version.test.mjs` |
| F-79 | MINOR | doc-only: unreachable on wasm32 (`usize` is `u32`), reachable natively; the ceiling is F-16's | — | `ingest.rs:49` |
| F-80 | MINOR | fixed (analysis half): closeness and betweenness over a negative `strength` answer `AnalysisFailed` instead of a debug panic or a silently wrong release score; layouts and posts never panic on extremes | `a_negative_strength_refuses_the_shortest_path_centralities_instead_of_answering`; `extreme_weights_and_strengths_never_panic_an_analysis`; `..._never_panic_a_layout_or_a_post_pass` | `ingest/tests/extremes.rs:75`, `:63`, `:39` |
| F-81 | MINOR | fixed: every `0` from `publish` carries a code | `a_zero_from_publish_always_carries_a_nonzero_code` | `wire/tests.rs:8` |
| F-82 | MINOR | fixed: a refused gate stage names `LayoutFailed` | `a_refused_gate_stage_names_its_reason` | `wire/tests.rs:18` |
| F-83 | MINOR | fixed: one out-buffer (`wire::OUT`) for the exports and the hash-gate shims | `a_published_gate_stage_never_leaves_a_stale_code` | `wire/tests.rs:25` |
| F-84 | MINOR | fixed: an address past `u32` is refused with `IndexOutOfRange`, not read as absent | `an_address_past_u32_is_refused_not_read_as_absent` | `views/tests/wire.rs:34` |
| F-85 | MINOR | false: every present column's element type is the one its contract row states, so `(ptr, len)` loses nothing | `every_present_column_has_the_element_type_its_contract_row_states` | `views/tests/wire.rs:86` |
| F-86 | MINOR | false: D9 is checked at the two faces by design (C8); a column view is the caller's memory | `non_finite_is_caught_in_node_columns_and_in_edge_points` | `views/tests.rs:248` |
| F-87 | MINOR | fixed: the seed writer states `ingest::VERSION`; no RED is possible while `VERSION` is `1` | `the_seed_document_states_the_version_the_reader_accepts` | `seed_ingest/tests.rs:88` |
| F-88 | MINOR | fixed: a non-finite seed number is refused, never written as `inf` | `a_non_finite_number_is_refused_never_written_as_json` | `seed_ingest/tests.rs:71` |
| F-89 | MINOR | fixed: an index past `FUNCTIONS` is refused, not aliased to `atan2` | `an_evaluate_index_past_functions_is_refused_not_read_as_atan2`; `an_inputs_index_past_functions_is_refused_not_swept_as_atan2` | `probe/tests.rs:125`, `:131` |
| F-90 | MINOR | false: both arms and the reader (`crates/graph-cli/src/probe_report.rs:3`) name functions from the one `probe::FUNCTIONS` of the tree graph-cli was built from | `probe_bytes_frames_every_function_and_record` | `probe/tests.rs:101` |
| F-91 | MINOR | doc-only: the `Ponytail:` header states it sums requested bytes and under-reports page growth | — | `memory_measure.rs:23` |
| F-92 | MINOR | fixed: the counters are per thread | `another_threads_allocation_is_not_counted_in_this_threads_peak` | `memory_measure.rs:127` |
| F-93 | MINOR | doc-only: one `ContractInvalid` is the documented contract (Errors row 14); splitting it changes what existing callers read | — | `contract.rs:31` |
| F-94 | MINOR | fixed-by F-81 (the same `publish`, now in `wire.rs`) | `a_zero_from_publish_always_carries_a_nonzero_code` | `wire/tests.rs:8` |
| F-95 | MINOR | fixed: a non-finite score or modularity is refused (`AnalysisFailed`), never written as `NaN` | `a_non_finite_score_is_refused_never_written_as_json`; `a_non_finite_modularity_is_refused_never_written_as_json` | `analysis/tests/finite.rs:24`, `:32` |
| F-96 | MINOR | fixed: `to_json` refuses a column past `u32` instead of writing `"nodeCount":0`; no RED is possible (a 2^32-element column) | — | `analysis/report.rs:91` |
| F-97 | MINOR | fixed: the id is escaped into a JSON string | `an_id_is_escaped_into_a_json_string` | `analysis/tests/finite.rs:42` |
| F-98 | MINOR | fixed: the post pass reads the geometry in place and keeps its one snapshot, no clone; no observable RED, the output is pinned | `a_post_run_replaces_the_edges_and_keeps_the_layouts_nodes`; `a_second_post_run_reads_the_layout_not_the_first_pass` | `stage_exports/tests.rs:83`, `:122` |
| R6 (report.rs half) | — | fixed-by F-95 | `a_non_finite_score_is_refused_never_written_as_json` | `analysis/tests/finite.rs:24` |
| unverified #5 (session) | — | promoted to F-31 (session half) and fixed | `an_empty_sessions_columns_read_zero_zero` | `session/tests/refusals.rs:214` |
| unverified #6 (build/state/session/stages exports) | — | `is_live`: false; `OUT`: fixed-by F-83 | `only_the_exact_recorded_range_is_live` | `alloc.rs:142` |
| unverified #7 (analysis/post children) | — | promoted to F-80 (centrality); rest false | `a_negative_strength_refuses_the_shortest_path_centralities_instead_of_answering` | `ingest/tests/extremes.rs:75` |

## RED and GREEN, filtered by test name

| run | result |
|---|---|
| RED 1 (F-31, F-81..F-84) | 8 passed; 6 failed. `an_empty_sessions_columns_read_zero_zero`: ptr, axis 0 left `Ok(8)` right `Ok(0)`; `a_present_but_empty_column_reads_zero_zero_like_an_absent_one`: left `Ok(4)` right `Ok(0)`; `an_address_past_u32_is_refused_not_read_as_absent`: a present column at address 0; `a_published_gate_stage_never_leaves_a_stale_code`: stale code survived (1 == 1); `a_refused_gate_stage_names_its_reason`: left 0 right 8; `a_zero_from_publish_always_carries_a_nonzero_code`: publish returned 0 with no error code |
| GREEN 1 | 14 passed; 0 failed |
| RED 2 (F-95, F-97) | 0 passed; 3 failed: `"values":[0.5,NaN]`, `"modularity":NaN`, id not JSON |
| GREEN 2 | `cargo test -p graph-wasm --lib`: 119 passed; 0 failed; 1 ignored |
| RED 3 (F-89) | both `*_past_functions_*` FAILED (did not panic) |
| RED 4 (F-92) | peak 67108952 B counted the other thread's 64 MiB |
| RED 5 (F-88) | left `Some("...\"weight\":inf...")` right `None` |
| GREEN 3..5 | `cargo test -p graph-wasm --lib`: 123 passed; 0 failed; 1 ignored |
| RED 6 (F-33 mirrors) | 5 passed; 2 failed: `wasm.ts` lacks `export const ABI_VERSION = 1;`; the doc's Errors table listed 15 names, the module 19 |
| RED 7 (F-33 SDK) | `not ok 1 - a module reporting another ABI version is refused, naming both numbers` (`the module loaded: nothing checked its ABI version`) |
| GREEN 6, 7 | mirrors 2 passed; SDK `# pass 2`, `# fail 0` |
| RED 8 (F-80) | 1 passed; 2 failed: `panicked at crates/graph-core/src/analysis/centrality.rs:54:5` |
| GREEN 8 | 20 passed; 0 failed (filter `analysis`) |
| RED 9 (F-01) | `wasm-abi.md must state: child_first is optional in version 1: omitted, it reads false` |
| GREEN 9 | `cargo test -p graph-wasm --lib`: 133 passed; 0 failed; 1 ignored |
| RED 10 (own regression) | F-98 made `post_run` return `&Snapshot`; `exports/` compiles for wasm32 only, so the native clippy and lib tests passed while the wasm32 build failed: `error[E0515]: cannot return value referencing temporary value --> crates/graph-wasm/src/exports/stages.rs:58:25`. The workspace test caught it (graph-cli `hashgate::tests::stages::arm`, `cli.rs`: "building graph-wasm for wasm32 failed: exit status: 101") |
| GREEN 10 | `exports/stages.rs` maps the borrow away inside the closure; wasm32 release build exit 0; `cargo clippy -p graph-wasm --target wasm32-unknown-unknown -- -D warnings` exit 0 |

## Gates, on the merged tree (`46f61b4`)

Cargo rows ran with `CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=4`.

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile` |
| `timeout 2400 scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 20 binaries and 3 doc-test sets: 1668 passed, 0 failed, 12 ignored; `graph_wasm` 134 passed, 1 ignored |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished dev profile` |
| `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0 | `Finished release profile` |
| `scripts/orch/gr cargo clippy -p graph-wasm --target wasm32-unknown-unknown -- -D warnings` (extra: `exports/` is wasm32-only) | 0 | `Finished dev profile` |
| `scripts/orch/node-slim.sh npm run sdk:typecheck` | 0 | (tsc, no output) |
| `scripts/orch/node-slim.sh node --test crates/graph-sdk-js/test/abi-version.test.mjs` | 0 | `# pass 2` `# fail 0` |
| `scripts/orch/node-slim.sh npm run sdk:smoke` | **1** | 304 ok, `not ok - layout.hierarchical3d: its bounds are finite and not a single point`, `# 1 failed`. Not this branch: see below |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (expected non-zero) | `FAIL: 8 of 8 seeds diverge` |
| `scripts/studio.sh wasm` | 0 | `staged graph_wasm.wasm (1317283 bytes) and fixtures/ into app/public` |
| `scripts/studio.sh check` | 0 | `# pass 515` `# fail 0`, `# pass 61` `# fail 0`, `[studio] ok` |
| `scripts/orch/gr cargo build --release -p graph-cli` | 0 | `Finished release profile` |
| `scripts/scigraphs-conformance.sh` | 0 | `CIRCULAR_HIERARCHY: ok ...` `PASS` |

**`sdk:smoke` is red on develop too, with the same row.** With `crates/graph-wasm` and
`crates/graph-sdk-js` checked out from `a20fdfe` (develop) and the wasm rebuilt, the smoke exits 1
on `layout.hierarchical3d: ... bounds x[0, 0] y[0, 0]`, `# 1 failed`; the branch's own sources
were restored afterwards. The cause is outside this job's paths: on the smoke's 2-node graph the
layout puts one node per level at its disk's centre, so the two nodes differ only in `z`, and
`harness/sdk-smoke/layouts.mjs:28-31` checks `x` and `y` only (`lib.mjs:79` `boundsOf`).

**Pre-existing, not a gate row:** `cargo clippy -p graph-wasm --target wasm32-unknown-unknown
--all-targets -- -D warnings` fails on develop's `heap.rs:31,40,42` (unused `Geometric` and
`reserve_after` under `cfg(test)` on wasm32). This branch does not touch `heap.rs`.

## Decisions needed

1. **F-16, the ingest ceiling.** The reader has no byte, node or edge ceiling. Picking one is a
   product number, because the studio's source scales to 1M nodes. Once it is chosen, it becomes
   a new refusal code, appended.
2. **F-01, requiring `child_first`.** Version 1 reads an omitted member as `false`, and the
   senders rely on that: `harness/sdk-smoke/{build,force,post}.mjs` and the documented example
   both omit it. Making it required is an ingest version 2, with those senders migrated.
3. **Strength range at ingest (F-80, the ingest half).** A negative `strength` is now refused
   only where it is wrong, by the shortest-path centralities. Refusing it at ingest would change
   what `gm_build` accepts.
4. **Wiring `crates/graph-sdk-js/test/*.test.mjs` into an npm script.** This needs the root
   `package.json`, which is fingerprinted and outside this job's paths. Until then the ABI-version
   test runs only through the command above.
5. **`sdk:smoke`'s `layout.hierarchical3d` row**, red on develop: either the smoke check reads
   `z` for a 3D layout, or the smoke graph gets a level with two nodes. Its owner is `harness/` or
   graph-core.
