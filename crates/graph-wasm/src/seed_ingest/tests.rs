use super::*;
use crate::ingest;
use graph_core::{gate_node_count, index_model, seeded_model};

#[test]
fn a_seeds_document_reads_back_to_the_same_node_and_edge_ids_in_order() {
    for seed in [0u32, 1, 5, 37] {
        let (nodes, edges) =
            seeded_model(seed, gate_node_count(seed), graph_core::REFERENCE_DEGREE);
        let text = document(&nodes, &edges);
        let (read_nodes, read_edges) = ingest::read(text.as_bytes())
            .unwrap_or_else(|err| panic!("seed {seed} did not round-trip: {err:?}"));
        assert_eq!(
            read_nodes, nodes,
            "seed {seed}: nodes differ after the round trip"
        );
        assert_eq!(
            read_edges, edges,
            "seed {seed}: edges differ after the round trip"
        );
    }
}

#[test]
fn for_seed_indexes_to_the_same_counts_as_seeded_model_directly() {
    for seed in [0u32, 3, 21] {
        let (nodes, edges) =
            seeded_model(seed, gate_node_count(seed), graph_core::REFERENCE_DEGREE);
        let direct = index_model(&nodes, &edges).expect("fits");
        let (via_json, _) = ingest::read(for_seed(seed).as_bytes()).expect("round-trips");
        let via_json = index_model(&via_json, &edges).expect("fits");
        assert_eq!(direct.node_count(), via_json.node_count(), "seed {seed}");
        assert_eq!(direct.edge_count(), via_json.edge_count(), "seed {seed}");
    }
}

#[test]
fn special_characters_in_a_label_survive_the_round_trip() {
    let mut nodes = vec![graph_core::NodeRecord {
        id: "a".into(),
        kind: graph_core::NodeKind::Note,
        database_id: None,
        source: "s".into(),
        label: "quote \" backslash \\ newline \n tab \t".into(),
        group: Some("g".into()),
        weight: 0.25,
        version: 1.0,
        has_note: true,
        icon: Some("star".into()),
    }];
    let text = document(&nodes, &[]);
    let (read_back, _) = ingest::read(text.as_bytes()).expect("valid JSON");
    assert_eq!(read_back, nodes);
    nodes.clear();
    let empty = document(&nodes, &[]);
    assert_eq!(
        ingest::read(empty.as_bytes()).expect("empty is valid"),
        (vec![], vec![])
    );
}
