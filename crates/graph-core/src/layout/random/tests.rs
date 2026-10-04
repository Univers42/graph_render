use super::{run, run_3d, run_seeded};
use crate::layout::Geometry;
use crate::layout::basic_3d::SCALE;
use crate::layout::coords::probe::{graph, points};
use crate::synthetic::Mulberry32;
use graph_contract::geometry::NodeGeometry;
use graph_contract::snapshot::Dim;

/// `np.random.RandomState(981798123).rand(2, 3)` row-major, as bits. numpy 2.3.3.
///
/// `_random_layout` is exactly this array times `scale` (`basic.py:5-9`), so these six bits
/// are the whole of what the SciGraphs arm drew for two nodes.
const REFERENCE_TWO_NODES: [u64; 6] = [
    0x3FED_2B61_1EED_9D7B,
    0x3FE2_38BF_EACB_3589,
    0x3FD7_C4D8_0207_BEAE,
    0x3FC9_03DC_489C_5238,
    0x3FED_46A6_05A3_FD6D,
    0x3FDB_C15F_B35B_7CE4,
];

/// `x`, `y`, `z` or a panic: `run_seeded` is the 3D entry point, and a 2D geometry would
/// answer the third column wrongly and quietly.
fn space(g: &Geometry) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    let Some(z) = g.z.as_ref() else {
        panic!("a z column: this layout is 3D");
    };
    (x.clone(), y.clone(), z.clone())
}

/// `_random_layout`'s six numbers at the layout seed, each times `scale`, row-major over the
/// three axes: node `i` takes draws `3i`, `3i+1`, `3i+2` as `x`, `y`, `z`.
#[test]
fn run_seeded_is_the_reference_draws_row_major_times_the_scale() {
    let geometry = run_seeded(&graph(2, &[]), 981_798_123).expect("never refuses");
    assert_eq!(geometry.dim(), Dim::D3, "rand(n, 3) is a 3D placement");
    let (x, y, z) = space(&geometry);
    for i in 0..2usize {
        for (axis, column) in [&x, &y, &z].into_iter().enumerate() {
            let want = (f64::from_bits(REFERENCE_TWO_NODES[3 * i + axis]) * SCALE) as f32;
            assert_eq!(column[i], want, "node {i} axis {axis}");
        }
    }
}

/// The control: one more in the seed and every coordinate moves. A `run_seeded` that
/// ignored its argument and hard-coded the layout seed would pass the row above.
#[test]
fn run_seeded_moves_every_coordinate_with_its_seed() {
    let a = space(&run_seeded(&graph(2, &[]), 981_798_123).expect("runs"));
    let b = space(&run_seeded(&graph(2, &[]), 981_798_124).expect("runs"));
    for (axis, (mine, theirs)) in [(&a.0, &b.0), (&a.1, &b.1), (&a.2, &b.2)]
        .into_iter()
        .enumerate()
    {
        assert_ne!(mine, theirs, "axis {axis} ignored the seed");
    }
}

/// The registered `run` is untouched by the seeded entry point: still the crate's own
/// [`Mulberry32`] stream in the unit square, still planar, still unscaled — so its
/// hash-gate record stands.
#[test]
fn run_seeded_does_not_disturb_the_registered_default() {
    let planar = run(&graph(2, &[])).expect("runs");
    assert_eq!(planar.dim(), Dim::D2, "run stays planar");
    assert_eq!(
        points(&planar)[0],
        (0.71003205, 0.28633666),
        "run's stream is fixed"
    );
    let (x, _, _) = space(&run_seeded(&graph(2, &[]), 981_798_123).expect("runs"));
    assert_eq!(
        x[0],
        (f64::from_bits(REFERENCE_TWO_NODES[0]) * SCALE) as f32
    );
    assert_ne!(
        x[0],
        points(&planar)[0].0,
        "the seeded arm is the reference's, not Mulberry32's"
    );
}

#[test]
fn empty_graph_has_no_points_and_one_node_is_in_the_unit_square() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
    let one = points(&run(&graph(1, &[])).unwrap());
    assert_eq!(one.len(), 1);
    assert!((0.0..1.0).contains(&one[0].0) && (0.0..1.0).contains(&one[0].1));
}

/// **This is the draw-order test, and the seed *value* is not what it pins.** It re-derives
/// with `super::SEED` and the same generator, so it holds the **order and the count** — two
/// draws per node, `x` before `y`, off one stream — and it stays green if `SEED` is changed
/// to any other value. The value itself is held by
/// [`the_first_pair_is_pinned`](the_first_pair_is_pinned) below, which asserts two literal
/// `f32`s recomputed independently from the reference `Mulberry32` stream. Between them the
/// two questions are answered separately, which is the point: a reader who wants to know
/// *which* seed this id draws from reads the second test, not this one.
#[test]
fn positions_are_the_seeded_stream_row_major() {
    let mut rng = Mulberry32::new(super::SEED);
    let want: Vec<(f32, f32)> = (0..3)
        .map(|_| (rng.next_f64() as f32, rng.next_f64() as f32))
        .collect();
    assert_eq!(points(&run(&graph(3, &[(0, 1)])).unwrap()), want);
}

