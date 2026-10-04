//! The phase-10 **ingest contract** at the ABI boundary: contract bytes in, an indexed
//! graph out, through `graph_core::ingest`'s one derivation.
//!
//! ## Additive, and deliberately so
//!
//! [`crate::ingest`] still reads the **provisional** node/edge JSON for
//! [`gm_build`](crate::exports::gm_build), and it stays exactly as it was: that format
//! is what the host studio and the hash gate's C20 stage already speak, and rewriting it
//! would move a published ABI's meaning without adding anything. This module is the
//! *other* way in — [`gm_build_contract`](crate::exports::gm_build_contract) — and the two
//! documents are not interchangeable: each reader refuses the other's, which is what
//! keeps the two exports from drifting into one meaning by accident (a test, here:
//! `the_two_ingest_formats_are_not_interchangeable`).
//!
//! ## Why the derivation is not repeated
//!
//! Graph derivation existed in three copies in the host and they had already diverged, so
//! two live code paths produced different layouts for the same data. There is now one
//! derivation, [`graph_core::ingest::build`], and this module *calls* it. What lives here
//! is only the boundary: UTF-8, the contract's strict reader, and the error's trip
//! across an ABI that speaks a `u32`. There is no graph logic in this file, and the
//! tests below assert the identity rather than describing it — a copy of the derivation
//! here would fail `the_contract_path_derives_what_graph_core_derives_and_nothing_else`
//! the moment the two disagreed.
//!
//! The contract's reader refuses an unknown member, an unnamed role, a version it does
//! not know and a dangling collection; the derivation refuses the one thing the reader
//! cannot see (H5: a tag *value* containing `:`, which would parse back shifted). Both
//! refusals are kept apart in [`ContractError`] for native callers and its `Display`; the
//! wire carries **one** code for every variant, `ContractInvalid`, and a JS caller cannot
//! tell them apart (F-93). That is the documented contract (`docs/contract/wasm-abi.md`
//! "Errors", row 14): splitting it into new codes would change what a caller already
//! handling `ContractInvalid` reads for the same document.

use graph_contract::ingest::{IngestError, read};
use graph_core::Topology;
use graph_core::ingest::{BuildError, Derived, build_topology};

#[cfg(test)]
pub(crate) mod tests;

/// Why a contract document was refused at this boundary.
///
/// Two document-level steps, kept apart because they are different steps: the contract's
/// reader says no about the *document*, the derivation says no about the *graph the
/// document describes*. A caller debugging a refusal needs to know which, and
/// [`Display`](std::fmt::Display) names the coordinate for the reader's refusals exactly
/// as `graph_contract::ingest::IngestError` does.
#[derive(Debug, Clone, PartialEq)]
pub enum ContractError {
    /// The buffer is not UTF-8. Checked before parsing, like every other reader here.
    Utf8,
    /// The document is not one the contract accepts (`graph_contract::ingest::read`).
    Document(IngestError),
    /// The document is a valid contract, and the graph it describes cannot be derived.
    Derivation(BuildError),
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8 => write!(f, "the buffer is not UTF-8"),
            Self::Document(err) => write!(f, "{err}"),
            Self::Derivation(err) => write!(f, "{err}"),
        }
    }
}

/// Derives `bytes` — one contract document — into the graph and its indexed topology, or
/// the refusal.
///
/// Both steps are `graph_core`'s: the reader is the contract's own, the derivation is
/// `graph_core::ingest::build_topology`, so the topology a caller gets here is the same
/// one `graph-cli ingest` and the convergence fixture are pinned against.
pub fn derive(bytes: &[u8]) -> Result<(Derived, Topology), ContractError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ContractError::Utf8)?;
    let document = read(text).map_err(ContractError::Document)?;
    build_topology(&document).map_err(ContractError::Derivation)
}
