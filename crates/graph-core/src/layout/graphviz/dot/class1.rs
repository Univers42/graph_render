//! `class1` (`class1.c:65-107`): build the fast graph the rank pass ranks.
//!
//! **This is the pass that turns input edges into ranked constraints**, and it is the one
//! stage of `dot1_rank` whose job is not obvious from its name. The reference's `dot_init_*`
//! allocates the node and edge records and nothing else, so at the start of the rank pass
//! `ND_in` and `ND_out` are *empty*: `decompose` would find every node its own component and
//! `acyclic` would have nothing to walk. `class1` is what fills them, and it does it by
//! giving each input edge a **copy** in the fast graph rather than by moving the input edge
//! itself into it.
//!
//! The copy is not bookkeeping. `find_fast_edge(t, h)` in `class1` and in `class2` asks
//! whether the pair already has an edge, and the answer has to be "no" for the edge being
//! examined — so the edge being examined cannot already be in the fast graph. And the copy
//! carries `to_virt`, which is how `class2` finds the chain it built for an input edge
//! later. Moving the input edge in instead would answer "yes" and make `merge_oneway` merge
//! an edge into itself.
//!
//! A second pair of endpoints is folded into the first, so two input edges between the same
//! pair are one constraint with double the weight — before ranking, not after, so the
//! simplex sees the same graph the reference's does.
//!
//! What drops out because this port has no clusters: `mark_clusters`, `interclust1`'s slack
//! node, and the `UF_find` indirection (with no `rank=same` set every node is its own
//! leader). Self-loops are dropped for the reason the reference drops them — a node cannot
//! constrain itself — and `class2` files them as `other` edges afterwards.
//!
//! Determinism: the walk is the input order (the dense node order), the inner walk is
//! declaration order, and `find_fast_edge` returns the first edge in the tail's out list,
//! which is the one declared first.

use super::fast::Fast;

/// `class1`: every input edge becomes a fast-graph constraint, parallel pairs becoming one.
pub fn run(g: &mut Fast) {
    for list in g.out.iter_mut().chain(g.inn.iter_mut()) {
        list.clear();
    }
    for node in 0..g.nodes.len() {
        for edge in g.orig_out[node].clone() {
            let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
            if tail == head {
                continue;
            }
            match g.find_edge(tail, head) {
                Some(rep) => g.merge_oneway(edge, rep),
                None => {
                    g.add_chain(tail, head, edge);
                }
            }
        }
    }
}