#[test]
fn a_disconnected_graph_gets_the_same_points_as_an_edgeless_one() {
    let a = points(&run(&graph(4, &[])).unwrap());
    assert_eq!(a, points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap()));
    assert!(
        a.iter()
            .all(|p| (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1))
    );
}

/// **This is the seed-value test**, the pair
/// [`positions_are_the_seeded_stream_row_major`](positions_are_the_seeded_stream_row_major)
/// above is not: two literal `f32`s at `SEED = 0x00_5EED`, recomputed independently from the
/// `Mulberry32` recurrence rather than read out of the layout. Changing `SEED` to anything
/// else fails here, which is what makes the pair a pair.
#[test]
fn the_first_pair_is_pinned() {
    let got = points(&run(&graph(2, &[])).unwrap());
    assert_eq!(got[0], (0.71003205, 0.28633666));
}

/// `layout.random.3d` is the 3-D placement and nothing else: `x`, `y`, `z` or a panic, and
/// `dim()` says `D3`. A 2-D geometry here would answer the oracle's third column wrongly
/// and quietly, since the harness would compare two columns against a uniform cube.
#[test]
fn run_3d_is_a_three_column_draw() {
    let geometry = run_3d(&graph(3, &[(0, 1)])).expect("never refuses");
    assert_eq!(geometry.dim(), Dim::D3, "the arm is 3D");
    let (x, y, z) = space(&geometry);
    assert_eq!((x.len(), y.len(), z.len()), (3, 3, 3));
    for column in [&x, &y, &z] {
        assert!(
            column.iter().all(|v| (0.0..1.0).contains(v)),
            "uniform [0,1)^3, unscaled: {column:?}"
        );
    }
}

/// **This is the draw-order test for the 3-D arm**, and like its 2-D sibling it re-derives
/// with [`super::SEED`] rather than pinning literals, so it holds the ORDER and the COUNT —
/// three draws per node, `x`, `y`, `z` — and not the seed's value.
///
/// The cross-arm half says what the widths really are, because it is the part that is easy
/// to get wrong: the two arms read ONE stream at two widths, so they agree on node 0's `x`
/// and `y` (draws 0 and 1) and part company immediately after. The 2-D arm's node 1 `x` is
/// draw 2, which is this arm's node 0 `z`. A `draw` that read a fresh stream per node, or a
/// 3-D arm built on a second implementation, would move node 0 and fail here.
#[test]
fn run_3d_draws_three_per_node_off_the_same_stream() {
    let mut rng = Mulberry32::new(super::SEED);
    let want: Vec<(f32, f32, f32)> = (0..3)
        .map(|_| (rng.next_f64() as f32, rng.next_f64() as f32, rng.next_f64() as f32))
        .collect();
    let (x, y, z) = space(&run_3d(&graph(3, &[(0, 1)])).expect("runs"));
    let got: Vec<(f32, f32, f32)> = (0..3).map(|i| (x[i], y[i], z[i])).collect();
    assert_eq!(got, want);

    let planar = points(&run(&graph(3, &[(0, 1)])).expect("runs"));
    assert_eq!((planar[0].0, planar[0].1), (x[0], y[0]), "node 0: draws 0 and 1");
    assert_eq!(planar[1].0, z[0], "the 2D arm's node 1 x is this arm's node 0 z");
    assert_ne!(
        (planar[1].0, planar[1].1),
        (x[1], y[1]),
        "node 1 is where the two widths part company"
    );
}

/// **The control for the row above, and the claim that makes it a pair.** The registered 2-D
/// snapshot is hashed by the gate, so `run_3d` must not move a single byte of it. One extra
/// draw per node in the 2-D arm would shift every node after the first and turn the gate
/// red; `draw` is what keeps it green, and this is the test that would notice.
#[test]
fn run_3d_leaves_the_registered_2d_snapshot_exactly_where_it_was() {
    let planar = run(&graph(4, &[(0, 1), (2, 3)])).expect("runs");
    assert_eq!(planar.dim(), Dim::D2, "run stays planar");
    assert_eq!(points(&planar)[0], (0.71003205, 0.28633666), "the seed is unmoved");
    let mut rng = Mulberry32::new(super::SEED);
    let want: Vec<(f32, f32)> = (0..4)
        .map(|_| (rng.next_f64() as f32, rng.next_f64() as f32))
        .collect();
    assert_eq!(points(&planar), want, "two draws per node, in node order");
}

