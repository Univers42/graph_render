# split-300 — every Rust file back under the 300-line house limit

Date: **2026-10-04**. Tree: **43ab3af6**. Job: `prompts/jobs/split-300.md`.
A pure move: no behaviour, no output, no public path changed.

The three `crates/graph-cli/src/hashgate/` files the job body named are split. Every other file
the original brief listed was already under 300 at the baseline commit (see *What was already
split*). Two files repo-wide are still over 300 and both are skipped (see *Skips*).

## BEFORE

The brief's table, measured on develop 2f413858, plus the three hashgate files as they stood at
this job's baseline commit **5ca23e79** (the job resumed after ux-params-abi un-skipped
`hashgate/`, which is why the brief's own table did not list them).

| Lines | File | In scope |
|---|---|---|
| 437 | `crates/graph-core/src/post/tests.rs` | already split by 220dc783 |
| 398 | `crates/graph-core/src/layout/graphviz/dot/rank_fixture_edges.rs` | **skip** (fixture, no seam) |
| 394 | `crates/graph-core/src/layout/graphviz/osage/tests.rs` | already split by 220dc783 |
| 325 | `crates/graph-core/src/layout/graphviz/circo/tests.rs` | already split by 220dc783 |
| 325 | `crates/graph-cli/src/hashgate/tests/knob/controls.rs` | **split here** |
| 311 | `crates/graph-cli/src/snapshot_cmd/tests.rs` | already split by 220dc783 |
| 310 | `crates/graph-core/src/layout/graphviz/neato/solve.rs` | already split by 220dc783 |
| 309 | `crates/graph-cli/src/hashgate/knob.rs` | **split here** |
| 305 | `crates/graph-cli/src/hashgate/knobs.rs` | **split here** |

Measured `git ls-tree` top-12 at 5ca23e79, for the record:

```
 398  crates/graph-core/src/layout/graphviz/dot/rank_fixture_edges.rs
 325  crates/graph-cli/src/hashgate/tests/knob/controls.rs
 319  crates/graph-cli/src/oracle_python/cli.rs
 309  crates/graph-cli/src/hashgate/knob.rs
 308  crates/graph-wasm/src/exports/build.rs
 305  crates/graph-cli/src/hashgate/knobs.rs
 300  crates/graph-core/tests/geometry_invariants.rs
 300  crates/graph-core/src/registry/layouts.rs
 300  crates/graph-core/src/layout/force/session/tests/carry.rs
 299  crates/graph-core/src/layout/circle_packing/fallback/seed/tests.rs
 299  crates/graph-cli/src/oracle_python/conformance/motor/tests.rs
 299  crates/graph-cli/src/hashgate/tests/knob/table.rs
```

## AFTER

`git ls-files 'crates/*.rs' | xargs wc -l | sort -rn | head -n 12` on 43ab3af6:

```
 398  crates/graph-core/src/layout/graphviz/dot/rank_fixture_edges.rs   <- skip, no seam
 319  crates/graph-cli/src/oracle_python/cli.rs                        <- skip, merge-p12-t4b-2
 308  crates/graph-wasm/src/exports/build.rs                           <- skip, graph-render-4f
 300  crates/graph-core/tests/geometry_invariants.rs
 300  crates/graph-core/src/registry/layouts.rs
 300  crates/graph-core/src/layout/force/session/tests/carry.rs
 300  crates/graph-cli/src/hashgate/knob/kind.rs                       <- new
 299  crates/graph-core/src/layout/circle_packing/fallback/seed/tests.rs
 299  crates/graph-cli/src/oracle_python/conformance/motor/tests.rs
 299  crates/graph-cli/src/hashgate/tests/knob/table.rs
 297  crates/graph-core/src/post/routed.rs
```

The three splits, each along a seam the file already had:

| Before | After | New child | Seam |
|---|---|---|---|
| `tests/knob/controls.rs` 325 | 262 | `tests/knob/controls/rescale.rs` 65 | the two `GM_MUTATE_SPLIT_RESCALE` tests, one subject (the threaded rescale merge), with `TIER_SEED`/`RING`/`SPIRAL` |
| `knobs.rs` 305 | 252 | `knobs/three_d.rs` 57 | the `THREE_D_LAYOUT_STAGES` table, the largest of the three stage tables |
| `knob.rs` 309 | 19 | `knob/kind.rs` 300 | the `Knob` enum itself |

