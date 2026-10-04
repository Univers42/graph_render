//! Registry-facing entry points for `layout.spectral`, `layout.mds.pivot` and their two
//! 3D siblings: the layouts' own `run` returns per-component reports, the registry wants a
//! bare [`Geometry`].
//!
//! **LF-10: the reports are no longer dropped.** Before this module mapped
//! `|(geometry, _)| geometry`, a component that missed the residual/orthonormality gate was a
//! *silent* collapse — its nodes left at the origin and then at their packing cell, with the
//! measured residual printed nowhere on the product path, which is C12's exact prohibition.
//! Every entry point below now refuses when any attempted component missed the gate, so a
//! caller either gets a picture in which **every** component was placed by a solve that passed,
//! or an error that says a component was not.
//!
//! **What `StageError` can and cannot carry, and why the number is not in it.**
//! `StageError::Param { name: &'static str, rule: &'static str }` (`stage.rs`) is `Copy` and
//! both fields are `&'static str`, so a per-component index and a measured `f64` cannot go
//! into it without changing `stage.rs` — outside this job's paths, and a change to every
//! stage's error type. So the rule text below names the *condition* and the *field* that
//! carries the numbers, and [`spectral::ComponentReport::peak_residual`] /
//! [`pivot_mds::ComponentReport::peak_residual`] are that field: `graph_core::layout::spectral`
//! and `::pivot_mds` return them today, and `graph-cli` printing them is a one-line
//! integration step that needs no new type. Putting the number *into* the error needs
//! `StageError` to own a `String`, and is reported under "decisions needed" rather than done
//! here.
//!
//! C12's other half is unchanged: a run in which every attempted component failed is refused
//! with [`NOTHING_SOLVED`], never turned into a random picture.

use super::{Geometry, pivot_mds, spectral};
use crate::index::Topology;
use crate::stage::StageError;

/// Every attempted component failed: there is no picture to return at all.
const NOTHING_SOLVED: StageError = StageError::Param {
    name: "topology",
    rule: "no component passed the eigensolver's residual and orthonormality gate",
};

/// Some components passed and at least one did not. A different condition from
/// [`NOTHING_SOLVED`] and a different message, because the two need different responses: this
/// one is a graph whose spectrum is outside what the solver can place, and the reference's own
/// answer (`networkx_layouts.py:152-153`) is to leave that component collapsed.
const UNSOLVED_COMPONENT: StageError = StageError::Param {
    name: "topology",
    rule: "a connected component missed the eigensolver's residual and orthonormality gate; \
           its size and peak residual are in layout::spectral::ComponentReport::peak_residual",
};

/// `layout.spectral` at its defaults.
pub fn spectral(topology: &Topology) -> Result<Geometry, StageError> {
    solve(spectral::run(topology))
}

/// `layout.mds.pivot` at its defaults.
pub fn pivot_mds(topology: &Topology) -> Result<Geometry, StageError> {
    solve(pivot_mds::run(topology))
}

/// `layout.spectral3d` at its defaults: the reference's `_spectral_layout_3d`, whose `n < 4`
/// branch draws [`spectral::DEFAULT_SEED`] rather than a layout seed.
pub fn spectral_3d(topology: &Topology) -> Result<Geometry, StageError> {
    spectral_3d_seeded(topology, spectral::DEFAULT_SEED)
}

/// `layout.spectral3d` at an explicit layout seed, which is the arm the conformance harness
/// runs: below four nodes `_spectral_layout_3d:257-258` is `_random_layout`, and that draws
/// `RandomState(get_layout_seed())`. The same shape as `sfdp::run_seeded`.
pub fn spectral_3d_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    solve(spectral::run_3d(topology, seed))
}

/// `layout.mds.pivot3d` at its defaults.
pub fn pivot_mds_3d(topology: &Topology) -> Result<Geometry, StageError> {
    pivot_mds_3d_seeded(topology, spectral::DEFAULT_SEED)
}

/// `layout.mds.pivot3d` at an explicit layout seed — see [`spectral_3d_seeded`], whose
/// `n < 4` guard is the same line of the reference (`_mds_layout_3d:283-284`).
pub fn pivot_mds_3d_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    solve(pivot_mds::run_3d(topology, seed))
}

/// The layouts' own result, with the reports read rather than discarded: the gate the layout
/// already applied per component becomes the stage's own accept/refuse rule. One shape for
/// both families because both report `solved` per attempted component.
fn solve<T, E>(result: Result<(Geometry, Vec<T>), E>) -> Result<Geometry, StageError>
where
    T: Solved,
{
    let (geometry, reports) = result.map_err(|_| NOTHING_SOLVED)?;
    if reports.iter().any(|report| !report.solved()) {
        return Err(UNSOLVED_COMPONENT);
    }
    Ok(geometry)
}

/// The one field both report types agree on, so [`solve`] needs no trait of its own.
trait Solved {
    fn solved(&self) -> bool;
}

impl Solved for spectral::ComponentReport {
    fn solved(&self) -> bool {
        self.solved
    }
}

