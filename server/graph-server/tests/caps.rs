//! The caps table (Verdict condition 2): exactly one row per id the service runs, none for an
//! id it does not, and no cap past the ceiling the ledger records for that id.

use graph_server::caps::Caps;
use graph_server::motor;
use std::collections::BTreeSet;

/// The placeholder rule of `src/caps.rs`, for the message of a missing row.
const PLACEHOLDER_NODES: u64 = 1000;
const PLACEHOLDER_EDGES: u64 = 4000;

/// The ledger's ceiling of `id`: a layout's counts nodes, a POST pass's counts edges.
/// `post.route.grid` has none in graph-core (graph-cli holds its 5000-node ceiling).
fn ceiling(id: &str) -> (Option<u64>, Option<u64>) {
    if let Some(layout) = graph_core::registry::find(id) {
        return (Some(layout.meta.scale_ceiling), None);
    }
    if let Some(post) = graph_core::post::find(id) {
        return (None, Some(post.meta.scale_ceiling));
    }
    let style = graph_core::post::styles::find(id);
    (None, style.map(|style| style.meta.scale_ceiling))
}

fn placeholder_row(id: &str) -> String {
    let (nodes, edges) = match ceiling(id) {
        (Some(nodes), _) => (
            nodes.min(PLACEHOLDER_NODES),
            4 * nodes.min(PLACEHOLDER_NODES),
        ),
        (None, edges) => (
            PLACEHOLDER_NODES,
            edges.map_or(PLACEHOLDER_EDGES, |e| e.min(PLACEHOLDER_EDGES)),
        ),
    };
    format!("{id}\t{nodes}\t{edges}")
}

#[test]
fn every_service_id_has_exactly_one_row() {
    let caps = Caps::committed().expect("the committed table parses");
    let have: BTreeSet<&str> = caps.ids().collect();
    let wanted: Vec<&str> = motor::layout_ids().chain(motor::post_ids()).collect();
    let missing: Vec<String> = wanted
        .iter()
        .filter(|id| !have.contains(*id))
        .map(|id| placeholder_row(id))
        .collect();
    assert!(
        missing.is_empty(),
        "rows missing; the placeholder rule gives:\n{}",
        missing.join("\n")
    );
    let wanted: BTreeSet<&str> = wanted.into_iter().collect();
    let extra: Vec<&&str> = have.difference(&wanted).collect();
    assert!(
        extra.is_empty(),
        "rows for ids the service does not run: {extra:?}"
    );
}

#[test]
fn no_cap_is_past_its_ledger_ceiling() {
    let caps = Caps::committed().expect("the committed table parses");
    for id in caps.ids() {
        let cap = caps.get(id).expect("a listed id has a row");
        let (nodes, edges) = ceiling(id);
        assert!(
            nodes.is_none_or(|ceiling| cap.nodes <= ceiling),
            "{id}: {cap:?} > {nodes:?} nodes"
        );
        assert!(
            edges.is_none_or(|ceiling| cap.edges <= ceiling),
            "{id}: {cap:?} > {edges:?} edges"
        );
    }
}

#[test]
fn a_second_row_for_one_id_is_refused_by_line() {
    let table = "id\tcap_n\tcap_m\nforce\t1\t1\nforce\t2\t2\n";
    let refused = Caps::parse(table).expect_err("a duplicate is refused");
    assert!(refused.contains("line 3"), "{refused}");
}

#[test]
fn a_malformed_row_is_refused_by_line() {
    for (table, line) in [
        ("id\tcap_n\tcap_m\nforce\t1\n", "line 2"),
        ("id\tcap_n\tcap_m\nforce\t1\t-1\n", "line 2"),
        ("id cap_n cap_m\n", "first line"),
    ] {
        let refused = Caps::parse(table).expect_err("a malformed table is refused");
        assert!(refused.contains(line), "{table:?}: {refused}");
    }
}
