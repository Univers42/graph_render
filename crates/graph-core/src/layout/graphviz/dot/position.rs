//! `dot_position` (`position.c:127-153`), the third of `dot`'s four passes: it turns a ranked,
//! ordered graph into a drawing.
//!
//! The pass is five steps, and the order is the reference's:
//!
//! 1. [`ycoords::run`] — the y coordinate of every rank, from the tallest node on it and
//!    `ranksep`. Rank 0 ends up at the **top**.
//! 2. [`aux::build`] — the auxiliary graph: a zero-weight constraint between every pair of
//!    neighbours on a rank, and a weighted pair of edges through a slack node for every input
//!    edge. This is where every node's box width becomes a *length* rather than a decoration.
//! 3. **The second network simplex.** [`simplex::rank2`] again, over the auxiliary graph and
//!    with `LR_balance` instead of `TB_balance` — a call through [`simplex::Params`], never a
//!    second implementation. One engine, two jobs; that is what `Params` is for.
//! 4. [`xcoords::run`] — the answer moves from `ND_rank` into `ND_coord.x`, and the rank number
//!    goes back where it was. Then [`aux::Aux::remove`], which restores the graph.
//! 5. [`frame::run`] — the shift that puts the drawing's lower-left node-box corner at the
//!    origin, which is the frame `-Tplain` prints in.
//!
//! **What the pass does not port.** Five things. Clusters: the whole of `pos_clusters`,
//! `contain_nodes` and `compress_graph`, plus the cluster half of the rank gap and of the frame.
//! Ports: the per-end offsets in `make_edge_pairs`. The same-rank-edge constraints: the `ND_alg`
//! and `ND_flat_out` branches of `make_LR_constraints`. `set_aspect`: the ratio and size
//! rescale, inert at the default and unreachable without a drawing size. And `connectGraph`,
//! the retry arm at `position.c:142-148`. None of the five is reachable from a `Topology`: the
//! motor publishes a flat node set, no attribute channel and no drawing size, so there is
//! nothing for them to read. Each carries a `Ponytail:` line where it is dropped, naming the
//! input that would exercise it — except `connectGraph`, whose line is here because the omission
//! is a *failure* rather than a different answer, and a failing omission needs naming louder:
//!
//! Ponytail (`connectGraph`, `position.c:79-125`): the reference hands `rank(g, 2, …)` the whole
//! graph, and when the x simplex reports the graph is not connected it joins the pieces with
//! zero-length edges and runs again. This port does not, so a `Topology` of two or more
//! disconnected pieces returns [`Error::Disconnected`] and the caller gets no drawing at all,
//! where the reference draws the pieces side by side. Failing input: a disconnected `Topology`.
//! Direction: no drawing, not a different one. Escape hatch: run the pass once per component —
//! the pieces are independent, which is exactly what the rank pass already does per component.
//! Measured out of reach for the fixtures: every seeded graph of the 1000 is connected, so no
//! seed in the sweep reaches this arm.
//!
//! Determinism: every value the second simplex sees is an integer (the constraint lengths are
//! rounded, the slack placements are integer offsets), the node list is fixed by the reference's
//! prepend order, and nothing in the pass reads a clock, a hash order or a random number
//! (`prompt.md` §6 D1-D10).

pub mod aux;
pub mod frame;
pub mod rows;
pub mod xcoords;
pub mod ycoords;

use super::fast::Fast;
use super::simplex::{self, Error, Params};

pub use rows::Rows;

/// Run the whole position pass over a graph that has been through [`rank`](fn@super::rank) and
/// [`mincross`](super::mincross).
///
/// The graph is left with every node's `coord` in the frame `-Tplain` prints and its `rank`
/// back to being the rank index. Chain dummies carry coordinates too — the drawing's edges are
/// polylines through them, and this crate emits those polylines rather than splines.
pub fn position(g: &mut Fast) -> Result<(), Error> {
    let rows = Rows::of(g);
    ycoords::run(g, &rows);
    let aux = aux::build(g, &rows);
    let nlist = aux.node_list();
    simplex::rank2(g, &nlist, &lr_balance())?;
    xcoords::run(g, &rows);
    aux.remove(g);
    frame::run(g, &rows);
    Ok(())
}

/// `rank(g, 2, nsiter2(g))` (`position.c:142`): the same engine, the x-coordinate balance, and
/// the reference's `nsiter2` with no `nslimit` attribute set — which is `INT_MAX`.
///
/// `Ponytail:` `nslimit` and `searchsize`. The reference reads both from the graph and this
/// reads neither, because the motor publishes no attribute channel to a layout. Failing input: a
/// DOT graph with `nslimit` or `searchsize`. Direction: an unbounded pivot loop where the
/// reference would stop early — the same answer for any graph the loop terminates on, which is
/// all of them, since the loop stops when no tree edge has a negative cut value. Escape hatch:
/// [`Params`] carries both fields and a caller outside this module can set them.
fn lr_balance() -> Params {
    Params::left_right()
}
