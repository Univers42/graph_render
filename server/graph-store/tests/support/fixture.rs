//! The request values every store case builds: the limits, a manifest, a batch.
//!
//! WHY here and not in each test binary: three binaries spelled these the same way, and a body a
//! case builds by hand is a second reader of the wire format. Each one goes through graph-contract's
//! own reader instead, so a case cannot store something the hub would have refused.

use graph_contract::hub::batch::{Batch, read_batch};
use graph_contract::hub::{Limits, Manifest, read_manifest};

/// The `Limits` every case reads a batch under.
pub const LIMITS: Limits = Limits::DEFAULT;

/// A manifest read for `plugin`, through graph-contract's own reader.
///
/// WHY read the text rather than build the struct: a `Manifest` has no public constructor that
/// skips the qualification pass, and a case that built one by hand would store link targets the
/// reader never qualified — which is exactly the mistake the store must not be able to make.
pub fn manifest_of(text: &str, plugin: &str) -> Manifest {
    read_manifest(text, plugin).unwrap_or_else(|e| panic!("the manifest reads: {e}"))
}

/// One batch: `upserts` as `(collection, id, updatedAt, cells)` and `deletes` as
/// `(collection, id)`, read through graph-contract's own reader so no case spells a body twice.
pub fn batch_of(upserts: &[(&str, &str, u32, &str)], deletes: &[(&str, &str)]) -> Batch {
    let ups = upserts
        .iter()
        .map(|(c, id, at, cells)| {
            format!(r#"{{"collection":"{c}","id":"{id}","updatedAt":{at},"values":{{{cells}}}}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");
    let dels = deletes
        .iter()
        .map(|(c, id)| format!(r#"{{"collection":"{c}","id":"{id}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    read_batch(
        &format!(r#"{{"upserts":[{ups}],"deletes":[{dels}]}}"#),
        &LIMITS,
    )
    .expect("the batch reads")
}
