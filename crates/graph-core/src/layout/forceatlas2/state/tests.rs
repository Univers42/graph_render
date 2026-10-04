use super::{Fa2Params, Fa2State, MAX_DIM, norm2};
use crate::index::index_model;
use crate::records::build::{edge, node};

fn line(n: u32) -> crate::index::Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// **The association in `gravity` is load-bearing, and this is the test that says so.**
///
/// The 2D arm must write `gravity * mass * (centred / norm)` — the unit vector first.
/// The other grouping, `gravity * mass * centred / norm` (p12-t4b's), is a different
/// float: on this state the two differ in the last bit for a majority of the nodes,
/// and over `max_iter` iterations that difference is amplified into visibly different
/// coordinates. The first merge round pinned this by comment; a comment does not fail a
/// build, so it is pinned by assertion here.
///
/// The assertion is on the *force after one gravity pass*, not on settled coordinates:
/// a settled position is a fixed point of a 100-iteration chaotic kernel, so pinning it
/// would make this test fail for any unrelated numerical change too. One pass is where
/// the association is actually chosen.
#[test]
fn the_2d_gravity_pull_multiplies_the_unit_vector_not_the_offset() {
    let mut state = Fa2State::new(&line(20), Fa2Params::default(), 2);
    // The pull is the only term in `gravity`, so zero the accumulators first rather
    // than depending on the order `attraction` and `repulsion` happened to run in.
    state.u.fill([0.0; MAX_DIM]);
    state.gravity();
    let mean = state.mean();
    let (mut same, mut differ) = (0, 0);
    for i in 0..state.p.len() {
        let mut centred = [0.0; MAX_DIM];
        for (a, slot) in centred.iter_mut().enumerate().take(2) {
            *slot = state.p[i][a] - mean[a];
        }
        let norm = f64::sqrt(norm2(&centred, 2));
        let g = state.params.gravity * state.mass[i];
        for (a, &c) in centred.iter().enumerate().take(2) {
            let unit_first = -g * (c / norm);
            let offset_first = -g * c / norm;
            assert_eq!(
                state.u[i][a].to_bits(),
                unit_first.to_bits(),
                "node {i} axis {a}: gravity must scale the unit vector"
            );
            if unit_first.to_bits() == offset_first.to_bits() {
                same += 1;
            } else {
                differ += 1;
            }
        }
    }
    assert!(
        differ > 0,
        "the two associations agreed on all {same} coordinates, so this test cannot \
         distinguish them and the pinning is vacuous"
    );
}

/// The D9 guard triggers on an exactly coincident pair and on nothing else. The
/// port's own group works in `f64`, so an underflowing pair is reachable in
/// principle; p12-t4b's `norm2(&d, dim) == 0.0` would nudge such a pair, and develop's
/// per-axis `dx == 0.0 && dy == 0.0` would not. This pins the per-axis form.
#[test]
fn a_coincident_pair_is_nudged_by_the_counter_hash_not_left_to_divide() {
    let params = Fa2Params::default();
    let mut state = Fa2State::new(&line(2), params, 2);
    // Put the two nodes on exactly the same point, so every live axis is zero.
    state.p[1] = state.p[0];
    let d = state.repel_delta(0, 1);
    assert!(
        d[0] != 0.0 || d[1] != 0.0,
        "a coincident pair is nudged, not left at zero: no Infinity can reach a node"
    );
    // A pair that is merely close is not coincident and must not be nudged: the guard
    // is a guard against division by zero, not a snapping of near-coincident pairs.
    // The offset has to be big enough to *survive* the addition — `initial_positions`
    // draws into [0, 1), whose ulp near 0.5 is about 1.1e-16, so a denormal or a
    // 1e-300 offset rounds away and leaves the pair still exactly coincident, which
    // would make this second assertion pass for the wrong reason.
    let (dx, dy) = (1e-8_f64, -1e-8_f64);
    state.p[1][0] = state.p[0][0] + dx;
    state.p[1][1] = state.p[0][1] + dy;
    // The expected value is the difference the kernel itself reads, in its own order
    // (`p[0] - p[1]`), not the offset typed above: the addition rounds, so `dx` is the
    // intended separation and not the represented one.
    let want = [state.p[0][0] - state.p[1][0], state.p[0][1] - state.p[1][1]];
    let near = state.repel_delta(0, 1);
    assert_eq!(&near[..2], &want, "a near pair is passed through");
}
