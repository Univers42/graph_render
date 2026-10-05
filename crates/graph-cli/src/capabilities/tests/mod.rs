use super::verdict::{Evidence, MIN_SEEDS};
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod depth;
mod ledger;
use super::ceilings::{ceiling_coverage, ceiling_findings};
use ledger::find_row;
mod force;
mod graphviz;
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
/// control per stage that reaches nothing else at all: the two force layouts, the
/// four Phase 3 layouts and the one Graphviz packing layout, each of which now has a
/// control filed under its own stage id.
///
/// **Every gated row has a control here**, which is what makes this the honest set rather
/// than a convenient one: a `gated` row whose stage no control in this list diverges is
/// refused by `hash_4way`, so leaving osage out while shipping it `gated` would put two
/// permanent problems into every whole-ledger test below. `without_osage_control` is how a
/// test asks for the set *without* that one row's backing.
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
                "layout.spectral3d",
                "layout.mds.pivot3d",
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
        control(
            "hashgate-control-packing-osage-nodes",
            &["layout.packing.osage"],
        ),
    ]
}

/// The honest evidence with the one control that backs `layout.packing.osage` taken back out
/// — the set `hash_4way` refuses a `gated` osage row on.
///
/// A helper rather than a hand-built list so the removal is by **record name**, the one
/// `Knob::PackingOsageNodes::record()` returns: rebuilding the list instead would let a
/// rename drift and the test would keep passing on a control the real run no longer writes.
pub(super) fn without_osage_control(evidence: &mut Evidence) {
    evidence
        .controls
        .retain(|(name, _)| *name != "hashgate-control-packing-osage-nodes");
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
        by_name: BTreeMap::from([
            (
                "oracle-diff".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true,
                    "functions": functions
                }),
            ),
            (
                "roundtrip".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true,
                    "functions": {
                        "layout.grid": hand(7),
                        "layout.circular.radial": hand(6),
                        "layout.packing.circle": hand(5),
                        "layout.dag.sugiyama": hand(9),
                        "layout.dag.lanes": hand(5),
                    }
                }),
            ),
            (
                "oracle-layouts".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true,
                    "functions": {
                        "layout.tree.tidy": hand(9),
                        "layout.treemap.squarified": hand(11),
                    }
                }),
            ),
            (
                "stress".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true,
                    "functions": { "layout.force.barnes_hut": hand(4) }
                }),
            ),
            (
                "oracle-fa2".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true,
                    "functions": { "layout.forceatlas2": hand(4) }
                }),
            ),
            (
                "oracle-spectral".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true, "tolerance": true,
                    "functions": {
                        "layout.spectral": hand(12),
                        "layout.mds.pivot": hand(13),
                    }
                }),
            ),
            (
                "oracle-osage".to_owned(),
                json!({
                    "fingerprint": "tree", "seeds": 1000, "pass": true, "tolerance": true,
                    "functions": { "layout.packing.osage": hand(5) }
                }),
            ),
        ]),
    }
}

/// `name`'s record as a passing 1000-seed run on this tree whose one function is `id` and
/// whose verdict is a measured ceiling rather than a byte comparison — the shape every
/// record a `tolerance: true` differential writes has.
pub(super) fn recorded(evidence: &mut Evidence, name: &str, id: &str) {
    evidence.by_name.insert(
        name.to_owned(),
        json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "tolerance": true,
            "functions": { id: hand(5) }
        }),
    );
}

/// `name`'s record, or the failure to read it, out of `evidence`.
pub(in crate::capabilities) fn record_of<'a>(
    evidence: &'a Evidence,
    name: &str,
) -> Option<&'a Value> {
    evidence.by_name.get(name)
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
