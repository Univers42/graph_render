//! F-80: `weight`, `version` and `strength` are read as any finite number, with no range.
//! What that costs is pinned here: every registered layout and every POST pass over a
//! graph carrying the extremes JSON admits either answers or refuses, and never panics
//! (a panic is a trap that takes the whole wasm instance down with it).

use super::{doc, edge_json, node_json};
use crate::ingest::read;
use graph_core::index_model;
use graph_core::registry::LAYOUTS;

const WEIGHTS: [&str; 4] = ["1e300", "-1e308", "0.0", "5e-324"];
const STRENGTHS: [&str; 4] = ["1e300", "-1e300", "0.0", "-1.0"];

fn extreme_topology() -> graph_core::Topology {
    let ids = ["a", "b", "c", "d"];
    let nodes: Vec<String> = ids
        .iter()
        .zip(WEIGHTS)
        .map(|(id, weight)| {
            node_json(id).replace("\"weight\":0.5", &format!("\"weight\":{weight}"))
        })
        .map(|node| node.replace("\"version\":0.0", "\"version\":-1e308"))
        .collect();
    let pairs = [("a", "b"), ("b", "c"), ("c", "d"), ("d", "a")];
    let edges: Vec<String> = pairs
        .iter()
        .zip(STRENGTHS)
        .enumerate()
        .map(|(i, ((s, t), strength))| {
            edge_json(&format!("e{i}"), s, t)
                .replace("\"strength\":0.5", &format!("\"strength\":{strength}"))
        })
        .collect();
    let (nodes, edges) = read(doc(&nodes, &edges).as_bytes()).expect("finite, so accepted");
    index_model(&nodes, &edges).expect("indexes")
}

#[test]
fn extreme_weights_and_strengths_never_panic_a_layout_or_a_post_pass() {
    let topology = extreme_topology();
    let mut answered = 0;
    for layout in LAYOUTS {
        let Ok(geometry) = (layout.run)(&topology) else {
            continue;
        };
        // `Snapshot::new` is D9's check: a non-finite coordinate is an `Err`, not bytes.
        if graph_core::layout::snapshot(&topology, geometry.clone()).is_err() {
            continue;
        }
        answered += 1;
        for post in 0..crate::post::count() {
            let ran = crate::post::snapshot(post, &topology, &geometry);
            assert!(ran.is_some(), "{}: post {post} is registered", layout.id);
        }
    }
    assert!(
        answered > 0,
        "no layout answered at all: the test checked nothing"
    );
}

#[test]
fn extreme_weights_and_strengths_never_panic_an_analysis() {
    let topology = extreme_topology();
    for (index, entry) in crate::analysis::ANALYSES.iter().enumerate() {
        let index = u32::try_from(index).expect("small");
        let ran = std::panic::catch_unwind(|| {
            crate::analysis::run(index, &topology).map(|r| r.to_json())
        });
        assert!(ran.is_ok(), "{}: panicked over the extremes", entry.id);
    }
}

#[test]
fn a_negative_strength_refuses_the_weighted_centralities_instead_of_answering() {
    let topology = extreme_topology();
    for id in [
        graph_core::analysis::centrality::CLOSENESS,
        graph_core::analysis::centrality::BETWEENNESS,
        graph_core::analysis::centrality::EIGENVECTOR,
    ] {
        let index = crate::analysis::ANALYSES
            .iter()
            .position(|entry| entry.id == id);
        let index = u32::try_from(index.expect("registered")).expect("small");
        assert_eq!(
            crate::analysis::to_json(index, &topology),
            None,
            "{id} answered"
        );
    }
}
