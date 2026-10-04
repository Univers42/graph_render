//! Which registry ids the manifest has to carry: every layout and POST id whose committed cap
//! admits the fixture (`docs/contract/service-api.md` condition 7).
//!
//! The set is computed here, from `Caps::committed()` and the motor's own registries, so
//! dropping a row from `manifest.json` fails this test rather than shrinking what the digest
//! proves. Caps are the service's, not the manifest's: a graph the service would answer 413
//! for is not a row a digest may pin.

use crate::manifest::Entry;
use graph_server::caps::Caps;
use graph_server::motor::{self, Source};
use std::collections::{BTreeMap, BTreeSet};

/// The manifest's rows grouped by fixture, each group in file order. A `BTreeMap`, so the
/// fixtures are visited in a fixed order and a failure names them the same way twice.
pub fn by_fixture<'a>(entries: &'a [Entry]) -> BTreeMap<&'a str, Vec<&'a Entry>> {
    let mut grouped: BTreeMap<&'a str, Vec<&'a Entry>> = BTreeMap::new();
    for entry in entries {
        grouped.entry(&entry.fixture).or_default().push(entry);
    }
    grouped
}

/// The size `bytes` builds to under `source`: the numbers the caps are checked against, read
/// off the topology the seam built rather than off the document, so a reader that drops a node
/// is caught by the caps and not only by the hash.
pub fn size(source: Source, bytes: &[u8]) -> Result<(u32, u32), String> {
    let topology = motor::build(bytes, source).map_err(|code| refused(code))?;
    Ok((topology.node_count(), topology.edge_count()))
}

/// The ids of `ids` whose cap admits a graph of `size`, sorted. An id with no row in the table
/// admits nothing: a missing cap row means 0, never "uncapped" (`Caps::admit`).
pub fn admitting<'a>(ids: impl Iterator<Item = &'a str>, caps: &Caps, size: (u32, u32)) -> BTreeSet<String> {
    ids.filter(|id| admits(caps, id, size)).map(str::to_owned).collect()
}

/// Whether `id`'s committed cap admits a graph of `size`.
pub fn admits(caps: &Caps, id: &str, size: (u32, u32)) -> bool {
    let cap = caps.get(id).unwrap_or(graph_server::caps::Cap { nodes: 0, edges: 0 });
    u64::from(size.0) <= cap.nodes && u64::from(size.1) <= cap.edges
}

/// The refusal a build answers with, as a string: a fixture no reader accepts has no size and
/// so admits nothing, which the coverage test reports rather than passing over.
fn refused(code: motor::Code) -> String {
    format!("refused:{}", code.name())
}