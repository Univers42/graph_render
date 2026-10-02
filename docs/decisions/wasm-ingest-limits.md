# Wasm ingest limits

**Status:** decided. **Date:** 2026-10-02. **Code:** `crates/graph-wasm/src/ingest.rs`,
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
3. `MAX_INGEST_BYTES` is the largest power of two at or below the largest document that built.
4. If the 1M-node document itself traps, that is a scale defect, not a ceiling: stop and report.

The constant carries a `Ponytail:` line: it bounds bytes, not the work they imply, so a document
under the ceiling with an unusually high edge-to-node ratio can still exhaust memory.

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
