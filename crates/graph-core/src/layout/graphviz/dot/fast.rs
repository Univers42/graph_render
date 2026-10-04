//! The "fast graph" `dot` lays out: one flat array of nodes and edges with per-node
//! in/out adjacency, the fields the four passes read and write, and nothing else.
//!
//! This is `lib/dotgen/fastgr.c`'s `node_t`/`edge_t` pair with the C pointers replaced
//! by dense indices: `nodes[i]` and `edges[i]`, every adjacency list a `Vec<u32>` of
//! edge indices in insertion order. The reference's `elist` order is load-bearing — the
//! network simplex's tree walks, `class2`'s `prev` scan for multi-edges, `build_ranks`'s
//! BFS and `decompose`'s stack all read it — so it is reproduced rather than re-sorted,
//! and every removal is the reference's `zapinlist` (move the last entry into the hole).
//!
//! What each pass reads, and why the field is here:
//!
//! | field | read by | reference |
//! |---|---|---|
//! | `rank` | every pass | `ND_rank` |
//! | `order` | mincross | `ND_order` |
//! | `coord` | position | `ND_coord` |
//! | `lw`, `rw`, `ht` | position | `ND_lw`, `ND_rw`, `ND_ht` |
//! | `mval` | mincross | `ND_mval` |
//! | `kind` | mincross, position | `ND_node_type` |
//! | `flat_in`, `flat_out`, `other` | mincross, position | the same three lists |
//!
//! Determinism: every list is built by pushing in the reference's iteration order; nothing
//! here reads a clock, a hash order or a random number (`prompt.md` §6 D1-D10).
//!
//! **Edges are never removed from `edges`, only unhooked.** The reference frees the
//! record; here the index stays because `to_virt`/`to_orig` are still read through it and
//! because a dense index must stay dense. `Edge::live` says which records are still in
//! one of the two adjacency lists, so a dead edge is never visited. Two passes unhook in
//! bulk rather than one at a time and say so: `cleanup1` empties both lists at once (it is
//! the boundary between the rank pass and `class2`), and `class2` never puts the input
//! edges back into them — the reference keeps those in the cgraph, which `orig_out` is.
//!
//! `orig_out` is that cgraph: every node's **input** out-edges, in declaration order, built
//! once by `add_edge` and never touched again. `class2` walks it rather than `out`, because
//! by then `out` holds only the chains `class2` itself has built, and `class1` walks it
//! because it is the pass that fills `out` in the first place.

mod edge;
mod node;

pub use edge::{Edge, round};
pub use node::{Coord, Kind, Node};

/// `zapinlist` (`fastgr.c:78-88`): remove `edge`, moving the last entry into the hole.
/// The reference's own removal, so the surviving order is the answer — `find_fast_edge`
/// scans it, and every later pass walks it.
pub fn zap(list: &mut Vec<u32>, edge: u32) {
    if let Some(at) = list.iter().position(|&e| e == edge) {
        let last = list.len() - 1;
        list[at] = list[last];
        list.pop();
    }
}

/// The fast graph: nodes, edges, and the two adjacency directions.
#[derive(Clone, Debug, Default)]
pub struct Fast {
    /// Every node: the real ones in dense-index order, then the virtual ones.
    pub nodes: Vec<Node>,
    /// Every edge: the real ones in dense-index order, then chains, then aux edges.
    pub edges: Vec<Edge>,
    /// `ND_out`: a node's out-edges, in insertion order.
    pub out: Vec<Vec<u32>>,
    /// `ND_in`: a node's in-edges, in insertion order.
    pub inn: Vec<Vec<u32>>,
    /// `agfstout`: every node's **input** out-edges, in declaration order. See the module
    /// doc for why `class2` reads this and not `out`.
    pub orig_out: Vec<Vec<u32>>,
}

impl Fast {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// `fast_node` (`fastgr.c:184`): add a node. It is not appended to any list here —
    /// `GD_nlist` is owned by `decompose`, which is the only thing that fills it.
    pub fn add_node(&mut self, node: Node) -> u32 {
        let id = u32::try_from(self.nodes.len()).expect("node index fits u32");
        self.nodes.push(node);
        self.out.push(Vec::new());
        self.inn.push(Vec::new());
        self.orig_out.push(Vec::new());
        id
    }

    /// `agedge`: record an **input** edge. The record joins `edges` and the node's
    /// `orig_out`, and stops there: an input edge is not in the fast graph. `class1` is what
    /// puts edges into `out` and `inn`, and it makes a *copy* of each one, so the reference's
    /// "is this edge already in the fast graph" test in `find_fast_edge` has an answer other
    /// than "yes, this one" — which is why this is not a shortcut through `Fast::link`.
    pub fn add_edge(&mut self, edge: Edge) -> u32 {
        let mut edge = edge;
        edge.live = false;
        let id = u32::try_from(self.edges.len()).expect("edge index fits u32");
        self.edges.push(edge.clone());
        self.orig_out[edge.tail as usize].push(id);
        id
    }

    /// `fast_edge` (`fastgr.c:71-93`): file an edge in both adjacency directions. This is
    /// what makes an edge part of the fast graph, and only the passes call it — `class1`,
    /// `acyclic`'s reversal, `class2`'s chains.
    fn link(&mut self, edge: Edge) -> u32 {
        let id = u32::try_from(self.edges.len()).expect("edge index fits u32");
        self.out[edge.tail as usize].push(id);
        self.inn[edge.head as usize].push(id);
        self.edges.push(edge);
        id
    }

