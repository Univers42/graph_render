//! `dot`'s second pass: the order of the nodes within each rank.
//!
//! The rank pass decides which row a node is on. This pass decides where on that row, and it
//! is the one that has to answer a question no single node can answer alone: the number of
//! edge crossings depends on every edge at once, so the order is improved by moving nodes
//! towards the middle of their neighbours and by swapping neighbours that cross, repeatedly,
//! until neither move helps.
//!
//! ## What the pass does, in order
//!
//! 1. [`class2`] — chain every edge that spans more than one rank, so an edge has a node on
//!    every rank it crosses. The crossings are counted over these chains, not over the input
//!    edges. (Already ported; see [`class2`].)
//! 2. [`decomp::decompose`](super::decomp::decompose) — the connected components, each with its own node order. The
//!    pass runs per component, because a component's rows are the only rows its edges reach.
//! 3. [`ranks::Ranks::allocate`] — one array per rank, sized for every node on it and every
//!    edge crossing it.
//! 4. [`driver::mincross`] per component: an order from a walk out of every source, an
//!    order from a walk out of every sink, then repeated median/transpose sweeps, keeping the
//!    best order any sweep found.
//! 5. [`ranks::Ranks::install_complete_ranks`] — the components' rows become whole ranks and
//!    the orders become positions in them.
//!
//! ## What the pass produces
//!
//! Every node — real or a chain dummy — carries its position within its rank in
//! [`Node::order`](super::fast::Node::order), and the total number of crossings the order has is [`run`]'s return
//! value. That count is a property of the *order*, so [`crossings::crossings`] recomputes it
//! from the nodes rather than reading a cache: it is what the 1000-seed sweep compares.
//!
//! ## What is not here, and why it does not cost anything on the fixtures
//!
//! Three things the reference has are absent: clusters (this port has none, so the whole
//! cluster path is a cluster path), edge ports (this port has none, and the half of the
//! crossing count that exists only for ports is gone with them), and the same-rank edge
//! precedence (see [`transpose`]'s `Ponytail` line: **no fixture seed of the 1000 has a
//! same-rank edge**, measured over the probe's own rank column). The reference's edge
//! *ordering* attribute is absent for the same reason as the ports — there is no attribute
//! channel — and it is a no-op when the attribute is not there at all.
//!
//! Ponytail: the flat-edge precedence is the one omission with a direction. A graph holding
//! an edge whose ends land on the same rank has an order this port is free to contradict,
//! and the reference would not. No fixture of the 1000 seeds has one, so nothing measured
//! here can see it; a graph that does is the escape hatch's failing input.
//!
//! Determinism: the node order of a component is the components pass's, every walk and every
//! sweep visits nodes and edges in adjacency-list order, every count is a sum in a fixed
//! order, and nothing in the pass reads a clock, a hash order or a random number
//! (`prompt.md` §6 D1-D10). The 1000-seed sweep is the check: 1000 independent runs of a
//! pure function, so its counts are a measurement and not a sample.

pub mod build;
pub mod crossings;
pub mod driver;
pub mod median;
pub mod ranks;
pub mod transpose;

use super::class2;
use super::decomp::decompose;
use super::fast::Fast;
use ranks::Ranks;

/// Run the whole pass over a ranked graph, returning the number of crossings the order it
/// leaves has.
///
/// The graph must have been through [`rank`](mod@super::rank): this pass reads the ranks and
/// builds the chains it works on, and both come from there.
pub fn run(g: &mut Fast) -> i64 {
    class2::run(g);
    let components = decompose(g);
    let mut ranks = Ranks::allocate(g);
    let mut total = 0;
    for (index, component) in components.iter().enumerate() {
        ranks.enter_component(index, component);
        total += driver::mincross(g, &mut ranks, 0);
    }
    ranks.install_complete_ranks(g);
    total
}
