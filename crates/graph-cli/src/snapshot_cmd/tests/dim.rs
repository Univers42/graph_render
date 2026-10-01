//! The 3D half of the exercise: which seeds carry a z column, what label they take, and
//! that each one round-trips through both faces.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because 3D is a separate
//! concern from the 2D sweep the parent pins. No 3D layout exists yet, so every 3D
//! snapshot here comes from the exercise's own generator.

use super::*;

#[test]
fn a_3d_exercise_snapshot_round_trips_and_is_labelled_0_4() {
    let mut three_d = 0;
    let mut labels = Vec::new();
    for seed in 0..30 {
        let snapshot = exercise::snapshot(seed).expect("valid");
        let parts = snapshot.parts();
        if parts.dim().is_3d() {
            three_d += 1;
            labels.push(parts.version.minor);
            assert_eq!(
                parts.z.as_ref().map(Vec::len),
                Some(parts.node_ids.len() as usize)
            );
            assert_eq!(snapshot.to_bytes()[14], 1, "seed {seed}: byte 14 says 3D");
            assert_eq!(faces_agree(&snapshot), Ok(()), "seed {seed}");
        } else {
            assert_eq!(parts.z, None, "seed {seed}");
            assert_eq!(snapshot.to_bytes()[14], 0);
            // A 2D snapshot is 0.2 or 0.3 — the generator's notes cases — but never 0.4: a
            // 2D snapshot is labelled the lowest version that can express it, and 0.3 can.
            assert!(
                parts.version.minor == 2 || parts.version.minor == 3,
                "seed {seed}: 2D is labelled 0.2 or 0.3, not {}",
                parts.version.minor
            );
        }
    }
    assert_eq!(three_d, 10, "a third of the seeds are 3D");
    assert!(
        labels.iter().all(|&minor| minor == 4),
        "every 3D snapshot is labelled 0.4, never 0.3: {labels:?}"
    );
}
