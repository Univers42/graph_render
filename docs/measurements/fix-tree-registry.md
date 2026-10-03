# fix-tree-registry: the layout registry's ceilings and its index order (2026-10-03)

Branch `fix-tree-registry`, worktree `$GM_SCRATCH/wt/fix-tree-registry`. Review:
`docs/reviews/review-layout-tree.md` (L ids). All four findings are `registry/three_d.rs`,
`registry/radial.rs`, `registry/hierarchy.rs` and `registry.rs`; the only file touched outside
`crates/graph-core/src/` is the `bench` constant's own (`crates/graph-cli/src/bench/scale.rs`),
because L-27 moves it.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| L-25 | MINOR | fixed: doc-only — the ceiling's doc now names the basis it really has: the `bench` node cap, a timing on **four** of its six rows (`sphere`/`helix`/`cube`/`hierarchical3d` at 1 M, block pasted), and the honest limit that **no** 3D row has a bytes-per-node figure (`memory.rs` has no 3D arm), so the memory half of the argument is borrowed from the 2D rows. A per-row memory ceiling for `HIERARCHICAL_3D` is **deferred**: it needs a `memory.rs` 3D arm, which is outside this job's paths. | none (doc-only) | `crates/graph-core/src/registry/three_d.rs:35-93` (const at `:94`, was `:17-75`) |
| L-26 | MINOR | fixed: doc-only — `HIERARCHY_LAYOUT_CEILING`'s doc says one constant backs **four** rows, names which three `memory.rs` measures, and names `layout.circular.hierarchy` as the fourth, riding the 933 B tidy-tree figure with the reason one ceiling can stand for four and what it gets wrong. | none (doc-only) | `crates/graph-core/src/registry/hierarchy.rs:9-38` (const at `:39`) |
| L-27 | MINOR | fixed: both ceilings now derive from one constant the motor owns, `registry::bench_cap::MAX_BENCH_NODES`, and `bench::scale::MAX_SCALE_NODES` derives from it in return — graph-cli depends on graph-core, never the reverse, and `graph-cli` is bin-only so the other order is not expressible. Values unchanged (1 000 000). The test cannot fail pre-fix (the defect is two literals that agree, not a wrong number), so its negative control is a deliberate drift. | `registry::tests::the_radial_and_basic_3d_ceilings_are_the_one_bench_node_cap` | `registry/bench_cap.rs:26`, `registry/radial.rs:31`, `registry/three_d.rs:94`, `crates/graph-cli/src/bench/scale.rs:38` |
| L-28 | MINOR | fixed: the ids at the index-keyed front of `LAYOUTS` are asserted by name. Only indices 0..=4 are pinned, because those are the whole dependency set (`LAYOUTS[0].run` in `graph-wasm/src/handle.rs:184` and `post/tests.rs:201`, `LAYOUTS[3]` in `bench/campaign.rs:128`) and a later append must not have to edit a file its own job cannot touch. Also corrected the block comment's dead citation `build.rs:166` (that line is `insert()`'s `0`; the positional lookup is `:143`). | `registry::tests::the_index_keyed_front_of_layouts_still_holds_the_ids_their_callers_name` | `crates/graph-core/src/registry/tests.rs:44-62` (test at `:58`), comment at `registry.rs:228-232` |

## RED → GREEN (last lines, filtered runs)

- L-27 RED — the test cannot name the constant that does not exist yet:

```
error[E0425]: cannot find value `MAX_BENCH_NODES` in this scope
  --> crates/graph-core/src/registry/tests.rs:82:42
```

  GREEN: `test registry::tests::the_radial_and_basic_3d_ceilings_are_the_one_bench_node_cap ... ok`
  Negative control (`RADIAL_CEILING = MAX_BENCH_NODES as u64 + 1`, reverted):

```
test registry::tests::the_radial_and_basic_3d_ceilings_are_the_one_bench_node_cap ... FAILED
  left: 1000001
 right: 1000000
```

- L-28 RED — `LAYOUTS[1]` and `LAYOUTS[2]` swapped locally (compiles, length still 39), then reverted:

```
test registry::tests::the_index_keyed_front_of_layouts_still_holds_the_ids_their_callers_name ... FAILED
  left: ["layout.grid", "layout.treemap.squarified", "layout.tree.tidy", "layout.circular.radial", "layout.packing.circle"]
 right: ["layout.grid", "layout.tree.tidy", "layout.treemap.squarified", "layout.circular.radial", "layout.packing.circle"]
```

  GREEN after the revert: the same test `... ok`.

