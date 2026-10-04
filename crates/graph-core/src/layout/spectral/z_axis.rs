//! The z column of SciGraphs' `'2Z'` Yifan Hu mode: the last solved axis of a 2D spectral
//! solve, read per component, peak-normalised, centred, and handed back as one value per
//! node.
//!
//! Split out of `spectral.rs` for the house line cap, and because this is a *consumer* of
//! the eigensolver rather than part of it: `spectral::run` builds a layout out of the same
//! solve, and a reader of one column is a different job with different rules about what a
//! failed component means. Reference: `networkx_layouts.py:293-302` (`_center_z`) and
//! `:304-338` (`_generate_z_component`'s SPECTRAL branch).

use super::graph::ComponentGraph;
use super::{
    Width, find_components, local_positions, nothing_solved, pin_signs, simple_neighbors,
    solve_component,
};
use crate::index::Topology;

/// The last solved axis of every component's spectral solve, per node, peak-normalised per
/// component: SciGraphs' `'2Z'` z, before it is centred.
///
/// **A per-component column, not a layout.** `_generate_z_component` reads `coords[idx, -1]`
/// inside its loop over `components` (`networkx_layouts.py:330-334`), so the value is only
/// ever meaningful inside its own component. A component whose solve fails contributes
/// zeros — the reference's own reading of a node the solve placed nowhere — and that is a
/// different statement from "this node's z is zero".
///
/// `None` only on the one failure the reference also treats as a failure: a component was
/// big enough to attempt (`len >= 2`) and none solved. An all-singleton graph returns
/// `Some` with a column of zeros, which is a real answer — every node already sits at the
/// origin — and refusing it would make the `2Z` arm unusable on exactly the edgeless graphs
/// its 2D counterpart lays out.
pub(crate) fn last_axis(topology: &Topology) -> Option<Vec<f64>> {
    let n = topology.node_count() as usize;
    let neighbors = simple_neighbors(topology);
    let components = find_components(&neighbors);
    // `ComponentGraph::build` takes the node-to-local-position map, not the node count:
    // develop made the component addressing explicit so a matrix entry can renumber a
    // neighbour into its own component's indexing. This arm was written before that, so it
    // builds the same map `spectral.rs:181` does rather than passing `n`.
    let local_of = local_positions(&components, n);
    let mut z = vec![0.0_f64; n];
    let mut any_solved = false;
    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let graph = ComponentGraph::build(members, &neighbors, &local_of);
        let Some(mut eig) = solve_component(&graph, Width::Spectral2d).eig else {
            continue;
        };
        any_solved = true;
        pin_signs(&mut eig);
        let source = eig.k - 1;
        let peak = eig.vectors.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        for (li, &g) in members.iter().enumerate() {
            let value = eig.column(source)[li];
            z[g as usize] = if peak > 0.0 { value / peak } else { value };
        }
    }
    // `nothing_solved` is the crate's own C12 rule: at least one component was big enough to
    // attempt, and none of them passed the residual/orthonormality gate. Reused rather than
    // re-expressed, so `2Z` refuses exactly when `layout.spectral` refuses.
    if nothing_solved(&components, any_solved) {
        return None;
    }
    Some(z)
}

/// `_center_z` (`networkx_layouts.py:293-302`): subtract the mean, then scale so the largest
/// absolute value is 0.5.
///
/// An all-zero column stays all zero. That is the reference's own answer for a structureless
/// graph (`networkx_layouts.py:308-309`) and here it is a guard rather than a division by
/// zero: without it a graph with no spectral signal would put `NaN` in a snapshot's z
/// column, which the contract refuses (D9).
pub(crate) fn center_z(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let mut out: Vec<f64> = values.iter().map(|v| v - mean).collect();
    let max_abs = out.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if max_abs > 0.0 {
        for v in &mut out {
            *v = *v / max_abs * 0.5;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    fn graph(n: u32, pairs: &[(u32, u32)]) -> crate::index::Topology {
        let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
        let edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
            .collect();
        index_model(&nodes, &edges).expect("fits")
    }

    /// The z is a real column, not a zero one: on a path the spectral solve succeeds, so the
    /// reference's own reading gives non-zero values. Without this, a `2Z` arm whose solve
    /// silently failed everywhere would still carry a z column and pass any check that only
    /// asks whether one exists.
    #[test]
    fn the_spectral_z_is_live_on_a_graph_the_solve_handles() {
        let z = last_axis(&graph(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)])).expect("solved");
        assert_eq!(z.len(), 6);
        assert!(z.iter().any(|v| *v != 0.0), "z is a column of zeros");
        assert!(
            z.iter().all(|v| v.is_finite()),
            "D9: nothing non-finite reaches the column"
        );
    }

    /// Centring: mean zero, and the largest absolute value scaled to exactly 0.5 — the two
    /// properties `_center_z` states, and the reason z is comparable across graphs.
    #[test]
    fn centering_zeros_the_mean_and_scales_the_peak_to_a_half() {
        let out = center_z(&[1.0, 2.0, 3.0, 4.0]);
        let mean = out.iter().sum::<f64>() / out.len() as f64;
        assert!(mean.abs() < 1e-12, "mean {mean}");
        let peak = out.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        assert!((peak - 0.5).abs() < 1e-12, "peak {peak}");
    }

    /// The all-zero column the reference returns for a structureless graph stays zero rather
    /// than becoming `NaN`: this is the division-by-zero guard, and the case that reaches it
    /// is a real one (a graph with no spectral signal, or one whose solves all failed).
    #[test]
    fn an_all_zero_column_centres_to_zero_and_not_to_nan() {
        let out = center_z(&[0.0; 5]);
        assert_eq!(out, vec![0.0; 5]);
    }

    /// An all-singleton graph is *not* a failure. Every node already sits at the origin, so
    /// a column of zeros is the honest answer and the reference does not refuse it — a
    /// refusal here would make `2Z` fail on every edgeless graph its 2D arm lays out.
    #[test]
    fn an_edgeless_graph_is_an_answer_rather_than_a_refusal() {
        let z = last_axis(&graph(3, &[])).expect("singletons are not a failure");
        assert_eq!(z, vec![0.0; 3]);
    }
}
