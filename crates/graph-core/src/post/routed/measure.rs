//! The measurement behind `docs/measurements/phase08-routing.md` and behind
//! `post.route.grid`'s `scale_ceiling`.
//!
//! `#[ignore]`d, like `crates/graph-core/tests/memory.rs` and Phase 7's scaling
//! measurements: it is a benchmark, not a gate. Run it with
//!
//! ```sh
//! cargo test -p graph-core --release -- --ignored --nocapture routing_measurement
//! ```
//!
//! It is kept in the tree rather than deleted after use, because the numbers in that
//! document have to be re-derivable by whoever reads them next — a number nobody can
//! reproduce is a number nobody can check. It asserts nothing: it prints a table.

use super::*;
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;
use graph_contract::geometry::NodeGeometry;
use std::time::Instant;

/// A layout of `count` nodes on a jittered lattice filling `0..8` on **both** axes, so the
/// drawing is square and the grid's cubic cells are cubic in the layout too.
///
/// `pitch_cells` is the lattice pitch in cells at resolution 128, and it is swept because
/// it decides everything: at a small pitch each node's clearance ring touches its
/// neighbours' and the graph seals, while at a large pitch most of the grid is free. The
/// lattice is square and `count` is the caller's, so a sweep varies the spacing and the
/// node count together and the **fill fraction** is what the reader should compare.
fn spanned_layout(count: u32, pitch_cells: f64) -> NodeGeometry {
    let span = 8.0_f64;
    let pitch = pitch_cells * span / 128.0;
    // Square lattice, as wide as the count allows, so the drawing fills 0..8 on both axes.
    let cols = (f64::sqrt(f64::from(count)).ceil() as u32).max(2);
    let (mut x, mut y) = (
        Vec::with_capacity(count as usize),
        Vec::with_capacity(count as usize),
    );
    for i in 0..count {
        let (col, row) = (i % cols, i / cols);
        // A deterministic offset per node, from its index — no RNG to pin, and enough to
        // break the exact alignment that would put every node on a lattice boundary. The
        // multiply wraps in u32, the value every build computes (review finding M11).
        let jx = f64::from(i.wrapping_mul(2_654_435_761) % 1000) / 1000.0 - 0.5;
        let jy = f64::from(i.wrapping_mul(40_503) % 1000) / 1000.0 - 0.5;
        x.push((f64::from(col) * pitch + jx * pitch * 0.4 + pitch / 2.0) as f32);
        y.push((f64::from(row) * pitch + jy * pitch * 0.4 + pitch / 2.0) as f32);
    }
    NodeGeometry::Point { x, y }
}

/// A node count that fills a `pitch_cells` lattice at a `fill` fraction of its slots, so a
/// pitch sweep varies the spacing while holding roughly how crowded the drawing is.
fn count_for(pitch_cells: f64, fill: f64) -> u32 {
    let side = ((128.0 - f64::from(pitch_cells as u32)) / f64::from(pitch_cells as u32))
        .floor()
        .max(1.0);
    ((side * side * fill).floor() as u32).max(2)
}

/// The edge columns of the gate's own model at `n` nodes, so the measured cost is the cost
/// of routing a real graph rather than of routing a ring.
fn gate_edges(n: u32, spacing_cells: f64) -> (NodeGeometry, EdgeColumns) {
    let (nodes, edges) = seeded_model(5, n, REFERENCE_DEGREE);
    let columns = crate::index::index_model(&nodes, &edges)
        .expect("a model of this size fits")
        .edges()
        .clone();
    (spanned_layout(n, spacing_cells), columns)
}

#[test]
#[ignore = "a measurement, not a gate: prints the table behind docs/measurements"]
fn routing_measurement() {
    let params = GridParams::default();
    println!(
        "post.route.grid — resolution {}, margin {}, clearance {}",
        params.resolution, params.margin, params.clearance
    );
    println!(
        "{:>7} {:>7} {:>9} {:>9} {:>10} {:>12} {:>10}",
        "n", "m", "cells", "occupied", "bytes", "wall-clock", "per edge"
    );
    for n in [200u32, 500, 1_000, 2_000, 5_000] {
        let (nodes, edges) = gate_edges(n, 4.0);
        let mut grid = GridIndex::new();
        grid.build(&nodes, &params).expect("builds");
        let cells = grid.cells();
        let bytes = grid.bytes();
        let started = Instant::now();
        let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
        let elapsed = started.elapsed();
        let m = edges.source.len() as u32;
        let per_edge = elapsed.as_secs_f64() / f64::from(m.max(1));
        println!(
            "{:>7} {:>7} {:>9} {:>9} {:>10} {:>11.3}ms {:>9.1}us",
            n,
            m,
            cells,
            grid.occupied_cells(),
            bytes,
            elapsed.as_secs_f64() * 1e3,
            per_edge * 1e6
        );
        assert_eq!(routed.routes.len(), m as usize, "one route per edge");
    }
}

#[test]
#[ignore = "a measurement, not a gate: the resolution/quality trade the Ponytail names"]
fn resolution_measurement() {
    // One fixed graph, routed at every resolution. This is the Ponytail's cost curve: the
    // cell count is quadratic in the resolution, and the wall-clock follows it.
    let n = count_for(4.0, 0.25);
    let (nodes, edges) = gate_edges(n, 4.0);
    println!("post.route.grid — {n} nodes, {} edges", edges.source.len());
    println!(
        "{:>11} {:>9} {:>9} {:>10} {:>12} {:>12}",
        "resolution", "cells", "occupied", "bytes", "wall-clock", "fallbacks"
    );
    for resolution in [16u32, 32, 64, 128, 256] {
        let params = GridParams {
            resolution,
            ..GridParams::default()
        };
        let mut grid = GridIndex::new();
        grid.build(&nodes, &params).expect("builds");
        let started = Instant::now();
        let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
        let elapsed = started.elapsed();
        println!(
            "{:>11} {:>9} {:>9} {:>10} {:>11.3}ms {:>12}",
            resolution,
            grid.cells(),
            grid.occupied_cells(),
            grid.bytes(),
            elapsed.as_secs_f64() * 1e3,
            routed.fallbacks
        );
    }
}

