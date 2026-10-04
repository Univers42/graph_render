//! The invariant at the breadth the job names: **every committed fixture, 100 random seeds,
//! and the two adversarial cases** — checked by an exhaustive all-pairs scan, at ≤ 2 000 nodes.
//!
//! Split from `tests.rs` for the house's file-length limit and because this is the row a gate
//! runs: `tests.rs` holds the per-property assertions, this holds the sweep over the inputs.
//! Both lean on the same `O(n²)` brute-force scan, which is a **test instrument and never
//! product** — nothing in `crates/` outside `#[cfg(test)]` calls it, which is the only reason
//! it can afford to be exact where the pass is `O(n · k)`.

use super::degradation::gridded;
use super::*;
use crate::post::fdeb;
use crate::stage::seeded_model;

/// The node count the exhaustive scan runs at, at most: 2 000 nodes is about two million pairs,
/// fast enough for a gate row and small enough that the instrument is not the thing being
/// timed. The pass itself is not capped at any size.
const SCAN_CEILING: usize = 2_000;

/// The node radius the fixtures and the seeds are given.
///
/// A `Point` has no extent and the pass is a documented no-op on one, so an invariant row over
/// a point layout would be checking nothing. 1.0 in layout units is the smallest radius that
/// puts the grid layout's own spacing (1.0) inside the overlap threshold — the drawing and the
/// discs are the same size, which is the crowded case the user reported.
const RADIUS: f32 = 1.0;

/// The invariant on one geometry: no two discs closer than `2r`, by an exhaustive scan.
///
/// **Returns the worst penetration rather than asserting**, so the caller decides what to do
/// with a failure — the per-input tests assert, and this sweep collects every failure so one
/// run names all of them instead of only the first.
fn worst(geometry: &Geometry) -> f32 {
    let NodeGeometry::Circle { x, y, .. } = &geometry.nodes else {
        return 0.0;
    };
    let mut worst = 0.0f32;
    for i in 0..x.len() {
        for j in i + 1..x.len() {
            let (dx, dy) = (x[i] - x[j], y[i] - y[j]);
            worst = worst.max(2.0 * RADIUS - f32::sqrt(dx * dx + dy * dy));
        }
    }
    worst
}

/// Assert the invariant over a geometry, naming where it came from.
fn separated(geometry: &Geometry, where_: &str) {
    let worst = worst(geometry);
    assert!(
        worst <= TOLERANCE,
        "{where_}: worst overlap {worst} exceeds the tolerance {TOLERANCE}"
    );
}

/// Lays `topology`'s nodes out as discs at [`RADIUS`], laid on a jittered lattice.
///
/// The lattice is deliberate: it is the **binding** case (a node wedged symmetrically between
/// eight neighbours partly cancels its own displacement), so a sweep that passes here is not
/// passing only because its inputs were easy.
fn lattice(topology: &Topology) -> Geometry {
    let n = topology.node_count() as usize;
    let k = (n as f32).sqrt().max(1.0);
    Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| (i % k as usize) as f32).collect(),
            y: (0..n).map(|i| (i / k as usize) as f32).collect(),
            r: vec![RADIUS; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// **Every committed fixture**, laid out as discs. The job's first named input set: a pass
/// that separated a synthetic lattice but not a real fixture would pass everything above.
#[test]
fn every_committed_fixture_is_separated() {
    for (name, _) in fdeb::FIXTURES {
        let (nodes, edges) = fdeb::load(name).expect("the fixture is committed");
        let topology = crate::index::index_model(&nodes, &edges).expect("the fixture indexes");
        assert!(
            topology.node_count() as usize <= SCAN_CEILING,
            "{name} is {} nodes, past the {SCAN_CEILING} the exhaustive scan runs at",
            topology.node_count()
        );
        let bundled = run(&topology, &lattice(&topology)).expect("runs");
        separated(&bundled.geometry, name);
        assert_eq!(
            bundled.unbundled, 0,
            "{name}: the pass reported a residue the scan does not see"
        );
    }
}

/// **100 random graphs**, one per seed. The seeds are `0..100` and nothing else — no clock, no
/// RNG (D2, D3) — so a failure is reproducible from its seed alone, which is printed in the
/// assert.
///
/// Seeded at the crate's own `seeded_model`, the same generator the hash gate draws from, so
/// the graphs are the ones the rest of the tree already reasons about rather than a second
/// distribution invented here.
#[test]
fn one_hundred_random_graphs_are_separated() {
    let mut failures = Vec::new();
    for seed in 0..100u32 {
        let count = 32 + (seed % 96);
        let (nodes, edges) = seeded_model(seed, count, crate::REFERENCE_DEGREE);
        let topology = crate::index::index_model(&nodes, &edges).expect("fits");
        assert!(topology.node_count() as usize <= SCAN_CEILING);
        let bundled = run(&topology, &lattice(&topology)).expect("runs");
        let worst = worst(&bundled.geometry);
        if worst > TOLERANCE {
            failures.push(format!(
                "seed {seed} ({count} nodes): worst overlap {worst}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 100 random graphs still overlap:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// **The adversarial pair**, both cases the job names and both the ones a relaxation is worst
/// at: every node on one point, and every node on one line.
///
/// Each is checked at the scan ceiling rather than at a comfortable size, because the point of
/// an adversarial case is that it is hard and a small one would not be.
#[test]
fn the_two_adversarial_cases_are_separated() {
    let n = SCAN_CEILING;
    let topology = path_topology(n);
    for (name, input) in [
        ("one point", stacked(n, RADIUS)),
        ("one line", in_line(n, RADIUS)),
    ] {
        let bundled = run(&topology, &input).expect("runs");
        separated(&bundled.geometry, name);
        assert_eq!(bundled.unbundled, 0, "{name}: the pass reported a residue");
    }
}

/// **A full lattice at the scan ceiling** — the binding case, and the one that set
/// `SeparateParams::max_iterations`. Checked here as well as in `tests.rs` because the sweep
/// over seeds above only reaches 127 nodes and would not notice the cap being cut.
#[test]
fn a_full_lattice_at_the_scan_ceiling_is_separated() {
    let n = SCAN_CEILING;
    let topology = path_topology(n);
    let bundled = run(&topology, &gridded(n, 1.0)).expect("runs");
    separated(&bundled.geometry, "lattice at the ceiling");
}
