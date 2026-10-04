//! Running one manifest row the way a request runs it, and hashing what comes back.
//!
//! The row goes through `graph_server::motor`, which re-exports `graph_wasm::service`: the
//! same `build` and `run` the wasm exports `gm_build`/`gm_build_contract` and `gm_run`/
//! `gm_post_run` call (`docs/contract/service-api.md` condition 1). Nothing here re-derives a
//! layout or a snapshot of its own, so a hash that moves means the motor moved.
//!
//! The digest is `sha256_hex` from `crates/graph-cli/src/runner.rs`, the hash gate's own:
//! `Sha256::digest(bytes)`, lowercase hex, over the bytes with no framing added.

use crate::manifest::Entry;
use graph_contract::binary::Snapshot;
use graph_contract::canonical_json;
use graph_server::motor::{self, Code};
use sha2::{Digest, Sha256};

/// One row's outcome: the binary face the motor produced, or the refusal it answered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The snapshot's binary bytes, as `gm_snapshot_bytes` would frame them.
    Bytes(Vec<u8>),
    /// `refused:<Code::name>`, the string the manifest pins for a refusal.
    Refused(String),
}

impl Outcome {
    /// The row's manifest digest. `GM_SVC_DIGEST_BREAK=1` flips one bit of the bytes first, so
    /// the row the negative control runs cannot pass by hashing nothing.
    pub fn hash(&self) -> String {
        match self {
            Outcome::Bytes(bytes) => sha256_hex(&broken(bytes)),
            Outcome::Refused(code) => code.clone(),
        }
    }

    /// The binary face, or the refusal that there is none.
    pub fn bytes(&self) -> Result<&[u8], String> {
        match self {
            Outcome::Bytes(bytes) => Ok(bytes),
            Outcome::Refused(code) => Err(code.clone()),
        }
    }
}

/// `bytes` read by `entry`'s reader, run through `entry`'s layout and POST pass.
pub fn run_row(entry: &Entry, bytes: &[u8]) -> Outcome {
    let topology = match motor::build(bytes, entry.source) {
        Ok(topology) => topology,
        Err(code) => return Outcome::Refused(refused(code)),
    };
    match motor::run(&topology, &entry.layout, entry.post.as_deref()) {
        Ok(snapshot) => Outcome::Bytes(snapshot),
        Err(code) => Outcome::Refused(refused(code)),
    }
}

/// The refusal string for `code`: the motor's own name, the one the SDK and the contract use.
pub fn refused(code: Code) -> String {
    format!("refused:{}", code.name())
}

/// Lowercase hex SHA-256 of `bytes`, no framing: `graph-cli`'s `sha256_hex`, copied rather
/// than re-derived so the digest this file pins is the digest the hash gate prints.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `bytes`, borrowed as they are, or with one bit flipped in the middle byte when
/// `GM_SVC_DIGEST_BREAK=1`. The negative control of condition 7: a row's hash must stop
/// matching the manifest the moment one bit of its snapshot moves. The copy is made only
/// under the break, so the normal run hashes the motor's own bytes without touching them.
fn broken(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if std::env::var("GM_SVC_DIGEST_BREAK").as_deref() != Ok("1") || bytes.is_empty() {
        return std::borrow::Cow::Borrowed(bytes);
    }
    let mut copy = bytes.to_vec();
    let at = copy.len() / 2;
    copy[at] ^= 0x01;
    std::borrow::Cow::Owned(copy)
}

/// Whether the canonical JSON face of `bytes` reads back to the same bytes: binary to JSON,
/// JSON to binary, and then the JSON of that, each hop equal to the last. Both hops, because
/// a reader that loses a field on the way out and a writer that drops one on the way back
/// cancel exactly once.
pub fn json_round_trips(bytes: &[u8]) -> Result<(), String> {
    let snapshot = Snapshot::from_bytes(bytes).map_err(|error| format!("not a snapshot: {error}"))?;
    let json = canonical_json::to_json(&snapshot);
    let read = canonical_json::from_json(&json).map_err(|error| format!("its own JSON: {error}"))?;
    let again = read.to_bytes();
    if again != bytes {
        return Err(format!("{} bytes in, {} back", bytes.len(), again.len()));
    }
    let written = canonical_json::to_json(&read);
    if written != json {
        return Err("the JSON face is not canonical: reading and writing it moved it".to_owned());
    }
    Ok(())
}