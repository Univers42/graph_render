# Job fix-ingest-scale (agent build: the 1M-node document must build on wasm32)

Read `prompts/jobs/fix-common.md` first for the loop and the report shape.

Why. The studio targets 1M nodes (`packages/graph-studio/src/source/synthetic.ts`, `MAX_NODES`).
`docs/measurements/fix-wasm-ingest.md` swept `gm_build` on the wasm32 release artifact under Node.
The 1M-node documents at degree 4 (842,132,644 B) and degree 5 (992,910,579 B) trap with
`unreachable` in `alloc::handle_alloc_error`, under `graph_core::arena::StringArena::intern`, under
`graph_core::index::index_model`, under `gm_build`. The 950k-node degree-4 document traps too. A
32-bit linear memory holds 4 GiB, and the input itself is under 1 GiB. Something between the
copied buffer and the built topology holds several times the input. That is the defect.
`docs/decisions/wasm-ingest-limits.md` step 4 classes it as a scale defect, not a ceiling.

Do:
1. Measure before cutting. Reuse `harness/ingest-ceiling.mjs` (documents under
   `target/ingest-ceiling/`, root-owned, never committed). On the artifact, record linear memory
   (`memory.buffer.byteLength`) after each phase of `gm_build`, at 1M nodes for degrees 3, 4 and 5:
   after the copy, after the parse, after `index_model`, after the CSRs, and after the build
   returns. If a phase cannot be observed from outside, add a test-only probe that is compiled out
   of the release artifact. Paste the table and name the phase that holds the bytes.
2. Cut the peak in that phase, in this order, and stop at the first rung that makes 1M degree 5
   build:
   - pre-size from counts the document already gives (no doubling `Vec` growth in a hot path);
   - free what is dead before the next phase allocates (the parse tree once the arena owns the
     strings);
   - borrow ids from the input buffer instead of copying them;
   - parse into typed records instead of a generic JSON tree.
   No new dependency (`graph-core`'s list is closed: `libm`, `indexmap`, `petgraph`).
3. The output must not move. `hashgate --seeds 8` exits 0, every per-stage digest equals develop's,
   and its `GM_MUTATE_REFERENCE_DEGREE=9` control is non-zero. `cargo test --workspace` is green.
   Ingest refusal codes and their order are unchanged.
4. Re-run the sweep upward, doubling from 1M degree 5 until a document traps. Then set
   `crates/graph-wasm/src/ingest.rs`'s `MAX_INGEST_BYTES` by the decision record's step 3 (the
   largest power of two at or below the largest that built). The ceiling must refuse nothing that
   built before this job; if the power-of-two rule would, keep the previous number and say so.
   Update the decision record, the constant's `Ponytail:`, `docs/contract/wasm-abi.md` and the
   tests that name the number.
5. Delete `target/ingest-ceiling/` before your block (`scripts/orch/gr rm -rf target/ingest-ceiling`).

If no rung of step 2 makes 1M degree 4 build, stop. Report the per-phase table and the rung that
came closest, under "decisions needed".

Paths: `crates/graph-core/src/{arena.rs,index.rs,index/**,ingest/**}`,
`crates/graph-wasm/src/{ingest.rs,ingest/**,exports/build.rs}`, `harness/ingest-ceiling.mjs`,
`docs/decisions/wasm-ingest-limits.md`, `docs/contract/wasm-abi.md` (the ceiling's number only),
`docs/measurements/fix-ingest-scale.md`.

Done when: the fix-common done-when; the per-phase table before and after; 1M nodes at degrees 3,
4 and 5 build on the artifact (paste each); the new ceiling's boundary run (`ceiling` → built,
`ceiling+1` → `refused code=19`).
