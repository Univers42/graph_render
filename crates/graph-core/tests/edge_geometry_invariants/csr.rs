//! The CSR itself: that every style's `offsets`/`pts` are well formed at every boundary,
//! and that each boundary's own row contains what only that boundary can show. Split
//! from the parent, which holds the case builders.

use super::*;

/// The named boundary checks, for every style and every node kind.
#[test]
fn polyline_and_curve_offsets_are_well_formed_at_every_boundary() {
    for case in cases() {
        for style in Style::ALL {
            for kind in 0..3 {
                let at = format!("{}: {style:?}: node kind {kind}", case.name);
                let edges = styled(&case, style, kind);
                let m = case.topology.edge_count();
                edges.check(m).unwrap_or_else(|err| panic!("{at}: {err}"));
                assert_eq!(edges.kind(), style.kind(), "{at}: declared kind");
                let Some(rows) = paths(&edges) else {
                    assert_eq!(style, Style::Straight, "{at}: only straight emits Line");
                    continue;
                };
                let offsets = &rows.offsets;
                assert_eq!(offsets.len(), m as usize + 1, "{at}: m + 1 offsets");
                assert_eq!(offsets[0], 0, "{at}: the first offset is 0");
                assert!(
                    offsets.windows(2).all(|w| w[1] >= w[0]),
                    "{at}: offsets never decrease: {offsets:?}"
                );
                assert_eq!(
                    rows.pts.len(),
                    2 * offsets[m as usize] as usize,
                    "{at}: pts holds 2 x the last offset"
                );
                assert!(rows.pts.iter().all(|v| v.is_finite()), "{at}: D9");
                if let EdgeGeometry::Curve { degree, .. } = &edges {
                    assert_eq!(*degree, style.degree(), "{at}: declared degree");
                }
            }
        }
    }
}

/// "Well formed" is not the whole of any of these boundaries: here each one says
/// what its own row has to contain. `Straight` is skipped — it stores no rows by
/// construction, so there is nothing for it to contain.
#[test]
fn a_boundary_row_says_what_it_has_to_contain() {
    for style in Style::ALL {
        if style == Style::Straight {
            continue;
        }
        let zero = Case::new("zero edges", &[(0.0, 0.0), (4.0, 0.0)], &[]);
        let none = paths_of(&zero, style);
        assert_eq!(none.offsets, [0], "{style:?}: zero edges, one offset");
        assert!(none.pts.is_empty(), "{style:?}: zero edges, no points");

        let one = Case::new("a single edge", &[(0.0, 0.0), (4.0, 0.0)], &[("a", "b")]);
        let alone = paths_of(&one, style);
        assert_eq!(alone.offsets.len(), 2, "{style:?}: one edge, two offsets");
        assert_eq!(
            alone.offsets[1] as usize,
            one.row(&alone, 0).len(),
            "{style:?}: the tail offset counts the only row"
        );

        let looped = Case::new("a self-loop", &[(0.0, 0.0)], &[("a", "a")]);
        let row = rows_of(&looped, style).remove(0);
        assert!(
            row.len() >= 3,
            "{style:?}: a loop needs >= 3 points: {row:?}"
        );
        assert!(
            row.iter().all(|p| *p != (0.0, 0.0)),
            "{style:?}: no vertex sits on the node centre: {row:?}"
        );
        let ((x0, x1), (y0, y1)) = bounds(&row);
        assert!(
            x1 - x0 > 0.0 && y1 - y0 > 0.0,
            "{style:?}: a loop has area: {row:?}"
        );
    }
}

/// `((min x, max x), (min y, max y))` over a row.
fn bounds(row: &[(f32, f32)]) -> ((f32, f32), (f32, f32)) {
    row.iter().fold(
        ((f32::MAX, f32::MIN), (f32::MAX, f32::MIN)),
        |((x0, x1), (y0, y1)), &(x, y)| ((x0.min(x), x1.max(x)), (y0.min(y), y1.max(y))),
    )
}
