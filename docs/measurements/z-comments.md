# z-comments — the stale "no 3D layout" comments

Source: `prompts/CONTINUE.md` §4 item 8 and `docs/reports/STATUS.md` §5 (the
`exercise/z.rs:5-6` line). Comments and doc comments only; no code and no test changed.

## What is true now

Seven 3D layouts are in `graph_core::registry::LAYOUTS`, appended in index order and never
inserted (`crates/graph-core/src/registry.rs:231-281`):

| id | registry.rs | own `hashgate` control |
|---|---|---|
| `layout.basic3d.sphere` | `:231` | `GM_MUTATE_BASIC3D_SPHERE_NODES` |
| `layout.basic3d.helix` | `:236` | `GM_MUTATE_BASIC3D_HELIX_NODES` |
| `layout.basic3d.cube` | `:241` | `GM_MUTATE_BASIC3D_CUBE_NODES` |
| `layout.hierarchical3d` | `:246` | `GM_MUTATE_HIERARCHICAL3D_NODES` |
| `layout.force.spring3d` | `:251` | `GM_MUTATE_FORCE_SPRING3D_NODES` |
| `layout.bipartite_3d` | `:271` | none |
| `layout.basic3d.spiral` | `:279` | none |

The five controls are `hashgate::knobs::THREE_D_LAYOUT_STAGES`
(`crates/graph-cli/src/hashgate/knobs.rs:127-151`), listed in the twin knob list at
`crates/graph-cli/tests/common/mod.rs:23-27`. `layout.bipartite_3d` and
`layout.basic3d.spiral` have no entry there; that gap is review finding S3-1
(`docs/reviews/rv-sg-spiral3d.md:20`) and belongs to a knob job, not this one.

`snapshot_cmd::roundtrip` already sweeps all of them: `swept_layouts()`
(`roundtrip.rs:54`) is built from `registry::LAYOUTS` itself, so a layout registered there is
swept without being listed again.

Two premises that survive, and so were kept:

- `graph-contract` cannot run a layout at all — it depends on no `graph-core`
  (`crates/graph-contract/Cargo.toml`, whose comment names the closed allow-list, and zero
  `graph_core` imports under `src/`). Its 3D knowledge stops at "a snapshot has a z column":
  `Dim::D3` is inferred from `z.is_some()` (`binary.rs:125`) and the 0.4 label is required iff
  3D (`binary.rs:229`). So the hand-built 3D fixtures there are still necessary, for a
  different reason than the one they gave.
- A hand-built *malformed* 3D snapshot is still needed in `roundtrip`: a registered 3D layout
  emits a valid z column or it is a bug, so the wrong-length and 0.3-labelled cases can only
  be had by hand. That is what keeps `z_refusal_faults` a property of the reader.

## Rows