#[test]
#[ignore = "a measurement, not a gate: node density vs fallbacks, the Ponytail's real failing input"]
fn density_measurement() {
    // The Ponytail says a gap narrower than one cell gives "a detour, or a straight line
    // drawn through a node". Which of the two happens is decided by **density**, not by
    // the gap: when a node's clearance ring touches its neighbours' the graph seals and
    // there is no route at all. This sweeps the lattice pitch, in cells at resolution 128,
    // and counts how many edges found a route against how many fell back.
    println!("post.route.grid — resolution 128, lattice 25% full, clearance swept");
    println!(
        "{:>13} {:>6} {:>6} {:>6} {:>10} {:>9} {:>8} {:>10}",
        "pitch", "nodes", "edges", "clear", "occupied", "%blocked", "routed", "fallbacks"
    );
    for pitch in [16.0f64, 12.0, 8.0, 6.0, 4.0, 3.0, 2.0] {
        for clearance in [0.0f64, 0.4, 0.9, 1.0] {
            let n = count_for(pitch, 0.25);
            let (nodes, edges) = gate_edges(n, pitch);
            let params = GridParams {
                clearance,
                ..GridParams::default()
            };
            let mut grid = GridIndex::new();
            grid.build(&nodes, &params).expect("builds");
            let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
            let m = edges.source.len();
            let ok = m - routed.fallbacks as usize;
            let blocked = 100.0 * f64::from(grid.occupied_cells()) / f64::from(grid.cells().max(1));
            println!(
                "{:>13} {:>6} {:>6} {:>6} {:>10} {:>8.1}% {:>8} {:>10}",
                pitch,
                n,
                m,
                clearance,
                grid.occupied_cells(),
                blocked,
                ok,
                routed.fallbacks
            );
        }
    }
}

/// Two nodes at (0, 4) and (8, 4), and a wall of point nodes at x = 6, one per cell from
/// y = 0 to y = 8, with `gap_cells` of them left out around y = 4. At resolution 32 over a
/// span of 8 a cell is 0.25 and the point at `k · 0.25` lands in its own cell, so the hole
/// is exactly `gap_cells` cells (review finding M13: the old bounds left one cell open at
/// a gap of 0 and 1, and three at 2).
fn wall_with_gap(gap_cells: u32) -> (NodeGeometry, EdgeColumns) {
    let hole = (16 - gap_cells / 2)..(16 - gap_cells / 2 + gap_cells);
    let mut points = vec![(0.0_f32, 4.0_f32), (8.0, 4.0)];
    points.extend(
        (0..=32_u16)
            .filter(|k| !hole.contains(&u32::from(*k)))
            .map(|k| (6.0, f32::from(k) * 0.25)),
    );
    let records: Vec<_> = (0..points.len())
        .map(|i| crate::records::build::node(&format!("n{i}"), ""))
        .collect();
    let links = vec![crate::records::build::edge("e0", "n0", "n1")];
    let edges = crate::index::index_model(&records, &links)
        .expect("fits")
        .edges()
        .clone();
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    (nodes, edges)
}

#[test]
#[ignore = "a measurement, not a gate: the cost of a narrow gap, the Ponytail's failing input"]
fn gap_measurement() {
    // The Ponytail's failing input, swept: two nodes either side of a wall, with the gap
    // in the wall narrowing from eight cells to none. The margin leaves a way round the
    // wall's ends, so a closed gap is a detour, not a fallback.
    println!("post.route.grid — gap sweep, resolution 32 over a span of 8 (cell = 0.25)");
    println!(
        "{:>10} {:>12} {:>12} {:>12}",
        "gap", "hole", "fallbacks", "wall-clock"
    );
    let params = GridParams {
        resolution: 32,
        margin: 2,
        clearance: 0.0,
    };
    for gap_cells in [8u32, 4, 2, 1, 0] {
        let (nodes, edges) = wall_with_gap(gap_cells);
        let mut grid = GridIndex::new();
        grid.build(&nodes, &params).expect("builds");
        // Column 26 is x = 6; rows 2..=34 are y = 0..=8, the margin excluded.
        let hole = (2..=34)
            .filter(|row| !grid.is_occupied(row * grid.shape().0 + 26))
            .count();
        let started = Instant::now();
        let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
        println!(
            "{:>10} {:>12} {:>12} {:>11.3}ms",
            format!("{gap_cells} cells"),
            format!("{hole} cells"),
            routed.fallbacks,
            started.elapsed().as_secs_f64() * 1e3
        );
        assert_eq!(
            hole, gap_cells as usize,
            "the hole is the gap the row names"
        );
    }
}

#[test]
fn the_lattice_jitter_wraps_rather_than_overflowing_in_a_debug_build() {
    // Node 2 is the first whose `i · 2 654 435 761` passes u32::MAX. A release build wraps
    // it, and the published numbers were taken with that wrap, so a debug build must too.
    let NodeGeometry::Point { x, .. } = spanned_layout(3, 4.0) else {
        unreachable!("a lattice is points")
    };
    assert_eq!(x.len(), 3);
}
