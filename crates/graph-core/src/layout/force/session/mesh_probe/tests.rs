//! The probe's own cases: that its three columns are the mesh's own and not each other's,
//! that the instrument the repo already trusts agrees with it bit for bit, that the frame
//! it reports is the mesh's own hand-placed one, and that asking changes nothing.

use super::MeshProbe;
use crate::index::{Topology, index_model};
use crate::layout::force::{ForceParams, ForceSession, LiveParams};
use crate::stage::{gate_node_count, seeded_model};
use crate::weights::REFERENCE_DEGREE;

/// The gate's own model for `seed` (`prompt.md` §7.1), on the mesh: a connected graph of
/// 2 to 601 nodes with real degrees, which is the only kind a pass delta means anything on.
fn topology(seed: u32) -> Topology {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the gate's model fits the u32 index space")
}

/// A mesh session over the gate's model for `seed`.
fn session(seed: u32) -> ForceSession {
    ForceSession::new(&topology(seed), LiveParams::default())
        .expect("the defaults are in range")
        .with_particle_mesh()
}

/// Every column as its bits, so "equal" means bit-equal and not "close".
fn bits(column: &[f64]) -> Vec<u64> {
    column.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn the_three_passes_move_different_things() {
    let probe = session(5).mesh_probe().expect("a field to solve");
    let n = probe.charge_dx.len() as u32;
    assert!(n > 1, "the model has more than one node");
    let columns: [(&str, &Vec<f64>, &Vec<f64>); 3] = [
        ("link", &probe.link_dx, &probe.link_dy),
        ("charge", &probe.charge_dx, &probe.charge_dy),
        ("collide", &probe.collide_dx, &probe.collide_dy),
    ];
    for (name, x, y) in &columns {
        assert_eq!(x.len(), n as usize, "{name}: one column per node");
        assert_eq!(y.len(), n as usize, "{name}: one column per node");
        assert!(
            x.iter().chain(y.iter()).any(|v| *v != 0.0),
            "{name}: moved something"
        );
    }
    assert_ne!(
        probe.link_dx, probe.collide_dx,
        "link and collide are not the same sum"
    );
    assert_ne!(
        probe.charge_dx, probe.link_dx,
        "charge and link are not the same sum"
    );
    assert_ne!(
        probe.charge_dx, probe.collide_dx,
        "charge and collide are not the same sum"
    );
}

#[test]
fn every_column_is_the_meshes_own() {
    let s = session(5);
    let probe = s.mesh_probe().expect("a field to solve");
    let (theirs_x, theirs_y) = s.charge_deltas(0.9);
    assert_eq!(
        bits(&probe.charge_dx),
        bits(&theirs_x),
        "the probe and the trusted instrument must not differ by a bit on x"
    );
    assert_eq!(bits(&probe.charge_dy), bits(&theirs_y), "…nor on y");
}

#[test]
fn a_delta_is_the_pass_and_not_the_tick() {
    let mut s = session(5);
    s.step(7);
    let before = (
        s.xs().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        s.ys().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        s.alpha(),
        s.params(),
        s.tick_no(),
    );
    let probe = s.mesh_probe().expect("a field to solve");
    assert!(
        probe.charge_dx.iter().any(|v| *v != 0.0),
        "the probe moved something"
    );
    let after = (
        s.xs().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        s.ys().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        s.alpha(),
        s.params(),
        s.tick_no(),
    );
    assert_eq!(after, before, "the probe moved the session's own state");
}

#[test]
fn the_probe_leaves_the_next_tick_byte_identical() {
    let mut asked = session(5);
    let mut untouched = session(5);
    asked.step(3);
    untouched.step(3);
    let _ = asked.mesh_probe().expect("a field to solve");
    asked.step(1);
    untouched.step(1);
    assert_eq!(
        asked.xs().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        untouched
            .xs()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        "the next tick's positions are the same bits"
    );
    assert_eq!(
        asked.ys().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        untouched
            .ys()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        "…on both axes"
    );
    assert_eq!(
        asked.tick_no(),
        untouched.tick_no(),
        "the tick number did not move"
    );
}

#[test]
fn the_solution_matches_a_hand_placed_frame() {
    let s = session(11);
    let probe = s.mesh_probe().expect("a field to solve");
    let frame = crate::layout::force::particle_mesh::Mesh::new(probe.side)
        .placed_over((s.xs(), s.ys()), probe.side, s.params().distance_max)
        .expect("the mesh placed a frame for the same positions");
    assert_eq!(
        probe.side, frame.side,
        "the transform side is the mesh's own"
    );
    assert_eq!(probe.step, frame.step, "the rung is the mesh's own");
    assert_eq!(
        probe.h.to_bits(),
        frame.h.to_bits(),
        "the cell size is the mesh's own"
    );
    assert_eq!(
        probe.origin_x.to_bits(),
        frame.origin_x.to_bits(),
        "the origin is snapped down to a multiple of h"
    );
    assert_eq!(
        probe.origin_y.to_bits(),
        frame.origin_y.to_bits(),
        "…on both axes"
    );
    assert_eq!(probe.cells, frame.cells, "the cell count is the mesh's own");
    assert_eq!(probe.reach, frame.reach, "the reach is the mesh's own");
}

#[test]
fn the_tables_are_the_lengths_the_wire_declares() {
    let probe = session(11).mesh_probe().expect("a field to solve");
    let side = probe.side as usize;
    assert!(
        probe.side.is_power_of_two(),
        "a radix-2 side is a power of two"
    );
    assert!((128..=1024).contains(&side), "the mesh's side clamp holds");
    assert_eq!(probe.twiddle_re.len(), side, "one twiddle per line sample");
    assert_eq!(probe.twiddle_im.len(), side, "…and one imaginary word each");
    assert_eq!(probe.spectrum_re.len(), side * side, "the spectrum is P·P");
    assert_eq!(
        probe.spectrum_im.len(),
        side * side,
        "…and its imaginary half"
    );
}

#[test]
fn the_graph_columns_are_the_collapsed_ones() {
    let probe = session(9).mesh_probe().expect("a field to solve");
    let m = probe.lo.len();
    assert_eq!(probe.hi.len(), m, "one endpoint per simple edge");
    assert_eq!(probe.strength.len(), m, "one strength per simple edge");
    let mut seen: Vec<(u32, u32)> = Vec::with_capacity(m);
    for e in 0..m {
        let (lo, hi) = (probe.lo[e], probe.hi[e]);
        assert!(
            lo < hi,
            "edge {e}: the lower endpoint is first, so no self-loop survives"
        );
        seen.push((lo, hi));
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen.len(),
        m,
        "no two rows name the same pair, so none is duplicated"
    );
}

#[test]
fn the_two_states_have_different_frames() {
    let mut s = session(13);
    let start = s.mesh_probe().expect("a field to solve");
    s.step(40);
    let settled = s.mesh_probe().expect("a field to solve");
    assert_ne!(
        (start.h.to_bits(), start.step),
        (settled.h.to_bits(), settled.step),
        "the frame is recomputed from the positions every tick, so a settled state has its own"
    );
}

#[test]
fn there_is_no_probe_without_a_field_to_solve() {
    let mut no_mesh = ForceSession::new(&topology(5), LiveParams::default()).expect("in range");
    assert!(
        no_mesh.mesh_probe().is_none(),
        "a session that does not tick the mesh has no mesh to read"
    );
    // A zero reach is a *range* the live setter refuses, so the probe's own `None` arm for
    // it is reached through the frozen constructor, which checks finiteness only.
    let mut params = ForceParams::default();
    params.distance_max = 0.0;
    let mut no_field = ForceSession::from_frozen(&topology(5), &params)
        .expect("finite is the frozen acceptance path")
        .with_particle_mesh();
    assert!(
        no_field.mesh_probe().is_none(),
        "a zero reach solves no field"
    );
    no_mesh.step(1);
    no_field.step(1);
    assert!(no_mesh.mesh_probe().is_none(), "still no mesh after a tick");
    assert!(
        no_field.mesh_probe().is_none(),
        "still no field after a tick"
    );
}
