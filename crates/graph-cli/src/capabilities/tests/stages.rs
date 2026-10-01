//! The hashgate record's shape, restated for the ledger fixtures.

use serde_json::{Value, json};

/// Every hashgate stage's key, in `hashgate::STAGES` order, so this fixture's `equal`
/// maps can be built at the same shape a real record has, without importing the
/// hashgate module just for the constant.
const STAGES: [&str; 22] = [
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
    "layout.random.3d",
    "layout.spiral.3d",
    "layout.bipartite.3d",
    "layout.spectral.3d",
    "layout.mds.pivot.3d",
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

/// The spectral family, 2D and 3D — the ids whose ledger row is `oracle-spectral`. The 3D
/// arms route with their 2D siblings: same harness, same record shape, at `dims = 3`
/// (`registry/layout_row.rs`'s `SCIPY_ORACLE_LAYOUTS`, which is the routing this restates).
/// The three 3D *closed-form* arms are deliberately absent: they fall through to
/// `roundtrip`, exactly as their 2D siblings do.
pub(super) fn is_spectral(id: &str) -> bool {
    matches!(
        id,
        "layout.spectral" | "layout.mds.pivot" | "layout.spectral.3d" | "layout.mds.pivot.3d"
    )
}
