//! The `horizontal` parameter's own tests: the same rows, the same lanes, the same numbers,
//! with x and y exchanged. `docs/decisions/dag-horizontal.md` conditions 1 and 5.
//!
//! Split out of [`super`] by the house's 300-line file limit, not by a difference of kind:
//! these are the same claims as the hand-built shapes above, about the one parameter that
//! changes the frame.

use super::*;

/// The `horizontal` parameter is the whole of the change: the same rows, the same lanes, the
/// same numbers, with x and y exchanged. `a` merges `b` and `c`, so the paths bend and the
/// swap has interior points to move.
#[test]
fn horizontal_draws_the_rows_along_x_bit_for_bit() {
    let n = [
        vertex("a", 1.0),
        vertex("b", 2.0),
        vertex("c", 3.0),
        vertex("m", 4.0),
    ];
    let e = [
        arc("mb", "m", "b"),
        arc("mc", "m", "c"),
        arc("ba", "b", "a"),
        arc("ca", "c", "a"),
    ];
    let t = index_model(&n, &e).expect("fits");
    let vertical = run(&t, &LanesParams::default()).expect("unit spacing is legal");
    let horizontal = run(
        &t,
        &LanesParams {
            horizontal: true,
            ..LanesParams::default()
        },
    )
    .expect("unit spacing is legal");
    let NodeGeometry::Point { x: vx, y: vy } = &vertical.nodes else {
        panic!("not Point nodes");
    };
    let NodeGeometry::Point { x: hx, y: hy } = &horizontal.nodes else {
        panic!("not Point nodes");
    };
    assert_eq!(bits(hx), bits(vy), "the horizontal x is the vertical y");
    assert_eq!(bits(hy), bits(vx), "the horizontal y is the vertical x");
    let EdgeGeometry::Polyline(turned) = &horizontal.edges else {
        panic!("not Polyline edges");
    };
    let EdgeGeometry::Polyline(straight) = &vertical.edges else {
        panic!("not Polyline edges");
    };
    assert!(
        !straight.pts.is_empty(),
        "the merge bends the paths, so they carry interior points"
    );
    assert_eq!(
        turned.offsets, straight.offsets,
        "the CSR shape is not a coordinate"
    );
    assert_eq!(turned.pts.len(), straight.pts.len());
    for (pair, plain) in turned
        .pts
        .as_chunks::<2>()
        .0
        .iter()
        .zip(straight.pts.as_chunks::<2>().0)
    {
        assert_eq!(
            bits(pair),
            bits(&[plain[1], plain[0]]),
            "x takes y and y takes x"
        );
    }
}

/// `false` is the drawing that was there before the parameter existed: the default run and an
/// explicit `horizontal: false` are one answer, so the flag cannot move a byte at its default.
#[test]
fn horizontal_false_is_the_default_drawing() {
    let n = [vertex("a", 1.0), vertex("b", 2.0)];
    let e = [arc("ab", "a", "b")];
    let t = index_model(&n, &e).expect("fits");
    assert_eq!(
        run(&t, &LanesParams::default()),
        run(
            &t,
            &LanesParams {
                horizontal: false,
                ..LanesParams::default()
            }
        )
    );
}
