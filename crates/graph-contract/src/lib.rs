//! graph-contract — the single source of truth for the graph-motor wire format.
//!
//! The geometry vocabulary (`prompt.md` §4), the format version every reader checks,
//! and a snapshot's two faces: the binary face the hash is taken over
//! (`docs/contract/binary-layout.md`, authoritative) and the canonical JSON face any
//! third-party frontend reads. Dependency-free by default, so graph-core and graph-wasm
//! can write both faces. With the `codegen` feature it also emits the JSON Schemas and
//! the TypeScript types, derived from Rust types so they cannot drift from the code.

pub mod binary;
pub mod canonical_json;
#[cfg(feature = "codegen")]
pub mod codegen;
pub mod geometry;
pub mod hub;
pub mod ingest;
pub mod ingest_columns;
pub mod notes;
pub mod params;
pub mod snapshot;
pub mod version;
