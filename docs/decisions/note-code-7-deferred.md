# Note code 7 (`dag.phase_skipped`) is deferred

Date: 2026-10-03. Finding: `docs/reviews/review-layout-tree.md` L-12 (MINOR).

Above `PRIORITY_NODE_BUDGET` (200 000 layered vertices, `layout/sugiyama/coords.rs:17`) the
Sugiyama X phase runs zero passes, so x is the raw ordering slot. The finding asks for a note.

Decision: no new note code now. The registry's `degradation` field for the layered layout
states the skip (fix-tree-sugiyama, `registry/grid.rs`), which is the half that needs no
contract change.

Why: a code 7 is a contract minor bump (`notes.rs`: a 0.3 reader refuses an unknown code),
touching four closed-set lists (`notes.rs`, `canonical_json/schema.rs`, `notes/tests.rs`,
`binary-layout.md`) and every reader, for a state no consumer reads today. Nothing in the
studio or the SDK branches on a degraded X phase.

Reopen when a consumer needs to tell a skipped X phase from a run one, or when another
contract minor bump is already under way: add it to that bump rather than cutting one for it.
