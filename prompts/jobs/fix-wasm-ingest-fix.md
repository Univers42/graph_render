# Job fix-wasm-ingest, round 1 (same session, same worktree): your two decisions, answered

Your last block was `status: partial`, so no gate ran. This round ends in `status: done` and a
commit past `6fcd60b`. The 8.6 GB under `target/ingest-ceiling/` was deleted; regenerate only the
documents you need and delete them before your block (`scripts/orch/gr rm -rf target/ingest-ceiling`:
they are root-owned).

1. The ceiling. Step 4 of `docs/decisions/wasm-ingest-limits.md` fired: the 1M-node degree-4
   document traps. A ceiling must never refuse a document that builds today, because the trap is
   the only failure it replaces: at `2^29` it refuses the studio's own 1M-node degree-3 document
   (678,016,813 B), which builds now. While the scale defect is open, set `MAX_INGEST_BYTES` to the
   largest document that built (774,568,785 B) rounded down to a whole MiB. Its `Ponytail:` must
   say that it has no margin and why: no document that built is refused, and a document under it
   with more work can still trap, as it does today. Amend the decision record's F-16 section to
   say the same, with step 3's power-of-two rule restored by the scale job, and update the tests
   and `docs/contract/wasm-abi.md` that name the number. The scale defect (the arena allocation in
   `index_model`) is a separate job, `fix-ingest-scale`; do not touch graph-core.
2. `sdk:test` in the gate: yes. Add a row to `scripts/orch/rows/develop-full.rows` next to the
   SDK typecheck row, and its negative control, a second row that must fail. Make it fail by
   breaking the test's input, not by inverting the expected exit. Add the command to `CLAUDE.md`'s
   command block, next to `sdk:typecheck`. Paste both rows' runs.
3. The wasm32 release warning you found (`ingest.rs:21`: unused `EdgeKind`, `NodeKind`) is in
   your paths. Warnings are errors here: fix it, and paste
   `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` with no
   warning.
4. Keep the `exports/build.rs` deviation (6 lines). It is accepted.

Paths, added this round: `docs/decisions/wasm-ingest-limits.md`, `scripts/orch/rows/develop-full.rows`,
`CLAUDE.md` (the one command line).

Done when: the fix-common done-when; the artifact boundary run (`ceiling` → built, `ceiling+1` →
`refused code=19`) at the new number; the 1M-node degree-3 document builds on the artifact (paste
it); the two new rows pass and fail as intended.
