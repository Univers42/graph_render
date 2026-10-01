//! The z column's negative controls, and the two refusals its own contract demands.
//!
//! Split out of `exercise.rs` by the house's 300-line limit, and because the z column is the
//! one thing in the exercise with controls of its own: no 3D layout is registered, so the
//! `hashgate` has no 3D stage to perturb, and the z column is proved on the surface that
//! carries one today — the `roundtrip` sweep.

use graph_contract::binary::{Snapshot, SnapshotParts};
use graph_contract::snapshot::SnapshotError;
use graph_contract::version::FormatVersion;

use super::Stream;

/// The negative control for the z column: `GM_MUTATE_NODE_Z=1` makes each 3D exercise
/// snapshot's z column one value longer than the node count, so the sweep's own
/// construction refuses it by name and the `roundtrip` row goes red. `false` for an honest
/// run.
///
/// This is the design's own control (`docs/decisions/contract-3d.md` §4.3): a `dim = 1`
/// snapshot whose z column is the wrong length must be refused as
/// `Length { column: "node.z" }`. Injecting the fault is what proves the check is there —
/// without a row that injects it, a reader that accepted a long z would sit green.
///
/// Ponytail: read straight from the environment rather than through `hashgate::Knob`,
/// because there is no 3D *stage* for it to perturb: no 3D layout is registered yet, so
/// the hashgate has no 3D row to move and a `Knob` arm would have no stage to file itself
/// under. Escape hatch: when the first 3D layout is registered, this becomes a real `Knob`
/// arm and its row moves from `roundtrip` to `hashgate`; until then it perturbs the
/// exercise alone, which is the only place a z column exists.
pub fn perturb_z() -> bool {
    std::env::var("GM_MUTATE_NODE_Z").is_ok_and(|text| text.trim() == "1")
}

/// The z column's stream seed, distinct from [`Stream`]'s per-snapshot seed on purpose: see
/// the note where the z column is drawn in `exercise.rs`.
pub(super) const Z_STREAM: u64 = 0xD1CE_0000;

/// The z column `node_count` values long, drawn from `Z_STREAM` seeded by `seed`.
pub(super) fn draw(seed: u32, n: u32) -> Vec<f32> {
    Stream(Z_STREAM ^ u64::from(seed)).floats(n)
}

/// `snapshot` with the control's fault injected into the z column — one value too many,
/// which `Snapshot::new` refuses by name. The honest run takes the first branch unchanged,
/// so this is the same generator with a fault and nothing else.
pub fn snapshot_or_perturbed(snapshot: Snapshot, seed: u32) -> Result<Snapshot, String> {
    if !perturb_z() {
        return Ok(snapshot);
    }
    let mut parts = snapshot.into_parts();
    let Some(z) = parts.z.clone() else {
        return Snapshot::new(parts).map_err(|e| format!("exercise seed {seed}: {e}"));
    };
    parts.z = Some([z.as_slice(), &[0.0]].concat());
    Snapshot::new(parts).map_err(|e| format!("perturbed exercise seed {seed}: {e}"))
}

/// The z column's own two refusals (`docs/decisions/contract-3d.md` §4.3), each on a
/// snapshot rebuilt here because no 3D layout produces one: a z column of the wrong length,
/// and a z column under a label that names no dimension. A returned fault string is a
/// failure the sweep reports, so a reader that accepted either would leave the row green —
/// which is the reason these checks are here at all.
pub fn z_refusal_faults(snapshot: &Snapshot) -> Vec<String> {
    let Some(z) = snapshot.parts().z.as_deref() else {
        return vec!["the z controls need a 3D snapshot".to_owned()];
    };
    let mut faults = Vec::new();
    let short = rebuilt(snapshot, |p| {
        p.z = Some(z[..z.len() - 1].to_vec());
    });
    if !matches!(
        short,
        Err(SnapshotError::Length {
            column: "node.z",
            ..
        })
    ) {
        faults.push(format!(
            "a z column one value short was {short:?}, not Length {{ column: \"node.z\" }}"
        ));
    }
    let unnameable = rebuilt(snapshot, |p| {
        p.version = FormatVersion { major: 0, minor: 3 };
    });
    if !matches!(unnameable, Err(SnapshotError::DimUnnameable { .. })) {
        faults.push(format!(
            "a z column under a 0.3 label was {unnameable:?}, not DimUnnameable"
        ));
    }
    faults
}

fn rebuilt(
    snapshot: &Snapshot,
    edit: impl FnOnce(&mut SnapshotParts),
) -> Result<Snapshot, SnapshotError> {
    let mut parts = snapshot.clone().into_parts();
    edit(&mut parts);
    Snapshot::new(parts)
}
