//! `layout.dag.sugiyama`'s per-seed structural invariants, read from the snapshot alone:
//! the checks `roundtrip` records for it in place of a byte oracle (dagre's own drawing
//! is not byte-comparable; its crossing counts are measured by
//! `harness/oracle-layouts.mjs --dag`).
//!
//! Ponytail: `LAYER_SPACING = 1` is assumed, so every hop of a routed edge must be
//! exactly one layer in `y`; a run with another spacing would be reported as a violation
//! (over-reporting, never a silent pass). An edge with note 4 is skipped: it is drawn
//! straight on purpose and says so.

use graph_contract::binary::Snapshot;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::NoteCode;

/// Layers are monotone along every edge (reversed edges run against the grain, loops
/// stay put), every routed edge crosses exactly one layer per hop through its dummies,
/// and each reversed edge carries note 5 and only those do.
pub fn invariants(snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Point { y, .. }, EdgeGeometry::Polyline(paths)) = (&p.nodes, &p.edges)
    else {
        return Err("not Point nodes with Polyline edges".into());
    };
    let has_note = |code: NoteCode, e: usize| {
        let at = p.notes.code.iter().zip(&p.notes.index);
        at.into_iter()
            .any(|(&c, &i)| c == code.code() && i as usize == e)
    };
    for (e, (&s, &t)) in p.source.iter().zip(&p.target).enumerate() {
        if has_note(NoteCode::DummyBudgetExceeded, e) {
            continue;
        }
        let (from, to) = (y[s as usize], y[t as usize]);
        let dummies = (paths.offsets[e + 1] - paths.offsets[e]) as f32;
        let span = to - from;
        let reversed = has_note(NoteCode::EdgeReversed, e);
        let ok = match (s == t, reversed) {
            (true, _) => span == 0.0 && dummies == 0.0 && !reversed,
            (false, false) => span == dummies + 1.0,
            (false, true) => span == -(dummies + 1.0),
        };
        if !ok {
            return Err(format!(
                "edge {e} ({s} -> {t}): y {from} -> {to} through {dummies} dummies, reversed={reversed}"
            ));
        }
        let at = paths.offsets[e] as usize * 2;
        let mut last = from;
        for point in paths.pts[at..paths.offsets[e + 1] as usize * 2].chunks(2) {
            if (point[1] - last).abs() != 1.0 || (point[1] - from) * span < 0.0 {
                return Err(format!("edge {e}: dummy at y {} after y {last}", point[1]));
            }
            last = point[1];
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_contract::binary::SnapshotParts;

    /// A 0 -> 1 -> 2 chain plus edge 3 = 0 -> 2 routed through one dummy at layer 1.
    fn parts() -> SnapshotParts {
        let table = |name, ids: &[&str]| {
            graph_contract::binary::StringTable::from_strs(name, ids.iter().copied()).expect("fits")
        };
        SnapshotParts {
            version: graph_contract::version::CURRENT_VERSION,
            node_ids: table("node.id", &["a", "b", "c"]),
            edge_ids: table("edge.id", &["ab", "bc", "ac"]),
            source: vec![0, 1, 0],
            target: vec![1, 2, 2],
            nodes: NodeGeometry::Point {
                x: vec![0.0, 0.0, 0.0],
                y: vec![0.0, 1.0, 2.0],
            },
            edges: EdgeGeometry::Polyline(graph_contract::geometry::Paths {
                offsets: vec![0, 0, 0, 1],
                pts: vec![1.0, 1.0],
            }),
            notes: graph_contract::notes::Notes::default(),
        }
    }

    fn check(edit: impl FnOnce(&mut SnapshotParts)) -> Result<(), String> {
        let mut p = parts();
        edit(&mut p);
        invariants(&Snapshot::new(p).expect("valid snapshot"))
    }

    #[test]
    fn a_layered_drawing_passes() {
        assert_eq!(check(|_| {}), Ok(()));
    }

    #[test]
    fn a_flipped_layer_a_skipped_layer_and_a_stray_dummy_are_named() {
        let flipped = check(|p| {
            p.nodes = NodeGeometry::Point {
                x: vec![0.0; 3],
                y: vec![2.0, 1.0, 0.0],
            }
        });
        assert!(
            flipped
                .expect_err("against the grain")
                .starts_with("edge 0 (0 -> 1)")
        );
        let skipped = check(|p| {
            p.nodes = NodeGeometry::Point {
                x: vec![0.0; 3],
                y: vec![0.0, 2.0, 3.0],
            }
        });
        assert!(
            skipped
                .expect_err("two layers, no dummy")
                .starts_with("edge 0")
        );
        let stray = check(|p| {
            p.edges = EdgeGeometry::Polyline(graph_contract::geometry::Paths {
                offsets: vec![0, 0, 0, 1],
                pts: vec![1.0, 5.0],
            });
        });
        assert_eq!(
            stray.expect_err("off-layer dummy"),
            "edge 2: dummy at y 5 after y 0"
        );
    }

    #[test]
    fn note_5_marks_exactly_the_reversed_edges() {
        let reversed = |p: &mut SnapshotParts| {
            p.source = vec![1, 1, 0];
            p.target = vec![0, 2, 2];
            p.notes = graph_contract::notes::Notes {
                code: vec![5],
                index: vec![0],
            };
        };
        assert_eq!(check(reversed), Ok(()));
        let unmarked = check(|p| {
            reversed(p);
            p.notes = graph_contract::notes::Notes::default();
        });
        assert!(
            unmarked
                .expect_err("reversed without note 5")
                .contains("reversed=false")
        );
    }

    #[test]
    fn a_budget_exceeded_edge_is_exempt_and_other_geometry_is_refused() {
        let exempt = check(|p| {
            p.edges = EdgeGeometry::Polyline(graph_contract::geometry::Paths {
                offsets: vec![0, 0, 0, 0],
                pts: vec![],
            });
            p.notes = graph_contract::notes::Notes {
                code: vec![4],
                index: vec![2],
            };
        });
        assert_eq!(exempt, Ok(()));
        let line = check(|p| p.edges = EdgeGeometry::Line);
        assert_eq!(line, Err("not Point nodes with Polyline edges".into()));
    }
}