    /// `find_fast_edge` (`fastgr.c:37`): the live edge from `u` to `v`, if there is one.
    /// The reference scans the shorter of the two lists, which is a search-order detail;
    /// the answer is the same edge, so the out list is scanned.
    pub fn find_edge(&self, u: u32, v: u32) -> Option<u32> {
        self.out[u as usize]
            .iter()
            .copied()
            .find(|&e| self.edges[e as usize].head == v)
    }

    /// `delete_fast_edge` (`fastgr.c:95-100`): unhook an edge from both directions.
    pub fn delete_edge(&mut self, edge: u32) {
        let (tail, head) = {
            let e = &mut self.edges[edge as usize];
            e.live = false;
            (e.tail, e.head)
        };
        zap(&mut self.out[tail as usize], edge);
        zap(&mut self.inn[head as usize], edge);
    }

    /// `new_virtual_edge` + `fast_edge` (`fastgr.c:132,177`): a link carrying the
    /// original's `count`, `xpenalty`, `weight` and `minlen`, appended to both lists of
    /// its new endpoints.
    pub fn add_chain(&mut self, tail: u32, head: u32, orig: u32) -> u32 {
        let mut edge = Edge::real(tail, head);
        let source = self.edges[orig as usize].clone();
        edge.count = source.count;
        edge.xpenalty = source.xpenalty;
        edge.weight = source.weight;
        edge.minlen = source.minlen;
        edge.to_orig = Some(orig);
        if self.edges[orig as usize].to_virt.is_none() {
            self.edges[orig as usize].to_virt = Some(self.edges.len() as u32);
        }
        self.link(edge)
    }

    /// `basic_merge` (`fastgr.c:231-242`): fold `e`'s weight, penalty and count into
    /// `rep` and into every link of `rep`'s chain, and keep the **larger** of the two
    /// minimum lengths.
    pub fn basic_merge(&mut self, e: u32, rep: u32) {
        let (count, xpenalty, weight, minlen) = {
            let src = &self.edges[e as usize];
            (src.count, src.xpenalty, src.weight, src.minlen)
        };
        let mut cursor = Some(rep);
        while let Some(at) = cursor {
            let edge = &mut self.edges[at as usize];
            edge.count += count;
            edge.xpenalty += xpenalty;
            edge.weight += weight;
            if at == rep && edge.minlen < minlen {
                edge.minlen = minlen;
            }
            cursor = edge.to_virt;
        }
    }

    /// `merge_oneway` (`fastgr.c:244-255`): point `e` at `rep` and fold it in. `e` is
    /// not unhooked here — the reference frees it, and every caller has already done the
    /// unhook or is about to.
    pub fn merge_oneway(&mut self, e: u32, rep: u32) {
        if self.edges[e as usize].to_virt == Some(rep)
            || self.edges[rep as usize].to_virt == Some(e)
        {
            return;
        }
        self.edges[e as usize].to_virt = Some(rep);
        self.basic_merge(e, rep);
    }

    /// `reverse_edge` (`acyclic.c:22-33`): unhook the edge, then either fold it into an
    /// existing edge the other way round or make a virtual edge that carries its weight
    /// the other way round. **This is how `dot` decides a cycle's direction**, and the
    /// merge is not a detail: a cycle whose reverse edge already exists loses one edge
    /// and doubles the other's weight.
    pub fn reverse_edge(&mut self, edge: u32) {
        let (tail, head) = (
            self.edges[edge as usize].tail,
            self.edges[edge as usize].head,
        );
        self.delete_edge(edge);
        match self.find_edge(head, tail) {
            Some(rev) => self.merge_oneway(edge, rev),
            None => {
                self.add_chain(head, tail, edge);
            }
        }
    }

    /// `other_edge` (`fastgr.c:110`): file a self-loop under its node.
    pub fn other_edge(&mut self, edge: u32) {
        let tail = self.edges[edge as usize].tail;
        self.nodes[tail as usize].other.push(edge);
    }

    /// `flat_edge` (`fastgr.c:215-218`): file a same-rank edge in both flat lists.
    pub fn flat_edge(&mut self, edge: u32) {
        let (tail, head) = (
            self.edges[edge as usize].tail,
            self.edges[edge as usize].head,
        );
        self.nodes[tail as usize].flat_out.push(edge);
        self.nodes[head as usize].flat_in.push(edge);
    }

    /// `delete_flat_edge` (`fastgr.c:220-227`): unhook a same-rank edge.
    pub fn delete_flat_edge(&mut self, edge: u32) {
        let (tail, head) = (
            self.edges[edge as usize].tail,
            self.edges[edge as usize].head,
        );
        zap(&mut self.nodes[tail as usize].flat_out, edge);
        zap(&mut self.nodes[head as usize].flat_in, edge);
    }

    /// `cleanup1`'s `renewlist` (`rank.c:45-53`) over every node at once: empty both
    /// adjacency directions and mark every edge as out of them. This is the boundary
    /// between the rank pass and `class2` — the input edges survive in `orig_out` and in
    /// `edges`, and the fast graph is rebuilt from the two by `class2`.
    pub fn clear_adjacency(&mut self) {
        for list in self.out.iter_mut().chain(self.inn.iter_mut()) {
            list.clear();
        }
        for edge in &mut self.edges {
            edge.live = false;
        }
        for node in &mut self.nodes {
            node.mark = false;
        }
    }
}
