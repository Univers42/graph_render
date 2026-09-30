use super::{merge, point_geometry, rescale, rescale_under};
use graph_contract::geometry::NodeGeometry;

#[test]
fn rescale_centres_then_divides_by_the_largest_absolute_coordinate() {
    let (mut x, mut y) = (vec![0.0, 4.0], vec![1.0, 1.0]);
    rescale(&mut x, &mut y);
    assert_eq!((x, y), (vec![-1.0, 1.0], vec![0.0, 0.0]));
}

#[test]
fn rescale_leaves_a_collapsed_cloud_at_the_origin_and_an_empty_one_alone() {
    let (mut x, mut y) = (vec![3.0, 3.0], vec![-2.0, -2.0]);
    rescale(&mut x, &mut y);
    assert_eq!((x, y), (vec![0.0, 0.0], vec![0.0, 0.0]));
    rescale(&mut [], &mut []);
}

/// The honest merge is `Iterator::sum` **term for term and bit for bit** — the claim every
/// threaded layout's equality rests on, and the reason a threaded arm cannot land on a
/// different double by summing the same values in a different order.
///
/// The column is built to make the order visible: inexact fractions, both signs, a
/// magnitude that swamps the small terms, and a pair that cancels exactly.
#[test]
fn the_honest_merge_is_the_iterator_sum_bit_for_bit() {
    let column = [
        1.0 / 3.0,
        -2.0 / 7.0,
        1.0e17,
        1.0,
        -1.0,
        core::f64::consts::PI,
        1.0 / 3.0 - 2.0 / 7.0,
        f64::MIN_POSITIVE,
    ];
    let folded: f64 = column.iter().sum();
    assert_eq!(
        merge(&column, false).to_bits(),
        folded.to_bits(),
        "the loop and the iterator disagree, so `rescale` is not `rescale_layout`"
    );
    // A column of the ring's own shape, gathered in index order: cosines whose sum is
    // near zero, which is where a reordered sum would show first.
    let ring: Vec<f64> = (0..64).map(|k| libm::cos(k as f64 * 0.09817477)).collect();
    assert_eq!(
        merge(&ring, false).to_bits(),
        ring.iter().sum::<f64>().to_bits()
    );
}

/// The control reaches the **merge**, not the gather: node `i`'s sum reads node `i + 1`'s
/// term, which is the shape a wrong partition of the outputs would take. Two nodes, the
/// smallest cloud the gate's own model can draw (`2 + seed % 600`).
#[test]
fn the_control_steals_the_next_nodes_term_in_the_merge() {
    let (mut honest, mut stolen) = (
        (vec![1.0, -1.0], vec![0.0, 0.0]),
        (vec![1.0, -1.0], vec![0.0, 0.0]),
    );
    rescale(&mut honest.0, &mut honest.1);
    rescale_under(&mut stolen.0, &mut stolen.1, true);
    // Hand-worked. The stolen sum is (1 + -1) + (-1 + 0) = -1 over two nodes, so the mean
    // x is -0.5 against the honest 0.0; the recentred cloud is [1.5, -0.5] and the limit
    // is 1.5, against the honest [1.0, -1.0] over a limit of 1.0.
    assert_eq!(honest, (vec![1.0, -1.0], vec![0.0, 0.0]));
    assert_eq!(stolen.0[0].to_bits(), 1.0_f64.to_bits());
    assert_eq!(stolen.0[1].to_bits(), (-1.0_f64 / 3.0).to_bits());
    assert_ne!(
        honest, stolen,
        "the merge did not move, so the control cannot bite"
    );
}

/// One node has no neighbour to steal from, so a control run whose only stage was a
/// one-node cloud would be a **vacuous pass**: the floor is one seed, and this is what
/// says the floor is honest.
#[test]
fn the_control_cannot_bite_on_a_one_node_cloud_and_does_on_two() {
    let (mut one, mut one_split) = ((vec![0.0], vec![0.0]), (vec![0.0], vec![0.0]));
    rescale(&mut one.0, &mut one.1);
    rescale_under(&mut one_split.0, &mut one_split.1, true);
    assert_eq!(one, one_split);
    let (mut two, mut two_split) = (
        (vec![1.0, -1.0], vec![0.0, 0.0]),
        (vec![1.0, -1.0], vec![0.0, 0.0]),
    );
    rescale(&mut two.0, &mut two.1);
    rescale_under(&mut two_split.0, &mut two_split.1, true);
    assert_ne!(two, two_split, "two nodes is where the control bites");
}

#[test]
fn point_geometry_narrows_to_f32_and_draws_lines() {
    let g = point_geometry(&[0.5], &[0.25]);
    assert_eq!(
        g.nodes,
        NodeGeometry::Point {
            x: vec![0.5],
            y: vec![0.25]
        }
    );
    assert!(g.notes.is_empty());
}
