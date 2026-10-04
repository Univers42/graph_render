# fix-core-base — graph-core topology base (F-03, F-06, F-20…F-24, F-35…F-46)

Source: `docs/reviews/review-core-base.md`. Loop: `prompts/jobs/fix-common.md`.

**State on arrival.** The brief says the branch carries commit `cb124a1c` plus an uncommitted
edit to `ids.rs`. Neither is true of this worktree: `git status` is clean, and the F-03 /
F-20…F-24 / F-35…F-41 work sits in `cb124a1c`..`HEAD` (21 commits, `HEAD` = `39a7703a`).
What arrived was **tests only** — eight of them RED, and one of them
(`csr::tests::a_row_count_past_the_u32_index_space_is_the_capacity_error`) allocated 16 GiB
and was SIGKILLing the whole `--lib` binary, so nothing else in the crate could run. Every
row below is the state *after* this job.

| id | severity | verdict | test name | file:line | reason |
|---|---|---|---|---|---|
| F-03 | MAJOR | fixed | `csr_petgraph::tests::an_index_past_the_u32_space_panics_instead_of_wrapping` | `crates/graph-core/src/csr_petgraph.rs:235` | `from_index` narrowed `usize`→`u32` with `as`, so `2³²` read node 0; now `u32::try_from(i).expect("node index exceeds u32")` — the caller guarantees it (`node_bound()` is the node count, itself below `u32::MAX`) |
| F-06 | MAJOR | **false** (D10 waiver recorded) | `weights::tests::the_weights_do_not_depend_on_the_order_the_edges_arrive_in` | `crates/graph-core/src/weights.rs:1-17` | D10 binds *per-step kernels*; the degree pass is a single ingest walk that must run before indexing (dangling/duplicate/self-loop edges count as the oracle counts them), and the only CSR in the crate is built by `index_model`, which drops exactly those edges — a gather over it would move every weight. Quote and waiver in the module doc |
| F-20 | MAJOR | doc-only (tests already landed) | `index::view::tests::incident_on_the_empty_model_panics`, `…::node_at_the_node_count_panics`, `…::edge_at_the_edge_count_panics` | `crates/graph-core/src/index/view.rs:46,84,144` | The three accessors indexed the columns directly and panicked with no stated contract; each now carries the same `# Precondition` block as `parent`/`child`. No check added: `Option` would change every call site's shape on a public accessor |
| F-21 | MAJOR | fixed | `csr::tests::a_row_count_past_the_u32_index_space_is_the_capacity_error` | `crates/graph-core/src/csr.rs:57` | `vec![0u32; rows as usize + 1]` overflowed a 32-bit `usize` at `u32::MAX` and asked for 16 GiB natively; now `checked_add(1)` plus a 1 GiB `TABLE_BYTES_MAX` ceiling (`table_len`, counted in `u64` so both targets agree), refused before any allocation |
| F-22 | MAJOR | fixed | `csr::tests::a_second_pass_shorter_than_the_first_panics` | `crates/graph-core/src/csr.rs:57` | The placing walk trusted the iterator: a shorter second pass left holes read as edge 0, a longer one overwrote the next row. Now every write is bounds-checked and the placed count is compared with the counted total — `"a different sequence on the second pass"` |
| F-23 | MAJOR | fixed | `csr::tests::merging_an_unsorted_row_panics_in_a_debug_build`, `index::view::tests::out_and_inbound_rows_are_ascending_so_incident_merges_them` | `crates/graph-core/src/csr.rs:143` | `from_pairs` keeps arrival order (documented at `csr.rs:3-6`) and nothing enforced `Incident::merge`'s "each ascending"; a `debug_assert` on both rows now states it, with the precondition spelled out. The construction invariant is pinned on a 300-node model |
| F-24 | MAJOR | **false** (rejected by oracle parity) | `diff::tests::a_nan_strength_edge_is_updated_against_its_own_model`, `diff::tests::edges_equal_sees_every_compared_field_and_ignores_id` | `crates/graph-core/src/diff.rs:40` | IEEE `==` *is* the oracle's `===` (`eval.rs` diffs this), so a NaN strength is reported as updated on every run — non-idempotent, but identically so on both targets. `to_bits()` would make `-0.0 → 0.0` a patch the oracle does not emit. Policy already documented at `diff.rs:26-34` and pinned |
| F-35 | MINOR | fixed | `arena::tests::fnv1a_matches_the_published_64_bit_vectors` | `crates/graph-core/src/arena.rs:54` | `write_u32` inherited core's `to_ne_bytes`; `neighborhood.rs` keys two `IndexSet<u32>` on it, so a big-endian target would lay its tables out differently. Now `to_le_bytes` — the same four bytes on every little-endian target, so nothing moves. (`write_usize` left alone: no `FixedState` map in the crate has a `usize` key.) The test is vacuous on an LE host by construction; it pins the LE contract |
| F-36 | MINOR | fixed | `arena::tests::the_handle_past_usize_max_is_the_capacity_error` | `crates/graph-core/src/arena.rs:198` | `handle_at` did `u32::try_from(index + 1)`; at `usize::MAX` that add overflow-panics in a debug wasm32 build before the `try_from` could refuse. `checked_add(1)` now |
| F-37 | MINOR | fixed | `columns::tests::node_byte_len_counts_each_column_by_its_own_length`, `…::edge_byte_len_counts_each_column_by_its_own_length` | `crates/graph-core/src/columns.rs:99,160` | `byte_len` derived `n`/`m` from `id.len()` and multiplied, so a column set one push short was mis-sized. Each column is now counted by its own length. Identical total for any well-formed set, so the memory report does not move |
| F-38 | MINOR | fixed | `legend::tests::a_tag_past_the_degree_column_is_refused`, `…::a_tag_inside_the_degree_column_reads_its_own_degree` | `crates/graph-core/src/legend.rs:87` | `columns.degree[node as usize]` now goes through `.get(..).expect("every node has a degree: the columns are filled in one pass")`. The construction invariant is pinned by `index::view::tests::the_degree_column_holds_one_entry_per_node` |
| F-39 | MINOR | fixed | `legend::tests::a_member_count_past_the_u32_index_space_is_refused`, `…::a_member_count_inside_the_u32_index_space_is_written_as_is` | `crates/graph-core/src/legend.rs:80` | `nodes.len() as u32` → `wire_count`, a `u32::try_from(..).expect(..)`. Unreachable through `derive_legend` (a database's members are a slice of the `node_ids` row, and a node index is below `u32::MAX`), so the guard is pinned at the helper, as `handle_at(usize::MAX)` is |
| F-40 | MINOR | **false** (with `Ponytail:`, arrived on the branch) | `ids::tests::a_separator_inside_an_endpoint_collides_as_documented` | `crates/graph-core/src/ids.rs:76` | `make_edge_id` joins with `--` unescaped, so `("a--b","c")` and `("a","b--c")` share an id. Escaping it would change every emitted edge id, and edge ids are wire-visible; `eval.rs` differentials this against the oracle's `makeEdgeId`. Recorded `false` with two `Ponytail:` lines naming the collision, the direction and the escape hatch |
| F-41 | MINOR | **false** (same rejection as F-24) | `diff::tests::a_zero_changing_sign_is_no_change`, `index::tests::nodes_equal_ignores_id_and_compares_floats_like_js_strict_equality` | `crates/graph-core/src/index.rs:98` | The patch and the hashed topology disagree about `-0.0` only if `nodes_equal` uses `to_bits()`; using it would make `0.0 → -0.0` a patch the oracle's `===` does not produce |
| F-42 | MINOR | doc-only (variant **not** added) | `stage::tests::an_empty_input_is_a_valid_no_op` | `crates/graph-core/src/stage.rs:99` | Empty input is a legal document and the run is `Ok` with an empty topology and a 0-node snapshot. A `StageError::EmptyInput` would turn an exported `Ok` into an `Err` for every caller that ingests an empty record set (the wasm `build` export, the conformance fixtures) — a change to the public path, not an addition. Documented and pinned instead; see *decisions needed* |
| F-43 | MINOR | doc-only | `stage::tests::the_layout_label_is_the_word_the_caller_passed` | `crates/graph-core/src/stage.rs:107` | `id: &'static str` cannot be derived from the closure without a second type parameter, and both gate callers (hashgate, conformance) already pass the registry's `layout.id` beside the same `layout.run`. Documented on `run_with`, with `run_pipeline` named as the form that cannot mislabel; the test pins the difference rather than pretending it is checked |
| F-44 | MINOR | fixed | `synthetic::tests::picking_from_an_empty_table_is_refused`, `…::every_pick_of_every_reachable_table_is_inside_it` | `crates/graph-core/src/synthetic.rs:60` | `debug_assert!(len > 0)` before the multiply, so no draw is consumed. Cannot fire for any accepted count: every call site passes a table's own length or `count` |
| F-45 | MINOR | fixed (overflow) / **deferred** (clamp) | `synthetic::tests::a_count_past_the_oracles_cap_is_still_built_at_the_count_asked_for` | `crates/graph-core/src/synthetic.rs:135` | `count as usize * 2` wrapped a 32-bit `usize` past 2³¹ and panicked in the allocator; `count.saturating_mul(2) as usize` asks both targets for the same buffer. The `2..=100_000` clamp is **not** re-applied: `seeded_model` is public and the bench campaign hands it up to `registry::MAX_BENCH_NODES` = 10 × the cap, so clamping here would refuse hashed fixtures. See *decisions needed* |
| F-46 | MINOR | doc-only | `synthetic::tests::no_draw_lands_on_a_branch_boundary` | `crates/graph-core/src/synthetic/tests.rs:29` | The 1 000 000-draw bound is empirical (2.2× what the largest model consumes). `Ponytail:` line added naming the margin as measured, and a second test now sweeps for *all three* constants a `<` is written against (0.5, 0.6, 0.8) instead of the one the original sweep chose |

## Unverified items in these paths

None: `docs/reviews/review-core-base.md` lists no "unverified" row under
`crates/graph-core/src/{csr,csr_petgraph,index,diff,arena,columns,legend,ids,stage,synthetic,weights}.rs`.

## Commands

```
scripts/orch/gr cargo fmt --all --check                                  -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings    -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                    -> 0
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown-> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8          -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8 -> 1
scripts/scigraphs-conformance.sh                                         -> 0
```

### `cargo fmt --all --check`
```
FMT EXIT=0
```

### `cargo clippy --workspace --all-targets -- -D warnings`
```
    Checking graph-core v0.1.0 (/w/crates/graph-core)
    Compiling graph-cli v0.1.0 (/w/crates/graph-cli)
    Checking graph-wasm v0.1.0 (/w/crates/graph-wasm)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.04s
CLIPPY EXIT=0
```

### `cargo test --workspace --no-fail-fast` (filtered)
```
test result: ok. 432 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 89.24s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.45s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 306.77s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.57s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.76s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.71s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.26s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.80s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.96s
test result: ok. 185 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.93s
test result: ok. 1472 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 39.86s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 239.05s
test result: ok. 0 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
test result: ok. 180 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 36.64s
TEST EXIT=0
```
(`self-check FAILED: the settle is 112 ticks` appears twice inside that run: it is the
negative-control arm of a test that passes — both arms are printed and asserted.)

### `hashgate --seeds 8`
```
  post.separate.grid: 4-way equal on 8/8 seeds
  transport.wasm.columnar: 4-way equal on 8/8 seeds
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 8/8 seeds
PASS
HASHGATE EXIT=0
```

### `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` (negative control)
```
  transport.wasm.columnar: the real ABI matched layout.grid on 8/8 seeds
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
NEGCTL EXIT=1
```

### `scripts/scigraphs-conformance.sh`
```
  GRAPHVIZ_PATCHWORK: ok — 399 f64, 831 f32 of 1020 coordinates, median 3.450e-16 <= 1.000e-15
  SUGIYAMA: ok — 597 f64, 1020 f32 of 1020 coordinates, median 1.259e-16 <= 1.000e-15
  CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15
PASS
CONFORMANCE EXIT=0
```
No conformance row moved.

## Decisions needed

1. **F-45 clamp — recommendation: do not clamp `synthetic_records`.** `seeded_model` is
   `pub` and `crates/graph-cli/src/bench/scale.rs:38` sets
   `MAX_SCALE_NODES = registry::MAX_BENCH_NODES = 10 × MAX_SYNTHETIC_NODES`, so a
   `2..=100_000` clamp inside `synthetic_records` would refuse the bench campaign's own
   fixtures — hashed ones. Only the wasm32 `usize` overflow was fixed. If a cap is wanted,
   it belongs on `seeded_model`'s callers, not on the generator.
2. **F-42 `StageError::EmptyInput` — recommendation: keep it a documented no-op.** The
   variant is additive in itself, but *returning* it turns an exported `Ok` into an `Err`
   for an empty record set (wasm `build`, conformance fixtures), which fix-common treats as
   a change to a public path.
3. **`.cargo/mutants.toml:31` is now one word stale** (out of this job's paths, so not
   edited): it says "the 100 000-node model, the largest", and F-45's new test builds
   100 001 to prove no clamp. The `exclude_re` entry itself stays correct.
4. **F-24 / F-41 stay rejected** unless the review is re-opened: IEEE `==` is the oracle's
   `===`, and `eval.rs` differentials the Rust diff against it.

## Deviations

- `crates/graph-core/src/arena/tests.rs` and `crates/graph-core/src/synthetic/tests.rs` are
  new: both files passed the 300-line limit once the tests for F-35/F-36 and F-44…F-46 were
  added, and the house rule is to split into a child module (the `csr.rs` + `csr/append/tests.rs`
  shape). Test names are unchanged (`arena::tests::*`, `synthetic::tests::*`).
- No file outside the brief's path list was edited, except this report.