//! `link.rs`'s own test, split out for the house 300-line cap — the same split
//! `charge.rs`, `collide.rs` and `tests.rs` already use.

use super::*;
use crate::index::index_model;
use crate::layout::force::ForceParams;
use crate::layout::force::simple_graph;
use crate::records::build::{edge, node};

#[test]
fn bias_favours_the_lower_degree_endpoint_moving_more() {
    // hub (many edges) -- mid (one edge): hub's degree dwarfs mid's.
    let nodes = [
        node("hub", ""),
        node("mid", ""),
        node("a", ""),
        node("b", ""),
        node("c", ""),
    ];
    let edges = [
        edge("e0", "hub", "mid"),
        edge("e1", "hub", "a"),
        edge("e2", "hub", "b"),
        edge("e3", "hub", "c"),
    ];
    let t = index_model(&nodes, &edges).expect("fits");
    let g = simple_graph(&t);
    let (_, _, bias) = geometry(&g, &ForceParams::default().into());
    // edge0 is (hub=lo=0, mid=hi=1): bias = degree(lo)/(degree(lo)+degree(hi)) = 4/5.
    assert!((bias[0] - 0.8).abs() < 1e-12);
}

/// The edge's two halves are exact negations of one another in `x` and in `y`, which is
/// what makes the node-partitioned kernel the same computation as the edge-partitioned
/// loop: the higher endpoint's share is the lower one's, negated, rather than a second
/// derivation of the same arithmetic.
///
/// They are not exact negations of the *shares* (the bias is not one half), so this pins
/// the difference's antisymmetry — the claim the kernel rests on — by taking the two
/// directions through the same function and comparing them exactly.
#[test]
fn the_shared_difference_read_from_either_end_is_the_exact_negation() {
    let nodes = [node("a", ""), node("b", ""), node("c", "")];
    let edges = [edge("e0", "a", "b"), edge("e1", "b", "c")];
    let t = index_model(&nodes, &edges).expect("fits");
    let sim = super::super::sim::Sim::new(&t, ForceParams::default().into(), 0);
    let mut forward = displaced(&sim, 1, 0);
    let backward = displaced(&sim, 0, 1);
    forward.0 = -forward.0;
    forward.1 = -forward.1;
    assert_eq!(forward, backward);
}

/// Coincident endpoints give `+0.0` from either direction — not `-0.0` — so the two
/// halves take the same `jiggle` branch and the halves stay exact negations. A `-0.0`
/// would still compare equal but would print and hash differently.
#[test]
fn coincident_endpoints_read_as_positive_zero_from_either_end() {
    let nodes = [node("a", ""), node("b", "")];
    let edges = [edge("e0", "a", "b")];
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = super::super::sim::Sim::new(&t, ForceParams::default().into(), 0);
    for i in 0..sim.x.len() {
        sim.x[i] = 1.0;
        sim.y[i] = 1.0;
        sim.vx[i] = 0.0;
        sim.vy[i] = 0.0;
    }
    let (dx, dy) = displaced(&sim, 1, 0);
    assert_eq!((dx.is_sign_positive(), dy.is_sign_positive()), (true, true));
    let ((lox, loy), (hix, hiy)) = halves(&sim, 0);
    assert!(lox.is_finite() && loy.is_finite());
    assert!(hix.is_finite() && hiy.is_finite());
}
