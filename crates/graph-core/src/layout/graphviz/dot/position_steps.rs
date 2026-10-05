//! One closed case per step of the position pass, so a failure names the step.
//!
//! The six closed cases as a whole live in `position_tests.rs`; this file takes them apart and
//! pins each step's own answer, on the closed case that discriminates that step. The six tests
//! below cover every step `dot_position` runs: the rank heights, the rank constraints, the edge
//! pairs, the x-coordinate simplex, `set_xcoords` with the auxiliary graph's removal, and the
//! frame.
//!
//! Every expected value is written out rather than recomputed from the implementation: a test
//! that recomputes its own answer checks that two pieces of code agree, not that either is right.

use super::fast::{Fast, Kind};
use super::oracle_probe::{centres, fixture_ids, positioned};
use super::position::{Rows, aux, frame, xcoords, ycoords};
use super::rank::rank;
use super::simplex::{self, Params};
use super::{build, mincross};

/// The 5-star `n0 -- n1 .. n4`: one node on rank 0 and four on rank 1, which is the case where
/// every step of the x pass has something to do.
const STAR: &[(u32, u32)] = &[(0, 1), (0, 2), (0, 3), (0, 4)];

/// A fast graph through the rank and mincross passes, so a step can be run on its own.
fn ranked(count: u32, edges: &[(u32, u32)]) -> Fast {
    let ids = fixture_ids(count);
    let borrowed: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut g = build(&borrowed, edges);
    rank(&mut g).expect("the closed cases are rankable");
    mincross::run(&mut g);
    g
}

/// The auxiliary graph, built over an already-ranked graph, with the y step run first because
/// that is the reference's order and the pairs read the y step's pre-placed ranks.
fn auxed(count: u32, edges: &[(u32, u32)]) -> (Fast, Rows, aux::Aux) {
    let mut g = ranked(count, edges);
    let rows = Rows::of(&g);
    ycoords::run(&mut g, &rows);
    let aux = aux::build(&mut g, &rows);
    (g, rows, aux)
}

