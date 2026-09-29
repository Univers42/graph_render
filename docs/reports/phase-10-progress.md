# Phase 10 progress

Branch `p10`, based on `develop` (p3, p4, p5, p6e, p6f, p7 merged). Phase 9's gate is **not** green
(Phase 9 is unbuilt), so this phase is being built in independent slices that do not depend on it.
This file records the first slice; it is written to be continued, not read as a close-out.

## Slice 1 — `strength.rs`, the divergence fix (DONE)

**Why this one first.** It is the only step in `prompts/phase-10-ingest-sdk-publish.md` that is
self-contained: a constant table with no dependency on `roles.rs`, `build.rs`, the adapters, the
fixtures, or anything from p8/p9. Every later step needs at least one of those.

### Changed

| Path | |
|---|---|
| `crates/graph-core/src/ingest/mod.rs` | CREATE — module, re-exports the table |
| `crates/graph-core/src/ingest/strength.rs` | CREATE — the table, 5 tests |
| `crates/graph-core/src/lib.rs` | MODIFY — one line, `pub mod ingest;` |
| `docs/decisions/edge-strength-table.md` | CREATE — the required record |

Nothing else. `src/`, `tests/`, `verify/` and osionos untouched.

### The table

Strictly descending, total over `EdgeKind::ALL`, no two kinds sharing a value:
`hierarchy 2.0`, `relation 1.0`, `tag 0.75`, `note_link 0.625`, `note_of 0.5`.

Every value is a **dyadic rational** (a multiple of 2⁻⁶), so each is exact in `f64`, exact in `f32`,
and exact through the JSON face's `f32 → decimal → f64 → f32` round-trip. A test pins that, and it
would fail on a value like `0.7` — the class of value the old tables used. That is the D9 requirement
met by construction rather than by assertion after the fact.

`edge_strength` is `const fn` and `panic!`s on a missing row. Since `STRENGTH_TABLE` is total over
`EdgeKind::ALL`, and `ALL` is the only way to obtain an `EdgeKind`, the panic is unreachable — and a
sixth variant is caught by `the_table_is_total_over_all_kinds` instead of by a panic at runtime.

### TDD trail (each step run, not asserted)

1. **RED** — tests written against `edge_strength` / `STRENGTH_TABLE`, which did not exist.
   `cargo test -p graph-core ingest::strength` → 11 compile errors, `E0425` / `E0432`.
2. **GREEN** — implementation landed; 5 passed, 0 failed. Two `E0614` fixes en route: array iteration
   yields values, not references, so `*value` was wrong in two tests.
3. **Negative control A** — `note_link 0.625 → 0.7`: 2 tests FAILED (exit 101).
4. **Negative control B** — `tag` and `note_link` strengths exchanged: 2 tests FAILED (exit 101).
   My first attempt at this produced a duplicate `0.625` rather than a clean swap, so the result was
   discarded and the swap redone properly; the clean swap is the one recorded above.
5. **Negative control C** — the `hierarchy` row deleted: 2 tests FAILED (exit 101).

### Checks, with real exit codes

| Command | Exit |
|---|---|
| `gr cargo fmt --all --check` | 0 (after one `cargo fmt --all`, which reformatted one `collect()` in a test) |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `gr cargo test --workspace` | 0 — 763 passed, 0 failed, 8 ignored |

Not run, and not claimed: `hashgate`, `roundtrip`, `capabilities --check`, the wasm32 build, the
`--adapter-convergence` row, `ge-check`. No full hashgate, gate.sh or cargo mutants was run — the
orchestrator owns those under the host-wide lock.

### The Ponytail

On `edge_strength`, and it says the load-bearing part: **the table is a chosen convention, not a
derived truth.** No measurement says an annotation should pull `0.625`. The failure mode is silent
and global — change a value and every layout changes, so every pinned snapshot hash and every
recorded geometry measurement goes stale with nothing in the output saying so. Direction: it is a
convention precisely *because* the two existing implementations disagreed and neither could be shown
wrong from geometry alone. Escape hatch: `edge_strength` is the only reader of the table, so a caller
needing a different convention overrides the edge's own `strength` after `index_model` rather than
editing the table.

## Blocked

**The host source tree is not readable from this worktree.** `/home/dlesieur/Documents/` holds
`life` only, and no `deriveGraph.ts`, `mapGraphResponse.ts` or `bridge-graph.mjs` exists anywhere
findable. Consequences:

- `docs/decisions/edge-strength-table.md` records the three divergent sets **as the phase prompt
  states them**, not as verified against the files. The prompt gives the client's set
  (`1.2 / 1.8 / 0.7 / 0.5`) but **not which kind gets which value**, and gives the query router's
  set as "a third set" with **no values at all**. Both gaps are marked in the document under
  "What could not be read, and what that costs" and "Open".
- This does not block the slice. The divergence is established by the phase prompt's own account and
  the fix is one table replacing all three. It does mean the decision record is **documentation-
  incomplete** on the "the three existing sets with citations" requirement, and that is open work,
  not a completed record.

No reference needed from `/goinfre/dlesieur/refs` or `SciGraphs` for this slice: the table is a
convention, not a port, and nothing was ported.

## Next step

**Slice 2 — `roles.rs`, the declared roles.** The largest and most important step, and the one the
whole phase exists for: replacing type-sniffing heuristics (`deriveTagConfig.ts:23-32` picks "the
first `multi_select`"; `deriveGraph.ts:92-98` picks "the first `status`, else the first `select`")
with eight declared roles. It also fixes **H6** — `groupValue` depends on `Object.values()` ordering,
which is non-deterministic across JSON reserializations, and a declared role has no ordering
dependency.

Note for whoever takes it: `graph-core`'s dependency list is **closed** (`libm`, `indexmap`,
petgraph), and the `ingest.rs` contract types belong in `graph-contract`, whose `serde` is behind
the `codegen` feature so the motor's allow-list does not grow a serializer through the back door.
Where the `records: [{ id, collection, deleted, updatedAt, values: { fieldId: JsonValue } }]`
`JsonValue` type actually lands is the first design question of that slice, and it is a
stop-and-ask if the answer needs a new dependency.

Then, in order: `build.rs` (one derivation, plus the H5 id-grammar decision — the phase requires
deciding and recording it, not hoping for it), the two adapters, the convergence fixtures, and
publishing the SDK.
