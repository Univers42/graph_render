//! `x_cutval` and `x_val` (`ns.c:1043-1108`): where the edge weights become an objective.
//!
//! The cut value of a tree edge is a signed weight sum over the edges at one node, and the
//! sign falls out of a single question — is the far endpoint inside that node's subtree? An
//! edge leaving it contributes `+weight`; one inside it contributes `-weight`, and a tree
//! edge its *own* cut value subtracted, which is what lets the recursion telescope: at the
//! root the sum is over the whole tree and every tree edge below has already been evaluated.
//!
//! The convention is fixed by the two-node case: a tree with one edge gives that edge a cut
//! value of `+weight`, and `leave_edge` looks for **negative** ones, so a cut value is "the
//! weight leaving the tail side minus the weight entering it". A negative one is the
//! certificate that the tree can be improved.
//!
//! Determinism: the sum runs over the node's out list and then its in list, in insertion
//! order, and the reference guards the running total against overflow; here it is an `i64`
//! with a checked add, and an answer that does not fit `int` is [`Error::Overflow`] rather
//! than a silent wrap.

use super::super::fast::Fast;
use super::Error;

/// `x_cutval` (`ns.c:1043-1070`): the cut value of tree edge `f`, as the signed weight sum
/// over the edges at the node on the side already searched — its tail when `par` points
/// from there, its head otherwise, and the other sign in the second case.
pub fn cutval(g: &mut Fast, f: u32) -> Result<(), Error> {
    let tail = g.edges[f as usize].tail;
    let head = g.edges[f as usize].head;
    let down = g.nodes[tail as usize].par == Some(f);
    let (v, dir) = if down { (tail, 1) } else { (head, -1) };
    let outgoing = g.out[v as usize].clone();
    let incoming = g.inn[v as usize].clone();
    let mut sum: i64 = 0;
    for edge in outgoing.into_iter().chain(incoming) {
        sum = sum
            .checked_add(i64::from(value(g, edge, v, dir)))
            .ok_or(Error::Overflow)?;
    }
    g.edges[f as usize].cutvalue = i32::try_from(sum).map_err(|_| Error::Overflow)?;
    Ok(())
}

/// `x_val` (`ns.c:1072-1108`): one edge's contribution to a cut value. An edge that leaves
/// the searched subtree contributes its weight; one inside it contributes its weight
/// subtracted, and a tree edge its own cut value subtracted, so the recursion telescopes.
fn value(g: &Fast, edge: u32, v: u32, dir: i32) -> i32 {
    let tail = g.edges[edge as usize].tail;
    let head = g.edges[edge as usize].head;
    let other = if tail == v { head } else { tail };
    let (crossing, amount) = if seq(g, v, other) {
        let inner = if g.edges[edge as usize].tree_index >= 0 {
            g.edges[edge as usize].cutvalue
        } else {
            0
        };
        (1, inner - g.edges[edge as usize].weight)
    } else {
        (-1, g.edges[edge as usize].weight)
    };
    let down = if dir > 0 { head == v } else { tail == v };
    let side = if down { 1 } else { -1 };
    side * crossing * amount
}

/// `SEQ(ND_low(a), ND_lim(b), ND_lim(a))` (`ns.c:44`): is `b` inside `a`'s subtree?
fn seq(g: &Fast, a: u32, b: u32) -> bool {
    let low = g.nodes[a as usize].low;
    let lim_a = g.nodes[a as usize].lim;
    let lim_b = g.nodes[b as usize].lim;
    low <= lim_b && lim_b <= lim_a
}
