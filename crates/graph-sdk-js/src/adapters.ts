// The ingest contract's adapters. Re-exported from the entry point so a consumer
// imports one thing, and imported here (not only in a harness) because an adapter with
// no live caller in the same phase is a dead export — `prompt.md` §11, guardrail 2.
// Both are pure mappings from a source shape to the contract document; the derivation
// from that document to a graph is the motor's, and lives in Rust.
export {
  Cardinality,
  Role,
  RowsAdapterError,
  rowsToIngest,
  type CardinalityName,
  type Ingest,
  type IngestCollection,
  type IngestField,
  type IngestLink,
  type IngestRecord,
  type JsonValue,
  type RoleName,
  type RowsColumn,
  type RowsLink,
  type RowsRecord,
  type RowsSource,
  type RowsTable,
} from "./adapters/rows.ts";
export {
  notionToIngest,
  type NotionDatabase,
  type NotionOverrides,
  type NotionPage,
  type NotionProperty,
  type NotionRelationConfig,
  type NotionSource,
} from "./adapters/notion.ts";