`knobs::THREE_D_LAYOUT_STAGES` and `knob::Knob` are `pub use`d from their parents, so every
existing path (`knobs::THREE_D_LAYOUT_STAGES`, `hashgate::knob::Knob`, `super::Knob`) still
resolves unchanged. `knob/kind.rs` carries four `#[cfg(doc)]` imports so that every intra-doc
link in the moved comment resolves from its new module; nothing else in it needed an import,
because the enum is entirely fieldless unit variants.

## Proof nothing moved

Test counts, `cargo test --workspace -- --list | grep -c ': test$'`:

| | count |
|---|---|
| BEFORE (5ca23e79) | **2287** |
| AFTER (43ab3af6) | **2287** |

`hashgate --seeds 8`, both exit 0 and both `PASS`. The two outputs are **byte-identical**
(`diff` clean over all 70 lines), not merely equal in verdict:

```
hashgate: wasm artifact .../graph_wasm.wasm sha256 7fc6975ff159e6e8b9666ffb7d444a49c676985e0a8c4f6532dd3bf8b2da8b2d
  native run 1  digest 66cf59fc98595e0d9cb294ffeeae5ae95d47e123102d32338becf2acb58a27d4
  native run 2  digest 66cf59fc98595e0d9cb294ffeeae5ae95d47e123102d32338becf2acb58a27d4
  wasm32 run 1  digest 66cf59fc98595e0d9cb294ffeeae5ae95d47e123102d32338becf2acb58a27d4
  wasm32 run 2  digest 66cf59fc98595e0d9cb294ffeeae5ae95d47e123102d32338becf2acb58a27d4
```

Those five lines are the whole of the hash-bearing output, and they are identical before and
after. Artifacts: `target/split-before-hashgate.txt`, `target/split-after-hashgate.txt`.

The `GM_MUTATE_REFERENCE_DEGREE=9` control is **non-zero**: rc 1, `FAIL: 8 of 8 seeds diverge`,
so the gate is still reading the control rather than passing vacuously.

## Commands and exit codes

| command | rc |
|---|---|
| `gr cargo test --workspace -- --list \| grep -c ': test$'` (after) | 0 (2287) |
| `gr cargo run -q -p graph-cli -- hashgate --seeds 8` (after) | 0 |
| `gr cargo test --workspace -- --list \| grep -c ': test$'` (before) | 0 (2287) |
| `gr cargo run -q -p graph-cli -- hashgate --seeds 8` (before) | 0 |
| `gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | **1** (expected non-zero) |
| `gr cargo run -q -p graph-cli -- codegen --check` | 0 |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `gr cargo fmt --all --check` | 0 |
| `gr -e RUSTDOCFLAGS='-D warnings' cargo doc -p graph-cli --no-deps --document-private-items` | **101** (see below) |
| `scripts/scigraphs-conformance.sh` | 0 |

### `cargo doc` is red, and was reder before this job

`cargo doc -p graph-cli --no-deps` with `-D warnings` **fails on this tree, and failed before
this job too**: 21 rustdoc errors at 5ca23e79, **19** now. This job's move *reduced* the count
by two and introduced none — the `SeparateParams::over_relaxation` link started resolving once
`kind.rs` gained its `#[cfg(doc)] use`, and the orchestrator retargeted the other one (see
*Deviations*). Every remaining lint is pre-existing and lives in a file this job does not own.
Recorded, not fixed:

| file:line | lint |
|---|---|
| `crates/graph-cli/src/hashgate/knob/setting.rs:164` | unresolved link to `setting` |
| `crates/graph-cli/src/hashgate/knob/setting.rs:170` | unresolved link to `setting` |
| `crates/graph-cli/src/hashgate/knob/setting.rs:195` | unresolved link to `setting` |
| `crates/graph-cli/src/hashgate/knobs.rs:123` | unresolved link to `stage_bytes_from_own_model` |
| `crates/graph-cli/src/hashgate/knobs.rs:240` | `super::stages` is both a function and a module |
| `crates/graph-cli/src/hashgate/tier.rs:116` | unresolved link to `super::threaded_bytes` |
| `crates/graph-cli/src/mb_fidelity.rs:28` | unresolved link to `SELF_CHECK_TOLERANCE` |
| `crates/graph-cli/src/oracle_python/conformance/baseline/table.rs:36` | unresolved link to `sc_propose.classify` / `super::ROWS` |
| `crates/graph-cli/src/oracle_python/conformance/baseline/table.rs:71` | unresolved link to `super::ROWS` |
| `crates/graph-cli/src/oracle_python/conformance/motor/gv_post.rs:53` | unresolved link to `tests::the_mean_of_an_n_by_2_array_is_the_left_to_right_sum` |
| `crates/graph-cli/src/bench/tick.rs:13` | redundant explicit link target |
| `crates/graph-cli/src/forcecheck/native.rs:2` | redundant explicit link target |
| `crates/graph-cli/src/evidence.rs:11` | `write` is both a function and a macro |
| `crates/graph-cli/src/evidence.rs:133` | `write` is both a function and a macro |
| `crates/graph-cli/src/oracle_python/igraph.rs:11` | `super::judge` is both a function and a module |
| `crates/graph-cli/src/oracle_python/scale.rs:76` | `line` is both a function and a macro |
| `crates/graph-cli/src/overlap_cmd.rs:23` | `measure` is both a function and a module |

