//! Phase 0: `acyclic` — break every cycle by reversing edges, so the ranking pass sees a
//! DAG.
//!
//! Reference: `lib/dotgen/acyclic.c` (70 lines, read whole). It is a depth-first walk
//! from each node of each component, in `nlist` order, marking nodes as it goes; an edge
//! whose head is *on the current DFS path* is a back edge, and is reversed in place — and
//! then **re-read**, which is the reference's `i--` followed by the loop's `i++`. That
//! re-read is what makes the pass agree with the reference on a graph whose reversal
//! exposes a new back edge from the same slot, so the slot is left where it is here too.
//!
//! **This is the one place `dot` silently changes the drawing's direction.** The motor's
//! [`Topology`](crate::index::Topology) is undirected as far as the caller is concerned,
//! but the reference's ranking ranks a *directed* graph and needs one. A cycle has no
//! consistent layering, so the reference picks a direction per back edge and the layering
//! that comes out is one of the valid ones. Which one is a function of the DFS order,
//! hence of the dense node index, and it is reproduced exactly.
//!
//! Determinism: the walk is depth-first over `nlist`, reading each node's out-edges in
//! insertion order. Nothing here reads a clock, a hash order or a random number
//! (`prompt.md` §6 D1-D10).

use super::fast::Fast;

/// `acyclic` (`acyclic.c:63-70`): break every cycle in one component.
///
/// `component` is the component's node list in the order `decompose` produced, which is
/// the order the reference's `GD_nlist` holds and therefore the order the walk starts
/// from. The marks are reset first, because `ND_mark` is shared with `decompose` and the
/// reference resets it per component for the same reason.
pub fn run(g: &mut Fast, component: &[u32]) {
    for &node in component {
        g.nodes[node as usize].mark = false;
    }
    for &root in component {
        if !g.nodes[root as usize].mark {
            dfs(g, root);
        }
    }
}

/// `dfs` (`acyclic.c:39-56`), iterative rather than recursive: the reference recurses
/// once per node, and a component can hold every node of a 100 000-node graph.
fn dfs(g: &mut Fast, root: u32) {
    // The DFS path, and per entry the out-edge slot it is about to read. A node is "on
    // the stack" exactly while it is on `path`, which is `ND_onstack`.
    let mut path: Vec<u32> = vec![root];
    let mut slot: Vec<usize> = vec![0];
    g.nodes[root as usize].mark = true;
    g.nodes[root as usize].onstack = true;
    while let Some(&node) = path.last() {
        let top = path.len() - 1;
        let at = slot[top];
        let out = &g.out[node as usize];
        let Some(&edge) = out.get(at) else {
            g.nodes[node as usize].onstack = false;
            path.pop();
            slot.pop();
            continue;
        };
        let head = g.edges[edge as usize].head;
        if g.nodes[head as usize].onstack {
            // A back edge: reverse it and leave the slot alone, so the same position is
            // read again — the reference's `reverse_edge(e); i--;`.
            g.reverse_edge(edge);
        } else {
            slot[top] = at + 1;
            if !g.nodes[head as usize].mark {
                g.nodes[head as usize].mark = true;
                g.nodes[head as usize].onstack = true;
                path.push(head);
                slot.push(0);
            }
        }
    }
}
