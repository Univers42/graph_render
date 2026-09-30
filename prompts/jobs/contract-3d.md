# Job contract-3d (agent build, one design document)

Why: the user approved 3D layouts on 2026-09-29, subject to a devil verdict on the contract change
(`prompts/RESUME.md`, "Update 2026-09-29 late": "It needs a contract change (dim once per snapshot, a
z column, 2D bytes unchanged), so the design goes through a devil verdict before any code."). The
first design job's brief and output were lost with `/sgoinfre`; `docs/decisions/contract-3d.md` does
not exist. This job writes it. No code.

Facts:
- The contract: `docs/contract/` (binary layout, canonical JSON), `crates/graph-contract/`
  (the single source of truth; `graph-cli codegen` writes the schema and TypeScript declarations,
  `--check` fails on a stale file). The hash is taken over the binary face; the JSON face must
  round-trip to identical bytes (`roundtrip`).
- The kind is declared once per snapshot, not per element (pure SoA, zero-copy). Wire integers are
  `u32`/`u64`.
- Consumers: `crates/graph-wasm` (extern "C" ABI), `crates/graph-sdk-js`, `packages/graph-render`
  (reads snapshot bytes only, `docs/contract/binary-layout.md`), the studio.
- 3D references: SciGraphs computes 3D natively (`SciGraphs/core/scigraphs_core/mesh/layouts/`;
  `docs/decisions/eigensolver.md:28` and `docs/measurements/phase08-routing.md:212` note where the motor
  deliberately stayed 2D); igraph FR-3D is deferred (`prompts/REFERENCES.md:120`).

Write `docs/decisions/contract-3d.md` with:
1. The change: where `dim` lives (once per snapshot header), the z column's placement in the binary
   layout, the canonical JSON shape, the version bump rule, and a proof sketch that every existing 2D
   snapshot keeps byte-identical output (so every recorded 2D hash stays valid).
2. What each consumer must do (wasm ABI, SDK, renderer, studio), and what a 2D-only consumer does
   when it receives a 3D snapshot (refuse with a named error, never silently drop z).
3. The first 3D layouts, easiest first, with their oracle (e.g. spectral with 3 eigenvectors against
   SciGraphs/scipy, random 3D, sphere/shell), and which stages (POST, SCALE) are 2D-only for now.
4. The gates: hashgate over 3D snapshots, `roundtrip` over 3D JSON, a negative control, and codegen.
5. Assumptions, failure modes, and what is unknown — written out for the verdict (`.claude/rules/risk.md`
   "Externalize before you rule").

Paths you may touch: `docs/decisions/contract-3d.md`. Nothing else.

Done when: the document exists, every claim about current code cites `file:line`, and section 5 lists
at least the blast radius (which crates and files change), reversibility, and the unverified assumptions.
