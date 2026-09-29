use super::{YifanHu, hierarchy};
use crate::index::{Topology, empty_model, index_model};
use crate::layout::force::{BarnesHut, ForceParams, simple_graph};
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

fn path(n: u32) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn points(t: &Topology, multilevel: bool) -> (Vec<f32>, Vec<f32>) {
    let p = ForceParams::default();
    let g = if multilevel {
        YifanHu::run(t, &p)
    } else {
        BarnesHut::run(t, &p)
    };
    match g.expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        _ => panic!("force layout is point geometry"),
    }
}

#[test]
fn a_path_of_200_coarsens_by_halves_down_to_13() {
    let t = path(200);
    let (levels, maps) = hierarchy(simple_graph(&t), 200);
    let sizes: Vec<u32> = levels.iter().map(|l| l.1).collect();
    assert_eq!(sizes, [200, 100, 50, 25, 13]);
    assert_eq!(maps.len(), 4);
}

#[test]
fn the_multilevel_layout_is_finite_and_repeatable() {
    let t = path(200);
    let (x, y) = points(&t, true);
    assert_eq!((x.len(), y.len()), (200, 200));
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
    assert_eq!(points(&t, true), (x, y));
}

#[test]
fn it_is_a_different_layout_from_the_single_level_solve() {
    let t = path(200);
    assert_ne!(points(&t, true), points(&t, false));
}

#[test]
fn neighbours_end_up_nearer_than_the_average_pair() {
    let t = path(200);
    let (x, y) = points(&t, true);
    let d = |a: usize, b: usize| libm::hypot(f64::from(x[a] - x[b]), f64::from(y[a] - y[b]));
    let edge_mean: f64 = (1..200).map(|i| d(i - 1, i)).sum::<f64>() / 199.0;
    let pair_mean: f64 = (0..200).map(|i| d(i, (i + 100) % 200)).sum::<f64>() / 200.0;
    assert!(edge_mean * 3.0 < pair_mean, "{edge_mean} vs {pair_mean}");
}

#[test]
fn empty_single_and_edgeless_graphs_are_handled() {
    assert!(points(&empty_model(), true).0.is_empty());
    let one = points(&path(1), true);
    assert!(one.0[0].is_finite() && one.1[0].is_finite());
    let nodes: Vec<_> = (0..40).map(|i| node(&format!("n{i}"), "")).collect();
    let t = index_model(&nodes, &[]).expect("fits");
    let (x, y) = points(&t, true);
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn a_disconnected_graph_stays_finite() {
    let nodes: Vec<_> = (0..60).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (0..60)
        .filter(|i| i % 30 != 29)
        .map(|i| edge(&format!("e{i}"), &format!("n{i}"), &format!("n{}", i + 1)))
        .collect();
    let t = index_model(&nodes, &edges[..edges.len() - 1]).expect("fits");
    let (x, y) = points(&t, true);
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

/// Pins the exact multilevel output on a 40-node path (three levels), as f32 bit
/// patterns so a change in coarsening, tick count or restart alpha cannot pass as
/// "still finite". Regenerate only on a deliberate change, recorded in a decision.
#[test]
fn the_output_on_a_path_of_40_is_pinned() {
    let (x, y) = points(&path(40), true);
    let bits =
        |v: &[f32]| -> Vec<u32> { [0, 13, 26, 39].iter().map(|&i| v[i].to_bits()).collect() };
    assert_eq!(bits(&x), [1132881269, 1125832160, 1111216345, 3275666193]);
    assert_eq!(bits(&y), [3260434285, 1129122063, 1127494705, 1127018024]);
}
