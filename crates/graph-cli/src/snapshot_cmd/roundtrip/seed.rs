//! One seed of the `roundtrip` sweep: the contract exercise, then every registered layout,
//! each put through both faces and, where it has one, its hand oracle.

use super::super::{dag, exercise, faces_agree, hand_oracles, pipeline};
use super::Findings;
use graph_contract::binary::Snapshot;
use graph_core::gate_node_count;

/// The line naming the seed about to be worked: its index, its place in the sweep and the
/// node count `gate_node_count` hands it, so the number after it is the graph size too.
pub(super) fn progress_line(seed: u32, seeds: u32) -> String {
    format!(
        "roundtrip: seed {seed}/{seeds} nodes {}",
        gate_node_count(seed)
    )
}

/// One seed's whole sweep: the contract exercise, then every registered layout, each
/// counted and checked, all into `found`.
pub(super) fn seed_finding(seed: u32, swept: &[&str], found: &mut Findings) -> Result<(), String> {
    let nodes = gate_node_count(seed);
    // Under `GM_MUTATE_NODE_Z=1` this is the same generator with one value too many in
    // the z column, so the sweep cannot finish and the row goes red: that is the
    // control working, not a gate that broke.
    let exercise = exercise::snapshot_or_perturbed(exercise::snapshot(seed)?, seed)?;
    exercise::count_notes_cases(&exercise, &mut found.notes);
    found.three_d += u64::from(exercise.parts().dim().is_3d());
    found.checked += 1;
    if let Err(why) = faces_agree(&exercise) {
        found.faces.push(format!("seed {seed} exercise: {why}"));
    }
    // The negative control: with `GM_MUTATE_NODE_Z` set, this seed's 3D z column is
    // moved and the sweep requires the round trip to FAIL on it. A control that the
    // round trip survives is a control the round trip is not comparing — so a green
    // control run is a failure, exactly as `negctl-degree` is one for the hashgate.
    // The z column is compared, not merely carried: a snapshot whose z differs in
    // exactly one value must differ on both faces, and `GM_MUTATE_NODE_Z=1` perturbs
    // the JSON text of a 3D seed and demands the reader notice. Either a face that
    // writes the z from somewhere else, or a reader that ignores the z it was given,
    // fails here — which is the silent-drop bug (F1, F6) this column is most prone to.
    // The z column's own two refusals, on a snapshot built here: a z column of the wrong
    // length, and a z column under a `dim` that does not name it. Every registered 3D
    // layout emits a valid z column or it is a bug, so the malformed one is had by hand
    // and these stay a property of the reader rather than of a layout stage. A fault list
    // is a failure, so a reader that accepted either would leave this row green — which
    // is the whole reason the checks are here.
    if exercise.parts().dim().is_3d() {
        for fault in exercise::z_refusal_faults(&exercise) {
            found.faces.push(format!("seed {seed} exercise: {fault}"));
        }
    }
    for name in swept {
        let snapshot = pipeline(seed, nodes, name)?.snapshot;
        found.checked += 1;
        if let Err(why) = faces_agree(&snapshot) {
            found.faces.push(format!("seed {seed} {name}: {why}"));
        }
        if let Err(why) = hand_oracle(name, seed, nodes, &snapshot) {
            convention(found, name, format!("seed {seed}: {why}"));
        }
    }
    Ok(())
}

/// The hand oracle for `name`, or `Ok(())` for the layouts gated on the d3-hierarchy
/// differential instead (`OTHER_LAYOUTS`).
fn hand_oracle(name: &str, seed: u32, nodes: u32, snapshot: &Snapshot) -> Result<(), String> {
    match name {
        "grid" => hand_oracles::grid(snapshot),
        "circular.radial" => hand_oracles::circular(seed, nodes, snapshot),
        "packing.circle" => hand_oracles::packing(snapshot),
        "dag.sugiyama" => dag::invariants(snapshot),
        _ => Ok(()),
    }
}

/// Records a convention failure under the layout that owns it, one ledger function row
/// per hand-checked layout.
fn convention(found: &mut Findings, name: &str, why: String) {
    match name {
        "grid" => found.grid.push(why),
        "circular.radial" => found.circular.push(why),
        "packing.circle" => found.packing.push(why),
        "dag.sugiyama" => found.dag.push(why),
        _ => {}
    }
}
