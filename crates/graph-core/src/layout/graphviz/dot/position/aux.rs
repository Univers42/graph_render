//! `create_aux_edges` (`position.c:525-532`) and its two halves: `make_LR_constraints`
//! (`position.c:219-325`) and `make_edge_pairs` (`position.c:327-352`), plus
//! `remove_aux_edges` (`position.c:534-569`).
//!
//! The x coordinates are **not** computed. They are chosen by a second run of the same network
//! simplex the rank pass used, over an auxiliary graph this module builds and then throws away.
//! Two kinds of edge go into it:
//!
//! - **A left-to-right constraint between two neighbours on one rank.** Its minimum length is
//!   `right half of the left node + left half of the right node + nodesep` — the three widths a
//!   reader needs to see two boxes not overlap. It carries **weight 0**: the constraint must
//!   hold, but the pass is not asked to make it cheap.
//! - **A pair of edges per input edge, through a slack node.** The slack node is placed one
//!   point left of whichever end is further left and reaches both ends with length one. That is
//!   not padding: it is the statement *an edge's two ends are not the same point*, which is the
//!   one thing the rank constraints do not say. These carry the edge's own **weight**, so the
//!   simplex trades them against each other, and it is this that pulls a node towards the middle
//!   of its neighbours instead of leaving it where the rank constraints put it.
//!
//! The auxiliary graph is removed after `set_xcoords`, which is why [`Aux`] keeps the two
//! adjacency lists it displaced: the fast graph's own edges survive in them, and only the
//! indices of what was added are remembered.
//!
//! **The node list the second simplex runs over is an input, not a convenience.**
//! `fast_node` prepends, so the slack nodes end up in front of every node the graph already
//! had, newest first — and the network simplex's warm start walks that list in order, so a
//! different order reaches a different optimal tree and draws a different picture.
//!
//! Determinism: the rank rows are walked in rank order, the constraints are made in slot order,
//! the pairs in the node list's order and each node's own out-list order, and nothing here
//! reads a clock, a hash order or a random number (`prompt.md` §6 D1-D10).

use super::Rows;
use super::super::fast::{Fast, Node, round};
use super::super::{NODESEP, decomp};

/// What `allocate_aux_edges` saves and `remove_aux_edges` restores: the graph's own adjacency,
/// displaced by the constraints, together with what this pass added.
pub struct Aux {
    /// `ND_save_out`: every node's out-edges as they were before the constraints went in.
    ///
    /// This is what `make_edge_pairs` walks rather than the live list, and it is what
    /// `remove_aux_edges` puts back.
    saved_out: Vec<Vec<u32>>,
    /// `ND_save_in`: the same for in-edges.
    saved_in: Vec<Vec<u32>>,
    /// The slack nodes, in the order `make_edge_pairs` created them. Reversed for the node
    /// list, because `fast_node` prepends.
    slack: Vec<u32>,
    /// Every edge this pass made, in the order it made them.
    edges: Vec<u32>,
    /// `GD_nlist` before the slack nodes existed: every component of the graph, concatenated
    /// in `decompose`'s order, which is `merge_components`' order in the reference.
    nlist: Vec<u32>,
}

impl Aux {
    /// `GD_nlist` as the second simplex sees it: the slack nodes first, newest first, then
    /// every node the graph already had.
    pub fn node_list(&self) -> Vec<u32> {
        let mut nodes = Vec::with_capacity(self.nlist.len() + self.slack.len());
        nodes.extend(self.slack.iter().rev());
        nodes.extend_from_slice(&self.nlist);
        nodes
    }

    /// `remove_aux_edges`: mark every constraint and pair as gone and put the graph's own
    /// adjacency back. The slack nodes stay in the dense array — the reference frees them, a
    /// dense index must stay dense — and nothing after this point reaches them, because
    /// [`node_list`](Aux::node_list) is what the simplex was given and it is dropped with `self`.
    pub fn remove(self, g: &mut Fast) {
        for edge in self.edges {
            g.edges[edge as usize].live = false;
        }
        g.out = self.saved_out;
        g.inn = self.saved_in;
    }
}

