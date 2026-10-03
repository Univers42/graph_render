# Wasm ingest limits

**Status:** decided. **Date:** 2026-10-02. **Amended:** 2026-10-03, F-16 steps 3-4 by
`fix-ingest-scale` (the arena no longer traps, so the ceiling is the largest power of two at
or below the largest document that built: `MAX_INGEST_BYTES = 1_073_741_824`). **Code:** `crates/graph-wasm/src/ingest.rs`,
`crates/graph-wasm/src/analysis/centrality.rs`. **Source:** the "Decisions needed" items 1–4 of
`docs/measurements/fix-wasm-abi.md` (F-16, F-01, F-80).

## F-16: a byte ceiling, measured

`ingest::read` has no ceiling. A document large enough to exhaust wasm32's 4 GiB address space
traps inside an infallible allocation (`Vec` growth in the parser or the topology), which the host
sees as `unreachable`, not as a refusal it can name.

Decision: `gm_build` refuses a document longer than `MAX_INGEST_BYTES` **before parsing**, with a
new code appended to the error enum (never renumbering one that exists). The number is measured,
not chosen:

1. Emit synthetic ingest documents at the studio's scale target (1M nodes, the edge density of the
   hash gate's synthetic models) and at doubling sizes above it.
2. Run `gm_build` on each under Node on the real wasm32 artifact; record the document's bytes and
   whether it built or trapped.
3. **`MAX_INGEST_BYTES` is the largest document that built, byte for byte**, rounded down to
   a power of two. Measured on the wasm32 artifact under Node at the studio's 1M-node scale
   target and upward: **1,499,403,588 bytes built** (1M nodes, 7,999,936 edges, at a peak of
   4,117,561,344 of the 4,294,967,296 bytes wasm32 can address) and **1,663,576,802 bytes
   trapped** (1M nodes, 8,999,919 edges), so the largest power of two at or below the largest
   that built is `2^30` and `MAX_INGEST_BYTES = 1_073_741_824`.
   The rounding is a trade and it is taken deliberately. It refuses the 425,661,764 bytes
   between `2^30` and the largest document that built — documents this build accepts — which
   step 3's original "no rounding" rule forbade. That rule was a workaround for a *trapping*
   reader, where every byte of headroom was a byte of margin against `unreachable`; with the
   reader no longer trapping, headroom is margin against nothing, and a ceiling with a rule
   behind it and room under it is worth more than a ceiling that is one measurement with none.
   What the ceiling may never do is refuse a document that built **before** the reader changed:
   `2^30` is above the 774,568,785 bytes `fix-wasm-ingest` measured as the largest that built
   then, and above the studio's own 1M-node degree-3 document (678,016,813 bytes), so it
   refuses nothing that used to work. A future raise must keep that property.
4. **The power-of-two step down fired.** `fix-wasm-ingest` deferred it here because the
   1M-node degree-4 document trapped, so at `2^29` the ceiling refused the studio's own
   degree-3 document, which builds. `fix-ingest-scale` removed the `Value` tree the trap lived
   in (it measured 3.2x the document's text) and re-ran the sweep; step 3's rule now applies.
5. If the 1M-node document itself traps, that is a scale defect, not a ceiling: stop and report.
   **This fired twice.** First in `fix-wasm-ingest` (the degree-4 and degree-5 documents
   trapped in the arena; the ceiling landed anyway at the then-largest that built). Then here:
   the sweep was re-run upward from 1M degree 5 until a document trapped, which is the
   degree-9 shape at 1,663,576,802 bytes.

The constant carries a `Ponytail:` line saying the same thing from the code's side: the
rounding is 425,661,764 bytes of margin and it refuses documents that build, it bounds bytes
and not the work they imply, and the first document the sweep saw trap sits 164,173,214 bytes
*above* the ceiling while the largest document that built sits 425,661,764 bytes below it —
so the band the ceiling refuses and the band the sweep says traps do not meet.

## F-01: `child_first` stays optional in version 1

Version 1 reads an omitted `child_first` as `false`, and every sender relies on it
(`harness/sdk-smoke/{build,force,post}.mjs`, the documented example). Requiring it is an ingest
version 2 with those senders migrated in the same change. Nothing asks for that today, so version 1
stays as documented in `docs/contract/wasm-abi.md`.

## F-80: a negative strength is refused where it is wrong

A negative `strength` is legal input: no layout reads it as a distance. The shortest-path and
eigenvector centralities do, so they refuse it (a NaN column, `AnalysisFailed`; fixed by
`fix-wasm-abi`). Refusing it at ingest would change what `gm_build` accepts for every caller to
protect three analyses. It stays at the analyses.

## SDK tests

`crates/graph-sdk-js/test/*.test.mjs` get a root npm script, `sdk:test`, so the gate rows can run
them by name. The root `package.json` is fingerprinted, so the change lands through `land.sh`.
