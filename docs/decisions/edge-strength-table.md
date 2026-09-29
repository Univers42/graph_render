# The edge-strength table

**Status:** decided. **Date:** 2026-09-29. **Code:** `crates/graph-core/src/ingest/strength.rs`,
one table, total over `EdgeKind::ALL`.

## The defect this fixes

Graph derivation exists in the host three times over, and the copies have already diverged.
Two live code paths produce **different layouts for the same data**, silently: nothing
downstream can detect it, because both paths return a well-formed graph with a plausible
snapshot hash.

The phase prompt (`prompts/phase-10-ingest-sdk-publish.md` §"Why this is last") records the
three sets:

| Path | Strengths | Citation |
|---|---|---|
| Client | `1.2` / `1.8` / `0.7` / `0.5` | `prompts/phase-10-ingest-sdk-publish.md:20-22` |
| Bridge | `3.2` for hierarchy, `1.4` for everything else | `scripts/bridge-graph.mjs:88-121` `pagesToGraph`, consumed via `mapGraphResponse.ts:54-57` |
| Query router | a third set | `prompts/phase-10-ingest-sdk-publish.md:22-24` |

### What could not be read, and what that costs

**The host source tree is not on this worktree**, so the citations above are recorded as the
phase prompt states them rather than verified against the files:

- No client-path file was readable, so the **per-kind assignment** of `1.2 / 1.8 / 0.7 / 0.5`
  is unknown here. The prompt gives the set, not which kind gets which value.
- No query-router file was readable, so the third set's **values are entirely unrecorded**.

Neither gap changes the decision — the divergence is established by the phase prompt's own
account, and the fix is a single table that replaces all three — but it does mean **this
document does not yet record the per-kind old values**, which it is supposed to. That is
open work, not a completed record: see "Open" below. The three-way divergence is also
exactly the defect `roles.rs` exists to remove at the source, and that is the step that
deletes the copies rather than merely picking between them.

## The chosen table

| Edge kind | Strength | Why it sits there |
|---|---|---|
| `hierarchy` | `2.0` | The structure the drawing is built on. Strongest pull, on the bridge's reasoning that containment dominates. |
| `relation` | `1.0` | The reference weight. An ordinary link is the unit everything else is compared to. |
| `tag` | `0.75` | A hub edge. Real structure, but it describes a facet of a record, not a relation between two records. |
| `note_link` | `0.625` | An annotation, weaker than a facet. |
| `note_of` | `0.5` | The weakest fact: a note hanging off a record. |

The ladder is strictly descending, and no two kinds share a value — so swapping two rows
changes the output and a test says so.

## Why these values and not one of the old sets

1. **The bridge's `3.2` is an outlier with no argument behind it.** It is 2.3× its own
   everything-else value. Whatever produced it, it is a magnitude nothing else in the
   project uses, and adopting it would make hierarchy edges dominate every force-directed
   layout by more than a factor of three.
2. **The client's four values are not a table.** They are per-call values along four code
   paths, and their spread (`0.5`–`1.8`, a factor of 3.6) is wider than the ordering this
   project argues for. Adopting them means adopting whichever path a given edge happened to
   travel.
3. **A single descending ladder is reviewable.** One ordering, stated once, with the reason
   for each position — the property the three divergent sets most conspicuously lack.

The choice is a **convention, not a derived truth**. No measurement says a `note_link` should
pull `0.625`; the ordering is an argument about which facts should dominate a layout, and the
magnitudes are round numbers under it. This is recorded in the `Ponytail` on
`edge_strength` so a reader meets the caveat at the code, not only here.

## Every value is a dyadic rational

All five are multiples of `2⁻⁶`, so each is **exact** in `f64` and in `f32`, and exact through
the JSON face's `f32 → shortest decimal → f64 → f32` round-trip. No strength in this table
can differ by one ULP between native and wasm32, or between a Rust and a JavaScript reader.
`every_value_is_a_dyadic_rational_and_survives_the_json_face` pins that, and would fail on a
value like `0.7` — which is exactly the class of value the old tables used.

## The visible consequence, stated plainly

**Layouts will differ from both current behaviours.** Every hashed snapshot taken under a
client-path or bridge-path strength set is stale the moment this table is adopted, and every
recorded geometry measurement from those runs no longer describes what the motor produces.
This was pre-accepted by the phase prompt; it is restated here so nobody discovers it from a
hash mismatch.

## Consequence for the rest of the project

`edge_strength` is the only reader of `STRENGTH_TABLE`. Once `build.rs` exists, every derived
edge takes its strength from here, and a caller needing a different convention overrides the
edge's own `strength` after `index_model` rather than editing the table. Editing the table is
the expensive path: it changes **every** layout, and the snapshot hashes pinned to it go with it.

## Negative controls

Each of these was applied to the table and run; each is killed by at least one test in
`crates/graph-core/src/ingest/strength.rs`:

| Mutation | Killed by |
|---|---|
| `note_link` `0.625` → `0.7` | `table_pins_every_kind_to_its_exact_value`, `every_value_is_a_dyadic_rational_and_survives_the_json_face` |
| `tag` and `note_link` strengths exchanged | `table_pins_every_kind_to_its_exact_value`, `the_ladder_is_strictly_descending_in_discriminant_order` |
| the `hierarchy` row deleted | `the_table_is_total_over_all_kinds` (and the pinned-values test) |

## Open

- The per-kind client-path values and the query router's third set are **not yet recorded**,
  because the host source tree is not readable from this worktree. Recording them is a
  documentation completion, not a code change, and does not gate anything.
- `roles.rs` and `build.rs` are the steps that actually **delete** the two other
  implementations. This slice makes the motor's own derivation agree with itself; the
  divergence survives in the host until those steps land and the host adopts one source of
  truth.
