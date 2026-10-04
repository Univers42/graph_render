use super::densify;
use super::rss::parse_status_kib;
use graph_core::{EdgeKind, REFERENCE_DEGREE, seeded_model};
use std::collections::BTreeSet;

#[test]
fn densify_reaches_the_ratio_without_self_edges_or_repeated_ids() {
    let (nodes, mut edges) = seeded_model(3, 1000, REFERENCE_DEGREE);
    let before = edges.len();
    densify(&nodes, &mut edges, 4.0);
    assert_eq!(edges.len(), 4000);
    assert!(edges[before..].iter().all(|e| e.source != e.target));
    assert!(edges[before..].iter().all(|e| e.kind == EdgeKind::Relation));
    let ids: BTreeSet<&str> = edges.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), edges.len(), "an appended id collides");
}

#[test]
fn densify_keeps_the_seeded_edges_and_is_deterministic() {
    let (nodes, seeded) = seeded_model(5, 300, REFERENCE_DEGREE);
    let (mut first, mut second) = (seeded.clone(), seeded.clone());
    densify(&nodes, &mut first, 3.0);
    densify(&nodes, &mut second, 3.0);
    assert_eq!(first, second);
    assert_eq!(first[..seeded.len()], seeded[..]);
}

#[test]
fn densify_spreads_the_new_edges_over_every_node() {
    let (nodes, mut edges) = seeded_model(1, 512, REFERENCE_DEGREE);
    let before = edges.len();
    densify(&nodes, &mut edges, 4.0);
    let touched: BTreeSet<&str> = edges[before..]
        .iter()
        .flat_map(|e| [e.source.as_str(), e.target.as_str()])
        .collect();
    assert_eq!(touched.len(), nodes.len());
}

#[test]
fn densify_below_the_current_ratio_appends_nothing() {
    let (nodes, mut edges) = seeded_model(2, 400, REFERENCE_DEGREE);
    let before = edges.clone();
    densify(&nodes, &mut edges, 1.0);
    assert_eq!(edges, before);
}

#[test]
fn a_status_field_is_read_in_kib_and_a_missing_one_is_none() {
    let status = "Name:\tgraph-cli\nVmHWM:\t    1792 kB\nVmRSS:\t  904 kB\n";
    assert_eq!(parse_status_kib(status, "VmHWM"), Some(1792));
    assert_eq!(parse_status_kib(status, "VmRSS"), Some(904));
    assert_eq!(parse_status_kib(status, "VmPeak"), None);
    assert_eq!(parse_status_kib("VmHWMx:\t5 kB\n", "VmHWM"), None);
}

/// One read of the file: two reads would let another test thread grow the resident size
/// between them.
#[test]
fn this_process_reports_a_high_water_mark_at_least_its_resident_size() {
    let status = std::fs::read_to_string("/proc/self/status").expect("Linux: the probe's host");
    let (Some(hwm), Some(rss)) = (
        parse_status_kib(&status, "VmHWM"),
        parse_status_kib(&status, "VmRSS"),
    ) else {
        panic!("/proc/self/status has no VmHWM or VmRSS: the probe cannot measure here");
    };
    assert!(hwm >= rss && rss > 0, "VmHWM {hwm} kB, VmRSS {rss} kB");
}

#[test]
fn the_edges_per_node_flag_refuses_what_densify_cannot_take() {
    assert_eq!(super::edges_per_node("4"), Ok(4.0));
    for bad in ["-1", "65", "NaN", "inf", "four"] {
        assert!(super::edges_per_node(bad).is_err(), "{bad} was accepted");
    }
}
