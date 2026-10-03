//! The memory budget for the motor's quadratic tables: Kamada-Kawai's n x n hop matrix,
//! neato's three packed n(n+1)/2 triangles and the circle-packing fallback's n x n adjacency.
//! Past it a layout refuses instead of asking for gigabytes. Natively the kernel may
//! overcommit (`vm.overcommit_memory=1` on the dev host, set by microk8s), so an oversized
//! table is granted and the host thrashes swap before the OOM killer acts; on wasm32 `n * n`
//! in `usize` wraps past 65 536 nodes and allocates a wrong, small table. Cells are counted in
//! `u64` so neither wraps.
//!
//! Caveat: one budget for every table, and it counts only the quadratic tables, not the linear
//! columns beside them, so a refused layout is refused at the same `n` on every host whatever
//! its RAM, and an accepted one still needs about 1.2 KB per node on top (measured by
//! `scripts/orch/memprofile.sh`).

use crate::stage::StageError;

/// The most bytes a layout's quadratic tables may take together: 1 GiB.
pub const QUADRATIC_BYTES_MAX: u64 = 1 << 30;

/// `n * n` cells, or `None` when that overflows `u64`.
pub fn square(n: u64) -> Option<u64> {
    n.checked_mul(n)
}

/// `n * (n + 1) / 2` cells, the packed upper triangle with its diagonal.
pub fn triangle(n: u64) -> Option<u64> {
    n.checked_add(1)
        .and_then(|m| n.checked_mul(m))
        .map(|c| c / 2)
}

/// `Ok` when `cells` of `bytes_per_cell` fit in [`QUADRATIC_BYTES_MAX`]; an overflowed count
/// is refused like an oversized one.
pub fn quadratic(cells: Option<u64>, bytes_per_cell: u64) -> Result<(), StageError> {
    match cells.and_then(|c| c.checked_mul(bytes_per_cell)) {
        Some(bytes) if bytes <= QUADRATIC_BYTES_MAX => Ok(()),
        _ => Err(StageError::Param {
            name: "nodes",
            rule: "too many for this layout: its quadratic tables must fit in 1 GiB \
                   (graph_core::budget::QUADRATIC_BYTES_MAX)",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{QUADRATIC_BYTES_MAX, quadratic, square, triangle};
    use crate::index::{Topology, index_model};
    use crate::layout::circle_packing;
    use crate::layout::force::kamada_kawai::{KamadaKawai, KkParams};
    use crate::layout::graphviz::neato;
    use crate::records::build::{edge, node};
    use crate::stage::{Stage, StageError};

    /// `count` nodes, the first five joined as K5 (non-planar, so circle packing takes its
    /// fallback) and the rest isolated.
    fn k5_and_isolated(count: u32) -> Topology {
        let nodes: Vec<_> = (0..count).map(|i| node(&format!("n{i}"), "")).collect();
        let pairs = (0..5u32).flat_map(|a| (a + 1..5).map(move |b| (a, b)));
        let edges: Vec<_> = pairs
            .map(|(a, b)| edge(&format!("e{a}{b}"), &format!("n{a}"), &format!("n{b}")))
            .collect();
        index_model(&nodes, &edges).expect("fits")
    }

    fn refused<T>(result: Result<T, StageError>) -> bool {
        matches!(result, Err(StageError::Param { name: "nodes", .. }))
    }

    /// One node past each layout's largest accepted graph is refused before any table is
    /// allocated, so each of these returns at once instead of asking for over 1 GiB.
    #[test]
    fn each_quadratic_layout_refuses_one_node_past_its_budget() {
        let past_square = k5_and_isolated(11_586);
        assert!(refused(KamadaKawai::run(
            &past_square,
            &KkParams::default()
        )));
        assert!(refused(circle_packing::run(&past_square)));
        assert!(refused(neato::run(&k5_and_isolated(13_377))));
    }

    #[test]
    fn exactly_the_budget_fits_and_one_cell_more_does_not() {
        let cells = QUADRATIC_BYTES_MAX / 8;
        assert!(quadratic(Some(cells), 8).is_ok());
        assert!(quadratic(Some(cells + 1), 8).is_err());
    }

    #[test]
    fn a_square_past_u64_is_refused_not_wrapped() {
        assert_eq!(
            square(u64::from(u32::MAX)),
            Some(18_446_744_065_119_617_025)
        );
        assert_eq!(square(1 << 32), None);
        assert!(quadratic(square(1 << 32), 8).is_err());
        assert!(quadratic(square(1 << 31), u64::MAX).is_err());
    }

    #[test]
    fn a_triangle_counts_its_diagonal() {
        assert_eq!(triangle(0), Some(0));
        assert_eq!(triangle(1), Some(1));
        assert_eq!(triangle(4), Some(10));
        assert_eq!(triangle(u64::MAX), None);
    }

    /// The largest `n` each caller accepts, pinned so a change to the budget is a visible
    /// change here: Kamada-Kawai and the circle-packing fallback at 8 B a square cell, neato
    /// at 12 B a triangle cell. Each is above its registry `scale_ceiling`.
    #[test]
    fn the_largest_accepted_graphs_are_pinned() {
        let largest = |cells: fn(u64) -> Option<u64>, bytes| {
            (1..1 << 20)
                .rev()
                .find(|&n| quadratic(cells(n), bytes).is_ok())
        };
        assert_eq!(largest(square, 8), Some(11_585));
        assert_eq!(largest(triangle, 12), Some(13_376));
    }
}
