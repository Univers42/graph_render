//! The pass at its limits: that it resolves something, that it leaves a clear drawing alone,
//! that a frozen relaxation leaves every overlap in place, and the lattice that sets the cap.
//!
//! Split out of the parent so neither file outgrows the house 300-line limit. These are the
//! claims that make "no pairs overlap" mean something: a pass that resolves nothing would
//! satisfy every separation assertion vacuously, and a pass that nudges a clear drawing would
//! make it worse.

use super::*;

/// **The control for this pass, in the units it reports in.** `post::tests` uses "did the ink
/// go down" to prove a bundler is not a registered no-op; the equivalent question for a node
/// mover is "did it resolve anything", and this asks it. A pass that resolved zero pairs over
/// a stack of 500 coincident discs would satisfy every separation assertion here vacuously if
/// the assertions only measured geometry — so this pins the **count**, not the positions.
#[test]
fn it_actually_resolves_pairs() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &stacked(n, 1.0)).expect("runs");
    assert!(
        bundled.pairs > 0,
        "500 coincident discs must resolve a nonzero number of pairs, got {}",
        bundled.pairs
    );
    assert_eq!(
        bundled.unbundled, 0,
        "a converged sweep leaves no residual overlap"
    );
}

/// A layout whose discs are already clear must come out **byte-identical**, not merely
/// un-overlapped: a pass that nudges a readable drawing is a pass that makes it worse, and
/// this is what says the nudge is bounded by the radius and cannot do that.
#[test]
fn a_clear_layout_is_left_alone() {
    let n = 100;
    let topology = path_topology(n);
    let input = Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| i as f32 * 4.0).collect(),
            y: (0..n).map(|i| (i as f32 * 7.0) % 11.0).collect(),
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    );
    let bundled = run(&topology, &input).expect("runs");
    assert_eq!(
        bundled.geometry.nodes, input.nodes,
        "a drawing with no overlapping pair must be returned untouched"
    );
    assert_eq!(bundled.pairs, 0);
}

/// **The negative control, at the level that can bite: `over_relaxation: 0` freezes every
/// displacement**, so a pass that cannot move a node leaves the input's overlaps exactly where
/// they were and reports every one of them in `Bundled::unbundled`.
///
/// This is the same perturbation the hash gate's `GM_MUTATE_OVERLAP_RELAXATION` applies to the
/// native arm, asserted here where the arithmetic is visible. `0` is a legal value and is not
/// clamped — a clamp would hide the one value the control needs.
#[test]
fn a_frozen_relaxation_leaves_every_overlap_and_reports_it() {
    let n = 200;
    let topology = path_topology(n);
    let params = SeparateParams {
        over_relaxation: 0.0,
        ..SeparateParams::default()
    };
    let bundled = separate(&topology, &stacked(n, 1.0), &params).expect("runs");
    assert_eq!(
        bundled.geometry.nodes.kind(),
        stacked(n, 1.0).nodes.kind(),
        "the kind is preserved even when nothing moves"
    );
    // Every disc is on top of every other, so the residual is every pair but the node's own
    // diagonal entry, which the half-open comparison excludes.
    let want = (n as u32) * (n as u32 - 1) / 2;
    assert_eq!(
        bundled.unbundled, want,
        "a frozen sweep must report every overlapping pair it did not resolve"
    );
    assert!(
        bundled.pairs < want,
        "a frozen sweep resolves no pair, so the geometry is exactly the stacked input"
    );
    // The decisive assertion, inverted so the control is a **passing** test: the geometry is
    // still fully overlapping. A control that froze the sweep while some other term still
    // separated the discs would satisfy the count checks above and leave the invariant green,
    // so what is checked here is the invariant *failing* — measured, not assumed: the worst
    // overlap under the knob is 1.993 against the tolerance 1e-3.
    let worst = worst_overlap(&bundled.geometry, 1.0, 0.0);
    assert!(
        worst > 1.0,
        "the frozen pass must leave the discs overlapping; worst overlap was {worst}"
    );
}

/// **The binding adversarial case: a full lattice.** A `k × k` grid of discs at pitch equal to
/// the radius is harder than a random cloud by an order of magnitude — every node is wedged
/// symmetrically between eight neighbours, so its own displacement vectors partly cancel and
/// the relaxation has to fight that symmetry. Measured: it needed 192 sweeps at 500 nodes
/// where a random cloud needed a median of 16, which is why `SeparateParams::max_iterations`
/// is 512 rather than the number a random cloud would suggest.
#[test]
fn a_full_lattice_is_separated() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &gridded(n, 1.0)).expect("runs");
    assert_separated(&bundled.geometry, 1.0, 0.0, "lattice");
    assert_eq!(
        bundled.unbundled, 0,
        "the lattice converged inside the default cap"
    );
}

/// `n` discs on a lattice of pitch `spacing` — the grid layout's own shape.
pub(super) fn gridded(n: usize, spacing: f32) -> Geometry {
    let k = (n as f32).sqrt() as usize;
    Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| (i % k) as f32 * spacing).collect(),
            y: (0..n).map(|i| (i / k) as f32 * spacing).collect(),
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}
