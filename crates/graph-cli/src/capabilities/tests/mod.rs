use super::verdict::{Evidence, MIN_SEEDS};
use super::*;
use serde_json::{Value, json};

mod depth;
mod ledger;
use super::ceilings::{ceiling_coverage, ceiling_findings};
use ledger::find_row;
mod force;
mod refusals;
mod registry;
mod scale;
mod stages;
mod sugiyama;
mod transport;

use stages::equal_map;

/// The 17 oracle functions of `prompt.md` §7.4, plus the H4 and H9 arms.
const COVERED: [&str; 19] = [
    "indexModel",
    "emptyModel",
    "nodesEqual",
    "makeRecordNodeId",
    "makeNoteNodeId",
    "makeTagNodeId",
    "makeEdgeId",
    "parseNodeId",
    "applyDegreeWeights",
    "edgeKindFromType",
    "diffGraph",
    "isEmptyPatch",
    "edgesEqual",
    "deriveLegend",
    "neighborhood",
    "neighborhoodEdges",
    "buildSyntheticModel",
    "hashString",
    "layoutGroups",
];

/// One `{cases, declared: 0, unexplained: 0}` function entry.
fn hand(cases: u64) -> Value {
    json!({ "cases": cases, "declared": 0, "unexplained": 0 })
}

/// One control's record: red, over `seeds` seeds, diverging exactly `diverged`'s stages.
fn control(name: &'static str, diverged: &[&str]) -> (&'static str, Option<Value>) {
    (
        name,
        Some(json!({
            "fingerprint": "tree", "seeds": 8, "pass": false,
            "equal": equal_map(8, diverged)
        })),
    )
}

/// The controls: reference degree (topology, treemap — it reads node weight), grid
/// spacing (grid alone), layer spacing (the layered drawing alone), node count —
/// restricted here to the layouts no other control reaches, since reference degree and
/// grid spacing already back topology/grid/treemap on their own (a real run may show it
/// diverging those too; the ledger only needs one control per stage to hold) — and one
/// control per stage that reaches nothing else at all: the two force layouts, and the
/// four Phase 3 layouts, each of which now has a control filed under its own stage id.
fn honest_controls() -> Vec<(&'static str, Option<Value>)> {
    vec![
        control(
            "hashgate-control-reference-degree",
            &["topology", "layout.treemap.squarified"],
        ),
        control(
            "hashgate-control-grid-spacing",
            &["layout.grid", "transport.wasm.columnar"],
        ),
        control(
            "hashgate-control-sugiyama-layer-spacing",
            &["layout.dag.sugiyama"],
        ),
        control(
            "hashgate-control-node-count",
            &[
                "layout.tree.tidy",
                "layout.treemap.squarified",
                "layout.circular.radial",
                "layout.packing.circle",
                "layout.spectral",
                "layout.mds.pivot",
                "layout.random.3d",
                "layout.spiral.3d",
                "layout.bipartite.3d",
                "layout.spectral.3d",
                "layout.mds.pivot.3d",
            ],
        ),
        control("hashgate-control-force-theta", &["layout.force.barnes_hut"]),
        control(
            "hashgate-control-fa2-scaling-ratio",
            &["layout.forceatlas2"],
        ),
        control("hashgate-control-tree-tidy-nodes", &["layout.tree.tidy"]),
        control(
            "hashgate-control-treemap-nodes",
            &["layout.treemap.squarified"],
        ),
        control(
            "hashgate-control-circular-nodes",
            &["layout.circular.radial"],
        ),
        control("hashgate-control-packing-scale", &["layout.packing.circle"]),
    ]
}

fn honest() -> Evidence {
    let functions: serde_json::Map<String, Value> =
        COVERED.iter().map(|f| ((*f).to_owned(), hand(5))).collect();
    Evidence {
        fingerprint: "tree".into(),
        hashgate: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "equal": equal_map(1000, &[]),
            "transport": {
                "stage": "transport.wasm.columnar", "reference": "layout.grid",
                "equal": 1000
            }
        })),
        controls: honest_controls(),
        oracle: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "functions": functions
        })),
        roundtrip: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": {
                "layout.grid": hand(7),
                "layout.circular.radial": hand(6),
                "layout.packing.circle": hand(5),
                "layout.dag.sugiyama": hand(9),
                // The three 3D *closed-form* arms route to `roundtrip`
                // (`registry/layout_row.rs`'s fall-through, same as their 2D siblings), so
                // a record that backs every gated row carries a case for each of them too.
                // The two 3D spectral arms are NOT here: they route to `oracle-spectral`
                // with `layout.spectral` / `layout.mds.pivot` and are in the `spectral`
                // fixture below.
                "layout.random.3d": hand(5),
                "layout.spiral.3d": hand(5),
                "layout.bipartite.3d": hand(5),
            }
        })),
        layouts: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": {
                "layout.tree.tidy": hand(9),
                "layout.treemap.squarified": hand(11),
            }
        })),
        stress: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": { "layout.force.barnes_hut": hand(4) }
        })),
        fa2: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": { "layout.forceatlas2": hand(4) }
        })),
        spectral: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "tolerance": true,
            "functions": {
                "layout.spectral": hand(12),
                "layout.mds.pivot": hand(13),
                // The two 3D arms of the same differential, at `dims = 3`. Case counts
                // are the measured per-component comparisons over the 1000 gate seeds:
                // 982 and 990 against 984 and 996 for the 2D arms
                // (`docs/measurements/p12-t4a.md`).
                "layout.spectral.3d": hand(12),
                "layout.mds.pivot.3d": hand(13),
            }
        })),
    }
}

/// One real row, found by id and restated at `status`, for the tests that need a row
/// the registry does not have. By id, not by index: a registry entry inserted above it
/// would otherwise hand these tests a different row and the assertion would keep
/// passing for the wrong reason.
fn row(status: Status) -> Capability {
    let mut row = find_row_by_id("topology.index");
    row.status = status;
    row
}

/// `registry()`'s row `id`, by id. Panics rather than returning a default: a test that
/// cannot find the row it is about is a test that must not pass.
pub(super) fn find_row_by_id(id: &str) -> Capability {
    registry()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no {id} row"))
}