impl Solved for pivot_mds::ComponentReport {
    fn solved(&self) -> bool {
        self.solved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::build::{edge, node};
    use crate::stage::{gate_node_count, run_with, seeded_model};
    use crate::weights::REFERENCE_DEGREE;
    use graph_contract::geometry::NodeGeometry;

    #[test]
    fn both_entry_points_agree_with_the_layouts_they_wrap() {
        let (nodes, edges) = seeded_model(3, gate_node_count(3), REFERENCE_DEGREE);
        let run = |id, f: fn(&Topology) -> Result<Geometry, StageError>| {
            run_with(&nodes, &edges, id, f).expect("runs").snapshot
        };
        let s = run("layout.spectral", spectral);
        let m = run("layout.mds.pivot", pivot_mds);
        assert_ne!(s.to_bytes(), m.to_bytes(), "two layouts, two pictures");
    }

    /// **LF-10's RED and GREEN.** A 1025-node path is one node past
    /// `shift_invert::DENSE_INVERT_LIMIT`, so no tier can place it: LOBPCG's block misses the
    /// gate and the retry is skipped for budget. Paired with a 4-path it forms a graph where
    /// *one* component solves and one does not — the case the layout's own
    /// `SpectralError::NothingSolved` does not cover, because something did solve.
    ///
    /// `spectral::run` returns `Ok` for that graph and its report says which component missed
    /// and by how much; this module used to map `|(geometry, _)| geometry` and hand the caller
    /// a picture whose 1025 nodes were all stacked on one point at one packing cell, with that
    /// residual printed nowhere. Now it is an error.
    #[test]
    fn a_component_that_misses_the_gate_is_refused_rather_than_collapsed() {
        let t = unplaceable_plus_small();
        let (geometry, reports) =
            spectral::run(&t).expect("the 4-path solved, so not NothingSolved");
        assert_eq!(reports.len(), 2, "both components are attempted");
        let (solved, unsolved): (Vec<_>, Vec<_>) = reports.iter().partition(|r| r.solved);
        assert_eq!(solved.len(), 1, "the 4-path placed");
        assert_eq!(unsolved[0].size, 1025, "the long path did not");
        assert!(
            unsolved[0].peak_residual.is_some_and(f64::is_finite),
            "and the report names the residual the gate read: {:?}",
            unsolved[0].peak_residual
        );
        // The residual gate *passes* here — 4.2e-7 against a limit of `1e-2 * max|λ|` — and
        // it is the orthonormality gate that refuses, the same block-column collapse
        // `spectral::tests::shift_invert::the_failing_gate_is_orthonormality_not_the_residual`
        // pins. So a caller reading this report must not conclude "small residual, therefore
        // placed"; `solved` is the verdict and the residual is one of its two inputs. That is
        // why the field is documented as the residual and not as the reason.
        assert!(
            unsolved[0].peak_residual.is_some_and(|r| r < 1e-2),
            "which is why this component is unsolved at a residual of {:?}",
            unsolved[0].peak_residual
        );
        assert!(
            matches!(&geometry.nodes, NodeGeometry::Point { x, .. } if x.len() == 1029),
            "the layout itself still returns a picture for all 1029 nodes — that is the \
             defect being closed"
        );

        let err = spectral(&t).expect_err("the stage must not return a collapsed picture");
        assert_eq!(
            err, UNSOLVED_COMPONENT,
            "a different condition from NOTHING_SOLVED and a different message"
        );
        assert!(
            err.to_string().contains("peak_residual"),
            "the message names the field carrying the numbers: {err}"
        );
    }

    /// The whole-run case the layout already refused, asserted here so the two refusals stay
    /// distinguishable: same graph, no solving component, `NOTHING_SOLVED` rather than
    /// [`UNSOLVED_COMPONENT`].
    #[test]
    fn a_graph_whose_only_component_failed_is_nothing_solved() {
        let t = path(1025);
        assert_eq!(
            spectral(&t).expect_err("no component solved"),
            NOTHING_SOLVED,
            "one condition, one message — and not the partial-failure one"
        );
    }

    /// The negative control: an *all-passing* run is untouched by LF-10, byte for byte. This
    /// is the fixture that fails if the rule were "refuse unless something went wrong" read
    /// backwards — i.e. if the check rejected solved components.
    #[test]
    fn a_run_whose_components_all_solved_still_returns_the_geometry() {
        let t = path(300);
        let (geometry, reports) = spectral::run(&t).expect("300 solves");
        assert!(reports.iter().all(|r| r.solved));
        assert_eq!(
            spectral(&t).expect("every component solved"),
            geometry,
            "LF-10 refuses a missing component, never a solved one"
        );
    }

    fn path(n: usize) -> Topology {
        let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
        let edges: Vec<_> = (0..n - 1)
            .map(|i| edge(&format!("e{i}"), &i.to_string(), &(i + 1).to_string()))
            .collect();
        crate::index::index_model(&nodes, &edges).expect("fits")
    }

    /// A 1025-node path (past `shift_invert::DENSE_INVERT_LIMIT`, so unplaceable) disjoint
    /// from a 4-path (placeable), as two components of one graph.
    fn unplaceable_plus_small() -> Topology {
        let mut nodes: Vec<_> = (0..1025).map(|i| node(&i.to_string(), "")).collect();
        nodes.extend((0..4).map(|i| node(&format!("s{i}"), "")));
        let mut edges: Vec<_> = (0..1024)
            .map(|i| edge(&format!("e{i}"), &i.to_string(), &(i + 1).to_string()))
            .collect();
        edges.extend(
            (0..3).map(|i| edge(&format!("s{i}"), &format!("s{i}"), &format!("s{}", i + 1))),
        );
        crate::index::index_model(&nodes, &edges).expect("fits")
    }
}
