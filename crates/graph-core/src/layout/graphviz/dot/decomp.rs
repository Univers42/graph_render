//! `decompose` — the connected components of the fast graph, and the node order each is
//! walked in.
//!
//! Reference: `lib/dotgen/decomp.c` (117 lines, read whole). Components are found by an
//! iterative depth-first search seeded from the *real* nodes in declaration order —
//! virtual nodes are only ever reached through an edge — and the search is a stack, not a
//! recursion, so the node order of a component is the reference's pop order, which the
//! ranking pass's `init_rank` and the mincross pass's `build_ranks` both walk.
//!
//! The reference marks nodes with a global counter `Cmark` and pushes with `Cmark + 1`,
//! so a node on the stack is distinguishable from a finished one. The counter only ever
//! increases, which is what makes stale marks from an earlier pass harmless; here the
//! marks are reset per call, which is the same thing stated directly and is the reason
//! [`decompose`] takes the whole graph rather than a component.
//!
//! Determinism: the stack is LIFO, the seeds are in dense-index order, and each node's
//! neighbours are pushed in the order `flat_in`, `flat_out`, `in`, `out`, each scanned
//! **backwards** (`decomp.c:99-108`) so the pops come out in list order.

use super::fast::Fast;

/// A node's state within one [`decompose`] call: 0 unseen, 1 on the stack, 2 finished.
const UNSEEN: u8 = 0;
const ON_STACK: u8 = 1;
const DONE: u8 = 2;

/// The components, each a node list in the reference's order.
pub type Components = Vec<Vec<u32>>;

/// `decompose` (`decomp.c:87-116`) at `pass == 0` and `pass == 1`, which differ only in
/// their cluster handling — and this port has no clusters, so the two are one function.
pub fn decompose(g: &Fast) -> Components {
    let mut state = vec![UNSEEN; g.nodes.len()];
    let mut comps: Components = Vec::new();
    for seed in 0..g.nodes.len() {
        if state[seed] == DONE {
            continue;
        }
        comps.push(search_component(g, &mut state, seed as u32));
    }
    comps
}

/// `search_component` (`decomp.c:99-125`): one component, in pop order.
fn search_component(g: &Fast, state: &mut [u8], seed: u32) -> Vec<u32> {
    let mut component = Vec::new();
    let mut stack = vec![seed];
    state[seed as usize] = ON_STACK;
    while let Some(node) = stack.pop() {
        if state[node as usize] == DONE {
            continue;
        }
        state[node as usize] = DONE;
        component.push(node);
        push_neighbours(g, state, node, &mut stack);
    }
    component
}

/// The reference's four adjacency lists, in its order, each scanned backwards so the pops
/// come out forwards. `flat_in`/`flat_out` are empty for a graph with no same-rank edges,
/// which is every graph this pass sees before `class2` and most after it.
fn push_neighbours(g: &Fast, state: &mut [u8], node: u32, stack: &mut Vec<u32>) {
    let n = node as usize;
    let lists = [
        &g.nodes[n].flat_in,
        &g.nodes[n].flat_out,
        &g.inn[n],
        &g.out[n],
    ];
    for list in lists {
        for &edge in list.iter().rev() {
            let e = &g.edges[edge as usize];
            let other = if e.head == node { e.tail } else { e.head };
            if state[other as usize] != DONE {
                state[other as usize] = ON_STACK;
                stack.push(other);
            }
        }
    }
}