/// `create_aux_edges`: copy the adjacency aside, then the rank constraints, then the edge pairs.
///
/// The copy is the reference's `allocate_aux_edges`, which saves the two list pointers and hands
/// the node fresh, larger lists. Here the lists are `Vec`s that grow, so saving them *is* copying
/// them and putting the copy back is the whole of `remove_aux_edges`.
pub fn build(g: &mut Fast, rows: &Rows) -> Aux {
    let nlist: Vec<u32> = decomp::decompose(g).into_iter().flatten().collect();
    let saved_out = g.out.clone();
    let saved_in = g.inn.clone();
    let mut edges: Vec<u32> = Vec::new();
    lr_constraints(g, rows, &mut edges);
    let slack = edge_pairs(g, &saved_out, &nlist, &mut edges);
    Aux {
        saved_out,
        saved_in,
        slack,
        edges,
        nlist,
    }
}

/// `make_LR_constraints` (`position.c:262-266`): walk each rank left to right and join every
/// node to its right-hand neighbour with a zero-weight constraint as wide as the two boxes and
/// the gap between them.
///
/// The running `last` is the reference's `double` and it is **truncated to an integer** at every
/// step, because the assignment goes through `ND_rank`, which is an `int`. That is not the same
/// as the edge's minimum length, which is *rounded* — so with a fractional box two consecutive
/// nodes end up a point closer together than their own constraint asks. Reproduced rather than
/// tidied: a node's `lw` is a measured fraction of a point for every label wider than the
/// default box, so this is the common case and not a corner.
///
/// `Ponytail:` the self-edge branch, which widens a node by the space its own loops need. The
/// fixtures have no self-loops — the seeded generator skips `a == b` — so nothing measured here
/// reaches it. Failing input: a DOT graph with a self-loop. Direction: a self-looping node keeps
/// the width of its label, where the reference widens it to make room for the loop. Escape
/// hatch: [`Fast::other_edge`] already files the loop under its node; reading `node.other` here
/// is where it would be consumed.
fn lr_constraints(g: &mut Fast, rows: &Rows, edges: &mut Vec<u32>) {
    for row in rows.iter() {
        if let Some(&first) = row.first() {
            g.nodes[first as usize].rank = 0;
        }
        let mut last = 0.0;
        for pair in row.windows(2) {
            let (u, v) = (pair[0], pair[1]);
            let width = g.nodes[u as usize].rw + g.nodes[v as usize].lw + NODESEP;
            edges.push(aux_edge(g, (u, v), width, 0));
            last += width;
            g.nodes[v as usize].rank = last as i32;
        }
    }
}

/// `make_edge_pairs` (`position.c:332-351`): one slack node per input edge, joined to both its
/// ends by a one-point edge carrying the input edge's weight. Returns the slack nodes in
/// creation order.
///
/// With no ports the reference's two offsets are both zero, so both of its edges have length
/// one and the slack node sits one point left of whichever end is further left — which is the
/// whole of what the pair says: the two ends are not the same point.
///
/// `Ponytail:` ports. The reference offsets each of the two edges by the port's own x, so a
/// ported edge is asked to leave room for where it attaches. This port has no ports and no
/// attribute channel to read them from. Failing input: any DOT graph with a `tailport` or
/// `headport`. Direction: a ported edge is drawn between the node centres, where the reference
/// draws it between the ports. Escape hatch: none; `Fast` records no port offset on an edge.
fn edge_pairs(g: &mut Fast, saved_out: &[Vec<u32>], nlist: &[u32], edges: &mut Vec<u32>) -> Vec<u32> {
    let mut slack = Vec::new();
    for &n in nlist {
        for &edge in &saved_out[n as usize] {
            let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
            let weight = g.edges[edge as usize].weight;
            let node = g.add_node(Node::virtual_node(NODESEP));
            slack.push(node);
            edges.push(aux_edge(g, (node, tail), 1.0, weight));
            edges.push(aux_edge(g, (node, head), 1.0, weight));
            let (t, h) = (g.nodes[tail as usize].rank, g.nodes[head as usize].rank);
            g.nodes[node as usize].rank = t.min(h) - 1;
        }
    }
    slack
}

/// `make_aux_edge` (`position.c:183-199`): a constraint edge of the given minimum length,
/// rounded, and the given weight. The length is capped on the way in so a width past `int`
/// cannot wrap round into a negative constraint — the reference's own `largeMinlen` guard.
fn aux_edge(g: &mut Fast, ends: (u32, u32), len: f64, weight: i32) -> u32 {
    let capped = if len > f64::from(i32::MAX) { f64::from(i32::MAX) } else { len };
    g.add_aux(ends.0, ends.1, round(capped), weight)
}
