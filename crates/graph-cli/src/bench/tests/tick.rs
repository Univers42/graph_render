//! `graph-cli tick`: one row per run, with every column the header names.

use crate::bench::tick::{HEADER, Layout, Plan, measure};

#[test]
fn a_tick_row_has_one_cell_per_header_column_for_either_layout() {
    for (layout, id, workers) in [
        (Layout::BarnesHut, "layout.force.barnes_hut", 1),
        (Layout::ParticleMesh, "layout.force.particle_mesh", 1),
        (Layout::ParticleMesh, "layout.force.particle_mesh", 3),
    ] {
        let plan = Plan {
            layout,
            n: 300,
            ticks: 3,
            warm: 1,
            seed: 0,
            workers,
            grow: None,
        };
        let row = measure(&plan).expect("a 300-node model builds");
        let columns = |line: &str| line.matches('|').count();
        let header = HEADER.lines().next().expect("a header line");
        assert_eq!(columns(&row), columns(header), "{row}");
        assert!(row.starts_with(&format!("| {id} | 300 | ")), "{row}");
        assert!(row.contains(&format!("| 3 | {workers} | ")), "{row}");
    }
}
