# Job fix-wasm-ingest, round 2 (same session, same worktree): decision (1) answered

Your decision (1) is right: round 1's instruction contradicted itself. "Rounded down to a whole
MiB" refuses the largest document that built (774,568,785 B) by 719,697 B, and the ceiling exists
only to never refuse a document that builds. The answer: **`MAX_INGEST_BYTES = 774_568_785`**, the
measurement itself, unrounded. It refuses nothing that built and accepts nothing unmeasured.
Not 739 MiB: the bytes between 774,568,785 and 774,897,664 were never run.

1. `crates/graph-wasm/src/ingest.rs`: the constant and its doc comment. The `Ponytail:` keeps
   "no margin" and its failing input, and drops the rounding: the number is the largest document
   that built, byte for byte.
2. `crates/graph-wasm/src/ingest/tests/ceiling.rs`: rename the first test, assert `774_568_785`,
   drop the whole-MiB assertion. Fix "738 MiB" in the comments (the buffer is 774,568,786 bytes).
3. `docs/decisions/wasm-ingest-limits.md` step 3, `docs/contract/wasm-abi.md` (lines naming the
   number and the 719,697-byte refusal), `docs/measurements/fix-wasm-ingest.md` (the sweep row
   for 774,568,785 now reads built under the ceiling; the boundary rows; decision (1) recorded
   as answered). `git grep -nE '773[_,]?849[_,]?08[89]|738 MiB|719[_,]697'` must print nothing.
4. Re-run the artifact boundary at the new number: `774568785` bytes → `built code=0`,
   `774568786` → `refused code=19`. Paste both. Delete `target/ingest-ceiling/` after
   (`scripts/orch/gr rm -rf target/ingest-ceiling`).
5. Re-run `scripts/orch/rows/quick.rows` via the fix-common loop and commit (`updated`).

Paths: as round 1. Done when: the fix-common done-when, the grep in step 3 empty, the two
boundary runs pasted, `status: done`.
