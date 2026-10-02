use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

fn path(n: u32) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "db")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// A ring of 8: dense indices 0..8, positions on a circle of radius 100, and a
/// viewport that keeps the left half of it.
pub(super) fn ring() -> (Topology, Vec<f64>, Vec<f64>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for i in 0..8_u32 {
        nodes.push(node(&format!("n{i}"), "db"));
        edges.push(edge(
            &format!("e{i}"),
            &format!("n{i}"),
            &format!("n{}", (i + 1) % 8),
        ));
    }
    let t = index_model(&nodes, &edges).expect("fits");
    let (x, y): (Vec<f64>, Vec<f64>) = (0..8)
        .map(|i| {
            let angle = f64::from(i) * std::f64::consts::FRAC_PI_4;
            (100.0 * libm::cos(angle), 100.0 * libm::sin(angle))
        })
        .unzip();
    (t, x, y)
}

pub(super) fn viewport() -> Viewport {
    Viewport {
        x0: -200.0,
        y0: -200.0,
        x1: 0.0,
        y1: 200.0,
        radius: 1.0,
    }
}

#[test]
fn a_node_is_visible_when_its_disc_straddles_the_rectangle_and_not_otherwise() {
    let v = viewport();
    assert!(
        meets(v, Some(-199.0), Some(0.0)),
        "just inside the left edge"
    );
    assert!(
        meets(v, Some(-201.0), Some(0.0)),
        "radius 1 still straddles"
    );
    assert!(!meets(v, Some(-202.0), Some(0.0)), "clear of the left edge");
    assert!(
        meets(v, Some(-100.0), Some(201.0)),
        "radius 1 still straddles the bottom edge"
    );
    assert!(
        !meets(v, Some(-100.0), Some(202.0)),
        "clear of the bottom edge"
    );
    assert!(meets(v, Some(0.0), Some(200.0)), "on the right edge");
    assert!(
        !meets(v, None, Some(0.0)),
        "a missing position is not visible"
    );
    assert!(!meets(v, Some(f64::NAN), Some(0.0)), "NaN is not visible");
    assert!(
        !meets(v, Some(f64::INFINITY), Some(0.0)),
        "infinity is not visible"
    );
}

#[test]
fn the_masks_have_one_entry_per_node_and_per_edge_and_cull_the_far_side() {
    let (t, x, y) = ring();
    let hints = hints(
        &t,
        &x,
        &y,
        &LodParams {
            viewport: viewport(),
            ..LodParams::default()
        },
    );
    assert_eq!(hints.visible.len(), t.node_count() as usize);
    assert_eq!(hints.edges.len(), t.edge_count() as usize);
    // The circle's left half: x <= 0 plus the node at angle 0 sitting on the edge.
    let visible: Vec<u32> = (0..8)
        .filter(|&i| hints.visible[i as usize] == 1)
        .map(|i| i as u32)
        .collect();
    assert_eq!(visible, vec![2, 3, 4, 5, 6]);
    // Every kept edge has two visible endpoints, and none was decimated at this tier.
    for e in 0..t.edge_count() as usize {
        if hints.edges[e] == 1 {
            assert_eq!(hints.visible[t.edges().source[e] as usize], 1);
            assert_eq!(hints.visible[t.edges().target[e] as usize], 1);
        }
    }
    assert!(
        hints.edges.contains(&1),
        "the ring has visible edges to draw"
    );
}

/// The reference's never-empty guarantee (`lod.py:88-90`): the smallest budget, 1,
/// still lets the single most important visible node keep its label (0 is no limit:
/// `mask_tests.rs`).
#[test]
fn the_label_budget_is_never_empty_and_ranks_by_degree_then_index() {
    let (t, x, y) = ring();
    let first = hints(
        &t,
        &x,
        &y,
        &LodParams {
            viewport: viewport(),
            label_budget: 1,
            ..LodParams::default()
        },
    );
    assert_eq!(
        first.labelled,
        vec![0, 0, 1, 0, 0, 0, 0, 0],
        "node 2 wins on (degree, index)"
    );
    let capped = hints(
        &t,
        &x,
        &y,
        &LodParams {
            viewport: viewport(),
            label_budget: 2,
            ..LodParams::default()
        },
    );
    assert_eq!(capped.labelled.iter().sum::<u8>(), 2);
    assert_eq!(capped.tier, Tier::Full);
}

