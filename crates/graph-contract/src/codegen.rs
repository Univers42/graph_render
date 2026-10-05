//! JSON Schema and TypeScript emitted from the contract types. Types only: the generated
//! TypeScript declares shapes and ships **zero runtime bytes** — no validator, no enum
//! objects, nothing a bundler would keep.
//!
//! Its own file rather than inline in `lib.rs` because it is one concern and `lib.rs` is a
//! list of modules — and because a *generated* thing deserves a file a reader can skip past
//! without reading its guts to reach the rest of the crate. The TypeScript half is
//! `typescript.rs` and the tests are `tests.rs`; this file holds the schemas and the output
//! list, which is the part every caller names.

#[cfg(test)]
mod tests;
mod typescript;

use serde_json::Value;

/// Where the header's generated files are committed, relative to the workspace root.
pub const GENERATED_DIR: &str = "crates/graph-contract/generated";

/// Where the JSON face's schema is committed, relative to the workspace root.
pub const SNAPSHOT_SCHEMA: &str = "docs/contract/snapshot-schema.json";

/// Where the ingest contract's schema is committed, relative to the workspace root.
pub const INGEST_SCHEMA: &str = "docs/contract/ingest-schema.json";

/// Where the hub wire's schema is committed, relative to the workspace root.
pub const HUB_SCHEMA: &str = "docs/contract/hub-schema.json";

/// Every generated file: its path from the workspace root and its exact contents.
/// Committed, so a change to the contract types shows up as a diff in review.
///
/// The order is a contract of its own: `graph-cli`'s test reads `outputs()[1]`, so a new
/// output is **appended** and never inserted. The hub's two are last for that reason.
pub fn outputs() -> [(String, String); 8] {
    [
        (
            format!("{GENERATED_DIR}/snapshot-header.schema.json"),
            format!("{:#}\n", json_schema()),
        ),
        (
            format!("{GENERATED_DIR}/snapshot-header.d.ts"),
            typescript(),
        ),
        (
            SNAPSHOT_SCHEMA.to_owned(),
            format!("{:#}\n", snapshot_schema()),
        ),
        (INGEST_SCHEMA.to_owned(), format!("{:#}\n", ingest_schema())),
        (
            format!("{GENERATED_DIR}/layout-params.schema.json"),
            format!("{:#}\n", layout_params_schema()),
        ),
        (
            format!("{GENERATED_DIR}/layout-params.d.ts"),
            params_typescript(),
        ),
        (
            HUB_SCHEMA.to_owned(),
            format!("{:#}\n", crate::hub::schema::schema()),
        ),
        (format!("{GENERATED_DIR}/hub.d.ts"), hub_typescript()),
    ]
}

/// TypeScript declarations for the hub wire, derived from [`crate::hub::schema::schema`]
/// by the same writer as every other `.d.ts` in the crate.
pub fn hub_typescript() -> String {
    typescript::of(&crate::hub::schema::schema())
}

/// The JSON Schema (draft 2020-12) of one layout's published parameters: the schema
/// a frontend reads to draw a control per parameter
/// (`docs/decisions/layout-params.md`).
pub fn layout_params_schema() -> Value {
    schemars::schema_for!(crate::canonical_json::schema::LayoutParamsSchema).to_value()
}

/// The JSON Schema (draft 2020-12) of the ingest contract: declared roles and
/// records, the neutral front of the pipeline (`crates/graph-contract/src/ingest.rs`).
pub fn ingest_schema() -> Value {
    crate::ingest::schema::schema()
}

/// The JSON Schema (draft 2020-12) of the canonical JSON face: a whole snapshot.
pub fn snapshot_schema() -> Value {
    schemars::schema_for!(crate::canonical_json::schema::Snapshot).to_value()
}

/// The JSON Schema (draft 2020-12) of the snapshot header and everything it references.
pub fn json_schema() -> Value {
    schemars::schema_for!(crate::snapshot::SnapshotHeader).to_value()
}

/// TypeScript declarations for the snapshot header, derived from [`json_schema`]. The
/// body lives in `typescript.rs`; this is the name every caller uses.
pub use typescript::typescript;

/// TypeScript declarations for the layout-parameter schema, derived from
/// [`layout_params_schema`] by the same path as [`typescript()`]: declarations only, zero
/// runtime bytes. Its types are named `LayoutParamSpec`/`LayoutParamKind` rather than
/// `ParamSpec`/`ParamKind`, which the studio already exports for its own knobs with
/// different meanings (`packages/graph-studio/src/actions/registry.ts`).
pub use typescript::params_typescript;
