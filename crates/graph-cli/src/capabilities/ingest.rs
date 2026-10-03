//! Phase 10's INGEST rows: the declared roles, the one derivation, and the two adapters.
//!
//! Split out of `capabilities.rs` for the same reason `analysis.rs` is: the house's
//! 300-line limit, and a fourth group of rows would have pushed that file over it.
//!
//! Every row here is `Status::Implemented`, **not** `Gated`, and that is the honest
//! status rather than a shortfall in the work. `problems()` accepts a `Gated` row only
//! while a recorded gate run backs it on the current tree, and the two oracles these
//! capabilities have are neither the TypeScript `oracle-diff` nor the 4-way hash gate:
//! they are the **convergence fixture** (two source shapes, one graph, identical bytes)
//! and the **contract round trip** (the reader, the writer, the committed JSON Schema).
//! `hash_stage` and `oracle_record` name the records that would back a row if those
//! runs were wired in, and until they are, `Implemented` is what the evidence supports.
//! `gate.sh` owns those runs, under the host-wide lock this phase does not take.

use super::registry::TOPOLOGY_CEILING;
use super::{Capability, Status};

/// `(id, oracle, complexity, scale_ceiling, degradation, ponytail)`.
type Row = (
    &'static str,
    &'static str,
    &'static str,
    u64,
    &'static str,
    &'static str,
);

/// The ingest contract's own ceiling: the same per-node cost class as the topology
/// columns it feeds, plus one `Vec<NodeRecord>`/one `Vec<EdgeRecord>` of derived records
/// that `index_model` then consumes. Not a new order of magnitude, and not
/// independently re-measured — the inherited estimate is what earns the shared marker.
const INGEST_DEGRADES: &str = "past the ceiling, the same shape as topology: wasm32 cannot \
allocate and the module traps (no partial result); natively, memory permitting, this refuses \
alongside index_model's own CapacityError rather than wrapping or truncating. Past the u32 \
dense-index space a node id cannot be interned and the refusal is the same one";

const INGEST_CEILING: u64 = TOPOLOGY_CEILING;

/// The contract reader refuses far more than the motor can compute, so a scale ceiling on
/// *it* is the reader's own bound, not a layout's: it is linear in the document and
/// bounded by `u32` on every count it reads (`version`, `updatedAt`), which no document a
/// person can hold reaches.
const READER_COMPLEXITY: &str = "O(bytes); one pass to parse, one pass to cross-check \
(duplicate ids, dangling references, the id grammar), then one pass per collection and per \
record";

const DERIVATION_COMPLEXITY: &str = "O(collections * fields + records * (fields + references)) \
— linear in the document, with a sort of the distinct tag values (IndexSet, insertion-ordered \
so the order is the document's own, never a hash order)";

