# Job z-comments (agent build: stale "no 3D layout" docs, comments only)

Read `prompts/jobs/fix-common.md` first. Source: `prompts/CONTINUE.md` §4 item 8 and
`docs/reports/STATUS.md` §5.

Five 3D layouts are registered now (`crates/graph-core/src/registry.rs`, `basic_3d::{sphere,helix,cube}`
and `hierarchical_3d`), and each has its own hashgate knob (`hashgate::knobs::THREE_D_LAYOUT_STAGES`,
`crates/graph-cli/tests/common/mod.rs:23-27`). Seven comments still say no 3D layout exists:

```
crates/graph-cli/src/snapshot_cmd/exercise.rs:5
crates/graph-cli/src/snapshot_cmd/exercise/z.rs:4,25,59
crates/graph-cli/src/snapshot_cmd/roundtrip.rs:168
crates/graph-contract/src/canonical_json/tests.rs:30
crates/graph-contract/src/canonical_json/tests/dim.rs:10
```

Rewrite each to what is true now. `GM_MUTATE_NODE_Z` stays on `roundtrip`: it controls the contract
reader's refusal of a wrong-length z column (`docs/decisions/contract-3d.md` §4.3), a property of
the wire face, not of a layout stage. Say that in `z.rs`'s header and replace the `Ponytail:` line at
:23-29, whose escape hatch ("when the first 3D layout is registered…") has already fired, with one
that names what the control does not cover. Where a comment explains why a 3D snapshot is built by
hand, keep the reason only if it still holds (a hand-built snapshot isolates the reader from a
layout), else delete the clause.

No code change, no test change. `git grep -n 'no 3D layout'` must return nothing at the end; paste it.

Paths: the seven files above, comments and doc comments only;
`docs/measurements/z-comments.md` (the before/after grep and the merge floor's last lines).

Done when: fix-common's done-when; the grep is empty.
