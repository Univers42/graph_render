//! SCRATCH review harness - not part of the deliverable, deleted before return.

use super::graphopt::{Graphopt, GraphoptParams};
use super::quadtree::Quadtree;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};
use crate::stage::Stage;

fn graph(n: u32, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn a_negative_displacement_cap_panics() {
    let params = GraphoptParams {
        max_sa_movement: -1.0,
        ..GraphoptParams::default()
    };
    let r = Graphopt::run(&graph(2, &[(0, 1)]), &params);
    println!("negative max_sa_movement returned {r:?}");
}

#[test]
fn a_zero_node_mass_returns_nan() {
    let params = GraphoptParams {
        node_mass: 0.0,
        ..GraphoptParams::default()
    };
    let r = Graphopt::run(&graph(2, &[(0, 1)]), &params);
    println!("zero node_mass returned {:?}", r.is_ok());
}

#[test]
fn quadtree_hangs_on_a_coordinate_past_2_pow_53() {
    let mut tree = Quadtree::default();
    println!("about to build with x=1e17");
    tree.build(&[1e17, 0.0], &[0.0, 1.0]);
    println!("built, cells={}", tree.cells().len());
}

#[test]
fn quadtree_hangs_on_an_infinite_coordinate() {
    let mut tree = Quadtree::default();
    println!("about to build with x=inf");
    tree.build(&[f64::INFINITY, 0.0], &[0.0, 1.0]);
    println!("built, cells={}", tree.cells().len());
}

#[test]
fn quadtree_hangs_when_two_points_cannot_be_separated() {
    let mut tree = Quadtree::default();
    println!("about to build with two huge far-apart points");
    tree.build(&[1e17, 9e299], &[0.0, 0.0]);
    println!("built, cells={}", tree.cells().len());
}

#[test]
fn davidson_harel_bounds_keep_the_f64_min_sentinel() {
    // Verbatim copy of davidson_harel.rs:264 `bounding` and :272 `grow`, so the
    // reproduction runs the shipped arithmetic, not a paraphrase.
    fn grow(b: &mut [f64; 4], p: [f64; 2]) {
        for a in 0..2 {
            if p[a] < b[a] {
                b[a] = p[a];
            } else if p[a] > b[a + 2] {
                b[a + 2] = p[a];
            }
        }
    }
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    // A scatter whose first draw is the largest on both axes: reachable at any n.
    for p in [[3.0, 4.0], [1.0, 2.0], [-1.0, -2.0]] {
        grow(&mut b, p);
    }
    println!("bounds = {b:?}");
    println!("max_x is f64::MIN sentinel: {}", b[2] == f64::MIN);
    let half = 5.0 * libm::sqrtf(3.0);
    let radius = 0.01 * (b[2] - b[0]).min(b[3] - b[1]);
    println!("fine radius = {radius} (half = {half})");
    assert!(b[2] == f64::MIN, "max_x was never promoted off the sentinel");
}

#[test]
fn an_fr_start_temp_of_nan_is_silently_accepted() {
    use super::fruchterman_reingold::{FrParams, FruchtermanReingold};
    let params = FrParams {
        start_temp: Some(f64::NAN),
        ..FrParams::default()
    };
    let r = FruchtermanReingold::run(&graph(4, &[(0, 1), (1, 2), (2, 3)]), &params);
    match r {
        Ok(g) => {
            use graph_contract::geometry::NodeGeometry;
            if let NodeGeometry::Point { x, .. } = g.nodes {
                println!("NaN start_temp accepted, x = {x:?}");
            }
        }
        Err(e) => println!("NaN start_temp refused: {e:?}"),
    }
}

#[test]
fn an_fr_negative_start_temp_inverts_the_step() {
    use super::fruchterman_reingold::{FrParams, FruchtermanReingold};
    let params = FrParams {
        start_temp: Some(-1.0),
        niter: 500,
        ..FrParams::default()
    };
    let r = FruchtermanReingold::run(&graph(4, &[(0, 1), (1, 2), (2, 3)]), &params);
    match r {
        Ok(g) => {
            use graph_contract::geometry::NodeGeometry;
            if let NodeGeometry::Point { x, .. } = g.nodes {
                println!("negative start_temp accepted, x = {x:?}");
            }
        }
        Err(e) => println!("negative start_temp refused: {e:?}"),
    }
}

#[test]
fn an_fr_run_that_overflows_f32_ships_inf() {
    use super::fruchterman_reingold::{FrParams, FruchtermanReingold};
    let params = FrParams {
        start_temp: Some(1e37),
        niter: 500,
        ..FrParams::default()
    };
    let r = FruchtermanReingold::run(&graph(2, &[]), &params);
    match r {
        Ok(g) => {
            use graph_contract::geometry::NodeGeometry;
            if let NodeGeometry::Point { x, y, .. } = g.nodes {
                println!("f64 finite -> f32 wire = x {x:?} y {y:?}");
            }
        }
        Err(e) => println!("refused: {e:?}"),
    }
}

#[test]
fn a_kamada_kawai_run_asks_for_an_80gb_distance_matrix() {
    use super::kamada_kawai::{KamadaKawai, KkParams};
    // No ceiling gate on this path; n below any plausible cap that still allocates n*n f64.
    let r = KamadaKawai::run(&graph(2, &[(0, 1)]), &KkParams::default());
    println!("kk on 2 nodes: {:?}", r.is_ok());
}