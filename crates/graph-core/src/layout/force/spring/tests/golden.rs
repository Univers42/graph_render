//! The bit-identity proof: the coordinates `Spring::run` produced **before** the dimension
//! became a const parameter on [`super::forces::Field`].
//!
//! Split into its own file because it is the one test in this module that is a golden
//! rather than a property, and it needs its own budget. Read it with the rest of the module
//! in mind: the properties around it are what a port bug breaks, and none of them can tell
//! whether a byte *moved*. This can, and it is the reason the dimension could be made a
//! parameter at all.

use super::super::{Spring, SpringParams};
use super::{path_edges, run};
use crate::layout::coords::probe::graph;
use crate::stage::Stage;
use graph_contract::snapshot::Dim;

/// `(x.to_bits(), y.to_bits())` per node, in node order, for `graph(n, &path_edges(n))`
/// at [`SpringParams::default`]. Captured from the pre-refactor build: n = 2, 3, 5 and 9,
/// the shapes the module's own properties are stated over.
///
/// Four sizes, chosen because they are the ones a reader can check by eye against the old
/// code path and because n = 2 and n = 5 pin both degenerate and settled cases.
const GOLDENS: &[(u32, &[(u32, u32)])] = &[
    (2, &[(3231711232, 1081357055), (1084227584, 3228840703)]),
    (
        3,
        &[
            (3211146788, 3231711232),
            (983645379, 1004487225),
            (1063642502, 1084213295),
        ],
    ),
    (
        5,
        &[
            (1084227584, 3227208168),
            (1076497202, 3219909002),
            (3162592110, 3162776470),
            (3223989552, 1072270665),
            (3231673774, 1079869520),
        ],
    ),
    (
        9,
        &[
            (1067624237, 3230927823),
            (1057951976, 3229117027),
            (3169556995, 3224521912),
            (3202071688, 3216990885),
            (3205304627, 3184386374),
            (3204056977, 1067916131),
            (3197919023, 1076487526),
            (3184984167, 1081834494),
            (1033896835, 1084227584),
        ],
    ),
];

/// Every coordinate is bit-identical to the two-column kernel's own output.
///
/// Compared as bits, never within a tolerance: the claim is identity, not closeness. A
/// tolerance here would let a reordered reduction through, which is exactly the change this
/// refactor risked — see the module doc on `squared`, which keeps `D = 2` summing
/// `dx * dx + dy * dy` in that order.
#[test]
fn the_two_dimensional_kernel_is_bit_identical_to_its_pre_dimension_form() {
    for &(n, want) in GOLDENS {
        let bits: Vec<(u32, u32)> = run(&graph(n, &path_edges(n)))
            .iter()
            .map(|p| (p.0.to_bits(), p.1.to_bits()))
            .collect();
        assert_eq!(bits, want, "n={n} moved a 2D bit");
    }
}

/// Axis order is part of the identity, not an accident of the golden: `x` must still be
/// `c[0]` and `y` `c[1]` in the geometry the stage emits. A transposed 2D arm passes every
/// other test in this module and moves every 2D digest.
#[test]
fn the_two_dimensional_stage_keeps_x_first_and_y_second() {
    let want: Vec<(f32, f32)> = GOLDENS
        .iter()
        .find(|(n, _)| *n == 5)
        .map(|(_, w)| {
            w.iter()
                .map(|&(a, b)| (f32::from_bits(a), f32::from_bits(b)))
                .collect()
        })
        .expect("n = 5 is pinned");
    let got = run(&graph(5, &path_edges(5)));
    assert_eq!(got, want, "the 2D columns are transposed");
    // Read from the geometry itself, not from `points`, so the claim is about the wire shape.
    let g = Spring::run(&graph(5, &path_edges(5)), &SpringParams::default()).expect("runs");
    assert_eq!(g.dim(), Dim::D2);
    assert_eq!(
        g.z, None,
        "generalising the kernel gave the 2D arm a z column"
    );
}
