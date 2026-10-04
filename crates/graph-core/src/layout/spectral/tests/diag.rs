use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

fn topology(n: usize, pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| edge(&format!("e{k}"), &a.to_string(), &b.to_string()))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn path_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n - 1).map(|i| (i, i + 1)).collect()
}

#[test]
fn diag_print() {
    for n in [800usize, 1024, 1025, 1200, 1500] {
        let t = topology(n, &path_pairs(n));
        let (res, reports) = run(&t);
        println!(
            "path n={n} 2d: ok={} reports={:?}",
            res.is_ok(),
            reports
                .iter()
                .map(|r| (r.size, r.tier, r.solved, r.peak_residual, r.iterations))
                .collect::<Vec<_>>()
        );
        let (res3, reports3) = run_3d(&t, DEFAULT_SEED);
        println!(
            "path n={n} 3d: ok={} reports={:?}",
            res3.is_ok(),
            reports3
                .iter()
                .map(|r| (r.size, r.tier, r.solved, r.peak_residual))
                .collect::<Vec<_>>()
        );
    }
}