/// Step 1, `set_ycoords`: the y coordinate of every rank.
///
/// The 4-cycle has four ranks and one node on each, and the table's y values are 234, 162, 90 and
/// 18 — the lowest rank at its own half-height (a 0.5 inch box is 36 points, so 18) and each rank
/// 72 above the next (36 + 36 of box, 36 of `ranksep`).
#[test]
fn set_ycoords_puts_each_rank_seventy_two_points_above_the_next() {
    let mut g = ranked(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
    let rows = Rows::of(&g);
    ycoords::run(&mut g, &rows);
    let ys: Vec<f64> = (0..4).map(|n| g.nodes[n].coord.y).collect();
    assert_eq!(ys, vec![234.0, 162.0, 90.0, 18.0]);
}

/// Step 2, `make_LR_constraints`: one zero-weight constraint between each pair of neighbours on a
/// rank, and the running total that pre-places them.
///
/// The 5-star's rank 1 is `n1 n2 n3 n4`, every box the 0.75 inch minimum, so each constraint is
/// `27 + 27 + 18 = 72` points and the row is pre-placed at 0, 72, 144 and 216 — placed, not
/// solved: the simplex has not run yet.
#[test]
fn lr_constraints_chain_a_rank_at_three_box_widths() {
    let (g, _rows, aux) = auxed(5, STAR);
    let placed: Vec<i32> = (0..5).map(|n| g.nodes[n].rank).collect();
    assert_eq!(
        placed,
        vec![0, 0, 72, 144, 216],
        "the row is pre-placed, not solved"
    );
    let lengths: Vec<i32> = aux
        .constraints()
        .iter()
        .map(|&e| g.edges[e as usize].minlen)
        .collect();
    assert_eq!(
        lengths,
        vec![72, 72, 72],
        "three constraints, one per pair of neighbours"
    );
    assert!(
        aux.constraints()
            .iter()
            .all(|&e| g.edges[e as usize].weight == 0),
        "and every one of them is weight 0: a constraint that must hold, not one to be cheap"
    );
}

/// Step 3, `make_edge_pairs`: one slack node per input edge, one point left of the further end,
/// reached by two one-point edges carrying the edge's weight.
///
/// The 3-path is the smallest case with an edge at all: two slack nodes, four one-point edges,
/// and the two-rank edge's slack node at rank `-1` because both of its ends are pre-placed at 0.
#[test]
fn edge_pairs_add_one_slack_node_and_two_one_point_edges_per_edge() {
    let (g, _rows, aux) = auxed(3, &[(0, 1), (1, 2)]);
    assert_eq!(aux.slack().len(), 2, "one slack node per input edge");
    let lengths: Vec<i32> = aux
        .pairs()
        .iter()
        .map(|&e| g.edges[e as usize].minlen)
        .collect();
    assert_eq!(
        lengths,
        vec![1, 1, 1, 1],
        "two one-point edges per slack node"
    );
    assert!(
        aux.pairs().iter().all(|&e| g.edges[e as usize].weight == 1),
        "each carrying the weight of the edge it stands in for"
    );
    let ranks: Vec<i32> = aux
        .slack()
        .iter()
        .map(|&n| g.nodes[n as usize].rank)
        .collect();
    assert_eq!(ranks, vec![-1, -1], "one point left of the further end");
}

/// Step 4, the second network simplex, `rank(g, 2, …)`.
///
/// **This is the step with no closed form.** The x simplex minimises a weighted total over a
/// polytope whose optimum is a *face*, and `LR_balance` relaxes inside it, so the answer depends
/// on which vertex the pivot loop reached. What *is* derivable is what the 5-star's hub does: the
/// only weighted edges reaching it are its four pairs, and each pair costs the distance between
/// its two ends, so the hub's cost is the sum of its distances to four neighbours and is
/// minimised anywhere between the middle two — the median. The four leaves are 72 apart and
/// symmetric about it, so the hub ends at the middle of the optimal face.
///
/// The absolute x is not derivable — the whole answer is only fixed up to a shift, which the
/// frame step removes — so the leaves' offsets from the hub are what is pinned.
#[test]
fn the_x_simplex_puts_the_star_hub_at_the_median_of_its_neighbours() {
    let (mut g, _rows, aux) = auxed(5, STAR);
    let nlist = aux.node_list();
    simplex::rank2(&mut g, &nlist, &Params::left_right()).expect("the star is connected");
    let hub = f64::from(g.nodes[0].rank);
    let leaves: Vec<f64> = (1..5).map(|n| f64::from(g.nodes[n].rank) - hub).collect();
    assert_eq!(
        leaves,
        vec![-108.0, -36.0, 36.0, 108.0],
        "symmetric about the hub, 72 apart"
    );
}

/// Step 5, `set_xcoords` and `remove_aux_edges`: the answer moves out of `ND_rank` and the
/// graph's own adjacency comes back.
///
/// `rank` is the rank number again on every node, the real nodes carry the x the simplex chose,
/// and the adjacency lists are exactly the graph's own edges — the auxiliary graph is gone.
#[test]
fn set_xcoords_restores_the_rank_and_remove_aux_edges_restores_the_graph() {
    let mut g = ranked(5, STAR);
    let before: Vec<Vec<u32>> = g.out.clone();
    let rows = Rows::of(&g);
    ycoords::run(&mut g, &rows);
    let aux = aux::build(&mut g, &rows);
    let nlist = aux.node_list();
    let constraints: Vec<u32> = aux
        .constraints()
        .iter()
        .chain(aux.pairs())
        .copied()
        .collect();
    simplex::rank2(&mut g, &nlist, &Params::left_right()).expect("the star is connected");
    xcoords::run(&mut g, &rows);
    aux.remove(&mut g);
    assert_eq!(g.out, before, "the graph's own adjacency, back");
    assert_eq!(
        (0..5).map(|n| g.nodes[n].rank).collect::<Vec<_>>(),
        vec![0, 1, 1, 1, 1],
        "rank is the rank number again"
    );
    assert!(
        constraints.iter().all(|&e| !g.edges[e as usize].live),
        "every constraint is dead"
    );
}

/// Step 6, the frame: the drawing's lower-left node-box corner is the origin.
///
/// The one-node closed case *is* the frame, because a single node has no relative position: its
/// whole answer is where the origin sits. At 27 points of x and 18 of y its box runs from the
/// origin to (54, 36), which is the 0.75 by 0.5 inch box `-Tplain`'s own `graph` line prints as
/// `0.75 0.5`.
#[test]
fn the_frame_puts_the_lower_left_node_box_corner_at_the_origin() {
    let g = positioned(1, &[]);
    let n = &g.nodes[0];
    assert_eq!((n.coord.x, n.coord.y), (27.0, 18.0));
    assert_eq!((n.lw, n.ht / 2.0), (27.0, 18.0));
    assert_eq!(n.kind, Kind::Normal);
}

/// The negative control for the frame: the shift is neither a constant nor nothing.
///
/// The 5-star's drawing is 243 points wide, so its frame moves x; its lowest rank is already on
/// its own half-height, so the frame does not move y on **any** graph this port can draw — and
/// the two-node case is what says so, because its bottom rank's centre sits at 18 before the
/// shift as well as after it. A frame that shifted by a fixed amount fails the 5-star; a frame
/// that shifted y as well fails the 3-path.
#[test]
fn the_frame_shifts_x_and_needs_no_shift_in_y() {
    let pair = positioned(2, &[(0, 1)]);
    let ys: Vec<f64> = centres(&pair).iter().map(|p| p.1).collect();
    assert_eq!(
        ys,
        vec![90.0, 18.0],
        "the bottom rank's centre is its own half-height"
    );
    assert_eq!(
        frame::lowest_offset(&pair, &Rows::of(&pair)),
        0.0,
        "so the vertical offset the reference subtracts is zero"
    );
    let star = positioned(5, STAR);
    assert_eq!(frame::lower_left(&star, &Rows::of(&star)), (0.0, 0.0));
}