/// The Ponytail's failing input, asserted rather than described: a graph whose only
/// important node is low-degree. The hub keeps its label, the entry point does not.
#[test]
fn a_low_degree_meaningful_node_loses_its_label_to_a_higher_degree_one() {
    let nodes = [
        node("hub", "db"),
        node("a", "db"),
        node("b", "db"),
        node("entry", "db"),
    ];
    let edges = [
        edge("e0", "hub", "a"),
        edge("e1", "hub", "b"),
        edge("e2", "hub", "entry"),
    ];
    let t = index_model(&nodes, &edges).expect("fits");
    let hints = hints(
        &t,
        &[0.0, 1.0, 2.0, 3.0],
        &[0.0, 0.0, 0.0, 0.0],
        &LodParams {
            label_budget: 1,
            ..LodParams::default()
        },
    );
    assert_eq!(hints.labelled[0], 1, "the hub wins on degree");
    assert_eq!(
        hints.labelled[3], 0,
        "the entry point is hidden: the named failure"
    );
}

#[test]
fn the_tier_ladder_is_pinned_and_its_edges_follow_it() {
    let params = LodParams::default();
    let cases = [
        (2_000, Tier::Full),
        (2_001, Tier::NoLabels),
        (20_000, Tier::NoLabels),
        (20_001, Tier::Decimated),
        (200_000, Tier::Decimated),
        (200_001, Tier::Clustered),
    ];
    for (n, want) in cases {
        assert_eq!(params.tier(n), want, "n={n}");
    }
    let t = path(40);
    let (x, y): (Vec<f64>, Vec<f64>) = (0..40).map(|i| (f64::from(i), 0.0)).unzip();
    let full = hints(&t, &x, &y, &params);
    assert_eq!(full.edges.iter().sum::<u8>(), t.edge_count() as u8);
    assert_eq!(
        full.labelled.iter().sum::<u8>(),
        40,
        "40 visible nodes fit the budget of 64"
    );
    let decimated = hints(
        &t,
        &x,
        &y,
        &LodParams {
            full_nodes: 5,
            no_label_nodes: 20,
            decimated_nodes: 100,
            ..params
        },
    );
    assert_eq!(decimated.tier, Tier::Decimated);
    assert_eq!(decimated.edges.iter().sum::<u8>(), 5, "one edge in 8 of 39");
    let clustered = hints(
        &t,
        &x,
        &y,
        &LodParams {
            full_nodes: 2,
            no_label_nodes: 20,
            decimated_nodes: 10,
            ..params
        },
    );
    assert_eq!(clustered.tier, Tier::Clustered);
    assert_eq!(
        clustered.edges.iter().sum::<u8>(),
        0,
        "no edges past the last threshold"
    );
    let unlabelled = hints(
        &t,
        &x,
        &y,
        &LodParams {
            full_nodes: 2,
            no_label_nodes: 100,
            ..params
        },
    );
    assert_eq!(
        unlabelled.labelled.iter().sum::<u8>(),
        0,
        "no labels in the NoLabels tier"
    );
}

/// The phase's own requirement: the topology is untouched. Same node and edge counts,
/// same bytes in the columns, whatever the hints say.
#[test]
fn the_hints_never_mutate_the_topology() {
    let t = path(30);
    let before = (
        t.node_count(),
        t.edge_count(),
        t.nodes().byte_len(),
        t.edges().byte_len(),
    );
    let (x, y): (Vec<f64>, Vec<f64>) = (0..30).map(|i| (f64::from(i) * 1e9, 0.0)).unzip();
    let culled = hints(
        &t,
        &x,
        &y,
        &LodParams {
            full_nodes: 1,
            decimated_nodes: 1,
            ..LodParams::default()
        },
    );
    assert!(
        culled.visible.iter().sum::<u8>() < 30,
        "most nodes are culled here"
    );
    assert_eq!(
        (
            t.node_count(),
            t.edge_count(),
            t.nodes().byte_len(),
            t.edges().byte_len()
        ),
        before
    );
}
