//! The hashgate record's shape, restated for the ledger fixtures.

use serde_json::{Value, json};

/// Every hashgate stage's key, in `hashgate::STAGES` order, so this fixture's `equal`
/// maps can be built at the same shape a real record has, without importing the
/// hashgate module just for the constant.
const STAGES: [&str; 17] = [
    "topology",
    "layout.grid",
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
    "layout.dag.sugiyama",
    "layout.random",
    "layout.circular.ring",
    "layout.spiral",
    "layout.bipartite",
    "layout.force.yifan_hu",
    "transport.wasm.columnar",
];

/// A hashgate-shaped `equal` map: `seeds` for every stage, except `diverged`'s, at `0`.
pub(super) fn equal_map(seeds: u64, diverged: &[&str]) -> Value {
    let map: serde_json::Map<String, Value> = STAGES
        .iter()
        .map(|&stage| {
            let count = if diverged.contains(&stage) { 0 } else { seeds };
            (stage.to_owned(), json!(count))
        })
        .collect();
    Value::Object(map)
}
