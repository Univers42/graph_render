use super::kernel::Law;
use super::mesh::Mesh;
use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};

fn line(n: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    (nodes, edges)
}

/// A simulation over `n` unlinked nodes at the given positions.
fn placed(x: Vec<f64>, y: Vec<f64>, params: ForceParams) -> Sim {
    let nodes: Vec<_> = (0..x.len()).map(|i| node(&format!("n{i}"), "")).collect();
    let t = index_model(&nodes, &[]).expect("fits");
    let mut sim = Sim::new(&t, LiveParams::from(params), SEED);
    (sim.x, sim.y) = (x, y);
    sim
}

/// d3's law summed over every other node: what the mesh approximates.
fn direct(x: &[f64], y: &[f64], i: usize, law: Law) -> (f64, f64) {
    let mut e = (0.0, 0.0);
    for j in (0..x.len()).filter(|&j| j != i) {
        let (rx, ry) = (x[i] - x[j], y[i] - y[j]);
        let mut l = rx * rx + ry * ry;
        if l >= law.dmax2 {
            continue;
        }
        if l < law.dmin2 {
            l = libm::sqrt(law.dmin2 * l);
        }
        e = (e.0 - rx / l, e.1 - ry / l);
    }
    e
}

#[test]
fn the_mesh_field_follows_the_direct_sum_at_range() {
    let params = ForceParams::default();
    let (x, y): (Vec<f64>, Vec<f64>) = (0..64)
        .map(|i| {
            (
                f64::from(i % 8) * 70.0 + f64::from(i / 8) * 9.0,
                f64::from(i / 8) * 70.0,
            )
        })
        .unzip();
    let law = Law {
        dmin2: params.distance_min * params.distance_min,
        dmax2: params.distance_max * params.distance_max,
    };
    let sim = placed(x.clone(), y.clone(), params);
    let mut mesh = Mesh::new(sim.rows());
    assert!(mesh.solve(&sim, &Serial, 1));
    let (mut err, mut norm) = (0.0, 0.0);
    for i in 0..x.len() {
        let (got, want) = (mesh.field_at((x[i], y[i])), direct(&x, &y, i, law));
        err += (got.0 - want.0).powi(2) + (got.1 - want.1).powi(2);
        norm += want.0.powi(2) + want.1.powi(2);
    }
    let relative = libm::sqrt(err / norm);
    assert!(relative < 0.05, "rms relative field error {relative}");
}

#[test]
fn a_node_out_of_everyone_s_range_feels_no_force_from_itself() {
    let sim = placed(vec![0.0, 3000.0], vec![0.0, 17.5], ForceParams::default());
    let mut mesh = Mesh::new(sim.rows());
    assert!(mesh.solve(&sim, &Serial, 1));
    for (px, py) in [(0.0, 0.0), (3000.0, 17.5)] {
        let (ex, ey) = mesh.field_at((px, py));
        assert!(
            ex.abs() < 1e-12 && ey.abs() < 1e-12,
            "self force ({ex}, {ey})"
        );
    }
}

#[test]
fn the_layout_is_finite_and_the_same_run_to_run_and_at_every_worker_count() {
    let (nodes, edges) = line(120);
    let t = index_model(&nodes, &edges).expect("fits");
    let params = ForceParams::default();
    let once = ParticleMesh::run(&t, &params).expect("finite");
    assert_eq!(once, ParticleMesh::run(&t, &params).expect("finite"));
    for workers in [2, 3, 7] {
        let split = ParticleMesh::run_with(&t, &params, &Serial, workers).expect("finite");
        assert_eq!(split, once, "workers={workers}");
    }
}

fn mean_link(x: &[f64], y: &[f64]) -> f64 {
    let gaps = (1..x.len()).map(|i| libm::hypot(x[i] - x[i - 1], y[i] - y[i - 1]));
    gaps.sum::<f64>() / (x.len() - 1) as f64
}

/// The same chain under Barnes-Hut: the mesh changes how the charge is summed, not the
/// law, so the settled chain's scale must agree (0.5% apart when written) and no link
/// may collapse.
#[test]
fn a_chain_settles_to_barnes_hut_s_scale() {
    let (nodes, edges) = line(60);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut run = ParticleMeshRun::from_frozen(&t, &ForceParams::default()).expect("valid");
    run.step_with(&Serial, 1, TICKS);
    let mut bh = crate::layout::force::ForceSession::from_frozen(&t, &ForceParams::default())
        .expect("valid");
    bh.step(TICKS);
    let (pm, bh) = (mean_link(run.xs(), run.ys()), mean_link(bh.xs(), bh.ys()));
    assert!(
        (pm / bh - 1.0).abs() < 0.05,
        "mean link {pm} against Barnes-Hut's {bh}"
    );
    let (x, y) = (run.xs(), run.ys());
    let closest = (1..x.len())
        .map(|i| libm::hypot(x[i] - x[i - 1], y[i] - y[i - 1]))
        .fold(f64::INFINITY, f64::min);
    assert!(closest > 1.0, "two linked nodes coincide: {closest}");
}

#[test]
fn an_empty_and_a_single_node_graph_run() {
    let empty = crate::index::empty_model();
    assert!(ParticleMesh::run(&empty, &ForceParams::default()).is_ok());
    let (nodes, edges) = line(1);
    let one = index_model(&nodes, &edges).expect("fits");
    assert!(ParticleMesh::run(&one, &ForceParams::default()).is_ok());
}
