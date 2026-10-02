//! The review's cost findings on the passes (`docs/reviews/review-core-post.md` R23,
//! R24): the declared row is `O(n + m)` per pass (`capabilities.rs:146`), and a pass
//! that rescans every edge per leaf, per chain hop or per community is `O(n·m)`.
//!
//! Ponytail: a wall-clock bound, not an operation count. Failing input: a host so loaded
//! that a linear pass over 300 000 nodes (well under a second in a debug build here)
//! takes [`BOUND`]; the quadratic passes made 4.5·10^10 edge visits. Direction: a false
//! red, never a false green. Escape hatch: re-run the test on its own.

use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(20);
const N: u32 = 300_000;

/// `work` on its own thread, failing the test when it has not returned within [`BOUND`].
/// The thread is left behind on a failure and dies with the test process.
fn finishes_within_bound<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(work()));
    receiver
        .recv_timeout(BOUND)
        .unwrap_or_else(|_| panic!("not finished within {BOUND:?}: a pass is super-linear"))
}

/// `n` nodes and one edge per `(a, b)` of `pairs`.
pub(super) fn graph(n: u32, pairs: impl Iterator<Item = (u32, u32)>) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "db")).collect();
    let edges: Vec<_> = pairs
        .enumerate()
        .map(|(e, (a, b))| edge(&format!("e{e}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn run(t: Topology, plan: Plan) -> Simplified {
    finishes_within_bound(move || simplify(&t, &plan))
}

/// R23, leaves: a star folds `N - 1` leaves, one edge lookup each.
#[test]
fn a_large_star_folds_its_leaves_in_linear_time() {
    let t = graph(N, (1..N).map(|leaf| (0, leaf)));
    let plan = Plan {
        fold_leaves: true,
        ..Plan::nothing()
    };
    let s = run(t, plan);
    assert_eq!(counts(&s), (N - 1, N - 1));
}

/// R23, chains: a path is one chain of `N - 2` interior nodes.
#[test]
fn a_large_path_contracts_its_chain_in_linear_time() {
    let t = graph(N, (1..N).map(|v| (v - 1, v)));
    let plan = Plan {
        contract_chains: true,
        ..Plan::nothing()
    };
    let s = run(t, plan);
    assert_eq!(counts(&s), (N - 2, N - 1));
}

/// R23, re-walks: a chain that is skipped (a ring, or a path whose ends were folded) is
/// walked once, not once per interior node.
#[test]
fn a_skipped_chain_is_walked_once() {
    let ring = graph(N, (0..N).map(|v| (v, (v + 1) % N)));
    let chains = Plan {
        contract_chains: true,
        ..Plan::nothing()
    };
    assert_eq!(counts(&run(ring, chains)), (0, 0));
    let path = graph(N, (1..N).map(|v| (v - 1, v)));
    let leaves_then_chains = Plan {
        fold_leaves: true,
        ..chains
    };
    assert_eq!(counts(&run(path, leaves_then_chains)), (2, 2));
}

/// R24: `K` disjoint triangles are `K` communities of three, and the pass reads each edge
/// once, not once per community. A singleton community never reached the rescan, so the
/// input needs communities of two or more.
#[test]
fn many_communities_collapse_in_linear_time() {
    const K: u32 = 40_000;
    let triangles = (0..K).flat_map(|c| {
        let v = 3 * c;
        [(v, v + 1), (v + 1, v + 2), (v + 2, v)]
    });
    let t = graph(3 * K, triangles);
    let plan = Plan {
        collapse_communities: true,
        ..Plan::nothing()
    };
    let s = run(t, plan);
    assert_eq!(counts(&s), (2 * K, 3 * K));
    assert_eq!(s.steps.len(), K as usize);
}
