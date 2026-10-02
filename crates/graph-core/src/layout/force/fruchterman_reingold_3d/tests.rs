//! `layout.force.fruchterman_reingold_3d`: the dimension is real, the snapshot carries it, and
//! the 2D id is untouched.
//!
//! Nothing here re-tests the algorithm — `super::super::fruchterman_reingold::tests` does that
//! over the identical kernel at `D = 2`, and pins those coordinates bit for bit. What these tests
//! exist for is the one thing that genuinely differs at `D = 3`: a third axis in the solve, and a
//! z column on the wire. A 3D stage that computed two dimensions and attached a zero z would hash,
//! decode, and lie.

use super::super::fruchterman_reingold::{FR_CEILING, FrParams, FruchtermanReingold};
use super::{FruchtermanReingold3D, ID_3D};
use crate::index::{Topology, empty_model};
use crate::layout::Geometry;
use crate::layout::coords::probe::graph;
use crate::registry;
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

/// `path_edges(n)`: the `n`-node path, the shape whose 3D drawing is a helix rather than a line.
fn path(n: u32) -> Topology {
    graph(
        n,
        &(0..n.saturating_sub(1))
            .map(|i| (i, i + 1))
            .collect::<Vec<_>>(),
    )
}

/// The three columns of a 3D geometry, in axis order. Panics rather than returning `None`: a
/// geometry with no z is the failure this file is about, so it must not be readable past.
fn columns(g: &Geometry) -> (&[f32], &[f32], &[f32]) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    (
        x.as_slice(),
        y.as_slice(),
        g.z.as_deref().expect("a 3D geometry carries a z column"),
    )
}

fn space(t: &Topology, params: &FrParams) -> Geometry {
    FruchtermanReingold3D::run(t, params).expect("finite")
}

/// **The third column is a real coordinate.** A z of all zeros would answer every other
/// assertion here correctly and hash as a planar drawing, which is exactly what the separate id
/// exists to make loud.
#[test]
fn the_third_column_is_a_real_coordinate() {
    let g = space(&path(6), &FrParams::default());
    let (x, y, z) = columns(&g);
    assert_eq!((x.len(), y.len(), z.len()), (6, 6, 6));
    assert!(
        z.iter().any(|v| *v != 0.0),
        "z is all zero: this is a 2D run"
    );
    assert!(
        x.iter().chain(y).chain(z).all(|v| v.is_finite()),
        "a non-finite coordinate would be refused, not returned"
    );
}

/// The third axis participates in the solve, so this is not the 2D drawing with a z attached.
/// Were `D` ignored inside the kernels, these two equalities would hold.
#[test]
fn the_third_axis_participates_rather_than_being_attached() {
    let t = path(6);
    let params = FrParams::default();
    let NodeGeometry::Point { x, y } = FruchtermanReingold::run(&t, &params).expect("finite").nodes
    else {
        panic!("point nodes");
    };
    let solid = space(&t, &params);
    let (x3, y3, z3) = columns(&solid);
    assert!(z3.iter().any(|v| *v != 0.0));
    assert_eq!(
        x.len(),
        x3.len(),
        "the two stages disagree on the node count"
    );
    assert_ne!(
        x, x3,
        "the 3D x column is the 2D one: z is not in the solve"
    );
    assert_ne!(
        y, y3,
        "the 3D y column is the 2D one: z is not in the solve"
    );
}

/// The id is registered apart from the 2D one, with its own hash-gate stage and the same ceiling.
#[test]
fn the_three_d_id_is_registered_apart_from_the_two_d_one() {
    let three = registry::find(ID_3D).expect("registered");
    let two = registry::find(FruchtermanReingold::ID).expect("registered");
    assert_ne!(three.id, two.id);
    assert_eq!(three.id, "layout.force.fruchterman_reingold_3d");
    assert_eq!(three.meta.stage, "layout");
    assert_eq!(three.meta.scale_ceiling, FR_CEILING);
}

/// Determinism: the same seed twice is the same answer, and a different seed is not. The start is
/// a random box, so this is where a generator that leaked state would show.
#[test]
fn a_run_is_repeatable_and_the_seed_matters() {
    let t = graph(
        8,
        &[
            (0, 7),
            (1, 0),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (6, 5),
            (7, 6),
        ],
    );
    let once = space(&t, &FrParams::default());
    let twice = space(&t, &FrParams::default());
    assert_eq!(once.nodes, twice.nodes, "the same seed drew two layouts");
    assert_eq!(once.z, twice.z, "the same seed drew two layouts");
    let other = space(
        &t,
        &FrParams {
            seed: 7,
            ..FrParams::default()
        },
    );
    assert_ne!(once.nodes, other.nodes, "the seed changed nothing");
}

/// The degenerate sizes stay finite in space, which is where a third axis has one more way to go
/// wrong than in the plane.
#[test]
fn empty_and_single_node_graphs_are_finite_in_space() {
    for t in [empty_model(), graph(1, &[])] {
        let g = space(&t, &FrParams::default());
        let (x, y, z) = columns(&g);
        let want = t.node_count() as usize;
        assert_eq!((x.len(), y.len(), z.len()), (want, want, want));
        assert!(x.iter().chain(y).chain(z).all(|v| v.is_finite()));
    }
}

/// A disconnected graph in 3D is the only path through the spec's extra `C = n sqrt(n)` pair
/// term, and the only block where the source carries a mistyped axis (recorded, with this port's
/// decision, in `docs/layouts/layout.force.fruchterman_reingold.md`). This holds the result finite
/// and bounded, which is what that decision was made for.
#[test]
fn a_disconnected_graph_stays_finite_and_bounded_in_space() {
    let g = space(
        &graph(6, &[(0, 1), (1, 2), (3, 4), (4, 5)]),
        &FrParams::default(),
    );
    let (x, y, z) = columns(&g);
    let largest = x
        .iter()
        .chain(y)
        .chain(z)
        .fold(0.0_f32, |a, b| a.max(b.abs()));
    assert!(x.iter().chain(y).chain(z).all(|v| v.is_finite()));
    assert!(largest < 1e6, "the drawing ran away: {largest}");
}

/// Zero iterations return the seeded start, un-iterated — the spec's `niter = 0` case. Checked in
/// space because it is the cheapest way to see the start box is three wide and not two.
#[test]
fn zero_iterations_return_the_seeded_start_in_space() {
    let g = space(
        &path(4),
        &FrParams {
            niter: 0,
            ..FrParams::default()
        },
    );
    let (x, y, z) = columns(&g);
    let side = (4.0_f32).sqrt();
    for column in [x, y, z] {
        assert!(
            column.iter().all(|v| v.abs() <= side / 2.0),
            "a start coordinate left the box of side sqrt(n)"
        );
    }
    assert!(z.iter().any(|v| *v != 0.0), "the start z is all zero");
}