| id | site | severity | verdict | evidence | file:line |
|---|---|---|---|---|---|
| Z-1 | `exercise.rs` module doc | MINOR | doc-only | `swept_layouts()` sweeps the 3D layouts; the exercise's 3D seeds now add node/edge-kind and float torture in a z column, which is what the last clause now says | `crates/graph-cli/src/snapshot_cmd/exercise.rs:4-8` |
| Z-2 | `z.rs` module doc | MINOR | doc-only | header now says `GM_MUTATE_NODE_Z` stays on `roundtrip` and is not a `Knob`, because the five `THREE_D_LAYOUT_STAGES` controls move layouts and this one does not | `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:3-13` |
| Z-3 | `z.rs` `Ponytail:` on `perturb_z` | MINOR | doc-only | escape hatch "when the first 3D layout is registered" has fired; replaced by one naming what the control does not cover (too-few case, layout-produced snapshots, the JSON face) | `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:31-37` |
| Z-4 | `z.rs` `z_refusal_faults` doc | MINOR | doc-only | "because no 3D layout produces one" rewritten as the reason that still holds: a layout emits a valid z column or it is a bug | `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:66-72` |
| Z-5 | `roundtrip.rs` inline comment | MINOR | doc-only | same rewrite as Z-4 at the sweep site | `crates/graph-cli/src/snapshot_cmd/roundtrip.rs:168-173` |
| Z-6 | `canonical_json/tests.rs` `spaced` doc | MINOR | doc-only | "no 3D layout exists yet" replaced by the real reason: this crate cannot call one | `crates/graph-contract/src/canonical_json/tests.rs:29-31` |
| Z-7 | `canonical_json/tests/dim.rs` doc + `spaced` doc | MINOR | doc-only | same, in the two places dim.rs says it (file header and the test's own doc) | `crates/graph-contract/src/canonical_json/tests/dim.rs:4-7`, `:11-12` |

No row is `deferred`. Nothing here needed a test: every row is a comment that was false, and
the comment-only diff cannot move a byte of output (see the diff check below).

## The grep

Before, over the whole tree — the seven source sites plus four prose sites:

```
crates/graph-cli/src/snapshot_cmd/exercise.rs:5
crates/graph-cli/src/snapshot_cmd/exercise/z.rs:4,25,59
crates/graph-cli/src/snapshot_cmd/roundtrip.rs:168
crates/graph-contract/src/canonical_json/tests.rs:30
crates/graph-contract/src/canonical_json/tests/dim.rs:10
docs/reports/STATUS.md:257
prompts/CONTINUE.md:169
prompts/jobs/p13-3d.md:1,25
prompts/jobs/z-comments.md:1,8,26
```

After, over `crates/` — the scope this job's paths cover — **empty**:

```
$ git grep -n 'no 3D layout' -- crates
$ echo $?
1
```

After, over the whole tree:

```
$ git grep -n 'no 3D layout'
docs/reports/STATUS.md:257:- `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:5-6` still says "no 3D layout is registered";
prompts/CONTINUE.md:169:   "no 3D layout is registered" comment in `crates/graph-cli/src/snapshot_cmd/exercise/z.rs:5-6`,
prompts/jobs/p13-3d.md:1:# Job p13-3d (agent build, the 3D contract change; no 3D layout yet)
prompts/jobs/p13-3d.md:25:   snapshot in a test, since no 3D layout exists yet.
prompts/jobs/z-comments.md:1:# Job z-comments (agent build: stale "no 3D layout" docs, comments only)
prompts/jobs/z-comments.md:8:`crates/graph-cli/tests/common/mod.rs:23-27`). Seven comments still say no 3D layout exists:
prompts/jobs/z-comments.md:26:No code change, no test change. `git grep -n 'no 3D layout'` must return nothing at the end; paste it.
```

All five remaining files are outside this job's paths (`prompts/`, `docs/reports/STATUS.md`),
and three of them are this job's own body and the job that wrote it. A whole-tree empty grep
is therefore not reachable from inside the listed paths; reported under "decisions needed"
rather than fixed by editing a file this job does not own. Of the five, only
`docs/reports/STATUS.md:257` states a finding that this change has now closed.

## Commands

| command | exit | last lines |
|---|---|---|
| `git grep -n 'no 3D layout' -- crates` | 1 | empty (no match) |
| `scripts/orch/gr cargo fmt --all --check` | 0 | no output |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 7.69s` |
| `scripts/orch/gr cargo test -p graph-contract --lib` | 0 | `test result: ok. 104 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `scripts/orch/gr cargo test -p graph-cli` | 0 | ten suites, `ok` — 332 / 8 / 3 / 4 / 4 / 3 / 5 / 4 / 4 / 3 passed, 0 failed |
| `scripts/orch/gr cargo test -p graph-cli --bin graph-cli -- snapshot_cmd::` | 0 | `test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 292 filtered out` (re-run after the final comment reflow) |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 3.99s` |
| `scripts/scigraphs-conformance.sh` | 1 | 30 rows `ok`, two `FAIL` — see below |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | SIGTERM | killed at the 30-minute cap; superseded by the per-crate runs above |

### Conformance, and the diff check

`scripts/scigraphs-conformance.sh` exits 1 with two rows failing on their pinned reference
bytes and 30 rows `ok`:

```
YIFAN_HU:      FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74...)
GRAPHVIZ_SFDP: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74...)
FAIL
```

Both are *reference bytes*, not motor output: the script hashes the engine's own rendering
before comparing, so the fault is the pinned hash of the engine in this image, not this
tree's geometry. They are not re-pinned here — a fix job never re-pins a conformance row.
The diff is comment-only, which is the check that rules this job out as the cause:

```
$ git diff --stat
 crates/graph-cli/src/snapshot_cmd/exercise.rs      |  5 +++-
 crates/graph-cli/src/snapshot_cmd/exercise/z.rs    | 30 ++++++++++++++--------
 crates/graph-cli/src/snapshot_cmd/roundtrip.rs     | 10 +++++---
 crates/graph-contract/src/canonical_json/tests.rs  |  3 ++-
 .../graph-contract/src/canonical_json/tests/dim.rs |  9 ++++---
 5 files changed, 37 insertions(+), 20 deletions(-)
```

Five files, 37 insertions, 20 deletions, every changed line a `//`, `///` or `//!` comment:
no expression, no binding and no test moved.

### Also observed, not mine

`cargo test -p graph-contract --lib` emits one warning on a tree this change did not touch:
`crates/graph-contract/src/ingest/tests.rs:7` `mod support;` is an unused import
(`crates/graph-contract/src/ingest/tests/support.rs:3`, `use super::support::*;`). It is
pre-existing and `clippy --all-targets -D warnings` does not see it, so it does not reach the
merge floor. Left alone: `ingest/` is not in this job's paths.