- GREEN, both together: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1258 filtered out`.

## No output moved

No `Metadata` field value changed: only doc comments on constants and two derivations whose
results are the same `u64`. `hashgate --seeds 8` is exit 0, so no registered layout's digest moved,
and the ledger's `scale_ceiling` column is byte-identical.

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` (control) | 1 | `FAIL: 8 of 8 seeds diverge` |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | 1 | `capabilities --check: 73 rows, 36 problems` |
| `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 | `up to date  docs/contract/ingest-schema.json` |

`capabilities --check`'s 36 problems are **not** these rows' and are pre-existing on this tree:
every one is `gated, but no <record>: run the gate` or `gated, but hashgate ran 8 seeds, need
1000`, i.e. a missing gate artefact for the first 16 registered stages plus
`transport.wasm.columnar`. Filtered to the rows this job touches: `layout.twopi`,
`layout.hierarchical3d`, `layout.circular.hierarchy`, `layout.basic3d.*`,
`layout.bipartite_3d` produce **no** problem in either run; `layout.tree.tidy`,
`layout.treemap.squarified` and `layout.circular.radial` carry only the two classes above,
which the `--seeds 8` this job mandates is what turns "no hashgate record" into "ran 8 seeds,
need 1000". Their `oracle-layouts` / `roundtrip` records come from the orchestrator's timed
gate, not from this job.

## Merge floor

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished \`dev\` profile` |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0, then **101** — see below | `test result: ok. 1256 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished \`dev\` profile` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` |

**The workspace test's second run exited 101, and every one of its 10 failing targets is one
root cause, not a code defect.** `graph-cli` refuses to record evidence built from a different
tree — `graph-cli was built from tree 9ff6a5dfa48b… but the tree is now 83153ba01a54…: rebuild
before recording` — and the orchestrator committed twice (`bf63a28`, `8ccc60c`) while this job
ran, so every `graph-cli` binary the run had cached was stamped with a superseded tree hash. The
symptom in the tests that only compare exit codes is `left: Some(2), right: Some(0)`: `2` is that
refusal, not a gate going red. After one rebuild on the final HEAD every one of those targets is
green:

| target | exit | last line |
|---|---|---|
| `cargo test -p graph-cli --bin graph-cli` | 0 | `345 passed; 0 failed` |
| `cargo test -p graph-cli --test cli --test cli_fa2` | 0 | `8 passed; 0 failed` / `3 passed; 0 failed` |
| `cargo test -p graph-cli --test cli_force --test cli_force_gate --test cli_igraph --test cli_ledger --test cli_oracles --test cli_p3 --test snapshot` | 0 | `4 passed; 0 failed` (each of the seven) |

So every workspace target has been observed green on the final tree: the ten that passed in the
`--no-fail-fast` run, plus those ten after the rebuild. `graph-core`'s lib alone is
`1256 passed; 0 failed; 8 ignored` on the final HEAD.

**Consequence for the orchestrator:** a `cargo test --workspace` run whose binary cache predates
the last commit will report these ten targets red on this worktree. Rebuild before gating.

## Decisions taken

- **L-27 direction.** `graph-core` cannot read `bench::scale::MAX_SCALE_NODES`: `graph-cli`
  depends on `graph-core` (`crates/graph-cli/Cargo.toml:13`) and has no `[lib]` target, so the
  reverse is not expressible. The job body's second option — a `pub const` the motor owns that
  `bench` also reads — is the one taken, so `bench/scale.rs` changed to a one-line derivation.
  The constant went into a new child module `registry/bench_cap.rs` rather than into
  `registry.rs` because that file is already over the house line cap (see below) and
  `radial.rs` / `three_d.rs` must not depend on each other to reach it.
- **L-28 assertion form.** The finding offered `const _: () = assert!(...)` first; that is not
  expressible here. `LAYOUTS` is a `static`, and a const context cannot read a static's element,
  and its element type `Capability` (`registry/capability.rs:32-41`) is a struct of `&'static str`,
  a fn pointer and `Metadata` — no discriminant to assert on. The finding's second option (a
  test) is what ships, and it is the one that fails, as the RED above shows.
- **L-28 scope.** Indices 0..=4 only, not all 39. Pinning every index would make each later
  append (three such jobs are queued) depend on `registry/tests.rs`, which their own path lists
  do not include. The rows above 4 have no caller that names them by index: the wasm ABI's
  `layout_id` is positional but the SDK resolves a string id by scanning at init
  (`graph-wasm/src/exports/build.rs:26-32`).
- **L-25 rung.** Widened the doc rather than giving `HIERARCHICAL_3D` its own ceiling: no
  bytes-per-node figure for any 3D layout exists anywhere in the tree, so a new ceiling would be
  another unmeasured number. The widening also corrects the module doc's stale "six rows" count.

## Deviations / notes for the orchestrator

- `crates/graph-core/src/registry.rs` is **304** lines after this job and was **301** before it —
  over the 300-line house cap on arrival. The three added lines are the `mod bench_cap;` and
  `pub use bench_cap::MAX_BENCH_NODES;` pair plus one comment line. The fix is to move `LAYOUTS`
  into its own child module, which is not this job's path list: recommend a follow-up job.
- `crates/graph-core/src/registry/tests.rs` is this crate's `#[cfg(test)]` module for `registry.rs`
  (declared at `registry.rs:301`), i.e. the test file beside `registry.rs`, and holds both new
  tests. `fix-common` allows new test files beside the listed paths; this one already existed.
- `registry/three_d/graph.rs` (the `HIERARCHICAL_3D` row) is **not** in this job's path list, so
  its own `ponytail` still carries no `scale_ceiling` disclosure. L-25's fix therefore lives in
  the shared constant's doc, which is where the review proposed putting it; a per-row pointer in
  `graph.rs` would be the tidier follow-up.
- `bench/campaign.rs:126-127`'s doc calls `LAYOUTS[3]` "the force layout"; it is
  `layout.circular.radial`. Outside this job's paths, so left alone — worth a one-line fix.