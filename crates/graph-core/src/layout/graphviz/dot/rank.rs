//! `dot1_rank` (`rank.c:509-526`): the whole rank pass, in the reference's order.
//!
//! ```text
//! edgelabel_ranks(g);        // no-op: no edge labels
//! collapse_sets(g, g);      // no-op: no rank=same/min/max sets, no clusters
//! class1(g);                // build the fast graph: one constraint per input edge
//! p = minmax_edges(g);      // no-op: no min or max set, so no edges reversed
//! decompose(g, 0);          // the connected components and each one's node order
//! acyclic(g);               // break every remaining cycle by reversing an edge
//! minmax_edges2(g, p);      // no-op, and with `p` zero it re-decomposes nothing
//! rank1(g);                 // one network simplex per component, TB-balanced
//! expand_ranksets(g);       // no-op: with no ranksets every node is its own leader
//! cleanup1(g);              // empty the fast graph, keeping the input edges
//! ```
//!
//! The four no-ops are named rather than dropped, because their absence is a decision and
//! not an oversight: each is a stage that reads an attribute this port does not carry, and
//! a stage that silently vanished would be a stage that could not come back. See
//! `docs/decisions/graphviz-oracle.md` for why there is no cluster and no edge-label path.
//! **`class1` is not one of them** — a first reading of the pass list calls it a cluster-only
//! stage and is wrong; it is what puts the edges into the fast graph, and without it the
//! simplex has an empty graph to rank. See [`class1`].
//!
//! **What the pass produces.** After it, every real node of `g` holds a `rank`, the lowest
//! one is 0, and every edge runs from a lower rank to a higher one. Rank 0 is the *top* row:
//! `set_ycoords` (`position.c:773-786`) puts `GD_maxrank` at the bottom and stacks the rest
//! upwards, so `y = y_of_rank_0 - rank * (height + ranksep)`. That is the whole of what
//! `mincross` and `position` inherit — the ranks are a layer assignment, and a wrong rank is
//! a row in the wrong place rather than a coordinate off by a few points.
//!
//! Determinism: the component order is `decompose`'s, the simplex inside a component is
//! deterministic in its own right, and nothing here reads a clock, a hash order or a random
//! number (`prompt.md` §6 D1-D10).

use super::acyclic;
use super::class1;
use super::decomp::decompose;
use super::fast::Fast;
use super::simplex::{self, Error, Params};

/// `dot_rank` (`rank.c:528-537`) with the default ranking algorithm: `dot1_rank`.
///
/// The `newrank` attribute selects a different engine (`dot2_rank`), which the motor does
/// not implement; `dot` never sets it, and a caller that wanted it would have to say so in
/// an attribute this port does not carry.
pub fn rank(g: &mut Fast) -> Result<(), Error> {
    class1::run(g);
    let components = decompose(g);
    for component in &components {
        acyclic::run(g, component);
    }
    rank1(g, &components)?;
    cleanup1(g);
    Ok(())
}

/// `rank1` (`rank.c:453-464`): the network simplex once per component, each with
/// top-bottom balance and no iteration cap. `rank1` reads `nslimit1`; nothing sets it, so
/// the cap is the reference's `INT_MAX`.
pub fn rank1(g: &mut Fast, components: &[Vec<u32>]) -> Result<(), Error> {
    let params = Params::top_bottom();
    for component in components {
        simplex::rank2(g, component, &params)?;
    }
    Ok(())
}

/// `cleanup1` (`rank.c:80-164`): the rank pass is over, so the fast graph it was run on is
/// handed back empty. The input edges survive — in the reference in the cgraph, here in
/// `Fast::orig_out` — and the chains `acyclic` built are dropped with the lists.
///
/// This is the boundary the next pass starts from, and it is why `class2` reads `orig_out`
/// and not `out`: after this call both adjacency lists are empty, and everything the
/// position pass will walk is a chain `class2` built afterwards.
fn cleanup1(g: &mut Fast) {
    for edge in &mut g.edges {
        edge.to_virt = None;
        edge.to_orig = None;
        edge.cutvalue = 0;
        edge.tree_index = -1;
    }
    g.clear_adjacency();
}