const ROWS: [Row; 4] = [
    (
        "ingest.roles",
        "the TypeScript oracle's `deriveTagConfig.ts:23-32` (\"the first `multi_select`, or a \
field named `/^tags?$/i`\") and `deriveGraph.ts:92-98` (\"the first `status`, else the first \
`select`\") replaced by eight declared roles; the differential is the convergence fixture \
rather than a function-for-function oracle, because there is no reference implementation of a \
declaration to compare against",
        READER_COMPLEXITY,
        INGEST_CEILING,
        INGEST_DEGRADES,
        "Ponytail: an absent cell and a present-but-empty one are kept apart, and a value of \
the wrong JSON type reads as absent rather than being coerced; failing input: a `title` field \
holding a number, a `tags` field holding a bare string, a `link` field whose value is the wrong \
shape for its declared cardinality; direction: SAFE — each produces a record with no label, no \
tags or no edges where a coercion would have produced a plausible wrong one, and a visibly \
empty result rather than a plausible wrong one; escape hatch: the declaration is the escape \
hatch — declare the field with the role whose shape matches what the source actually holds, and \
the coercion never happens because none is offered. Ponytail (id grammar): `source` and a \
collection id containing `:` cannot round-trip through `source:collection:record` and the parse \
returns a SHIFTED wrong result rather than null — inherited from the oracle's own `ids.ts:61-66`; \
direction: DANGEROUS, a shifted id silently attributes a node to another collection; escape \
hatch: the reader refuses such a coordinate by name (`check_id_grammar`), and a caller that \
builds an `Ingest` in Rust bypassing the reader has opted out knowingly",
    ),
    (
        "ingest.build",
        "none in this repo: the three host derivations this replaces (the client path, \
`scripts/bridge-graph.mjs:88-121` `pagesToGraph`, and the query router's generators) were \
already diverging, so no one of them is an oracle — the differential is the convergence \
fixture, which pins this derivation's exact output for a dataset two independent adapters map",
        DERIVATION_COMPLEXITY,
        INGEST_CEILING,
        INGEST_DEGRADES,
        "Ponytail (the strength table): every derived edge's strength is read from \
`graph_core::ingest::edge_strength`, a CHOSEN CONVENTION and not a derived truth — no \
measurement says an annotation should pull 0.625; failing input: none, which is the problem: \
change a value and every layout changes; direction: silent and global, every pinned snapshot \
hash and every recorded geometry measurement goes stale with nothing in the output saying so; \
escape hatch: the table is read in one place, so a caller needing another convention overrides \
an edge's own `strength` after `index_model` rather than editing it \
(`docs/decisions/edge-strength-table.md`). Ponytail (default weight): a record with no \
`weight`-role value gets 0.5, a convention with no measurement behind it; direction: silent \
but local to datasets that declare no weights; escape hatch: declare the role. Ponytail (a tag \
value containing `:`): REFUSED rather than encoded, because `tag:a:b` cannot be read back; \
direction: loud, and refusing is the safe direction; escape hatch: rename the value at the \
source, or declare the field `scalar`",
    ),
    (
        "adapter.rows",
        "`fixtures/ingest/rows.json` mapped by `crates/graph-sdk-js/src/adapters/rows.ts`; its \
oracle is the other half of the same pair — the byte-identical document `adapter.notion` \
produces from `notion.json` for the same logical dataset",
        "O(columns + rows * columns); a sort of a table's columns by name, which is the \
contract's canonical order",
        INGEST_CEILING,
        INGEST_DEGRADES,
        "none owed: a pure mapping with no inference to be wrong about — it reads each column's \
DECLARED role and writes it down, and a cell naming an undeclared column is refused by name. \
The one thing it decides beyond transcription is that a column's `name` becomes the contract's \
field `name` (the label a `link` draws), which is a copy rather than a judgement",
    ),
    (
        "adapter.notion",
        "`fixtures/ingest/notion.json` mapped by `crates/graph-sdk-js/src/adapters/notion.ts`; \
its oracle is the other half of the same pair — the byte-identical document `adapter.rows` \
produces from `rows.json`",
        "O(properties + pages * properties); a sort of a database's properties by id, which is \
the contract's canonical order",
        INGEST_CEILING,
        INGEST_DEGRADES,
        "Ponytail (the type-to-role table): a property whose Notion `type` is not in \
`typeToRole` becomes `scalar` — declared and read by nobody; failing input: `formula`, \
`created_time`, `files`, `people`, a type a future release adds, or a misspelling; direction: \
the SAFE one (a structural role inferred wrongly would add or remove edges) but still SILENT — \
a `relation` spelled `Relation` would carry real structure and contribute none, and nothing in \
the derived graph says so; escape hatch: the caller's `roles` override, a declaration that \
outranks the table. Note the contrast with the heuristics this replaces: the table is a TOTAL \
function on a closed set read from the property itself, and its output is checked by the \
convergence fixture byte for byte, which is not true of \"the first `multi_select`, or a field \
named /^tags?$/i/\"",
    ),
];

/// The one hash-gate stage every ingest row names, declared once rather than as four
/// literals.
///
/// The gate hashes a single ingest stage, so naming four would be inventing stages it does
/// not hash and a row that names one can never be promoted by evidence at all. What
/// naming a shared stage must not do is let one row's evidence be read as another's
/// without saying so, which is why this is a named constant with this comment rather than
/// the string four times: the shared stage is a deliberate declaration, and
/// `every_ingest_row_names_the_one_stage_this_file_declares` holds the rows to it.
pub(super) const INGEST_STAGE: &str = "ingest.build";

/// Every ingest row, its metadata carried above and its ledger shape filled in here.
pub fn rows() -> impl Iterator<Item = Capability> {
    ROWS.iter().map(
        |&(id, oracle, complexity, scale_ceiling, degradation, ponytail)| Capability {
            id,
            tier: 1,
            stage: "ingest",
            geometry: None,
            status: Status::Implemented,
            oracle,
            // No recorded run backs these yet; naming the record that would is what lets
            // `problems()` say so rather than the row quietly claiming a verdict.
            oracle_record: "roundtrip",
            functions: &[],
            hash_stage: INGEST_STAGE,
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling,
            degradation,
            ponytail,
            complexity,
        },
    )
}