17 locations, 18 messages (`table.rs:36` carries two). Both `knobs.rs` lints are inherited
line-for-line from doc comments that did **not** move — the `IGRAPH_LAYOUT_STAGES` comment
(`stage_bytes_from_own_model`, `knobs.rs:176` at 5ca23e79) and the `apply` comment
(`super::judge`, `knobs.rs:293`); both sit 53 lines lower now, exactly the height of the
`THREE_D_LAYOUT_STAGES` block that moved out. Neither is introduced here. Cleaning them is a
separate job: it needs edits to eight files outside this one's paths.

## Skips

- **`crates/graph-core/src/layout/graphviz/dot/rank_fixture_edges.rs` (398) — left as is.**
  The whole file is one item: a single `pub const FIXTURE_EDGES: &[(u32, &[(u32, u32)])]`
  holding all twenty seeds, plus four lines of module doc. There is no per-fixture seam to cut
  on, and the one consumer takes it as a unit — `rank_tests.rs:135-136` asserts
  `FIXTURE_EDGES.len() == FIXTURES.len()` and then zips the two, so splitting the const would
  mean changing the test's contract, not moving code. The brief allows exactly this: *"split it
  only if it splits along a seam it has (one fixture per child); otherwise leave it and say so."*
- **`crates/graph-wasm/src/exports/build.rs` (308) — owned by graph-render-4f.**
- **`crates/graph-cli/src/oracle_python/cli.rs` — owned by merge-p12-t4b-2**, which brings it to
  277 lines. It is still 319 on this tree because that merge has not landed here yet.

## What was already split

Commit 220dc783, verified as moves-only, had already taken the brief's other five files under
the limit before this job resumed. Their sizes at 5ca23e79:

| Lines | File |
|---|---|
| 292 | `crates/graph-core/src/layout/graphviz/neato/solve.rs` |
| 270 | `crates/graph-core/src/post/tests.rs` |
| 260 | `crates/graph-cli/src/snapshot_cmd/tests.rs` |
| 236 | `crates/graph-core/src/layout/graphviz/circo/tests.rs` |
| 192 | `crates/graph-core/src/layout/graphviz/osage/tests.rs` |

## Deviations

- **`crates/graph-cli/src/hashgate/knob/kind.rs:153`** — the orchestrator retargeted this job's
  `[`FruchtermanReingold::ID`](crate::layout::force::FruchtermanReingold)` to
  `graph_core::layout::force::FruchtermanReingold`, in commit 43ab3af6. The job body said *doc
  text unchanged*, and this job left it alone; the link was already broken on 5ca23e79
  (`graph-cli`'s `main.rs` declares no `layout` module) and was one of the 21 errors above. The
  retarget fixes it and is outside this job's mandate, so it is recorded rather than claimed.
- **`git checkout`/`git clean` are denied by the harness**, so the BEFORE measurement was taken
  by reconstructing the 5ca23e79 tree in place with read-only `git show` plus `mv` of the three
  new children aside, then restoring from `HEAD` the same way. `git status --short` prints
  nothing afterwards and `git diff --quiet` confirms the tree is clean, which is the check the
  job body asked for.
- The orchestrator committed this job's working tree mid-run as 7b0b96b0. That commit contains
  exactly the six split files and nothing else (`git diff --name-status 5ca23e79 7b0b96b0`), so
  the BEFORE/AFTER comparison is not confounded by anything else landing.
